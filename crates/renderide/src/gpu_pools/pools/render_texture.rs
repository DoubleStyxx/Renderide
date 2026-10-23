//! GPU render targets for host [`crate::shared::SetRenderTextureFormat`] (Unity `RenderTexture` assets).
//!
//! Color textures use `RENDER_ATTACHMENT | TEXTURE_BINDING` so the same asset can be sampled from
//! materials after the offscreen pass. Depth buffers are separate textures when `depth > 0`; depth
//! also includes `COPY_SRC` so [`crate::backend::frame_gpu::FrameGpuResources::copy_scene_depth_snapshot`]
//! can copy scene depth for intersection / frame bindings (same as main `renderide-depth`).
//!
//! ### Orientation
//!
//! Render textures are sampled through the same material UV path as host-uploaded textures.
//! The renderer does not rewrite material `_ST` values based on texture asset kind; tiling and
//! offset remain authored material data, and shader sampling helpers apply the renderer-wide
//! texture-origin convention uniformly.
//!
//! ### Wrap policy
//!
//! Render-texture sampler state intentionally mirrors the host's U/V wrap modes. Camera-preview
//! targets that must avoid opposite-edge bleed should be created with
//! [`crate::shared::TextureWrapMode::Clamp`] by the host; the renderer does not infer clamp from
//! "is written by a camera" because repeat is valid authorable behavior for sampled render
//! textures.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::assets::texture::estimate_gpu_texture_bytes;
use crate::gpu::GpuLimits;
use crate::gpu_pools::VramResourceKind;
use crate::gpu_pools::impl_gpu_resource;
use crate::gpu_pools::resource_pool::{
    GpuResourcePool, UntrackedAccess, impl_resident_pool_facade,
};
use crate::gpu_pools::sampler_state::SamplerState;
use crate::shared::SetRenderTextureFormat;

static NEXT_RENDER_TEXTURE_VIEW_GENERATION: AtomicU64 = AtomicU64::new(1);

/// One color image in a render texture's published/previous pair.
#[derive(Debug)]
struct RenderTextureColorImage {
    /// Color storage. Becomes the camera write target on the frame it is published.
    texture: Arc<wgpu::Texture>,
    /// Full-mip view of `texture`.
    view: Arc<wgpu::TextureView>,
    /// Bind-cache identity for `view`. Stays with the image across swaps.
    view_generation: u64,
}

/// Host render texture mirrored as a wgpu color target + optional depth.
#[derive(Debug)]
pub struct GpuRenderTexture {
    /// Host render-texture asset id.
    pub asset_id: i32,
    /// Color target (`Rgba16Float`); sampleable after offscreen draws.
    ///
    /// When double buffering is active this is the image the camera is writing this frame.
    /// Materials that are allowed to see the previous frame sample the history image.
    pub color_texture: Arc<wgpu::Texture>,
    /// Default view over the full color mip.
    pub color_view: Arc<wgpu::TextureView>,
    /// Monotonic identifier for the current bindable color view.
    pub color_view_generation: u64,
    /// Second color image allocated the first time a camera double-buffers this texture.
    ///
    /// Absent until that flag is set, so render textures that never double-buffer keep a single
    /// color allocation. Once created it is reused; each frame swaps it with the published image.
    previous_color: Option<RenderTextureColorImage>,
    /// Optional depth texture (always allocated for scene draws in [`Self::new_from_format`]).
    pub depth_texture: Option<Arc<wgpu::Texture>>,
    /// View over `depth_texture` when present.
    pub depth_view: Option<Arc<wgpu::TextureView>>,
    /// wgpu format of `color_texture`.
    pub wgpu_color_format: wgpu::TextureFormat,
    /// Pixel width of the render target.
    pub width: u32,
    /// Pixel height of the render target.
    pub height: u32,
    /// Estimated VRAM for color + depth.
    pub resident_bytes: u64,
    /// Sampler state mirrored from host format for material binds.
    pub sampler: SamplerState,
}

impl_gpu_resource!(GpuRenderTexture);

impl GpuRenderTexture {
    /// Creates GPU storage for a host [`SetRenderTextureFormat`].
    ///
    /// Color format is always [`wgpu::TextureFormat::Rgba16Float`] for host render-texture parity.
    /// Depth is always [`Depth32Float`] (or the device-preferred depth/stencil chosen by
    /// [`crate::gpu::main_forward_depth_stencil_format`]). Size is clamped per edge via
    /// [`GpuLimits::clamp_render_texture_edge`].
    pub fn new_from_format(
        device: &wgpu::Device,
        limits: &GpuLimits,
        fmt: &SetRenderTextureFormat,
    ) -> Option<Self> {
        let w = limits.clamp_render_texture_edge(fmt.size.x);
        let h = limits.clamp_render_texture_edge(fmt.size.y);
        if w == 0 || h == 0 {
            return None;
        }
        let max_dim = limits.max_texture_dimension_2d();
        if w > max_dim || h > max_dim {
            logger::warn!(
                "render texture {}: size {}x{} exceeds max_texture_dimension_2d ({max_dim})",
                fmt.asset_id,
                w,
                h
            );
            return None;
        }

        let wgpu_color_format = wgpu::TextureFormat::Rgba16Float;
        let size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };
        let published = create_color_image(
            device,
            &format!("RenderTexture {}", fmt.asset_id),
            size,
            wgpu_color_format,
            ColorImageRole::Published,
        );

