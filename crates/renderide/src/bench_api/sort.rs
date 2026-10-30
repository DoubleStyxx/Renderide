//! Transparent draw sorting workloads.

use std::sync::Arc;

/// Mixed transparent draws sorted by one [`TransparentSortWorkload`] sample at about a thousand rows.
pub const TRANSPARENT_SORT_1K: usize = 1_024;

/// Mixed transparent draws sorted by one [`TransparentSortWorkload`] sample at about ten thousand rows.
pub const TRANSPARENT_SORT_10K: usize = 10_240;

/// Unsorted transparent draws, half canvas stems and half world stems.
///
/// `sort_precomputed` reads the `f64` stored on the draw. `sort_recomputing_depth` rebuilds that
/// key on every compare, including the canvas stem-name match. `sort_master` is the `117a8df5`
/// farthest-corner comparator: it compares `camera_distance_sq` and does not match canvas stems.
/// Canvas-versus-world pairs can disagree between `sort_master` and `sort_precomputed`.
#[derive(Clone, Debug)]
pub struct TransparentSortWorkload {
    items: Vec<crate::world_mesh::WorldMeshDrawItem>,
}

impl TransparentSortWorkload {
    /// Builds `count` transparent draws with shuffled distances and equal `sorting_order`.
    ///
    /// Equal sorting order forces the comparator into the depth-key tie, which is the cost this
    /// bench measures. Canvas rows cycle `ui_unlit`, `ui_textunlit`, and `ui_circlesegment`.
    /// World rows use non-canvas embedded stems so the stem match still runs and returns false.
    #[must_use]
    pub fn mixed(count: usize) -> Self {
        use crate::materials::RasterPipelineKind;
        use crate::world_mesh::draw_prep::{pack_sort_prefix, sync_transparent_depth_key};
        use crate::world_mesh::test_fixtures::{DummyDrawItemSpec, dummy_world_mesh_draw_item};

        let mut items = Vec::with_capacity(count);
        for index in 0..count {
            let mut item = dummy_world_mesh_draw_item(DummyDrawItemSpec {
                material_asset_id: i32::try_from(index % 64).unwrap_or(0),
                property_block: None,
                skinned: false,
                sorting_order: 0,
                mesh_asset_id: 1,
                node_id: i32::try_from(index).unwrap_or(i32::MAX),
                slot_index: 0,
                collect_order: index,
                alpha_blended: true,
            });
            let mixed = index.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let meters = 0.4 + (mixed % 20_000) as f32 * 0.001;
            if index % 2 == 0 {
                let stem = match index % 6 {
                    0 => "ui_unlit_default",
                    2 => "ui_textunlit_multiview",
                    _ => "ui_circlesegment",
                };
                item.batch_key.pipeline = RasterPipelineKind::EmbeddedStem(Arc::from(stem));
                item.is_canvas_ui = true;
                item.canvas_plane_depth = Some(meters);
            } else {
                let stem = if index % 4 == 1 {
                    "unlit_default"
                } else {
                    "pbs_metallic_default"
                };
                item.batch_key.pipeline = RasterPipelineKind::EmbeddedStem(Arc::from(stem));
                item.is_canvas_ui = false;
                item.canvas_plane_depth = None;
            }
            item.camera_distance_sq = meters * meters;
            sync_transparent_depth_key(&mut item);
            item.sort_prefix = pack_sort_prefix(
                item.is_overlay,
                item.batch_key.render_queue,
                item.batch_key.uses_transparent_sorting(),
                item._opaque_depth_bucket,
                item.batch_key_hash,
            );
            items.push(item);
        }
        Self { items }
    }

