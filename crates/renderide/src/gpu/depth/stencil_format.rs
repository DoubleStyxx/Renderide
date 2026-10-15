//! Choice of depth-stencil attachment format for the main forward pass.

/// Chooses the main forward depth-stencil attachment format.
///
/// `Depth32FloatStencil8` preserves the 32-bit depth precision when the optional WebGPU feature
/// was enabled. `Depth24PlusStencil8` is the portable stencil-capable fallback.
pub fn main_forward_depth_stencil_format(features: wgpu::Features) -> wgpu::TextureFormat {
    if features.contains(wgpu::Features::DEPTH32FLOAT_STENCIL8) {
        wgpu::TextureFormat::Depth32FloatStencil8
    } else {
        wgpu::TextureFormat::Depth24PlusStencil8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_forward_depth_stencil_format_prefers_32f_when_enabled() {
        assert_eq!(
            main_forward_depth_stencil_format(wgpu::Features::DEPTH32FLOAT_STENCIL8),
            wgpu::TextureFormat::Depth32FloatStencil8
        );
        assert_eq!(
            main_forward_depth_stencil_format(wgpu::Features::empty()),
            wgpu::TextureFormat::Depth24PlusStencil8
        );
    }
}
