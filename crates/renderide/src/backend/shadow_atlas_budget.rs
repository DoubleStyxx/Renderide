//! Shadow-atlas allocation policy shared by planning and GPU resource synchronization.

use crate::gpu::GpuLimits;

/// Maximum resident realtime shadow atlas allocation in bytes.
pub(in crate::backend) const SHADOW_ATLAS_MAX_BYTES: u64 = 512 * 1024 * 1024;

const SHADOW_ATLAS_CONSERVATIVE_BYTES_PER_TEXEL: u64 = 4;

/// Frames a strictly smaller request must stay unchanged before the resident atlas shrinks.
///
/// One hundred twenty frames is about two seconds at 60 Hz. A one-frame spike does not
/// recreate the texture, and alternating shapes reset the streak.
pub(in crate::backend) const SHADOW_ATLAS_SHRINK_AFTER_FRAMES: u32 = 120;

/// Frames between repeated logs while one grant mismatch persists.
const SHADOW_ATLAS_GRANT_LOG_INTERVAL: u32 = 300;

/// Resident shadow-atlas shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) struct ShadowAtlasExtent {
    /// Edge length of every atlas layer, in texels.
    pub resolution: u32,
    /// Array layers retained for the atlas.
    pub layers: u32,
}

/// Resident extent plus the shrink streak for the last requested shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) struct ShadowAtlasResidency {
    /// Texture shape currently retained.
    pub extent: ShadowAtlasExtent,
    /// Resolution of the request being counted toward a shrink.
    pub shrink_resolution: u32,
    /// Layer count of the request being counted toward a shrink.
    pub shrink_layers: u32,
    /// Consecutive frames that request has stayed smaller than [`Self::extent`].
    pub shrink_frames: u32,
}

/// Rate-limit state for a grant that differs from the request.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::backend) struct ShadowAtlasGrantLog {
    /// Last logged request resolution. Zero until the first log.
    request_resolution: u32,
    /// Last logged request layer count.
    request_layers: u32,
    /// Last logged grant resolution.
    grant_resolution: u32,
    /// Last logged grant layer count.
    grant_layers: u32,
    /// Frames since the last log for this mismatch.
    frames_since_log: u32,
    /// Whether a mismatch has been logged.
    has_logged: bool,
}

/// Whether [`ShadowAtlasResources::sync`](crate::backend::frame_gpu::ShadowAtlasResources) should log.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) struct ShadowAtlasGrantLogDecision {
    /// Emit one log line this call.
    pub log: bool,
    /// The grant is below the request in resolution or layers.
    pub warn: bool,
    /// State to store for the next call.
    pub state: ShadowAtlasGrantLog,
}

/// Grows the atlas toward the request without snapping back below the byte budget.
///
/// A high-resolution few-layer request and a many-layer request can each fit in
/// [`SHADOW_ATLAS_MAX_BYTES`] while their product does not. Snapping to the instantaneous
/// request recreates the texture every time those shapes alternate. This keeps the high-water
/// layer count and lowers resolution to the budget instead.
#[must_use]
pub(in crate::backend) fn stable_shadow_atlas_extent(
    current: ShadowAtlasExtent,
    requested_resolution: u32,
    requested_layers: u32,
    limits: &GpuLimits,
    format: wgpu::TextureFormat,
) -> ShadowAtlasExtent {
    let layer_cap = limits.max_texture_array_layers().max(1);
    let requested_layers = requested_layers.max(1).min(layer_cap);
    let requested_resolution =
        clamp_shadow_atlas_resolution(limits, requested_resolution, requested_layers, format);
    let layers = current.layers.max(requested_layers).min(layer_cap).max(1);
    let mut resolution = current.resolution.max(requested_resolution).max(1);
    let budgeted = clamp_shadow_atlas_resolution(limits, resolution, layers, format);
    if budgeted != resolution {
        resolution = budgeted;
    }
    ShadowAtlasExtent { resolution, layers }
}

