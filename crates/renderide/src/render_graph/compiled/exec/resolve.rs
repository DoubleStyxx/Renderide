//! Resource resolution helpers: transient pool leases and imported-resource lookups plus
//! per-view target resolution used before every pass encode.

use hashbrown::HashMap;

use crate::camera::{CameraRenderRect, HostCameraFrame, ViewId};
use crate::frame_contract::{OffscreenWriteTarget, RenderTextureSelfSampling, ViewWinding};
use crate::gpu::GpuContext;
use crate::gpu_resource::TextureViewDescriptorKey;

use super::super::super::context::{
    GraphResolvedResources, ResolvedGraphBuffer, ResolvedGraphTexture, ResolvedImportedBuffer,
    ResolvedImportedHistoryTexture, ResolvedImportedTexture,
};
use super::super::super::error::GraphExecuteError;
use super::super::super::history::{HistoryRegistry, HistoryResourceScope};
use super::super::super::pool::{BufferKey, TextureKey, TransientPool};
use super::super::super::resources::{
    BackendFrameBufferKind, BufferImportSource, FrameTargetRole, HistorySlotId, ImportSource,
    ImportedBufferDecl, ImportedBufferHandle, ImportedTextureDecl, ImportedTextureHandle,
    SubresourceHandle, TextureHandle, TransientExtent,
};
use super::super::frame_view::FrameViewTargetKind;
use super::super::helpers;
use super::super::{
    CompiledRenderGraph, FrameViewLayout, FrameViewTarget, RenderPathProfile, ResolvedView,
};
use super::{OwnedResolvedView, ResolvedOffscreenColorCopy, TransientTextureResolveSurfaceParams};

fn subresource_view_dimension(
    dimension: wgpu::TextureDimension,
    array_layer_count: u32,
) -> Option<wgpu::TextureViewDimension> {
    match dimension {
        wgpu::TextureDimension::D1 => Some(wgpu::TextureViewDimension::D1),
        wgpu::TextureDimension::D2 if array_layer_count > 1 => {
            Some(wgpu::TextureViewDimension::D2Array)
        }
        wgpu::TextureDimension::D2 => Some(wgpu::TextureViewDimension::D2),
        wgpu::TextureDimension::D3 => Some(wgpu::TextureViewDimension::D3),
    }
}

/// Walks `compiled`, skipping entries with no physical slot, deduplicating leases per slot, and
/// invoking `store` with the resolved resource for each compiled index. `build` is called at most
/// once per physical slot and caches its result for subsequent compiled entries that alias to the
/// same slot. Shared by transient texture and buffer resolution.
fn resolve_transient_resources<C, R: Clone>(
    compiled: &[C],
    physical_slot: impl Fn(&C) -> Option<usize>,
    mut build: impl FnMut(&C) -> Result<R, GraphExecuteError>,
    mut store: impl FnMut(usize, R),
) -> Result<(), GraphExecuteError> {
    let mut slots: HashMap<usize, R> = HashMap::new();
    for (idx, c) in compiled.iter().enumerate() {
        let Some(slot) = physical_slot(c) else {
            continue;
        };
        let resolved = if let Some(existing) = slots.get(&slot) {
            existing.clone()
        } else {
            let r = build(c)?;
            slots.insert(slot, r.clone());
            r
        };
        store(idx, resolved);
    }
    Ok(())
}

