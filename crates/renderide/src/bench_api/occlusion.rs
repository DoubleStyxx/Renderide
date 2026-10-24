//! Occlusion hysteresis classification and history workloads.

use std::sync::atomic::{AtomicU32, Ordering};

use glam::{Mat4, Vec3};
use hashbrown::{HashMap, HashSet};
use rayon::prelude::*;

use crate::camera::reverse_z_perspective;
use crate::hi_z_cpu::project_world_aabb_uv;
use crate::occlusion::hysteresis::{
    CasterKey, CasterRecord, CasterRef, DepthClip, HistoryAction, HistoryInputs, MovedCaster,
    OcclusionFacts, PresentCaster, ScreenRect, SpaceMotion, VisibilityClass, VisibilityCullBuffers,
    VisibilityHistory, VisibilityHysteresis, append_disocclusion_rects, decide_visibility,
    plan_disocclusion, prepare_fast_visible, screen_motion_uv_pad,
};
use crate::scene::RenderSpaceId;

/// Casters classified by one [`HysteresisCullWorkload::kept_count`] sample.
pub const HYSTERESIS_BATCH: usize = 4_096;

/// Caster counts timed by [`HysteresisHistoryWorkload`].
///
/// The first entry is [`crate::occlusion::hysteresis::HISTORY_CASTER_BUDGET`], the production
/// cutoff. Larger counts measure the uncapped history step that cutoff skips. On a 4-core guest
/// that step was about 13 ms at 100_000 casters, past an 11.1 ms 90 Hz frame.
pub const HYSTERESIS_HISTORY_COUNTS: [usize; 4] = [
    crate::occlusion::hysteresis::HISTORY_CASTER_BUDGET,
    10_000,
    50_000,
    100_000,
];

/// Screen-motion pad below which the history bench treats the pyramid as still occluding.
///
/// The bench does not sample a Hi-Z pyramid. This stand-in keeps the timed path on
/// `plan_disocclusion`, the per-caster reprojection, the history lookup, and `finish`.
const HISTORY_CONFIRM_UV: f32 = 0.02;

/// Owned history-confirmation query reused by [`HysteresisCullWorkload`].
#[derive(Clone, Debug)]
struct HysteresisQuery {
    /// Index into the workload's caster records. `None` is a newly revealed caster.
    previous: Option<usize>,
    /// Bounds moved since the stored record.
    caster_moved: bool,
    /// Screen rect hits a moved occluder.
    overlaps_disocclusion: bool,
    /// Previous pyramid reports full occlusion.
    hi_z_occludes: bool,
    /// Covering depth is a separate occluder.
    other_occluder: bool,
}

/// Classifies a fixed mix of stable occluded casters and disoccluded casters.
#[derive(Debug)]
pub struct HysteresisCullWorkload {
    records: Vec<CasterRecord>,
    queries: Vec<HysteresisQuery>,
}

impl HysteresisCullWorkload {
    /// Builds [`HYSTERESIS_BATCH`] queries: one quarter stay culled, three quarters must be kept.
    #[must_use]
    pub fn standard() -> Self {
        let bounds = Vec3::new(-0.5, -0.5, -2.0);
        let records = vec![
            CasterRecord::new(
                VisibilityClass::Occluded,
                bounds,
                Vec3::new(0.5, 0.5, -1.0),
                1,
            ),
            CasterRecord::new(
                VisibilityClass::Visible,
                bounds,
                Vec3::new(0.5, 0.5, -1.0),
                1,
            ),
        ];
        let queries = (0..HYSTERESIS_BATCH)
            .map(|index| match index % 4 {
                0 => HysteresisQuery {
                    previous: Some(0),
                    caster_moved: false,
                    overlaps_disocclusion: false,
                    hi_z_occludes: true,
                    other_occluder: false,
                },
                1 => HysteresisQuery {
                    previous: None,
                    caster_moved: false,
                    overlaps_disocclusion: false,
                    hi_z_occludes: true,
                    other_occluder: false,
                },
                2 => HysteresisQuery {
                    previous: Some(0),
                    caster_moved: false,
                    overlaps_disocclusion: true,
                    hi_z_occludes: true,
                    other_occluder: false,
                },
                _ => HysteresisQuery {
                    previous: Some(1),
                    caster_moved: false,
                    overlaps_disocclusion: false,
                    hi_z_occludes: false,
                    other_occluder: false,
                },
            })
            .collect();
        Self { records, queries }
    }

