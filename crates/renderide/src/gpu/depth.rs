//! Depth conventions for the main forward pass: reverse-Z constants, depth-stencil format
//! selection, and output-depth layout classification.

mod output_mode;
mod reverse_z;
mod stencil_format;

pub use output_mode::OutputDepthMode;
pub use reverse_z::{MAIN_FORWARD_DEPTH_CLEAR, MAIN_FORWARD_DEPTH_COMPARE};
pub use stencil_format::main_forward_depth_stencil_format;
