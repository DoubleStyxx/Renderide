use crate::camera::EyeView;
use crate::occlusion::hysteresis::VisibilityScratch;
use crate::world_mesh::draw_prep::PreparedCasterBounds;

fn camera_in_front_of_the_grid() -> HostCameraFrame {
    // The projector keeps a corner only when clip.w > 0, which is view-space -Z.
    // A camera at z = 5 looking toward the origin puts the unit box in front of that plane.
    HostCameraFrame {
        clip: CameraClipPlanes::new(0.01, 100.0),
        explicit_view: Some(EyeView::new(
            Mat4::from_translation(glam::Vec3::new(0.0, 0.0, -5.0)),
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            glam::Vec3::new(0.0, 0.0, 5.0),
        )),
        ..HostCameraFrame::default()
    }
}

fn collect(prepared: &FramePreparedRenderables) -> Vec<PreparedCasterBounds> {
    let mut casters = Vec::new();
    prepared.for_each_world_caster_bounds(|_| true, |caster| casters.push(caster));
    casters
}

fn history_of(casters: &[PreparedCasterBounds]) -> VisibilityHistory {
    let mut map = HashMap::new();
    for caster in casters {
        map.insert(
            CasterKey::new(
                caster.space_id,
                caster.renderable_index,
                caster.instance_id,
                caster.node_id,
                caster.skinned,
            ),
            CasterRecord::new(
                VisibilityClass::Visible,
                caster.aabb_min,
                caster.aabb_max,
                caster.mesh_asset_id,
            ),
        );
    }
    VisibilityHistory::from_casters(map)
}

fn scene_with(spaces: &[RenderSpaceId]) -> SceneCoordinator {
    let mut scene = SceneCoordinator::new();
    for space_id in spaces {
        scene.test_seed_space_identity_worlds(
            *space_id,
            vec![RenderTransform::default()],
            vec![-1],
        );
    }
    scene
}

struct Published {
    draws: Vec<bool>,
    rows: Vec<(CasterKey, VisibilityClass, glam::Vec3, glam::Vec3, i32)>,
    visible_count: u32,
    epoch: u64,
    rects: Vec<ScreenRect>,
    armed: bool,
}

struct Lane {
    history: VisibilityHistory,
    spare: HashMap<CasterKey, CasterRecord>,
    frame_scratch: VisibilityFrameScratch,
    buffers: VisibilityCullBuffers,
}

impl Lane {
    fn new(history: VisibilityHistory) -> Self {
        Self {
            history,
            spare: HashMap::new(),
            frame_scratch: VisibilityFrameScratch::default(),
            buffers: VisibilityCullBuffers::default(),
        }
    }
}

struct Step<'a> {
    scene: &'a SceneCoordinator,
    prepared: &'a FramePreparedRenderables,
    scope: ViewRenderSpaceScope,
    calls: &'a [(usize, OcclusionFacts)],
    casters: &'a [PreparedCasterBounds],
    camera: &'a HostCameraFrame,
}

fn publish(step: &Step<'_>, lane: Lane, enable_fast: bool) -> (Published, Lane) {
    let scratch = VisibilityScratch {
        history: lane.history,
        spare: lane.spare,
        suspended: false,
        frame_scratch: lane.frame_scratch,
        cull_buffers: lane.buffers,
    };
    let (frame, frame_scratch, apply) = super::build_frame(
        step.scene,
        Some(step.prepared),
        step.scope,
        step.camera,
        &proj_with(60.0, None),
        None,
        scratch,
        enable_fast,
    );
    assert!(apply);
    let draws = step
        .calls
        .iter()
        .map(|(index, facts)| {
            let caster = step.casters[*index];
            frame.allows_draw(
                Some(CasterRef {
                    key: CasterKey::new(
                        caster.space_id,
                        caster.renderable_index,
                        caster.instance_id,
                        caster.node_id,
                        caster.skinned,
                    ),
                    mesh_asset_id: caster.mesh_asset_id,
                }),
                caster.aabb_min,
                caster.aabb_max,
                *facts,
            )
        })
        .collect();
    let recycled = frame.recycle();
    let mut rows: Vec<_> = recycled
        .history
        .iter()
        .map(|(key, record)| {
            (
                *key,
                record.class,
                record.aabb_min,
                record.aabb_max,
                record.mesh_asset_id,
            )
        })
        .collect();
    rows.sort_by_key(|row| (row.0.space_id, row.0.renderable_index, row.0.skinned));
    let published = Published {
        draws,
        rows,
        visible_count: recycled.history.visible_count(),
        epoch: recycled.history.epoch(),
        rects: recycled.buffers.stale_rects.clone(),
        armed: recycled.buffers.fast_published(),
    };
    (
        published,
        Lane {
            history: recycled.history,
            spare: recycled.spare,
            frame_scratch,
            buffers: recycled.buffers,
        },
    )
}

fn compare(step: &Step<'_>, fast: Lane, slow: Lane) -> (Published, Lane, Lane) {
    let (fast, fast_lane) = publish(step, fast, true);
    let (slow, slow_lane) = publish(step, slow, false);
    assert_published(&fast, &slow);
    (fast, fast_lane, slow_lane)
}

fn assert_published(fast: &Published, slow: &Published) {
    assert_eq!(fast.draws, slow.draws);
    assert_eq!(fast.rows, slow.rows);
    assert_eq!(fast.visible_count, slow.visible_count);
    assert_eq!(fast.epoch, slow.epoch);
    assert_eq!(fast.rects, slow.rects);
}