impl CompiledRenderGraph {
    /// Acquires transient texture leases for this view and inserts them into `resources`.
    pub(super) fn resolve_transient_textures(
        &self,
        device: &wgpu::Device,
        limits: &crate::gpu::GpuLimits,
        pool: &mut TransientPool,
        surface: TransientTextureResolveSurfaceParams,
        resources: &mut GraphResolvedResources,
    ) -> Result<(), GraphExecuteError> {
        profiling::scope!("render::resolve_transient_textures");
        resolve_transient_resources(
            &self.transient_textures,
            |compiled| {
                (compiled.lifetime.is_some() && compiled.physical_slot != usize::MAX)
                    .then_some(compiled.physical_slot)
            },
            |compiled| {
                let array_layers = compiled.desc.array_layers.resolve(surface.multiview_stereo);
                let format = compiled.desc.format.resolve(
                    surface.surface_format,
                    surface.depth_stencil_format,
                    surface.scene_color_format,
                );
                let extent = helpers::resolve_transient_extent(
                    compiled.desc.extent,
                    surface.viewport_px,
                    array_layers,
                );
                let mip_levels = helpers::clamp_mip_levels_for_transient_extent(
                    compiled.desc.mip_levels,
                    extent,
                    compiled.desc.dimension,
                    array_layers,
                );
                if mip_levels != compiled.desc.mip_levels.max(1) {
                    logger::trace!(
                        "transient texture '{}' mip count clamped from {} to {} for resolved extent {:?}",
                        compiled.desc.label,
                        compiled.desc.mip_levels.max(1),
                        mip_levels,
                        extent
                    );
                }
                let key = TextureKey {
                    format,
                    extent,
                    mip_levels,
                    sample_count: compiled.desc.sample_count.resolve(surface.sample_count),
                    dimension: compiled.desc.dimension,
                    array_layers,
                    usage_bits: u64::from(compiled.usage.bits()),
                };
                let (width, height) = match key.extent {
                    TransientExtent::Custom { width, height } => (width.max(1), height.max(1)),
                    TransientExtent::MultiLayer { width, height, .. } => {
                        (width.max(1), height.max(1))
                    }
                    TransientExtent::Backbuffer
                    | TransientExtent::BackbufferDivisor { .. }
                    | TransientExtent::BackbufferDivisorMip { .. }
                    | TransientExtent::BackbufferScaledMip { .. } => surface.viewport_px,
                };
                let lease = pool.acquire_texture_resource(
                    device,
                    limits,
                    key,
                    compiled.desc.label,
                    compiled.usage,
                )?;
                let layer_views = helpers::create_transient_layer_views(
                    &lease.texture,
                    lease.view_cache.as_ref(),
                    lease.resource_generation,
                    key,
                );
                Ok(ResolvedGraphTexture {
                    pool_id: lease.pool_id,
                    texture: lease.texture,
                    view: lease.view,
                    view_cache: lease.view_cache,
                    resource_generation: lease.resource_generation,
                    width,
                    height,
                    layer_views,
                    mip_levels: key.mip_levels.max(1),
                    array_layers: key.array_layers.max(1),
                    dimension: key.dimension,
                })
            },
            |idx, resolved| {
                resources.set_transient_texture(TextureHandle(idx as u32), resolved);
            },
        )
    }

    /// Acquires transient buffer leases for this view and inserts them into `resources`.
    pub(super) fn resolve_transient_buffers(
        &self,
        device: &wgpu::Device,
        limits: &crate::gpu::GpuLimits,
        pool: &mut TransientPool,
        viewport_px: (u32, u32),
        resources: &mut GraphResolvedResources,
    ) -> Result<(), GraphExecuteError> {
        profiling::scope!("render::resolve_transient_buffers");
        resolve_transient_resources(
            &self.transient_buffers,
            |compiled| {
                (compiled.lifetime.is_some() && compiled.physical_slot != usize::MAX)
                    .then_some(compiled.physical_slot)
            },
            |compiled| {
                let key = BufferKey {
                    size_policy: compiled.desc.size_policy,
                    usage_bits: u64::from(compiled.usage.bits()),
                };
                let size = helpers::resolve_buffer_size(compiled.desc.size_policy, viewport_px);
                let lease = pool.acquire_buffer_resource(
                    device,
                    limits,
                    key,
                    compiled.desc.label,
                    compiled.usage,
                    size,
                )?;
                Ok(ResolvedGraphBuffer {
                    pool_id: lease.pool_id,
                    buffer: lease.buffer,
                })
            },
            |idx, resolved| {
                resources.set_transient_buffer(
                    super::super::super::resources::BufferHandle(idx as u32),
                    resolved,
                );
            },
        )
    }

