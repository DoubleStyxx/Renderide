//! Shared setup helpers for concrete render-graph passes.
//!
//! Splits into:
//! - [`attachments`] -- pass-builder shorthands for declaring texture reads and color attachments,
//!   plus the missing-frame-params error builder.
//! - [`fullscreen_d2_array_pipeline`] -- shared cache for fullscreen blits that sample a single
//!   D2-array texture (ACES/AgX tonemap, scene-color compose).
//! - [`output_format`] -- helper for resolving the wgpu format a transient color attachment is
//!   bound to.

mod attachments;
mod fullscreen_d2_array_pipeline;
mod output_format;

pub(in crate::passes) use attachments::{
    color_attachment, imported_color_attachment, missing_pass_resource,
    read_fragment_sampled_texture,
};
pub(in crate::passes) use fullscreen_d2_array_pipeline::{
    FullscreenD2ArrayPipelineLabels, FullscreenD2ArraySampledPipelineCache,
    FullscreenD2ArrayShaders, define_fullscreen_d2_array_pipeline_cache,
};
pub(in crate::passes) use output_format::transient_output_format_or;
