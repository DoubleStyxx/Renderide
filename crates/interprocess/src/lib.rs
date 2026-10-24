//! Cloudtoid-compatible shared-memory queue for IPC between processes.
//!
//! ```text
//! Host process                Renderer / bootstrapper
//!     |  Publisher / Subscriber (same .qu + semaphore names)
//!     v
//! [ QueueHeader | ring buffer ]  <-- mmap / section
//! ```
//!
//! - **Unix**: file-backed read/write mapping under the configured directory. The portable default
//!   is [`default_memory_dir`]: `/dev/shm/.cloudtoid/...` on Linux (tmpfs), and
//!   [`std::env::temp_dir`] on macOS and other non-Linux Unix.
//! - **Windows**: named file mapping `CT_IP_{queue}` plus `Global\CT.IP.{queue}` semaphore; the default
//!   [`QueueOptions::path`] uses the same temp-dir subfolder as other platforms for consistency (the mapping does not read from disk).
//!
//! Binary layout details live in [`layout`]. Typical usage: build [`QueueOptions`], then
//! [`Publisher::new`] / [`Subscriber::new`] or [`QueueFactory`].
//!
//! `unsafe` is denied for this crate. Production exceptions are
//! `expect(unsafe_code)` on the mmap, ring, semaphore, publisher, subscriber,
//! queue-resource, and Windows security modules. Those modules wrap FFI
//! (`mmap` / `CreateFileMapping`, named semaphores) and raw ring views
//! (`RingView::from_raw`). [`test_env`] is a `doc(hidden)` test helper that
//! wraps `std::env::set_var` and `std::env::remove_var` behind one process-wide
//! mutex. New `unsafe` anywhere else does not compile.

#![deny(unsafe_code)]
#![deny(clippy::multiple_unsafe_ops_per_block)]

#[cfg(not(any(unix, windows)))]
compile_error!("The `interprocess` crate only supports `cfg(unix)` and `cfg(windows)` targets.");

mod error;
pub mod layout;
mod memory;
#[cfg(windows)]
mod naming;
mod options;
mod publisher;
mod queue;
mod queue_resources;
mod ring;
mod semaphore;
mod subscriber;
#[doc(hidden)]
pub mod test_env;
#[cfg(windows)]
mod windows_security;

pub use error::OpenError;
pub use options::{
    LINUX_SHM_MEMORY_DIR, QueueOptions, RENDERIDE_INTERPROCESS_DIR_ENV, default_memory_dir,
};
pub use publisher::Publisher;
pub use queue::QueueFactory;
pub use subscriber::Subscriber;
