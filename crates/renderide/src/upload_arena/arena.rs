//! `PersistentUploadArena`: coordinator across persistent slots, the GPU completion channel, and
//! the per-frame staging handout.

use std::sync::mpsc;

use super::slot::{
    UPLOAD_ARENA_POOLED_TEMPS, UPLOAD_ARENA_SLOTS, UploadArenaSlot, UploadArenaSlotState,
    create_persistent_slot_buffer, create_temporary_staging_buffer, log_oversized_upload,
    next_slot_capacity, select_writable_slot,
};
use super::staging::{PreparedUploadStaging, UploadStagingSource};
use crate::upload_stats::{UploadArenaAcquireStats, UploadArenaPressure};

/// Which staging buffer a completion callback refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StagingOwner {
    /// One of the persistent mapped slots.
    Persistent(usize),
    /// One of the overflow buffers kept after a persistent-ring miss.
    Pooled(usize),
}

/// One completed upload slot event delivered from a wgpu callback to the main thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UploadArenaCompletion {
    /// Submitted GPU work that references this slot has completed.
    Submitted {
        owner: StagingOwner,
        generation: u64,
    },
    /// A post-submit remap request has completed.
    Remapped {
        owner: StagingOwner,
        generation: u64,
        success: bool,
    },
}

fn select_pooled_slot(slots: &[UploadArenaSlot], required: u64) -> Option<usize> {
    slots
        .iter()
        .position(|slot| slot.state.can_write() && slot.capacity >= required)
        .or_else(|| {
            slots
                .iter()
                .position(|slot| matches!(slot.state, UploadArenaSlotState::Empty))
        })
        .or_else(|| {
            slots
                .iter()
                .position(|slot| matches!(slot.state, UploadArenaSlotState::Free))
        })
}

/// Persistent upload staging arena for render-graph and mesh buffer writes.
pub(crate) struct PersistentUploadArena {
    slots: [UploadArenaSlot; UPLOAD_ARENA_SLOTS],
    /// Overflow staging buffers recycled after GPU completion.
    ///
    /// These exist so a frame that finds every persistent slot busy can keep uploading without
    /// `device.poll` and without allocating a new GPU buffer every time.
    pooled: Vec<UploadArenaSlot>,
    completion_tx: mpsc::Sender<UploadArenaCompletion>,
    completion_rx: mpsc::Receiver<UploadArenaCompletion>,
    next_generation: u64,
}

impl PersistentUploadArena {
    /// Creates an empty arena. Slots allocate lazily on first use.
    pub(crate) fn new() -> Self {
        let (completion_tx, completion_rx) = mpsc::channel();
        Self {
            slots: std::array::from_fn(|_| UploadArenaSlot::empty()),
            pooled: Vec::new(),
            completion_tx,
            completion_rx,
            next_generation: 1,
        }
    }

    /// Drains submitted-work and remap callbacks.
    ///
    /// `device.poll` runs only when every persistent slot is waiting on the GPU and the overflow
    /// pool cannot serve another upload. Acquire itself never polls: a non-blocking poll still
    /// round-trips through the wgpu device thread, which shows up in frame time.
    pub(crate) fn maintain(&mut self, device: &wgpu::Device) {
        profiling::scope!("frame_upload_arena::maintain");
        self.drain_completions();
        if self.should_block_for_recycle() {
            self.poll_for_staging_pressure(device);
        }
    }

    /// Drops all retained staging slots and ignores stale completion callbacks from older slots.
    pub(crate) fn reset(&mut self) {
        profiling::scope!("frame_upload_arena::reset");
        self.drain_pending_completion_events();
        self.next_generation = self.next_generation.saturating_add(1).max(1);

        let released_capacity = self
            .slots
            .iter()
            .fold(0u64, |total, slot| total.saturating_add(slot.capacity));
        for slot in &mut self.slots {
            slot.buffer = None;
            slot.capacity = 0;
            slot.state = UploadArenaSlotState::Empty;
        }
        self.pooled.clear();

        logger::debug!(
            "frame upload arena: reset persistent slots released_capacity_bytes={released_capacity}"
        );
    }

