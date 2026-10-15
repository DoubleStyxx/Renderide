//! Shared parameter payload and helpers used by cubemap compute shaders.

use bytemuck::{Pod, Zeroable};

/// Default sky parameter sample grid used by SH2 projection.
pub(crate) const DEFAULT_SKYBOX_SAMPLE_SIZE: u32 = 64;

/// Parameter-only sky evaluator mode used by skybox compute shaders.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SkyboxParamMode {
    /// Procedural sky approximation from material scalar/color properties.
    Procedural = 1,
    /// Gradient sky approximation from material array properties.
    Gradient = 2,
}

/// Uniform payload shared by analytic skybox compute kernels.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct SkyboxEvaluatorParams {
    /// Sample grid edge for projection or generated cubemap face edge for baking.
    pub(crate) sample_size: u32,
    /// Evaluator mode from [`SkyboxParamMode`].
    pub(crate) mode: u32,
    /// Number of active gradient lobes.
    pub(crate) gradient_count: u32,
    /// Reserved alignment slot.
    pub(crate) _pad0: u32,
    /// Generic color slot 0.
    pub(crate) color0: [f32; 4],
    /// Generic color slot 1.
    pub(crate) color1: [f32; 4],
    /// Generic direction and scalar slot.
    pub(crate) direction: [f32; 4],
    /// Generic scalar slot.
    pub(crate) scalars: [f32; 4],
    /// Gradient direction/spread rows.
    pub(crate) dirs_spread: [[f32; 4]; 16],
    /// Gradient color rows A.
    pub(crate) gradient_color0: [[f32; 4]; 16],
    /// Gradient color rows B.
    pub(crate) gradient_color1: [[f32; 4]; 16],
    /// Gradient parameter rows.
    pub(crate) gradient_params: [[f32; 4]; 16],
}

impl SkyboxEvaluatorParams {
    /// Creates a parameter block with the default projection sample grid.
    pub(crate) fn empty(mode: SkyboxParamMode) -> Self {
        Self {
            sample_size: DEFAULT_SKYBOX_SAMPLE_SIZE,
            mode: mode as u32,
            gradient_count: 0,
            _pad0: 0,
            color0: [0.0; 4],
            color1: [0.0; 4],
            direction: [0.0, 1.0, 0.0, 0.0],
            scalars: [1.0, 0.0, 0.0, 0.0],
            dirs_spread: [[0.0; 4]; 16],
            gradient_color0: [[0.0; 4]; 16],
            gradient_color1: [[0.0; 4]; 16],
            gradient_params: [[0.0; 4]; 16],
        }
    }

    /// Returns a copy with the sample or face edge set.
    pub(crate) fn with_sample_size(mut self, sample_size: u32) -> Self {
        self.sample_size = sample_size.max(1);
        self
    }
}

/// Converts a storage-orientation boolean to the shader keyword float convention.
pub(crate) fn storage_v_inverted_flag(storage_v_inverted: bool) -> f32 {
    if storage_v_inverted { 1.0 } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_params_use_documented_defaults() {
        let params = SkyboxEvaluatorParams::empty(SkyboxParamMode::Procedural);

        assert_eq!(params.sample_size, DEFAULT_SKYBOX_SAMPLE_SIZE);
        assert_eq!(params.mode, SkyboxParamMode::Procedural as u32);
        assert_eq!(params.gradient_count, 0);
        assert_eq!(params.direction, [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(params.scalars, [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn with_sample_size_clamps_zero_to_one() {
        let params = SkyboxEvaluatorParams::empty(SkyboxParamMode::Gradient).with_sample_size(0);

        assert_eq!(params.sample_size, 1);
        assert_eq!(params.mode, SkyboxParamMode::Gradient as u32);
    }

    #[test]
    fn storage_v_inverted_flag_matches_shader_float_convention() {
        assert_eq!(storage_v_inverted_flag(false), 0.0);
        assert_eq!(storage_v_inverted_flag(true), 1.0);
    }
}
