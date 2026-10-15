//! IPC ingestion: command poll, dispatch effect application, and per-domain host submissions.
//!
//! - [`entry`] -- per-tick command drain and shader-resolution sweep at the top of each poll.
//! - [`effects`] -- decodes one host command and applies the resulting domain effect.
//! - [`shader_material`] -- shader uploads, shader-route resolution, and material batch handling.
//! - [`lights`] -- host light-buffer submission over shared memory.

pub(in crate::runtime) mod effects;
pub(in crate::runtime) mod entry;
pub(in crate::runtime) mod lights;
pub(in crate::runtime) mod shader_material;