    /// Prepares staging storage for `required` aligned bytes.
    pub(crate) fn prepare_staging_buffer(
        &mut self,
        device: &wgpu::Device,
        max_buffer_size: u64,
        required: u64,
        staged_writes: usize,
    ) -> PreparedUploadStaging {
        profiling::scope!("frame_upload_arena::prepare_staging");
        if required == 0 {
            return PreparedUploadStaging {
                buffer: None,
                source: UploadStagingSource::None,
                size: 0,
                acquire_stats: UploadArenaAcquireStats::default(),
            };
        }
        if required > max_buffer_size {
            log_oversized_upload(required, max_buffer_size);
            return PreparedUploadStaging {
                buffer: None,
                source: UploadStagingSource::QueueFallbackOversized,
                size: required,
                acquire_stats: UploadArenaAcquireStats {
                    oversized_queue_fallback_writes: staged_writes,
                    ..UploadArenaAcquireStats::default()
                },
            };
        }

        if let Some(slot) = select_writable_slot(&self.slots, required) {
            return self.prepare_persistent_slot(device, max_buffer_size, required, slot);
        }

        if let Some(prepared) = self.prepare_pooled_temporary(device, max_buffer_size, required) {
            return prepared;
        }

        PreparedUploadStaging {
            buffer: Some(create_temporary_staging_buffer(device, required)),
            source: UploadStagingSource::Temporary,
            size: required,
            acquire_stats: UploadArenaAcquireStats {
                temporary_staging_bytes: required,
                temporary_staging_fallbacks: 1,
                ..UploadArenaAcquireStats::default()
            },
        }
    }

    /// Current pressure/capacity sample for diagnostics.
    pub(crate) fn pressure(&self) -> UploadArenaPressure {
        let mut pressure = UploadArenaPressure::default();
        for slot in self.slots.iter().chain(self.pooled.iter()) {
            pressure.capacity_bytes = pressure.capacity_bytes.saturating_add(slot.capacity);
            match slot.state {
                UploadArenaSlotState::Free => {
                    pressure.free_slots = pressure.free_slots.saturating_add(1);
                }
                UploadArenaSlotState::InFlight { .. } => {
                    pressure.in_flight_slots = pressure.in_flight_slots.saturating_add(1);
                }
                UploadArenaSlotState::Remapping { .. } => {
                    pressure.remapping_slots = pressure.remapping_slots.saturating_add(1);
                }
                UploadArenaSlotState::Empty | UploadArenaSlotState::Writing { .. } => {}
            }
        }
        pressure
    }

    /// Returns the next slot generation for reset-policy unit tests.
    #[cfg(test)]
    pub(crate) fn next_generation_for_tests(&self) -> u64 {
        self.next_generation
    }

    fn prepare_persistent_slot(
        &mut self,
        device: &wgpu::Device,
        max_buffer_size: u64,
        required: u64,
        slot: usize,
    ) -> PreparedUploadStaging {
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1).max(1);

        let Some(arena_slot) = self.slots.get_mut(slot) else {
            return PreparedUploadStaging {
                buffer: Some(create_temporary_staging_buffer(device, required)),
                source: UploadStagingSource::Temporary,
                size: required,
                acquire_stats: UploadArenaAcquireStats {
                    temporary_staging_bytes: required,
                    temporary_staging_fallbacks: 1,
                    ..UploadArenaAcquireStats::default()
                },
            };
        };

