//! Staging-storage handles returned from `PersistentUploadArena::prepare_staging_buffer`.

use super::arena::PersistentUploadArena;
use crate::upload_stats::UploadArenaAcquireStats;

/// Origin of the staging storage handed out by the arena for one drain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UploadStagingSource {
    None,
    Persistent { slot: usize, generation: u64 },
    Temporary,
    QueueFallbackOversized,
}

/// Why a staging `get_mapped_range_mut` failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StagingMapFailure {
    /// The error text shows the buffer still has a host mapping.
    StillMapped,
    /// The buffer is not mapped, or the error does not prove that it is.
    NotMapped,
}

/// Classifies a [`wgpu::MapRangeError`] for [`PreparedUploadStaging::abort`].
///
/// Overlap and partial-range errors are produced only while a mapping exists, so
/// the slot can return to `Free`. Every other failure, including "not mapped",
/// drops the buffer. A slot left `Free` while unmapped would miss `get_mapped_range_mut`
/// on every later frame and fall back to `Queue::write_buffer` forever.
pub(crate) fn staging_map_failure(err: &wgpu::MapRangeError) -> StagingMapFailure {
    classify_staging_map_message(&err.to_string())
}

fn classify_staging_map_message(message: &str) -> StagingMapFailure {
    // `MapRangeError`'s string is crate-private, so classification matches the
    // stable sentences wgpu prints. Display prefixes them with "Buffer view error: ".
    // Partial-range and aliasing errors are emitted only after a mapping exists.
    // Alignment and "not mapped" errors are not, and out-of-bounds text does not
    // prove a mapping, so those drop the slot.
    if message.contains("not entirely mapped")
        || message.contains("already been mapped")
        || message.contains("would break Rust memory aliasing")
    {
        StagingMapFailure::StillMapped
    } else {
        StagingMapFailure::NotMapped
    }
}

/// Staging storage prepared for one upload drain.
pub(crate) struct PreparedUploadStaging {
    pub(super) buffer: Option<wgpu::Buffer>,
    pub(super) source: UploadStagingSource,
    pub(super) size: u64,
    pub(super) acquire_stats: UploadArenaAcquireStats,
}

impl PreparedUploadStaging {
    /// Buffer to fill while it is mapped.
    pub(crate) fn buffer(&self) -> Option<&wgpu::Buffer> {
        self.buffer.as_ref()
    }

    /// Stats for the acquisition path that produced this staging storage.
    pub(crate) fn acquire_stats(&self) -> UploadArenaAcquireStats {
        self.acquire_stats
    }

    /// Whether staged writes must be replayed through `Queue::write_buffer`.
    pub(crate) fn requires_queue_fallback(&self) -> bool {
        self.size > 0 && self.buffer.is_none()
    }

    /// Drops this staging attempt.
    ///
    /// `buffer_still_mapped` keeps a persistent slot `Free` with its mapping intact.
    /// Otherwise the slot is emptied so the next drain allocates a new mapped buffer.
    /// A temporary buffer is unmapped only when it is still mapped. Do not record
    /// copy commands afterwards: the staging bytes were never written.
    pub(crate) fn abort(self, arena: &mut PersistentUploadArena, buffer_still_mapped: bool) {
        match self.source {
            UploadStagingSource::None | UploadStagingSource::QueueFallbackOversized => {}
            UploadStagingSource::Temporary => {
                if buffer_still_mapped && let Some(buffer) = self.buffer.as_ref() {
                    buffer.unmap();
                }
            }
            UploadStagingSource::Persistent { slot, generation } => {
                arena.abort_persistent_write(slot, generation, buffer_still_mapped);
            }
        }
    }

    /// Unmaps staging storage and returns the buffer/callback pair required by submit.
    pub(crate) fn finish(self, arena: &mut PersistentUploadArena) -> FinishedUploadStaging {
        match self.source {
            UploadStagingSource::None | UploadStagingSource::QueueFallbackOversized => {
                FinishedUploadStaging {
                    buffer: None,
                    on_submitted_work_done: None,
                }
            }
            UploadStagingSource::Temporary => {
                if let Some(buffer) = self.buffer.as_ref() {
                    buffer.unmap();
                }
                FinishedUploadStaging {
                    buffer: self.buffer,
                    on_submitted_work_done: None,
                }
            }
            UploadStagingSource::Persistent { slot, generation } => {
                let on_submitted_work_done = arena.finish_persistent_write(slot, generation);
                FinishedUploadStaging {
                    buffer: self.buffer,
                    on_submitted_work_done,
                }
            }
        }
    }
}

/// Finished staging storage ready for copy-command recording and submit callbacks.
pub(crate) struct FinishedUploadStaging {
    /// Buffer used as `COPY_SRC` for staged writes.
    pub(crate) buffer: Option<wgpu::Buffer>,
    /// Callback that marks a persistent slot submitted after GPU completion.
    pub(crate) on_submitted_work_done: Option<Box<dyn FnOnce() + Send + 'static>>,
}

#[cfg(test)]
mod tests {
    use super::StagingMapFailure;
    use super::classify_staging_map_message;

    fn failure(message: &str) -> StagingMapFailure {
        classify_staging_map_message(&format!("Buffer view error: {message}"))
    }

    #[test]
    fn unmapped_range_error_drops_the_slot() {
        assert_eq!(
            failure("Buffer is not mapped"),
            StagingMapFailure::NotMapped
        );
        assert_eq!(
            failure("tried to call get_mapped_range(_mut) on an unmapped buffer"),
            StagingMapFailure::NotMapped
        );
        assert_eq!(
            failure("Buffer offset invalid: offset 1 must be multiple of 8"),
            StagingMapFailure::NotMapped
        );
    }

    #[test]
    fn overlap_and_partial_range_keep_the_mapping() {
        assert_eq!(
            failure("tried to call get_mapped_range(_mut) on a range that is not entirely mapped"),
            StagingMapFailure::StillMapped
        );
        assert_eq!(
            failure(
                "tried to call get_mapped_range(_mut) on a range that has already been mapped and would break Rust memory aliasing rules"
            ),
            StagingMapFailure::StillMapped
        );
    }
}