    /// Number of casters history confirmation still draws.
    #[must_use]
    pub fn kept_count(&self) -> u32 {
        let mut kept = 0_u32;
        for query in &self.queries {
            let previous = query.previous.and_then(|index| self.records.get(index));
            let decision = decide_visibility(HistoryInputs {
                previous,
                caster_moved: query.caster_moved,
                overlaps_disocclusion: query.overlaps_disocclusion,
                hi_z_occludes: query.hi_z_occludes,
                other_occluder: query.other_occluder,
            });
            if decision.action != HistoryAction::Culled {
                kept = kept.saturating_add(1);
            }
        }
        kept
    }
}

/// History lookup, screen-motion pad, and `finish` over a prebuilt caster set.
///
/// Hi-Z pyramid sampling is not included. A pad under [`HISTORY_CONFIRM_UV`] stands in for a
/// pyramid that still covers the expanded rectangle. This is the uncapped cost of
/// `plan_disocclusion`, the per-caster reprojection, the history lookup, and `finish`.
/// Production does not run it once the prepared caster count passes
/// [`crate::occlusion::hysteresis::HISTORY_CASTER_BUDGET`]. The sequential prepared-draw scan
/// that feeds the plan is a separate, cheaper walk and stops at that same cap. The count is
/// prepared casters in scope, not casters that survive the frustum.
pub struct HysteresisHistoryWorkload {
    present: Vec<PresentCaster>,
    /// Frustum survivors. The history bench calls `allows_draw` for these rows only.
    /// [`Self::present`] may be a larger prepared grid.
    drawn: Vec<PresentCaster>,
    present_keys: HashSet<CasterKey>,
    movers: Vec<MovedCaster>,
    previous_vp: Mat4,
    current_vp: Mat4,
    history: VisibilityHistory,
    spare: HashMap<CasterKey, CasterRecord>,
    buffers: VisibilityCullBuffers,
    /// `true` when occlusion facts are forced visible. The gauge scene culls nothing.
    keep_facts: bool,
    /// `true` when the last [`Self::step`] armed the stable-visible fast path.
    fast_engaged: bool,
}

impl HysteresisHistoryWorkload {
    /// Builds `count` casters. One quarter start occluded. `moving` separates the cameras.
    #[must_use]
    pub fn new(count: usize, moving: bool) -> Self {
        Self::build(count, moving, true, |_| true)
    }

    /// Builds `count` casters that all start visible, with facts that do not occlude.
    ///
    /// This is the bookkeeping the all-visible `many_static_meshes` frame pays: nothing was
    /// culled last frame, and nothing is culled this frame.
    #[must_use]
    pub fn all_visible(count: usize) -> Self {
        let mut workload = Self::build(count, false, false, |_| true);
        workload.keep_facts = true;
        workload
    }

    /// 1,600 prepared casters and the center 26×26 remembered, matching `many_static_meshes`.
    ///
    /// Counts above [`crate::occlusion::hysteresis::HISTORY_CASTER_BUDGET`] never arm.
    /// [`Self::all_visible`] at those counts stays on the full update.
    #[must_use]
    pub fn visible_grid_subset() -> Self {
        const SIDE: usize = 40;
        const INNER: usize = 26;
        const ORIGIN: usize = (SIDE - INNER) / 2;
        let mut workload = Self::build(SIDE * SIDE, false, false, |index| {
            let row = index / SIDE;
            let col = index % SIDE;
            (ORIGIN..ORIGIN + INNER).contains(&row) && (ORIGIN..ORIGIN + INNER).contains(&col)
        });
        workload.keep_facts = true;
        workload
    }

    /// `true` when [`Self::step`] last armed the stable-visible fast path.
    #[must_use]
    pub fn fast_path_engaged(&self) -> bool {
        self.fast_engaged
    }

    fn build(
        count: usize,
        moving: bool,
        occlude_quarter: bool,
        remember: impl Fn(usize) -> bool,
    ) -> Self {
        let (previous_vp, current_vp) = history_view_projections(moving);
        let mut side = 1_usize;
        while side.saturating_mul(side) < count.max(1) {
            side = side.saturating_add(1);
        }
        let mut casters = HashMap::with_capacity(count);
        let mut present = Vec::with_capacity(count);
        let mut drawn = Vec::new();
        for index in 0..count {
            let column = index % side;
            let row = index / side;
            let x = f32::from(u16::try_from(column).unwrap_or(0)).mul_add(0.45, -8.0);
            let z = f32::from(u16::try_from(row).unwrap_or(0)).mul_add(0.45, -4.0);
            let aabb_min = Vec3::new(x, 0.0, z);
            let aabb_max = aabb_min + Vec3::splat(0.2);
            let node_id = i32::try_from(index).unwrap_or(i32::MAX);
            let key = CasterKey::new(
                RenderSpaceId(1),
                index,
                u64::try_from(index).unwrap_or(u64::MAX),
                node_id,
                false,
            );
            let remembered = remember(index);
            let class = if occlude_quarter && index % 4 == 0 {
                VisibilityClass::Occluded
            } else {
                VisibilityClass::Visible
            };
            let caster = PresentCaster {
                key,
                mesh_asset_id: 1,
                aabb_min,
                aabb_max,
                space_id: RenderSpaceId(1),
            };
            if remembered {
                casters.insert(key, CasterRecord::new(class, aabb_min, aabb_max, 1));
                drawn.push(caster);
            }
            present.push(caster);
        }
        Self {
            present,
            drawn,
            present_keys: HashSet::new(),
            movers: Vec::new(),
            previous_vp,
            current_vp,
            history: VisibilityHistory::from_casters(casters),
            spare: HashMap::new(),
            buffers: VisibilityCullBuffers::default(),
            keep_facts: false,
            fast_engaged: false,
        }
    }