    /// Binds imported textures (frame color / depth attachments) into `resources`.
    pub(super) fn resolve_imported_textures(
        &self,
        resolved: &ResolvedView<'_>,
        history: &HistoryRegistry,
        resources: &mut GraphResolvedResources,
    ) -> Result<(), GraphExecuteError> {
        profiling::scope!("render::resolve_imported_textures");
        for (idx, import) in self.imported_textures.iter().enumerate() {
            let resolved_import = match &import.source {
                ImportSource::Frame(FrameTargetRole::ColorAttachment) => resolved
                    .backbuffer
                    .cloned()
                    .map(|view| ResolvedImportedTexture {
                        view,
                        history: None,
                    }),
                ImportSource::Frame(FrameTargetRole::DepthAttachment) => {
                    Some(ResolvedImportedTexture {
                        view: resolved.depth_view.clone(),
                        history: None,
                    })
                }
                #[cfg(test)]
                ImportSource::External => None,
                ImportSource::PingPong(slot) => {
                    let scope = history_scope_for_texture(*slot, resolved);
                    let texture_slot =
                        history.texture_slot_scoped(*slot, scope).ok_or_else(|| {
                            GraphExecuteError::missing_history_texture(*slot, import.label)
                        })?;
                    let (half_idx, half_name) = texture_history_half(import, history);
                    let (view, texture, mip_views) = {
                        let guard = texture_slot.lock();
                        let texture = guard.half(half_idx).ok_or_else(|| {
                            GraphExecuteError::unallocated_history_texture(*slot, half_name)
                        })?;
                        let resolved_texture = (
                            texture.view.clone(),
                            texture.texture.clone(),
                            texture.mip_views.clone(),
                        );
                        drop(guard);
                        resolved_texture
                    };
                    Some(ResolvedImportedTexture {
                        view,
                        history: Some(ResolvedImportedHistoryTexture { texture, mip_views }),
                    })
                }
            };
            if let Some(resolved_import) = resolved_import {
                resources.set_imported_texture(ImportedTextureHandle(idx as u32), resolved_import);
            }
        }
        Ok(())
    }

    /// Resolves subresource views declared on [`super::super::CompiledRenderGraph::subresources`]
    /// against their parent transient texture.
    ///
    /// Run after [`Self::resolve_transient_textures`] so the parent `wgpu::Texture` handles
    /// already exist. Subresources whose parent is not resolved (because the parent's transient
    /// index is culled or its lifetime is `None`) are left as `None` -- callers that look them up
    /// get a harmless `None` instead of an encoder-time panic.
    pub(super) fn resolve_subresource_views(&self, resources: &mut GraphResolvedResources) {
        if self.subresources.is_empty() {
            return;
        }
        profiling::scope!("render::resolve_subresource_views");
        for (idx, desc) in self.subresources.iter().enumerate() {
            let Some(parent) = resources.transient_texture(desc.parent) else {
                continue;
            };
            if !desc.fits_resolved_parent(parent.mip_levels, parent.array_layers) {
                logger::trace!(
                    "render graph subresource '{}' skipped: mip {}+{} layer {}+{} exceeds \
                     resolved parent {:?} (mips={}, layers={})",
                    desc.label,
                    desc.base_mip_level,
                    desc.mip_level_count,
                    desc.base_array_layer,
                    desc.array_layer_count,
                    desc.parent,
                    parent.mip_levels,
                    parent.array_layers
                );
                continue;
            }
            let desc = wgpu::TextureViewDescriptor {
                label: Some(desc.label),
                dimension: subresource_view_dimension(parent.dimension, desc.array_layer_count),
                base_mip_level: desc.base_mip_level,
                mip_level_count: Some(desc.mip_level_count.max(1)),
                base_array_layer: desc.base_array_layer,
                array_layer_count: Some(desc.array_layer_count.max(1)),
                ..Default::default()
            };
            let (view, created) = parent.view_cache.get_or_create(
                &parent.texture,
                parent.resource_generation,
                TextureViewDescriptorKey::from_descriptor(&desc),
                desc.label,
            );
            if created {
                crate::profiling::note_resource_churn!(
                    TextureView,
                    "render_graph::subresource_view"
                );
            }
            resources.set_subresource_view(SubresourceHandle(idx as u32), view);
        }
    }