        let mut acquire_stats = UploadArenaAcquireStats {
            persistent_staging_bytes: required,
            ..UploadArenaAcquireStats::default()
        };
        if arena_slot.buffer.is_none() || arena_slot.capacity < required {
            let Some(capacity) = next_slot_capacity(required, arena_slot.capacity, max_buffer_size)
            else {
                return PreparedUploadStaging {
                    buffer: None,
                    source: UploadStagingSource::QueueFallbackOversized,
                    size: required,
                    acquire_stats: UploadArenaAcquireStats {
                        oversized_queue_fallback_writes: 1,
                        ..UploadArenaAcquireStats::default()
                    },
                };
            };
            arena_slot.buffer = Some(create_persistent_slot_buffer(device, capacity));
            arena_slot.capacity = capacity;
            acquire_stats.persistent_slot_grows = 1;
        } else {
            acquire_stats.persistent_slot_reuses = 1;
        }
        arena_slot.state = UploadArenaSlotState::Writing { generation };
        PreparedUploadStaging {
            buffer: arena_slot.buffer.clone(),
            source: UploadStagingSource::Persistent { slot, generation },
            size: required,
            acquire_stats,
        }
    }

    pub(super) fn finish_persistent_write(
        &mut self,
        slot: usize,
        generation: u64,
    ) -> Option<Box<dyn FnOnce() + Send + 'static>> {
        self.finish_owned_write(StagingOwner::Persistent(slot), generation)
    }

    pub(super) fn finish_pooled_write(
        &mut self,
        slot: usize,
        generation: u64,
    ) -> Option<Box<dyn FnOnce() + Send + 'static>> {
        self.finish_owned_write(StagingOwner::Pooled(slot), generation)
    }

    fn finish_owned_write(
        &mut self,
        owner: StagingOwner,
        generation: u64,
    ) -> Option<Box<dyn FnOnce() + Send + 'static>> {
        let arena_slot = self.owner_slot_mut(owner)?;
        if arena_slot.state != (UploadArenaSlotState::Writing { generation }) {
            return None;
        }
        let buffer = arena_slot.buffer.as_ref()?;
        buffer.unmap();
        arena_slot.state = UploadArenaSlotState::InFlight { generation };
        let tx = self.completion_tx.clone();
        Some(Box::new(move || {
            let _ = tx.send(UploadArenaCompletion::Submitted { owner, generation });
        }))
    }

    fn drain_completions(&mut self) {
        while let Ok(completion) = self.completion_rx.try_recv() {
            match completion {
                UploadArenaCompletion::Submitted { owner, generation } => {
                    self.start_remap(owner, generation);
                }
                UploadArenaCompletion::Remapped {
                    owner,
                    generation,
                    success,
                } => self.finish_remap(owner, generation, success),
            }
        }
    }

    fn drain_pending_completion_events(&self) {
        while self.completion_rx.try_recv().is_ok() {}
    }

    fn should_poll_for_maintenance(&self) -> bool {
        let mut has_pending_completion_work = false;
        let mut has_writable_slot = false;
        for slot in &self.slots {
            has_writable_slot |= slot.state.can_write();
            has_pending_completion_work |= matches!(
                slot.state,
                UploadArenaSlotState::InFlight { .. } | UploadArenaSlotState::Remapping { .. }
            );
        }
        has_pending_completion_work && !has_writable_slot
    }

    /// Poll only when a recycle is required to avoid an unbounded one-shot allocation.
    fn should_block_for_recycle(&self) -> bool {
        self.should_poll_for_maintenance() && !self.pooled_temp_can_write()
    }

    fn pooled_temp_can_write(&self) -> bool {
        self.pooled.len() < UPLOAD_ARENA_POOLED_TEMPS
            || self.pooled.iter().any(|slot| slot.state.can_write())
    }

    fn prepare_pooled_temporary(
        &mut self,
        device: &wgpu::Device,
        max_buffer_size: u64,
        required: u64,
    ) -> Option<PreparedUploadStaging> {
        let slot = select_pooled_slot(&self.pooled, required);
        let slot = match slot {
            Some(slot) => slot,
            None if self.pooled.len() < UPLOAD_ARENA_POOLED_TEMPS => {
                self.pooled.push(UploadArenaSlot::empty());
                self.pooled.len() - 1
            }
            None => return None,
        };
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1).max(1);
        let arena_slot = self.pooled.get_mut(slot)?;
        let mut acquire_stats = UploadArenaAcquireStats {
            temporary_staging_bytes: required,
            ..UploadArenaAcquireStats::default()
        };
        if arena_slot.buffer.is_none() || arena_slot.capacity < required {
            let Some(capacity) = next_slot_capacity(required, arena_slot.capacity, max_buffer_size)
            else {
                return Some(PreparedUploadStaging {
                    buffer: None,
                    source: UploadStagingSource::QueueFallbackOversized,
                    size: required,
                    acquire_stats: UploadArenaAcquireStats {
                        oversized_queue_fallback_writes: 1,
                        ..UploadArenaAcquireStats::default()
                    },
                });
            };
            if let Some(buffer) = arena_slot.buffer.take()
                && arena_slot.state.can_write()
            {
                buffer.unmap();
            }
            arena_slot.buffer = Some(create_persistent_slot_buffer(device, capacity));
            arena_slot.capacity = capacity;
            acquire_stats.temporary_staging_fallbacks = 1;
        } else {
            acquire_stats.temporary_slot_reuses = 1;
        }
        arena_slot.state = UploadArenaSlotState::Writing { generation };
        Some(PreparedUploadStaging {
            buffer: arena_slot.buffer.clone(),
            source: UploadStagingSource::Pooled { slot, generation },
            size: required,
            acquire_stats,
        })
    }

    fn owner_slot_mut(&mut self, owner: StagingOwner) -> Option<&mut UploadArenaSlot> {
        match owner {
            StagingOwner::Persistent(slot) => self.slots.get_mut(slot),
            StagingOwner::Pooled(slot) => self.pooled.get_mut(slot),
        }
    }

    fn poll_for_staging_pressure(&mut self, device: &wgpu::Device) {
        profiling::scope!("frame_upload_arena::poll");
        self.drain_completions();
        let _ = device.poll(wgpu::PollType::Poll);
        self.drain_completions();
    }

    fn start_remap(&mut self, owner: StagingOwner, generation: u64) {
        let Some(arena_slot) = self.owner_slot_mut(owner) else {
            return;
        };
        if arena_slot.state != (UploadArenaSlotState::InFlight { generation }) {
            return;
        }
        let Some(buffer) = arena_slot.buffer.clone() else {
            arena_slot.state = UploadArenaSlotState::Empty;
            arena_slot.capacity = 0;
            return;
        };
        arena_slot.state = UploadArenaSlotState::Remapping { generation };
        let tx = self.completion_tx.clone();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Write, move |result| {
                let _ = tx.send(UploadArenaCompletion::Remapped {
                    owner,
                    generation,
                    success: result.is_ok(),
                });
            });
    }

    fn finish_remap(&mut self, owner: StagingOwner, generation: u64, success: bool) {
        let Some(arena_slot) = self.owner_slot_mut(owner) else {
            return;
        };
        if arena_slot.state != (UploadArenaSlotState::Remapping { generation }) {
            return;
        }
        if success {
            arena_slot.state = UploadArenaSlotState::Free;
            return;
        }
        logger::warn!("frame upload arena: staging slot remap failed; dropping slot");
        arena_slot.buffer = None;
        arena_slot.capacity = 0;
        arena_slot.state = UploadArenaSlotState::Empty;
    }
}