        // Host `depth` is Unity depth-stencil bits; when zero the asset may still be used as a full
        // scene target -- we always allocate a depth attachment so the forward pass can run.
        // `TEXTURE_BINDING` is required so Hi-Z build can bind the depth view for mip0 (`hi_z_mip0_d_bg`).
        let depth_format = crate::gpu::main_forward_depth_stencil_format(device.features());
        let dt = Arc::new(device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("RenderTextureDepth {}", fmt.asset_id)),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: depth_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        }));
        let dv = Arc::new(dt.create_view(&wgpu::TextureViewDescriptor::default()));
        crate::profiling::note_resource_churn!(TextureView, "gpu_pools::render_texture_depth_view");
        let depth_texture = Some(dt);
        let depth_view = Some(dv);

        let color_bytes = estimate_gpu_texture_bytes(wgpu_color_format, w, h, 1);
        let depth_bytes = estimate_gpu_texture_bytes(depth_format, w, h, 1);
        let resident_bytes = color_bytes.saturating_add(depth_bytes);

        let sampler = SamplerState::from_render_texture_format(fmt);

        Some(Self {
            asset_id: fmt.asset_id,
            color_texture: published.texture,
            color_view: published.view,
            color_view_generation: published.view_generation,
            previous_color: None,
            depth_texture,
            depth_view,
            wgpu_color_format,
            width: w,
            height: h,
            resident_bytes,
            sampler,
        })
    }

    /// `true` when the color target exists and can be sampled (always after successful creation).
    #[inline]
    pub fn is_sampleable(&self) -> bool {
        true
    }

    /// `true` when a second color image has been allocated for camera double buffering.
    #[cfg(test)]
    #[inline]
    pub(crate) fn has_previous_color(&self) -> bool {
        self.previous_color.is_some()
    }

    /// View a material should sample.
    ///
    /// `allow_previous` selects the image completed on the prior double-buffered frame. Callers
    /// that are not the camera writing this texture pass `false` and get the published image.
    pub(crate) fn color_view_for_sampling(&self, allow_previous: bool) -> Arc<wgpu::TextureView> {
        match self.previous_color_image(allow_previous) {
            Some(previous) => Arc::clone(&previous.view),
            None => Arc::clone(&self.color_view),
        }
    }

    /// Bind-cache generation of the view [`Self::color_view_for_sampling`] returns.
    pub(crate) fn sampled_color_view_generation(&self, allow_previous: bool) -> u64 {
        match self.previous_color_image(allow_previous) {
            Some(previous) => previous.view_generation,
            None => self.color_view_generation,
        }
    }

    fn previous_color_image(&self, allow_previous: bool) -> Option<&RenderTextureColorImage> {
        if allow_previous {
            self.previous_color.as_ref()
        } else {
            None
        }
    }

    /// Allocates the second color image the first time double buffering is requested.
    ///
    /// Returns `true` only on that first allocation. Later calls reuse the image.
    fn ensure_previous_color(&mut self, device: &wgpu::Device) -> bool {
        if self.previous_color.is_some() {
            return false;
        }
        let size = wgpu::Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        };
        let history = create_color_image(
            device,
            &format!("RenderTexture {} history", self.asset_id),
            size,
            self.wgpu_color_format,
            ColorImageRole::History,
        );
        let color_bytes =
            estimate_gpu_texture_bytes(self.wgpu_color_format, self.width, self.height, 1);
        self.resident_bytes = self.resident_bytes.saturating_add(color_bytes);
        self.previous_color = Some(history);
        true
    }

    /// Exchanges the published color image with the previous-frame image.
    ///
    /// No-op until [`Self::ensure_previous_color`] has run. The swap is three pointer exchanges.
    fn swap_published_color(&mut self) {
        let Some(previous) = self.previous_color.as_mut() else {
            return;
        };
        swap_ping_pong(&mut self.color_texture, &mut previous.texture);
        swap_ping_pong(&mut self.color_view, &mut previous.view);
        swap_ping_pong(
            &mut self.color_view_generation,
            &mut previous.view_generation,
        );
    }
}

/// Which color slot a newly created image fills. Selects the resource-churn site.
#[derive(Clone, Copy)]
enum ColorImageRole {
    /// The image created with the render texture.
    Published,
    /// The image created the first time double buffering is enabled.
    History,
}