    /// Binds imported backend buffers (lights, cluster tables, per-draw slab) into `resources`.
    pub(super) fn resolve_imported_buffers(
        &self,
        frame_resources: &dyn super::super::super::GraphFrameResources,
        history: &HistoryRegistry,
        resolved: &ResolvedView<'_>,
        resources: &mut GraphResolvedResources,
    ) -> Result<(), GraphExecuteError> {
        profiling::scope!("render::resolve_imported_buffers");
        // All views share one cluster buffer; safe under single-submit because each view's
        // compute-then-raster sequence completes before the next view's compute overwrites.
        let cluster_refs = frame_resources.shared_cluster_buffer_refs();
        for (idx, import) in self.imported_buffers.iter().enumerate() {
            let buffer = match &import.source {
                BufferImportSource::Frame(BackendFrameBufferKind::Lights) => {
                    frame_resources.lights_buffer(resolved.view_id)
                }
                BufferImportSource::Frame(BackendFrameBufferKind::FrameUniforms) => {
                    frame_resources.frame_uniform_buffer()
                }
                BufferImportSource::Frame(BackendFrameBufferKind::ClusterLightCounts) => {
                    cluster_refs
                        .as_ref()
                        .map(|refs| refs.cluster_light_counts.clone())
                }
                BufferImportSource::Frame(BackendFrameBufferKind::ClusterLightIndices) => {
                    cluster_refs
                        .as_ref()
                        .map(|refs| refs.cluster_light_indices.clone())
                }
                BufferImportSource::Frame(BackendFrameBufferKind::PerDrawSlab) => {
                    frame_resources.per_view_per_draw_storage(resolved.view_id)
                }
                #[cfg(test)]
                BufferImportSource::External => None,
                BufferImportSource::PingPong(slot) => {
                    let scope = history_scope_for_buffer(*slot, resolved);
                    let buffer_slot =
                        history.buffer_slot_scoped(*slot, scope).ok_or_else(|| {
                            GraphExecuteError::missing_history_buffer(*slot, import.label)
                        })?;
                    let (half_idx, half_name) = buffer_history_half(import, history);
                    let buffer = {
                        let guard = buffer_slot.lock();
                        guard
                            .half(half_idx)
                            .ok_or_else(|| {
                                GraphExecuteError::unallocated_history_buffer(*slot, half_name)
                            })?
                            .clone()
                    };
                    Some(buffer)
                }
            };
            if let Some(buffer) = buffer {
                resources.set_imported_buffer(
                    ImportedBufferHandle(idx as u32),
                    ResolvedImportedBuffer { buffer },
                );
            }
        }
        Ok(())
    }

