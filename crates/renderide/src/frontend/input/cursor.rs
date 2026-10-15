//! Host [`crate::shared::OutputState`] cursor policy and IME/grab adapter.
//!
//! Submodules:
//! - [`lock`] -- grab/visibility transitions and per-frame re-warp for cursor lock.
//! - [`ime`] -- IME enable-request construction.

mod ime;
mod lock;

pub use ime::enable_ime_on_window;
pub use lock::{
    CursorOutputTracking, apply_output_state_to_window, apply_per_frame_cursor_lock_when_locked,
};
