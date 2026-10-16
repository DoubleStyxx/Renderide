//! Batch-window routing for [`super::build_plan`].
//!
//! A "batch window" is a maximal run of consecutive draws sharing a `MaterialDrawBatchKey`. Within
//! a window the grouping policy is determined by material properties (intersection-pass, grab-pass,
//! transparency class) plus device capability (`supports_base_instance`).

use std::ops::Range;

use crate::world_mesh::draw_prep::WorldMeshDrawItem;
use crate::world_mesh::phase_classification::{WorldMeshBatchPhase, classify_world_mesh_batch};

use super::{DrawGroup, WorldMeshPhase};

/// Same-batch-key draw window and its subpass routing metadata.
#[derive(Clone, Debug)]
pub(super) struct BatchWindow {
    /// Draw index range covered by this window.
    pub(super) range: Range<usize>,
    /// Primary mesh render phase for the window.
    pub(super) phase: WorldMeshPhase,
    /// Whether every draw must remain a singleton group.
    pub(super) singleton: bool,
}

/// Returns the next same-batch-key window starting at `start`.
pub(super) fn next_batch_window(
    draws: &[WorldMeshDrawItem],
    start: usize,
    supports_base_instance: bool,
) -> BatchWindow {
    let key_hash = draws[start].batch_key_hash;
    let shadow_cast_mode = draws[start].shadow_cast_mode;
    let mut end = start + 1;
    while end < draws.len() {
        let candidate = &draws[end];
        // Hash mismatch implies a different batch key when the hash was refreshed with the key.
        // A matching hash still compares the key so collisions do not merge distinct materials.
        if candidate.batch_key_hash != key_hash
            || candidate.shadow_cast_mode != shadow_cast_mode
            || candidate.batch_key != draws[start].batch_key
        {
            break;
        }
        end += 1;
    }

    let classification = classify_world_mesh_batch(&draws[start].batch_key);

    BatchWindow {
        range: start..end,
        phase: classification.phase,
        singleton: draw_requires_singleton_for_classification(
            &draws[start],
            supports_base_instance,
            classification,
        ),
    }
}

/// Returns whether a draw must remain a singleton under the instancing policy.
pub(super) fn draw_requires_singleton_for_classification(
    item: &WorldMeshDrawItem,
    supports_base_instance: bool,
    classification: WorldMeshBatchPhase,
) -> bool {
    let order_dependent = !item.batch_key.transparent_class.allows_relaxed_batching();
    !supports_base_instance
        || item.skinned
        || item.material_stack_order.is_some()
        || (classification.strict_order && order_dependent)
        || classification.grab_pass
}

/// Appends `members` to `slab_layout` and returns a [`DrawGroup`] covering the new slab range.
#[inline]
pub(super) fn build_group(
    slab_layout: &mut Vec<usize>,
    representative_draw_idx: usize,
    members: &[usize],
) -> DrawGroup {
    let first_instance = slab_layout.len() as u32;
    slab_layout.extend_from_slice(members);
    let count = members.len() as u32;
    DrawGroup {
        representative_draw_idx,
        instance_range: first_instance..first_instance + count,
        material_packet_idx: 0,
    }
}
