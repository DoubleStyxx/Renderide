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

use std::sync::atomic::{AtomicU64, Ordering};

use std::sync::Arc;

use crate::assets::texture::estimate_gpu_texture_bytes;
use crate::gpu::GpuLimits;
use crate::gpu_pools::VramResourceKind;
use crate::gpu_pools::budget::VramAccounting;
use crate::gpu_pools::impl_gpu_resource;
use crate::gpu_pools::resource_pool::{GpuResourcePool, UntrackedAccess};
use crate::gpu_pools::sampler_state::SamplerState;
use crate::shared::SetRenderTextureFormat;

/// Planning passes with the double-buffer flag off before the history image is released.
///
/// A 1080p `Rgba16Float` history image is about 16.6 MB. Three passes hide a one-frame flag
/// flicker without keeping that image for the rest of the session.
pub(crate) const DOUBLE_BUFFER_HISTORY_IDLE_FRAMES: u8 = 3;

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
    /// color allocation. While it is present, each rendered frame swaps it with the published
    /// image. After `DOUBLE_BUFFER_HISTORY_IDLE_FRAMES` planning passes with the flag off, the
    /// image is retired and this is `None` again until the flag returns.
    previous_color: Option<RenderTextureColorImage>,
    /// Planning passes since an enabled camera last had the double-buffer flag set.
    history_idle_frames: u8,
    /// `true` after this frame's swap, until the next planning pass clears it.
    ///
    /// Cookie sampling runs before cameras write. History then holds the last completed frame
    /// and the published image is two frames old. A frame that does not swap leaves this false
    /// so cookies keep the published image, which is the newer one.
    history_is_last_completed_frame: bool,
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
            history_idle_frames: 0,
            history_is_last_completed_frame: false,
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

    /// Color image completed on the previous double-buffered frame.
    ///
    /// `None` until the first rendered double-buffered pass allocates it, and again after idle
    /// release. The first writer of this texture seeds this image. Later writers seed the
    /// published image instead.
    #[inline]
    pub(crate) fn history_color_texture(&self) -> Option<Arc<wgpu::Texture>> {
        self.previous_color
            .as_ref()
            .map(|previous| Arc::clone(&previous.texture))
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

    /// Color view for a pass that samples this texture before cameras write it.
    ///
    /// After this frame's swap, history is the last completed image. Otherwise the published
    /// image is the one to sample.
    pub(crate) fn color_view_for_early_sampling(&self) -> &wgpu::TextureView {
        if self.history_is_last_completed_frame
            && let Some(previous) = self.previous_color.as_ref()
        {
            return previous.view.as_ref();
        }
        self.color_view.as_ref()
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
    /// Returns `true` only on that allocation. Later calls reuse the image until idle release.
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
        self.history_is_last_completed_frame = true;
    }

    /// Removes the history image and subtracts its bytes from the resident total.
    fn take_previous_color(&mut self) -> Option<RetiredRenderTextureHistory> {
        let previous = self.previous_color.take()?;
        let bytes = estimate_gpu_texture_bytes(self.wgpu_color_format, self.width, self.height, 1);
        self.resident_bytes = self.resident_bytes.saturating_sub(bytes);
        self.history_idle_frames = 0;
        self.history_is_last_completed_frame = false;
        Some(RetiredRenderTextureHistory {
            bytes,
            texture: previous.texture,
            view: previous.view,
        })
    }
}

/// History color image kept alive until delayed removal drops it.
#[derive(Debug)]
pub(crate) struct RetiredRenderTextureHistory {
    /// Color-image bytes subtracted from the pool when this history was released.
    bytes: u64,
    /// History color storage.
    texture: Arc<wgpu::Texture>,
    /// Full-mip view of `texture`.
    view: Arc<wgpu::TextureView>,
}

impl RetiredRenderTextureHistory {
    /// Bytes the pool stopped accounting for when this image left the resident texture.
    pub(crate) fn resident_bytes(&self) -> u64 {
        debug_assert!(self.texture.size().width > 0 || self.view.texture().size().width > 0);
        self.bytes
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
    /// Asset ids that currently own a history image. Grown only when a history image is created.
    history_ids: Vec<i32>,
    /// Scratch filled by one idle-history pass and restored after the caller retires it.
    retired_history_scratch: Vec<RetiredRenderTextureHistory>,
}

impl Default for RenderTexturePool {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl RenderTexturePool {
    /// Creates an empty pool.
    #[inline]
    pub fn new() -> Self {
        Self {
            inner: GpuResourcePool::new(UntrackedAccess::new(VramResourceKind::Texture)),
            history_ids: Vec::new(),
            retired_history_scratch: Vec::new(),
        }
    }

