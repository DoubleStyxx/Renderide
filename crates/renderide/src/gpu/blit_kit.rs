//! Crate-internal helpers shared by the display blit and VR mirror surface blit.
//!
//! - [`sampler`] -- single shared linear-clamp sampler.
//! - [`layout`] -- BGLs for sampled 2D + filtering sampler (with or without a UV uniform slot).
//! - [`pipeline`] -- color-blit pipeline builder, single-slot per-format pipeline cache, and
//!   lazy 16-byte UV uniform buffer.
//!
//! Nothing here is re-exported through [`super`]'s public surface; consumers stay inside
//! [`super::display_blit`] and [`super::vr_mirror`].

pub(crate) mod layout;
pub(crate) mod pipeline;
pub(crate) mod sampler;