/// Advances residency, shrinking back to a stable smaller request after
/// [`SHADOW_ATLAS_SHRINK_AFTER_FRAMES`].
///
/// Growth still happens on the frame it is requested. A smaller request is adopted only after
/// that same shape has stayed under the resident byte size for the shrink streak, so one spike
/// cannot pin a high layer count (and the resolution clamp that comes with it).
#[must_use]
pub(in crate::backend) fn advance_shadow_atlas_residency(
    current: ShadowAtlasResidency,
    requested_resolution: u32,
    requested_layers: u32,
    limits: &GpuLimits,
    format: wgpu::TextureFormat,
) -> ShadowAtlasResidency {
    let requested =
        clamped_shadow_atlas_request(requested_resolution, requested_layers, limits, format);
    let grown = stable_shadow_atlas_extent(
        current.extent,
        requested_resolution,
        requested_layers,
        limits,
        format,
    );
    let requested_bytes = shadow_atlas_extent_bytes(requested, format);
    let resident_bytes = shadow_atlas_extent_bytes(current.extent, format);
    let same_request = current.shrink_resolution == requested.resolution
        && current.shrink_layers == requested.layers;
    let smaller = requested_bytes < resident_bytes;
    let shrink_frames = if smaller && same_request {
        current.shrink_frames.saturating_add(1)
    } else {
        u32::from(smaller)
    };
    if shrink_frames >= SHADOW_ATLAS_SHRINK_AFTER_FRAMES {
        return ShadowAtlasResidency {
            extent: requested,
            shrink_resolution: requested.resolution,
            shrink_layers: requested.layers,
            shrink_frames: 0,
        };
    }
    ShadowAtlasResidency {
        extent: grown,
        shrink_resolution: requested.resolution,
        shrink_layers: requested.layers,
        shrink_frames,
    }
}

/// Byte size of one resident atlas, using the same texel size as the allocation clamp.
#[must_use]
pub(in crate::backend) fn shadow_atlas_extent_bytes(
    extent: ShadowAtlasExtent,
    format: wgpu::TextureFormat,
) -> u64 {
    let edge = u64::from(extent.resolution.max(1));
    let layers = u64::from(extent.layers.max(1));
    edge.saturating_mul(edge)
        .saturating_mul(layers)
        .saturating_mul(shadow_atlas_format_bytes_per_texel(format))
}

/// Decides whether a grant that differs from the request should be logged this frame.
#[must_use]
pub(in crate::backend) fn note_shadow_atlas_grant(
    state: ShadowAtlasGrantLog,
    requested_resolution: u32,
    requested_layers: u32,
    granted: ShadowAtlasExtent,
) -> ShadowAtlasGrantLogDecision {
    let requested_resolution = requested_resolution.max(1);
    let requested_layers = requested_layers.max(1);
    if granted.resolution == requested_resolution && granted.layers == requested_layers {
        return ShadowAtlasGrantLogDecision {
            log: false,
            warn: false,
            state: ShadowAtlasGrantLog::default(),
        };
    }
    let same_mismatch = state.has_logged
        && state.request_resolution == requested_resolution
        && state.request_layers == requested_layers
        && state.grant_resolution == granted.resolution
        && state.grant_layers == granted.layers;
    let frames_since_log = if same_mismatch {
        state.frames_since_log.saturating_add(1)
    } else {
        0
    };
    let log = !same_mismatch || frames_since_log >= SHADOW_ATLAS_GRANT_LOG_INTERVAL;
    let warn = granted.resolution < requested_resolution || granted.layers < requested_layers;
    ShadowAtlasGrantLogDecision {
        log,
        warn,
        state: ShadowAtlasGrantLog {
            request_resolution: requested_resolution,
            request_layers: requested_layers,
            grant_resolution: granted.resolution,
            grant_layers: granted.layers,
            frames_since_log: if log { 0 } else { frames_since_log },
            has_logged: true,
        },
    }
}

fn clamped_shadow_atlas_request(
    requested_resolution: u32,
    requested_layers: u32,
    limits: &GpuLimits,
    format: wgpu::TextureFormat,
) -> ShadowAtlasExtent {
    let layer_cap = limits.max_texture_array_layers().max(1);
    let layers = requested_layers.max(1).min(layer_cap);
    let resolution = clamp_shadow_atlas_resolution(limits, requested_resolution, layers, format);
    ShadowAtlasExtent { resolution, layers }
}

/// Clamps a shadow atlas edge to device dimensions and the internal atlas allocation budget.
#[must_use]
pub(in crate::backend) fn clamp_shadow_atlas_resolution(
    limits: &GpuLimits,
    requested_resolution: u32,
    layers: u32,
    format: wgpu::TextureFormat,
) -> u32 {
    clamp_shadow_atlas_resolution_for_texel_size(
        limits,
        requested_resolution,
        layers,
        shadow_atlas_format_bytes_per_texel(format),
    )
}

fn clamp_shadow_atlas_resolution_for_texel_size(
    limits: &GpuLimits,
    requested_resolution: u32,
    layers: u32,
    bytes_per_texel: u64,
) -> u32 {
    let device_cap = limits.max_texture_dimension_2d().max(1);
    let budget_cap = shadow_atlas_budget_resolution_cap(layers, bytes_per_texel);
    requested_resolution.max(1).min(device_cap).min(budget_cap)
}

