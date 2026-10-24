//! Regression coverage for the Wayland tablet-pad launch abort.
//!
//! The integration test speaks to a compositor that announces a pad. winit must
//! accept that pad's child objects instead of aborting during event-loop startup.
#![forbid(unsafe_code)]