    /// A canvas whose plane is farther than a world draw's farthest corner, while the canvas
    /// corner itself is nearer.
    ///
    /// Master orders the world draw first. The plane key orders the canvas first.
    #[must_use]
    pub fn plane_versus_corner() -> Self {
        use crate::materials::RasterPipelineKind;
        use crate::world_mesh::draw_prep::{pack_sort_prefix, sync_transparent_depth_key};
        use crate::world_mesh::test_fixtures::{DummyDrawItemSpec, dummy_world_mesh_draw_item};

        let mut items = Vec::with_capacity(2);
        for (index, canvas) in [true, false].into_iter().enumerate() {
            let mut item = dummy_world_mesh_draw_item(DummyDrawItemSpec {
                material_asset_id: 1,
                property_block: None,
                skinned: false,
                sorting_order: 0,
                mesh_asset_id: 1,
                node_id: i32::try_from(index).unwrap_or(0),
                slot_index: 0,
                collect_order: index,
                alpha_blended: true,
            });
            if canvas {
                item.batch_key.pipeline =
                    RasterPipelineKind::EmbeddedStem(Arc::from("ui_unlit_default"));
                item.is_canvas_ui = true;
                item.canvas_plane_depth = Some(9.0);
                item.camera_distance_sq = 1.0;
            } else {
                item.batch_key.pipeline =
                    RasterPipelineKind::EmbeddedStem(Arc::from("unlit_default"));
                item.is_canvas_ui = false;
                item.canvas_plane_depth = None;
                item.camera_distance_sq = 64.0;
            }
            sync_transparent_depth_key(&mut item);
            item.sort_prefix = pack_sort_prefix(
                item.is_overlay,
                item.batch_key.render_queue,
                item.batch_key.uses_transparent_sorting(),
                item._opaque_depth_bucket,
                item.batch_key_hash,
            );
            items.push(item);
        }
        Self { items }
    }

    /// Sorts a clone with the production precomputed-key comparator.
    #[must_use]
    pub fn sort_precomputed(self) -> u64 {
        let mut items = self.items;
        crate::world_mesh::draw_prep::sort_order_sensitive_draws_precomputed(&mut items)
    }

    /// Sorts a clone while recomputing the depth key on every compare.
    #[must_use]
    pub fn sort_recomputing_depth(self) -> u64 {
        let mut items = self.items;
        crate::world_mesh::draw_prep::sort_order_sensitive_draws_recomputing_depth(&mut items)
    }

    /// Sorts a clone with the master farthest-corner comparator.
    #[must_use]
    pub fn sort_master(self) -> u64 {
        let mut items = self.items;
        crate::world_mesh::draw_prep::sort_order_sensitive_draws_master(&mut items)
    }

    /// Canvas rows jittered by about `1e-6` m inside a `0.1` mm snap cluster.
    ///
    /// [`Self::mixed`] sits on a 1 mm grid, which never writes during the snap pass. This
    /// constructor is the input for [`Self::snap_siblings`].
    #[must_use]
    pub fn jittered_canvas_planes(count: usize) -> Self {
        use crate::world_mesh::draw_prep::sync_transparent_depth_key;

        let mut workload = Self::mixed(count);
        for (index, item) in workload.items.iter_mut().enumerate() {
            if !item.is_canvas_ui {
                continue;
            }
            let cluster = (index / 8) as f32 * 0.01;
            let jitter = ((index % 8) as f32 - 3.5) * 1.0e-6;
            item.canvas_plane_depth = Some(1.0 + cluster + jitter);
            sync_transparent_depth_key(item);
        }
        workload
    }

    /// Snaps one chunk of sibling plane depths and folds the stored depths into a checksum.
    #[must_use]
    pub fn snap_siblings(self) -> u64 {
        let mut chunks = vec![self.items];
        crate::world_mesh::draw_prep::snap_canvas_sibling_plane_depths(&mut chunks);
        let mut checksum = 0_u64;
        for item in &chunks[0] {
            if let Some(depth) = item.canvas_plane_depth {
                checksum = checksum.wrapping_add(u64::from(depth.to_bits()));
            }
            checksum = checksum.wrapping_add(item.transparent_depth_key.to_bits());
        }
        checksum
    }
}
