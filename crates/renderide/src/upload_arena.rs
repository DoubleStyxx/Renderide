//! Fence-backed staging-buffer reuse for render-graph uploads.
//!
//! Frame recording paths collect many small buffer writes through
//! [`crate::frame_upload_batch::FrameUploadBatch`]. This arena owns persistent
//! `MAP_WRITE | COPY_SRC` buffers that those writes can reuse across frames, plus a small pool of
//! overflow buffers used when every persistent slot is still in flight. A slot is unmapped
//! before submit, marked reusable only after `Queue::on_submitted_work_done` fires, and remapped
//! from the main thread during the next maintenance pass. Maintenance polls the device only when
//! both the persistent ring and the overflow pool are exhausted.

mod arena;
mod slot;
mod staging;

pub(crate) use crate::upload_stats::{UploadArenaAcquireStats, UploadArenaPressure};
pub(crate) use arena::PersistentUploadArena;