    /// Resolves a [`FrameViewTarget`] into a [`ResolvedView`] with color/depth attachments.
    pub(super) fn resolve_view_from_target<'a>(
        view_id: ViewId,
        view_winding: ViewWinding,
        profile: RenderPathProfile,
        host_camera: &HostCameraFrame,
        target: &'a FrameViewTarget<'a>,
        gpu: &'a mut GpuContext,
        backbuffer_view_holder: Option<&'a wgpu::TextureView>,
    ) -> Result<ResolvedView<'a>, GraphExecuteError> {
        let layout = FrameViewLayout::resolve(host_camera, profile, target, gpu);
        match target {
            FrameViewTarget::Swapchain => {
                let Some(bb_ref) = backbuffer_view_holder else {
                    return Err(GraphExecuteError::MissingSwapchainView);
                };
                let (depth_tex, depth_view) = gpu
                    .ensure_depth_target()
                    .map_err(GraphExecuteError::DepthTarget)?;

                Ok(ResolvedView {
                    depth_texture: depth_tex,
                    depth_view,
                    backbuffer: Some(bb_ref),
                    surface_format: layout.surface_format,
                    viewport_px: layout.viewport_px,
                    multiview_stereo: layout.multiview_stereo,
                    offscreen_write_target: OffscreenWriteTarget::None,
                    view_winding,
                    view_id,
                    sample_count: layout.sample_count,
                    post_processing: layout.post_processing,
                    color_texture: None,
                    history_color_texture: None,
                    seed_published_color: false,
                    persistent_depth_target: non_offscreen_persistent_depth(
                        FrameViewTargetKind::Swapchain,
                        layout,
                    ),
                })
            }
            FrameViewTarget::ExternalMultiview(ext) => Ok(ResolvedView {
                depth_texture: ext.depth_texture,
                depth_view: ext.depth_view,
                backbuffer: Some(ext.color_view),
                surface_format: layout.surface_format,
                viewport_px: layout.viewport_px,
                multiview_stereo: layout.multiview_stereo,
                offscreen_write_target: OffscreenWriteTarget::None,
                view_winding,
                view_id,
                sample_count: layout.sample_count,
                post_processing: layout.post_processing,
                color_texture: None,
                history_color_texture: None,
                seed_published_color: false,
                persistent_depth_target: non_offscreen_persistent_depth(
                    FrameViewTargetKind::ExternalMultiview,
                    layout,
                ),
            }),
            FrameViewTarget::OffscreenRt(ext) => {
                let inputs = clear_load_inputs_from_offscreen(
                    ext.extent_px,
                    ext.copy_to_color,
                    layout.sample_count,
                    layout.surface_format,
                    ext.write_target,
                );
                Ok(ResolvedView {
                    depth_texture: ext.depth_texture,
                    depth_view: ext.depth_view,
                    backbuffer: Some(ext.color_view),
                    surface_format: layout.surface_format,
                    viewport_px: layout.viewport_px,
                    multiview_stereo: layout.multiview_stereo,
                    offscreen_write_target: ext.write_target,
                    view_winding,
                    view_id,
                    sample_count: layout.sample_count,
                    post_processing: layout.post_processing,
                    color_texture: inputs.has_color_texture.then_some(ext.color_texture),
                    history_color_texture: ext.history_color_texture,
                    seed_published_color: ext.seed_published_color,
                    persistent_depth_target: inputs.persistent_depth_target,
                })
            }
        }
    }

    /// Resolves a view target into owned metadata and reusable attachments without requiring a
    /// swapchain image to be acquired.
    pub(super) fn resolve_owned_view_metadata_from_target(
        view_id: ViewId,
        view_winding: ViewWinding,
        profile: RenderPathProfile,
        host_camera: &HostCameraFrame,
        target: &FrameViewTarget<'_>,
        gpu: &mut GpuContext,
    ) -> Result<OwnedResolvedView, GraphExecuteError> {
        let layout = FrameViewLayout::resolve(host_camera, profile, target, gpu);
        match target {
            FrameViewTarget::Swapchain => {
                let (depth_texture, depth_view) = gpu
                    .ensure_depth_target()
                    .map_err(GraphExecuteError::DepthTarget)?;
                Ok(OwnedResolvedView {
                    depth_texture: depth_texture.clone(),
                    depth_view: depth_view.clone(),
                    backbuffer: None,
                    surface_format: layout.surface_format,
                    viewport_px: layout.viewport_px,
                    multiview_stereo: layout.multiview_stereo,
                    offscreen_write_target: OffscreenWriteTarget::None,
                    view_winding,
                    view_id,
                    sample_count: layout.sample_count,
                    post_processing: layout.post_processing,
                    color_texture: None,
                    history_color_texture: None,
                    seed_published_color: false,
                    persistent_depth_target: non_offscreen_persistent_depth(
                        FrameViewTargetKind::Swapchain,
                        layout,
                    ),
                    offscreen_color_copy: None,
                })
            }
            FrameViewTarget::ExternalMultiview(ext) => Ok(OwnedResolvedView {
                depth_texture: (*ext.depth_texture).clone(),
                depth_view: (*ext.depth_view).clone(),
                backbuffer: Some((*ext.color_view).clone()),
                surface_format: layout.surface_format,
                viewport_px: layout.viewport_px,
                multiview_stereo: layout.multiview_stereo,
                offscreen_write_target: OffscreenWriteTarget::None,
                view_winding,
                view_id,
                sample_count: layout.sample_count,
                post_processing: layout.post_processing,
                color_texture: None,
                history_color_texture: None,
                seed_published_color: false,
                persistent_depth_target: non_offscreen_persistent_depth(
                    FrameViewTargetKind::ExternalMultiview,
                    layout,
                ),
                offscreen_color_copy: None,
            }),
            FrameViewTarget::OffscreenRt(ext) => {
                let inputs = clear_load_inputs_from_offscreen(
                    ext.extent_px,
                    ext.copy_to_color,
                    layout.sample_count,
                    layout.surface_format,
                    ext.write_target,
                );
                Ok(OwnedResolvedView {
                    depth_texture: (*ext.depth_texture).clone(),
                    depth_view: (*ext.depth_view).clone(),
                    backbuffer: Some((*ext.color_view).clone()),
                    surface_format: layout.surface_format,
                    viewport_px: layout.viewport_px,
                    multiview_stereo: layout.multiview_stereo,
                    offscreen_write_target: ext.write_target,
                    view_winding,
                    view_id,
                    sample_count: layout.sample_count,
                    post_processing: layout.post_processing,
                    color_texture: inputs
                        .has_color_texture
                        .then(|| (*ext.color_texture).clone()),
                    history_color_texture: ext
                        .history_color_texture
                        .map(|texture| (*texture).clone()),
                    seed_published_color: ext.seed_published_color,
                    persistent_depth_target: inputs.persistent_depth_target,
                    offscreen_color_copy: ext.copy_to_color.map(|copy| {
                        ResolvedOffscreenColorCopy {
                            source_texture: (*ext.color_texture).clone(),
                            destination_texture: (*copy.destination_texture).clone(),
                            destination_origin_px: copy.destination_origin_px,
                            extent_px: copy.extent_px,
                        }
                    }),
                })
            }
        }
    }
}