    /// Plans disocclusion, reprojects every caster, records the decision, and publishes history.
    ///
    /// The history map, spare map, mover list, and cull buffers stay on the workload. The next
    /// call clears and refills them, matching the view slot's recycle path.
    #[must_use]
    pub fn step(&mut self) -> u32 {
        let history = std::mem::take(&mut self.history);
        let spare = std::mem::take(&mut self.spare);
        let mut buffers = std::mem::take(&mut self.buffers);
        let fast_layout = prepare_fast_visible(&history, &self.present, &mut buffers);
        self.fast_engaged = fast_layout.is_some();
        let current_vp = self.current_vp;
        let previous_vp = self.previous_vp;
        buffers.stale_rects.clear();
        if fast_layout.is_none() {
            plan_disocclusion(
                &history,
                &self.present,
                &mut self.present_keys,
                |_| true,
                &mut self.movers,
            );
            append_disocclusion_rects(&mut buffers.stale_rects, &self.movers, |min, max| {
                project_screen(current_vp, min, max)
            });
        } else {
            self.movers.clear();
        }
        buffers.motions.clear();
        buffers.motions.insert(
            RenderSpaceId(1),
            SpaceMotion {
                mono: Some((previous_vp, current_vp)),
                stereo: None,
                clip: DepthClip {
                    near: 0.05,
                    far: 200.0,
                    orthographic: false,
                },
            },
        );
        let mut frame = VisibilityHysteresis::from_parts(history, spare, buffers);
        frame.install_fast(fast_layout);
        let kept = AtomicU32::new(0);
        let keep_facts = self.keep_facts;
        self.drawn.par_iter().for_each(|caster| {
            let pad =
                screen_motion_uv_pad(previous_vp, current_vp, caster.aabb_min, caster.aabb_max);
            let facts = if keep_facts {
                let _ = std::hint::black_box(pad);
                OcclusionFacts::keep()
            } else {
                facts_for_motion_pad(pad)
            };
            if frame.allows_draw(
                Some(CasterRef {
                    key: caster.key,
                    mesh_asset_id: caster.mesh_asset_id,
                }),
                caster.aabb_min,
                caster.aabb_max,
                facts,
            ) {
                kept.fetch_add(1, Ordering::Relaxed);
            }
        });
        let recycled = frame.recycle();
        self.history = recycled.history;
        self.spare = recycled.spare;
        self.buffers = recycled.buffers;
        kept.load(Ordering::Relaxed)
    }
}

fn facts_for_motion_pad(pad: Option<f32>) -> OcclusionFacts {
    match pad {
        Some(pad) if pad < HISTORY_CONFIRM_UV => OcclusionFacts {
            occluded: true,
            other_occluder: true,
            rects: [None, None],
        },
        _ => OcclusionFacts::keep(),
    }
}

fn project_screen(view_proj: Mat4, min: Vec3, max: Vec3) -> Option<ScreenRect> {
    let (uv_min, uv_max) = project_world_aabb_uv(view_proj, min, max)?;
    ScreenRect::from_uv(uv_min, uv_max)
}

fn history_view_projections(moving: bool) -> (Mat4, Mat4) {
    let projection = reverse_z_perspective(1.0, 70.0_f32.to_radians(), 0.05, 200.0);
    let previous = glam::camera::rh::view::look_at_mat4(
        Vec3::new(0.0, 1.5, 12.0),
        Vec3::new(0.0, 1.5, 0.0),
        Vec3::Y,
    );
    let current = if moving {
        glam::camera::rh::view::look_at_mat4(
            Vec3::new(2.0, 1.5, 12.0),
            Vec3::new(2.0, 1.5, 0.0),
            Vec3::Y,
        )
    } else {
        previous
    };
    (projection * previous, projection * current)
}
