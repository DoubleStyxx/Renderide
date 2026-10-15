//! Per-draw slab bind-group rebind helper for the world-mesh forward encoder.
//!
//! On base-instance-capable devices all per-draw slab rows are addressed by `first_instance`, so
//! the dynamic offset is fixed at zero and only the first draw needs to bind the group. On
//! downlevel devices each group carries a single instance and the slab row is selected via the
//! dynamic offset on `@group(2)`. This helper folds both into one rebind-if-changed call so the
//! encoder driver does not have to branch on the device capability.

use crate::gpu::GpuLimits;
use crate::mesh_deform::PER_DRAW_UNIFORM_STRIDE;

/// Per-draw slab bind request for one draw group.
pub(super) struct PerDrawSlabBind<'a> {
    /// Pipeline bind-group index used by the active shader.
    pub(super) bind_group_index: u32,
    /// Per-draw storage bind group.
    pub(super) bind_group: &'a wgpu::BindGroup,
    /// Device limits snapshot.
    pub(super) gpu_limits: &'a GpuLimits,
    /// First row in slab coordinates.
    pub(super) slab_first_instance: usize,
    /// Number of instances in the draw group.
    pub(super) instance_count: u32,
    /// Whether instance indices directly address slab rows.
    pub(super) supports_base_instance: bool,
}

/// Binds the per-draw slab `@group(N)` only when its dynamic offset differs from the previously
/// bound one. See module docs for the base-instance vs downlevel split.
pub(super) fn bind_per_draw_slab_if_changed(
    rpass: &mut wgpu::RenderPass<'_>,
    bind: PerDrawSlabBind<'_>,
    last_per_draw_dyn_offset: &mut Option<u32>,
) {
    let PerDrawSlabBind {
        bind_group_index,
        bind_group,
        gpu_limits,
        slab_first_instance,
        instance_count,
        supports_base_instance,
    } = bind;
    let storage_align = gpu_limits.min_storage_buffer_offset_alignment();
    let per_draw_dyn_offset = if supports_base_instance {
        0u32
    } else {
        debug_assert_eq!(instance_count, 1);
        let raw = (slab_first_instance * PER_DRAW_UNIFORM_STRIDE) as u32;
        debug_assert_eq!(
            raw % storage_align,
            0,
            "per-draw offset must match min_storage_buffer_offset_alignment"
        );
        raw
    };
    if *last_per_draw_dyn_offset != Some(per_draw_dyn_offset) {
        rpass.set_bind_group(bind_group_index, bind_group, &[per_draw_dyn_offset]);
        *last_per_draw_dyn_offset = Some(per_draw_dyn_offset);
    }
}