const fn shadow_atlas_format_bytes_per_texel(format: wgpu::TextureFormat) -> u64 {
    match format {
        wgpu::TextureFormat::Depth16Unorm => 2,
        wgpu::TextureFormat::Depth24Plus | wgpu::TextureFormat::Depth32Float => 4,
        _ => SHADOW_ATLAS_CONSERVATIVE_BYTES_PER_TEXEL,
    }
}

fn shadow_atlas_budget_resolution_cap(layers: u32, bytes_per_texel: u64) -> u32 {
    let layer_count = u64::from(layers.max(1));
    let texel_bytes = bytes_per_texel.max(1);
    let texels_per_layer = SHADOW_ATLAS_MAX_BYTES / layer_count / texel_bytes;
    floor_sqrt_u64(texels_per_layer).max(1)
}

fn floor_sqrt_u64(value: u64) -> u32 {
    if value == 0 {
        return 0;
    }
    let mut root = (value as f64).sqrt() as u64;
    while root < u64::from(u32::MAX) {
        let next = root + 1;
        if next > value / next {
            break;
        }
        root = next;
    }
    while root > value / root {
        root -= 1;
    }
    root.min(u64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use hashbrown::HashMap;

    use super::{
        SHADOW_ATLAS_SHRINK_AFTER_FRAMES, ShadowAtlasExtent, ShadowAtlasGrantLog,
        ShadowAtlasResidency, advance_shadow_atlas_residency, clamp_shadow_atlas_resolution,
        note_shadow_atlas_grant, stable_shadow_atlas_extent,
    };

    fn limits(max_texture_dimension_2d: u32) -> crate::gpu::GpuLimits {
        crate::gpu::GpuLimits::synthetic_for_tests(
            wgpu::Limits {
                max_texture_dimension_2d,
                ..Default::default()
            },
            wgpu::Features::empty(),
            HashMap::new(),
        )
    }

    #[test]
    fn shadow_atlas_budget_allows_four_ultra_depth32_layers() {
        let limits = limits(8192);

        assert_eq!(
            clamp_shadow_atlas_resolution(&limits, 4096, 4, wgpu::TextureFormat::Depth32Float),
            4096
        );
    }

    #[test]
    fn shadow_atlas_budget_clamps_many_depth32_layers() {
        let limits = limits(8192);

        assert_eq!(
            clamp_shadow_atlas_resolution(&limits, 4096, 16, wgpu::TextureFormat::Depth32Float),
            2896
        );
    }

    #[test]
    fn shadow_atlas_budget_accounts_for_depth16_texel_size() {
        let limits = limits(8192);

        assert_eq!(
            clamp_shadow_atlas_resolution(&limits, 4096, 16, wgpu::TextureFormat::Depth16Unorm),
            4096
        );
    }

    #[test]
    fn alternating_shadow_atlas_requests_keep_a_stable_extent() {
        let limits = limits(8192);
        let format = wgpu::TextureFormat::Depth32Float;
        let few_layers = stable_shadow_atlas_extent(
            ShadowAtlasExtent {
                resolution: 1,
                layers: 1,
            },
            4096,
            4,
            &limits,
            format,
        );
        assert_eq!(
            few_layers,
            ShadowAtlasExtent {
                resolution: 4096,
                layers: 4,
            }
        );
        let many_layers = stable_shadow_atlas_extent(few_layers, 1024, 32, &limits, format);
        let repeated = stable_shadow_atlas_extent(many_layers, 4096, 4, &limits, format);
        assert_eq!(repeated, many_layers);
        assert!(repeated.layers >= 32);
        assert_eq!(
            clamp_shadow_atlas_resolution(&limits, repeated.resolution, repeated.layers, format),
            repeated.resolution
        );
        assert_eq!(
            stable_shadow_atlas_extent(repeated, 4096, 4, &limits, format),
            repeated
        );
    }

    #[test]
    fn shadow_atlas_budget_respects_device_texture_limit() {
        let limits = limits(2048);

        assert_eq!(
            clamp_shadow_atlas_resolution(&limits, 4096, 1, wgpu::TextureFormat::Depth32Float),
            2048
        );
    }

    fn empty_residency() -> ShadowAtlasResidency {
        ShadowAtlasResidency {
            extent: ShadowAtlasExtent {
                resolution: 1,
                layers: 1,
            },
            shrink_resolution: 0,
            shrink_layers: 0,
            shrink_frames: 0,
        }
    }

    #[test]
    fn shadow_atlas_shrinks_after_a_stable_smaller_request() {
        let limits = limits(8192);
        let format = wgpu::TextureFormat::Depth32Float;
        let grown = advance_shadow_atlas_residency(empty_residency(), 4096, 4, &limits, format);
        assert_eq!(
            grown.extent,
            ShadowAtlasExtent {
                resolution: 4096,
                layers: 4,
            }
        );
        let mut resident = grown;
        for frame in 1..SHADOW_ATLAS_SHRINK_AFTER_FRAMES {
            resident = advance_shadow_atlas_residency(resident, 1024, 4, &limits, format);
            assert_eq!(
                resident.extent.resolution, 4096,
                "frame {frame} must keep the spike"
            );
            assert_eq!(resident.extent.layers, 4);
        }
        let shrunk = advance_shadow_atlas_residency(resident, 1024, 4, &limits, format);
        assert_eq!(
            shrunk.extent,
            ShadowAtlasExtent {
                resolution: 1024,
                layers: 4,
            }
        );
        assert_eq!(shrunk.shrink_frames, 0);
    }

    #[test]
    fn shadow_atlas_layer_spike_shrinks_back_to_the_steady_request() {
        let limits = limits(8192);
        let format = wgpu::TextureFormat::Depth32Float;
        let few = advance_shadow_atlas_residency(empty_residency(), 4096, 4, &limits, format);
        let spiked = advance_shadow_atlas_residency(few, 1024, 32, &limits, format);
        assert!(spiked.extent.layers >= 32);
        assert!(spiked.extent.resolution < 4096);
        assert_eq!(
            clamp_shadow_atlas_resolution(
                &limits,
                spiked.extent.resolution,
                spiked.extent.layers,
                format
            ),
            spiked.extent.resolution
        );
        let mut resident = spiked;
        for _ in 1..SHADOW_ATLAS_SHRINK_AFTER_FRAMES {
            resident = advance_shadow_atlas_residency(resident, 4096, 4, &limits, format);
            assert!(resident.extent.layers >= 32);
        }
        let restored = advance_shadow_atlas_residency(resident, 4096, 4, &limits, format);
        assert_eq!(
            restored.extent,
            ShadowAtlasExtent {
                resolution: 4096,
                layers: 4,
            }
        );
    }

    #[test]
    fn alternating_smaller_shadow_atlas_requests_do_not_shrink() {
        let limits = limits(8192);
        let format = wgpu::TextureFormat::Depth32Float;
        let mut resident =
            advance_shadow_atlas_residency(empty_residency(), 4096, 4, &limits, format);
        resident = advance_shadow_atlas_residency(resident, 1024, 32, &limits, format);
        let pinned = resident.extent;
        for frame in 0..SHADOW_ATLAS_SHRINK_AFTER_FRAMES.saturating_mul(2) {
            let (resolution, layers) = if frame % 2 == 0 {
                (4096, 4)
            } else {
                (1024, 32)
            };
            resident =
                advance_shadow_atlas_residency(resident, resolution, layers, &limits, format);
            assert_eq!(resident.extent, pinned);
        }
    }

    #[test]
    fn shadow_atlas_grant_log_is_rate_limited() {
        let granted = ShadowAtlasExtent {
            resolution: 2048,
            layers: 32,
        };
        let first = note_shadow_atlas_grant(ShadowAtlasGrantLog::default(), 4096, 4, granted);
        assert!(first.log);
        assert!(first.warn);
        let quiet = note_shadow_atlas_grant(first.state, 4096, 4, granted);
        assert!(!quiet.log);
        let mut state = quiet.state;
        for _ in 0..298 {
            let step = note_shadow_atlas_grant(state, 4096, 4, granted);
            assert!(!step.log);
            state = step.state;
        }
        let again = note_shadow_atlas_grant(state, 4096, 4, granted);
        assert!(again.log);
        let changed = note_shadow_atlas_grant(
            again.state,
            1024,
            32,
            ShadowAtlasExtent {
                resolution: 2048,
                layers: 32,
            },
        );
        assert!(changed.log);
        assert!(!changed.warn);
        let matched = note_shadow_atlas_grant(
            changed.state,
            2048,
            32,
            ShadowAtlasExtent {
                resolution: 2048,
                layers: 32,
            },
        );
        assert!(!matched.log);
        assert_eq!(matched.state, ShadowAtlasGrantLog::default());
    }
}