/// Allocates one color image and its full-mip view.
fn create_color_image(
    device: &wgpu::Device,
    label: &str,
    size: wgpu::Extent3d,
    format: wgpu::TextureFormat,
    role: ColorImageRole,
) -> RenderTextureColorImage {
    let texture = Arc::new(device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    }));
    let view = Arc::new(texture.create_view(&wgpu::TextureViewDescriptor::default()));
    match role {
        ColorImageRole::Published => {
            crate::profiling::note_resource_churn!(
                TextureView,
                "gpu_pools::render_texture_color_view"
            );
        }
        ColorImageRole::History => {
            crate::profiling::note_resource_churn!(
                TextureView,
                "gpu_pools::render_texture_history_color_view"
            );
        }
    }
    RenderTextureColorImage {
        texture,
        view,
        view_generation: NEXT_RENDER_TEXTURE_VIEW_GENERATION.fetch_add(1, Ordering::Relaxed),
    }
}

/// Records `asset_id` as double-buffered for this planning pass.
///
/// Returns `true` the first time the id is seen so the caller swaps once. The scratch vec is
/// cleared and reused each pass; a linear scan stays cheaper than a set for the handful of
/// cameras that set the flag.
pub(crate) fn claim_double_buffer_swap(scratch: &mut Vec<i32>, asset_id: i32) -> bool {
    if scratch.contains(&asset_id) {
        return false;
    }
    scratch.push(asset_id);
    true
}

/// Exchanges two ping-pong slots. Used for the color texture, its view, and its generation.
#[inline]
pub(crate) fn swap_ping_pong<T>(current: &mut T, previous: &mut T) {
    std::mem::swap(current, previous);
}

/// Pool of [`GpuRenderTexture`] entries keyed by host asset id.
#[derive(Debug)]
pub struct RenderTexturePool {
    /// Shared resident GPU resource table.
    inner: GpuResourcePool<GpuRenderTexture, UntrackedAccess>,
}

impl_resident_pool_facade!(
    RenderTexturePool,
    GpuRenderTexture,
    VramResourceKind::Texture,
);

impl RenderTexturePool {
    /// Number of resident render texture entries.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Allocates the second color image once, then swaps published and previous.
    ///
    /// Returns whether this call allocated the second image. `None` when `asset_id` is not
    /// resident. The swap is what makes the camera write one image while sampling the other.
    pub(crate) fn begin_double_buffered_frame(
        &mut self,
        device: &wgpu::Device,
        asset_id: i32,
    ) -> Option<bool> {
        let (before, after, allocated) = {
            let texture = self.inner.get_mut(asset_id)?;
            let before = texture.resident_bytes;
            let allocated = texture.ensure_previous_color(device);
            texture.swap_published_color();
            (before, texture.resident_bytes, allocated)
        };
        self.inner.account_resident_delta(before, after);
        Some(allocated)
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for host-driven render-texture sampler metadata.

    use crate::gpu_pools::sampler_state::SamplerState;
    use crate::gpu_pools::test_support::render_texture_format;
    use crate::shared::TextureWrapMode;

    /// Render textures must preserve the host's U/V wrap modes instead of renderer-forcing clamp.
    #[test]
    fn sampler_state_preserves_host_wrap_modes() {
        let fmt = render_texture_format(TextureWrapMode::Mirror, TextureWrapMode::Clamp);
        let sampler = SamplerState::from_render_texture_format(&fmt);

        assert_eq!(sampler.wrap_u, TextureWrapMode::Mirror);
        assert_eq!(sampler.wrap_v, TextureWrapMode::Clamp);
    }

    /// Explicit repeat stays repeat so authorable render-texture tiling keeps working.
    #[test]
    fn sampler_state_preserves_explicit_repeat() {
        let fmt = render_texture_format(TextureWrapMode::Repeat, TextureWrapMode::Repeat);
        let sampler = SamplerState::from_render_texture_format(&fmt);

        assert_eq!(sampler.wrap_u, TextureWrapMode::Repeat);
        assert_eq!(sampler.wrap_v, TextureWrapMode::Repeat);
    }

    /// Negative anisotropy values from the host are clamped before sampler creation.
    #[test]
    fn sampler_state_clamps_negative_anisotropy() {
        let mut fmt = render_texture_format(TextureWrapMode::Clamp, TextureWrapMode::Clamp);
        fmt.aniso_level = -4;

        let sampler = SamplerState::from_render_texture_format(&fmt);

        assert_eq!(sampler.aniso_level, 0);
    }

    /// The per-frame id scratch records each render texture once and keeps its allocation.
    #[test]
    fn claim_double_buffer_swap_reuses_scratch_capacity() {
        let mut scratch = Vec::new();

        assert!(super::claim_double_buffer_swap(&mut scratch, 4));
        assert!(!super::claim_double_buffer_swap(&mut scratch, 4));
        assert!(super::claim_double_buffer_swap(&mut scratch, 9));
        let capacity = scratch.capacity();

        scratch.clear();
        assert!(super::claim_double_buffer_swap(&mut scratch, 4));
        assert!(super::claim_double_buffer_swap(&mut scratch, 9));
        assert_eq!(scratch.capacity(), capacity);
        assert_eq!(scratch.len(), 2);
    }
}

#[cfg(test)]
#[path = "render_texture_ping_pong_gpu.rs"]
mod ping_pong_gpu;