/// Facts copied from a resolved view into the camera-clear load-op decision.
///
/// Window and OpenXR targets never expose a color texture. An offscreen target does only when
/// its viewport rect covers the whole render texture and no partial-color copy was recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClearLoadViewSource {
    /// Swapchain, OpenXR, or offscreen render texture.
    pub kind: FrameViewTargetKind,
    /// Camera rectangle inside the render texture. Ignored for window and OpenXR.
    pub viewport: CameraRenderRect,
    /// Full render-texture size. Ignored for window and OpenXR.
    pub target_extent_px: (u32, u32),
    /// `true` when this view copies a scratch color image onto a destination rect.
    ///
    /// Production sets this from `copy_to_color`. A copy means the viewport is partial even if
    /// the recorded rect happens to cover the destination.
    pub copies_partial_color: bool,
    /// Raster sample count from the view layout.
    pub sample_count: u32,
    /// Camera color-target format.
    pub surface_format: wgpu::TextureFormat,
    /// HDR scene-color format. Copied onto the decision inputs; the load-op policy compares it
    /// with [`Self::surface_format`].
    pub scene_color_format: wgpu::TextureFormat,
    /// Offscreen write target. Window and OpenXR ignore it and report no self-sampling policy.
    pub write_target: OffscreenWriteTarget,
}

/// Load-op inputs derived from [`ClearLoadViewSource`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClearLoadDecisionInputs {
    /// Raster sample count, copied from the view.
    pub sample_count: u32,
    /// `true` when a full-target offscreen color texture can be copied into HDR scene color.
    pub has_color_texture: bool,
    /// Camera color-target format, copied from the view.
    pub surface_format: wgpu::TextureFormat,
    /// HDR scene-color format, copied from the view.
    pub scene_color_format: wgpu::TextureFormat,
    /// Host render-texture self-sampling policy. `None` for window and OpenXR.
    pub self_sampling: Option<RenderTextureSelfSampling>,
    /// `true` when depth is the persistent camera image rather than a partial-viewport scratch.
    pub persistent_depth_target: bool,
}