#[test]
fn build_frame_runs_two_views_and_matches_the_slow_path() {
    let scene = scene_with(&[RenderSpaceId(1), RenderSpaceId(2)]);
    let prepared = FramePreparedRenderables::test_with_caster_runs(&[
        (RenderSpaceId(1), 3),
        (RenderSpaceId(2), 2),
    ]);
    let shorter = FramePreparedRenderables::test_with_caster_runs(&[(RenderSpaceId(1), 2)]);
    let casters = collect(&prepared);
    let main: Vec<_> = casters
        .iter()
        .copied()
        .filter(|caster| caster.space_id == RenderSpaceId(1))
        .collect();
    let secondary: Vec<_> = casters
        .iter()
        .copied()
        .filter(|caster| caster.space_id == RenderSpaceId(2))
        .collect();
    let keep = OcclusionFacts::keep();
    let mut fast_main = Lane::new(history_of(&main));
    let mut slow_main = Lane::new(history_of(&main));
    let mut fast_secondary = Lane::new(history_of(&secondary));
    let mut slow_secondary = Lane::new(history_of(&secondary));
    let camera = camera_in_front_of_the_grid();
    let main_scope = ViewRenderSpaceScope::single(RenderSpaceId(1));
    let secondary_scope = ViewRenderSpaceScope::single(RenderSpaceId(2));

    for frame in 0..2 {
        let main_prepared = if frame == 0 { &prepared } else { &shorter };
        let main_casters = if frame == 0 { &main[..] } else { &main[..2] };
        let main_calls: Vec<_> = (0..main_casters.len()).map(|index| (index, keep)).collect();
        let (published, next_fast, next_slow) = compare(
            &Step {
                scene: &scene,
                prepared: main_prepared,
                scope: main_scope,
                calls: &main_calls,
                casters: main_casters,
                camera: &camera,
            },
            fast_main,
            slow_main,
        );
        assert!(published.rows.iter().all(|row| row.0.space_id == 1));
        if frame == 0 {
            assert!(published.armed);
            assert_eq!(published.rows.len(), 3);
            assert!(published.rects.is_empty());
        } else {
            assert!(!published.armed);
            assert_eq!(published.rows.len(), 2);
            assert!(
                !published.rects.is_empty(),
                "the absent caster punches a hole"
            );
        }
        fast_main = next_fast;
        slow_main = next_slow;

        let secondary_calls: Vec<_> = (0..secondary.len()).map(|index| (index, keep)).collect();
        let (published, next_fast, next_slow) = compare(
            &Step {
                scene: &scene,
                prepared: &prepared,
                scope: secondary_scope,
                calls: &secondary_calls,
                casters: &secondary,
                camera: &camera,
            },
            fast_secondary,
            slow_secondary,
        );
        assert!(published.armed);
        assert!(published.rows.iter().all(|row| row.0.space_id == 2));
        assert_eq!(published.rows.len(), 2);
        assert!(published.rects.is_empty());
        fast_secondary = next_fast;
        slow_secondary = next_slow;
    }
}

#[test]
fn seeded_build_frame_matches_the_slow_path() {
    let scene = scene_with(&[RenderSpaceId(1)]);
    let prepared = FramePreparedRenderables::test_with_caster_runs(&[(RenderSpaceId(1), 8)]);
    let casters = collect(&prepared);
    let scope = ViewRenderSpaceScope::single(RenderSpaceId(1));
    let camera = camera_in_front_of_the_grid();
    let mut fast_publishes = 0usize;
    let mut dedicated_fast = 0usize;
    let mut dedicated_frames = 0usize;
    for seed in 0..16u64 {
        // Even seeds stay on the single-space unskinned grid and mostly draw every caster, so
        // `build_frame` publishes on the fast path. Odd seeds keep the noisy call pattern.
        let uniform = seed % 2 == 0;
        let mut rng = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let mut fast_lane = Lane::new(history_of(&casters));
        let mut slow_lane = Lane::new(history_of(&casters));
        for _ in 0..12 {
            let steady = uniform && next() % 5 < 3;
            let calls = if steady {
                (0..casters.len())
                    .map(|index| (index, OcclusionFacts::keep()))
                    .collect()
            } else {
                noisy_calls(&casters, &mut next)
            };
            let step = Step {
                scene: &scene,
                prepared: &prepared,
                scope,
                calls: &calls,
                casters: &casters,
                camera: &camera,
            };
            let (fast, next_fast) = publish(&step, fast_lane, true);
            let (slow, next_slow) = publish(&step, slow_lane, false);
            assert_published(&fast, &slow);
            if fast.armed {
                fast_publishes += 1;
            }
            if uniform {
                dedicated_frames += 1;
                if fast.armed {
                    dedicated_fast += 1;
                }
            }
            fast_lane = next_fast;
            slow_lane = next_slow;
        }
    }
    assert!(
        dedicated_fast * 4 >= dedicated_frames,
        "fast publishes {fast_publishes} of 192 frames; single-space {dedicated_fast} of {dedicated_frames}"
    );
}

fn noisy_calls(
    casters: &[PreparedCasterBounds],
    next: &mut impl FnMut() -> u64,
) -> Vec<(usize, OcclusionFacts)> {
    let mut calls = Vec::new();
    for index in 0..casters.len() {
        if next().is_multiple_of(8) {
            continue;
        }
        let occluded = next().is_multiple_of(7);
        calls.push((
            index,
            OcclusionFacts {
                occluded,
                other_occluder: occluded,
                rects: [None, None],
            },
        ));
    }
    calls
}