    /// VRAM accounting for resident render textures.
    #[inline]
    pub fn accounting(&self) -> &VramAccounting {
        self.inner.accounting()
    }

    /// Inserts or replaces a resource. Returns `true` if an entry was replaced.
    #[inline]
    pub fn insert(&mut self, resource: GpuRenderTexture) -> bool {
        self.inner.insert(resource)
    }

    /// Removes and returns a resource by host asset id when it was present.
    #[inline]
    pub(crate) fn take(&mut self, asset_id: i32) -> Option<GpuRenderTexture> {
        self.inner.take(asset_id)
    }

    /// Borrows a resident resource by host asset id.
    #[inline]
    pub fn get(&self, asset_id: i32) -> Option<&GpuRenderTexture> {
        self.inner.get(asset_id)
    }

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
            texture.history_idle_frames = 0;
            (before, texture.resident_bytes, allocated)
        };
        self.inner.account_resident_delta(before, after);
        if !self.history_ids.contains(&asset_id) {
            self.history_ids.push(asset_id);
        }
        Some(allocated)
    }

    /// Forgets which textures swapped, before this planning pass claims new swaps.
    ///
    /// Cookies sample history only while the flag is set. Clearing it first keeps a frame that
    /// does not swap on the published image.
    pub(crate) fn clear_history_completed_frame_flags(&mut self) {
        for &asset_id in &self.history_ids {
            if let Some(texture) = self.inner.get_mut(asset_id) {
                texture.history_is_last_completed_frame = false;
            }
        }
    }

    /// Ages history images and returns the scratch vec holding any that reached the idle limit.
    ///
    /// The caller retires the returned images, drains the vec, and passes it back to
    /// [`Self::finish_history_retirement`] so the next pass reuses the allocation.
    pub(crate) fn begin_history_retirement(
        &mut self,
        flagged: &[i32],
    ) -> Vec<RetiredRenderTextureHistory> {
        let mut retired = std::mem::take(&mut self.retired_history_scratch);
        retired.clear();
        self.age_idle_histories(flagged, &mut retired);
        retired
    }

    /// Restores the scratch vec from [`Self::begin_history_retirement`].
    pub(crate) fn finish_history_retirement(&mut self, scratch: Vec<RetiredRenderTextureHistory>) {
        self.retired_history_scratch = scratch;
    }

    /// Increments idle history images and moves those past the limit into `retired`.
    fn age_idle_histories(
        &mut self,
        flagged: &[i32],
        retired: &mut Vec<RetiredRenderTextureHistory>,
    ) {
        let mut index = 0;
        while index < self.history_ids.len() {
            let asset_id = self.history_ids[index];
            match self.idle_history_decision(asset_id, flagged) {
                IdleHistoryDecision::Missing => {
                    self.history_ids.swap_remove(index);
                }
                IdleHistoryDecision::Keep => index += 1,
                IdleHistoryDecision::Release {
                    before,
                    after,
                    history,
                } => {
                    self.inner.account_resident_delta(before, after);
                    retired.push(history);
                    self.history_ids.swap_remove(index);
                }
            }
        }
    }

    /// Decides whether one history image stays, leaves, or is already gone.
    fn idle_history_decision(&mut self, asset_id: i32, flagged: &[i32]) -> IdleHistoryDecision {
        let Some(texture) = self.inner.get_mut(asset_id) else {
            return IdleHistoryDecision::Missing;
        };
        if texture.previous_color.is_none() {
            return IdleHistoryDecision::Missing;
        }
        if flagged.contains(&asset_id) {
            texture.history_idle_frames = 0;
            return IdleHistoryDecision::Keep;
        }
        texture.history_idle_frames = texture.history_idle_frames.saturating_add(1);
        if texture.history_idle_frames < DOUBLE_BUFFER_HISTORY_IDLE_FRAMES {
            return IdleHistoryDecision::Keep;
        }
        let before = texture.resident_bytes;
        let Some(history) = texture.take_previous_color() else {
            return IdleHistoryDecision::Missing;
        };
        IdleHistoryDecision::Release {
            before,
            after: texture.resident_bytes,
            history,
        }
    }
}

/// What one idle pass should do with a tracked history image.
enum IdleHistoryDecision {
    /// The pool entry or its history image is already gone.
    Missing,
    /// The history image stays resident.
    Keep,
    /// The history image left the pool and must be retired.
    Release {
        /// Resident bytes before the history image was removed.
        before: u64,
        /// Resident bytes after the history image was removed.
        after: u64,
        /// Image the caller enqueues for delayed destruction.
        history: RetiredRenderTextureHistory,
    },
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