/// Maps one view onto the inputs `forward_clear_loads` reads.
#[must_use]
pub(crate) fn clear_load_inputs_from_view(source: ClearLoadViewSource) -> ClearLoadDecisionInputs {
    let full_offscreen = source.kind == FrameViewTargetKind::OffscreenRt
        && !source.copies_partial_color
        && source.viewport.is_full_target(source.target_extent_px);
    ClearLoadDecisionInputs {
        sample_count: source.sample_count,
        has_color_texture: full_offscreen,
        surface_format: source.surface_format,
        scene_color_format: source.scene_color_format,
        self_sampling: if source.kind == FrameViewTargetKind::OffscreenRt {
            source.write_target.render_texture_self_sampling()
        } else {
            None
        },
        persistent_depth_target: match source.kind {
            FrameViewTargetKind::Swapchain | FrameViewTargetKind::ExternalMultiview => true,
            FrameViewTargetKind::OffscreenRt => full_offscreen,
        },
    }
}

fn non_offscreen_persistent_depth(kind: FrameViewTargetKind, layout: FrameViewLayout) -> bool {
    let inputs = clear_load_inputs_from_view(ClearLoadViewSource {
        kind,
        viewport: CameraRenderRect {
            origin_px: (0, 0),
            extent_px: layout.viewport_px,
        },
        target_extent_px: layout.viewport_px,
        copies_partial_color: false,
        sample_count: layout.sample_count,
        surface_format: layout.surface_format,
        scene_color_format: layout.surface_format,
        write_target: OffscreenWriteTarget::None,
    });
    debug_assert!(!inputs.has_color_texture);
    inputs.persistent_depth_target
}

fn clear_load_inputs_from_offscreen(
    extent_px: (u32, u32),
    copy_to_color: Option<super::super::OffscreenColorCopyTarget<'_>>,
    sample_count: u32,
    surface_format: wgpu::TextureFormat,
    write_target: OffscreenWriteTarget,
) -> ClearLoadDecisionInputs {
    let (viewport, copies_partial_color) = match copy_to_color {
        None => (
            CameraRenderRect {
                origin_px: (0, 0),
                extent_px,
            },
            false,
        ),
        Some(copy) => (
            CameraRenderRect {
                origin_px: copy.destination_origin_px,
                extent_px: copy.extent_px,
            },
            true,
        ),
    };
    clear_load_inputs_from_view(ClearLoadViewSource {
        kind: FrameViewTargetKind::OffscreenRt,
        viewport,
        target_extent_px: extent_px,
        copies_partial_color,
        sample_count,
        surface_format,
        scene_color_format: surface_format,
        write_target,
    })
}

/// Resolves a texture history slot's scope for the current view.
fn history_scope_for_texture(
    slot: HistorySlotId,
    resolved: &ResolvedView<'_>,
) -> HistoryResourceScope {
    if slot == HistorySlotId::HI_Z {
        HistoryResourceScope::View(resolved.view_id)
    } else {
        HistoryResourceScope::Global
    }
}

/// Resolves a buffer history slot's scope for the current view.
fn history_scope_for_buffer(
    slot: HistorySlotId,
    resolved: &ResolvedView<'_>,
) -> HistoryResourceScope {
    if slot == HistorySlotId::HI_Z {
        HistoryResourceScope::View(resolved.view_id)
    } else {
        HistoryResourceScope::Global
    }
}

/// Selects the ping-pong half for a texture import based on declared access intent.
fn texture_history_half(
    import: &ImportedTextureDecl,
    history: &HistoryRegistry,
) -> (usize, &'static str) {
    if import.initial_access.writes() || import.final_access.writes() {
        (history.current_index(), "current")
    } else {
        (history.previous_index(), "previous")
    }
}

/// Selects the ping-pong half for a buffer import based on declared access intent.
fn buffer_history_half(
    import: &ImportedBufferDecl,
    history: &HistoryRegistry,
) -> (usize, &'static str) {
    if import.initial_access.writes() || import.final_access.writes() {
        (history.current_index(), "current")
    } else {
        (history.previous_index(), "previous")
    }
}

#[cfg(test)]
#[path = "resolve_clear_load_inputs_tests.rs"]
mod clear_load_inputs_tests;
