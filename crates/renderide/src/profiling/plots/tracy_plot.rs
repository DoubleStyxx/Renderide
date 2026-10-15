//! Private macro shared across the plot files in this module.
//!
//! `tracy_plot!` collapses the `cfg(feature = "tracy")` gate that every public plot fn would
//! otherwise carry by hand. On `tracy`, it forwards to [`tracy_client::plot!`]; without `tracy`,
//! it evaluates the value once and discards it so referenced locals stay used and the call has
//! the same side-effect timing as the enabled variant.

/// Records one Tracy plot sample under the literal name `$name`.
#[cfg(feature = "tracy")]
macro_rules! tracy_plot {
    ($name:literal, $value:expr) => {
        tracy_client::plot!($name, $value);
    };
}

/// No-op stub used when the `tracy` feature is off; evaluates `$value` once and discards it.
#[cfg(not(feature = "tracy"))]
macro_rules! tracy_plot {
    ($name:literal, $value:expr) => {{
        let _ = $value;
    }};
}

pub(super) use tracy_plot;