impl Default for PersistentUploadArena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_completion_does_not_free_newer_generation() {
        let mut arena = PersistentUploadArena::new();
        arena.slots[0].state = UploadArenaSlotState::InFlight { generation: 2 };

        arena.start_remap(StagingOwner::Persistent(0), 1);

        assert_eq!(
            arena.slots[0].state,
            UploadArenaSlotState::InFlight { generation: 2 }
        );
    }

    #[test]
    fn successful_remap_returns_slot_to_free_state() {
        let mut arena = PersistentUploadArena::new();
        arena.slots[0].state = UploadArenaSlotState::Remapping { generation: 4 };

        arena.finish_remap(StagingOwner::Persistent(0), 4, true);

        assert_eq!(arena.slots[0].state, UploadArenaSlotState::Free);
    }

    #[test]
    fn failed_remap_drops_slot() {
        let mut arena = PersistentUploadArena::new();
        arena.slots[0].state = UploadArenaSlotState::Remapping { generation: 4 };
        arena.slots[0].capacity = 1024;

        arena.finish_remap(StagingOwner::Persistent(0), 4, false);

        assert_eq!(arena.slots[0].state, UploadArenaSlotState::Empty);
        assert_eq!(arena.slots[0].capacity, 0);
    }

    #[test]
    fn reset_drops_slots_and_ignores_stale_completions() {
        let mut arena = PersistentUploadArena::new();
        arena.slots[0].state = UploadArenaSlotState::Free;
        arena.slots[0].capacity = 64;
        arena.slots[1].state = UploadArenaSlotState::InFlight { generation: 7 };
        arena.slots[1].capacity = 128;
        arena.slots[2].state = UploadArenaSlotState::Remapping { generation: 8 };
        arena.slots[2].capacity = 256;

        arena.reset();
        let next_generation_after_reset = arena.next_generation;
        let _ = arena.completion_tx.send(UploadArenaCompletion::Remapped {
            owner: StagingOwner::Persistent(2),
            generation: 8,
            success: true,
        });
        arena.drain_completions();

        for slot in &arena.slots {
            assert_eq!(slot.state, UploadArenaSlotState::Empty);
            assert_eq!(slot.capacity, 0);
        }
        assert!(
            arena.next_generation > 1,
            "reset must advance generations away from stale callbacks"
        );
        assert_eq!(arena.next_generation, next_generation_after_reset);
    }

    #[test]
    fn pressure_counts_in_flight_and_remapping_slots() {
        let mut arena = PersistentUploadArena::new();
        arena.slots[0].state = UploadArenaSlotState::Free;
        arena.slots[0].capacity = 64;
        arena.slots[1].state = UploadArenaSlotState::InFlight { generation: 1 };
        arena.slots[1].capacity = 128;
        arena.slots[2].state = UploadArenaSlotState::Remapping { generation: 2 };
        arena.slots[2].capacity = 256;

        let pressure = arena.pressure();

        assert_eq!(pressure.capacity_bytes, 448);
        assert_eq!(pressure.free_slots, 1);
        assert_eq!(pressure.in_flight_slots, 1);
        assert_eq!(pressure.remapping_slots, 1);
    }

    #[test]
    fn maintenance_poll_is_skipped_when_empty_slot_can_be_allocated() {
        let mut arena = PersistentUploadArena::new();
        arena.slots[0].state = UploadArenaSlotState::InFlight { generation: 1 };

        assert!(!arena.should_poll_for_maintenance());
    }

    #[test]
    fn maintenance_poll_is_skipped_when_free_slot_can_be_reused() {
        let mut arena = PersistentUploadArena::new();
        arena.slots[0].state = UploadArenaSlotState::InFlight { generation: 1 };
        arena.slots[1].state = UploadArenaSlotState::Remapping { generation: 2 };
        arena.slots[2].state = UploadArenaSlotState::Free;

        assert!(!arena.should_poll_for_maintenance());
    }

    #[test]
    fn maintenance_poll_is_requested_when_all_slots_wait_on_gpu_work() {
        let mut arena = PersistentUploadArena::new();
        for (index, slot) in arena.slots.iter_mut().enumerate() {
            slot.state = UploadArenaSlotState::InFlight {
                generation: index as u64 + 1,
            };
        }

        assert!(arena.should_poll_for_maintenance());
    }

    #[test]
    fn maintenance_poll_is_requested_when_all_slots_wait_on_remap_work() {
        let mut arena = PersistentUploadArena::new();
        for (index, slot) in arena.slots.iter_mut().enumerate() {
            slot.state = UploadArenaSlotState::Remapping {
                generation: index as u64 + 1,
            };
        }

        assert!(arena.should_poll_for_maintenance());
    }

    #[test]
    fn maintenance_poll_is_skipped_when_pooled_temp_can_serve_upload() {
        let mut arena = PersistentUploadArena::new();
        for (index, slot) in arena.slots.iter_mut().enumerate() {
            slot.state = UploadArenaSlotState::InFlight {
                generation: index as u64 + 1,
            };
        }
        arena.pooled.push(UploadArenaSlot {
            buffer: None,
            capacity: 1024,
            state: UploadArenaSlotState::Free,
        });

        assert!(arena.should_poll_for_maintenance());
        assert!(!arena.should_block_for_recycle());
    }

    #[test]
    fn maintenance_poll_is_skipped_while_the_overflow_pool_can_grow() {
        let mut arena = PersistentUploadArena::new();
        for (index, slot) in arena.slots.iter_mut().enumerate() {
            slot.state = UploadArenaSlotState::InFlight {
                generation: index as u64 + 1,
            };
        }

        assert!(arena.should_poll_for_maintenance());
        assert!(!arena.should_block_for_recycle());
    }

    #[test]
    fn maintenance_poll_is_requested_when_persistent_and_pooled_slots_wait() {
        let mut arena = PersistentUploadArena::new();
        for (index, slot) in arena.slots.iter_mut().enumerate() {
            slot.state = UploadArenaSlotState::InFlight {
                generation: index as u64 + 1,
            };
        }
        for index in 0..UPLOAD_ARENA_POOLED_TEMPS {
            arena.pooled.push(UploadArenaSlot {
                buffer: None,
                capacity: 256,
                state: UploadArenaSlotState::InFlight {
                    generation: 100 + index as u64,
                },
            });
        }

        assert!(arena.should_block_for_recycle());
    }

    #[test]
    fn maintenance_poll_is_skipped_while_slots_are_only_being_written() {
        let mut arena = PersistentUploadArena::new();
        for (index, slot) in arena.slots.iter_mut().enumerate() {
            slot.state = UploadArenaSlotState::Writing {
                generation: index as u64 + 1,
            };
        }

        assert!(!arena.should_poll_for_maintenance());
    }
}
