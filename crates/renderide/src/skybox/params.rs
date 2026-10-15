//! Shared CPU-side parameter packing for cubemap compute helpers.
//!
//! The remaining runtime users only need the compact parameter block, mode tags, and storage
//! orientation helper used by reflection-probe cubemap projection and constant-color bakes.

mod common;

pub(crate) use common::{
    DEFAULT_SKYBOX_SAMPLE_SIZE, SkyboxEvaluatorParams, SkyboxParamMode, storage_v_inverted_flag,
};
