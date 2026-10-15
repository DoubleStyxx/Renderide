//! Deferred texture uploads for one asset-integration drain.
//!
//! Each `Queue::write_texture` locks the shared queue gate and allocates its own staging chunk
//! inside wgpu. A drain that uploads a mip chain does that once per mip. This batch copies every
//! region into one mapped staging buffer and records `copy_buffer_to_texture` commands that share
//! the mesh-upload submit, so the texels that land on the GPU match the tight layout
//! `write_texture` would have uploaded.

use std::cell::RefCell;
use std::ops::Range;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::gpu::{GpuQueueAccessGate, GpuQueueAccessMode};
use crate::upload_arena::PersistentUploadArena;

thread_local! {
    static ACTIVE_BATCH: RefCell<Option<TextureUploadScratch>> = const { RefCell::new(None) };
}

/// CPU-side texture copies accumulated while a [`TextureUploadSession`] is installed.
#[derive(Default)]
pub(crate) struct TextureUploadScratch {
    copies: Vec<PendingTextureCopy>,
    bytes: Vec<u8>,
}

struct PendingTextureCopy {
    texture: wgpu::Texture,
    mip_level: u32,
    origin: wgpu::Origin3d,
    aspect: wgpu::TextureAspect,
    bytes_per_row: u32,
    rows_per_image: u32,
    size: wgpu::Extent3d,
    bytes: Range<usize>,
}

/// Where one tight texture region sits inside the shared staging buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TextureStagingPlacement {
    staging_offset: u64,
    padded_bytes_per_row: u32,
}

/// Installs the shared scratch as the active texture-upload batch for this thread.
pub(crate) struct TextureUploadSession {
    home: Arc<Mutex<TextureUploadScratch>>,
    active: bool,
}

impl TextureUploadSession {
    /// Moves the scratch out of `home` and records subsequent texture writes on this thread.
    pub(crate) fn begin(home: Arc<Mutex<TextureUploadScratch>>) -> Self {
        let scratch = std::mem::take(&mut *home.lock());
        ACTIVE_BATCH.with(|slot| {
            *slot.borrow_mut() = Some(scratch);
        });
        Self { home, active: true }
    }

    fn store(&mut self) {
        if !self.active {
            return;
        }
        let scratch = ACTIVE_BATCH.with(|slot| slot.borrow_mut().take());
        if let Some(scratch) = scratch {
            *self.home.lock() = scratch;
        }
        self.active = false;
    }
}

impl Drop for TextureUploadSession {
    fn drop(&mut self) {
        self.store();
    }
}

/// Records one texture region when a session is active.
///
/// Returns `false` when no session is installed or the layout cannot be padded into a
/// `copy_buffer_to_texture`. Callers then keep the existing `Queue::write_texture` path.
pub(crate) fn try_record_texture_copy(
    destination: &wgpu::TexelCopyTextureInfo<'_>,
    bytes: &[u8],
    layout: &wgpu::TexelCopyBufferLayout,
    size: wgpu::Extent3d,
) -> bool {
    let Some(bytes_per_row) = layout.bytes_per_row else {
        return false;
    };
    let Some(rows_per_image) = layout.rows_per_image else {
        return false;
    };
    if bytes_per_row == 0
        || rows_per_image == 0
        || size.depth_or_array_layers == 0
        || bytes.is_empty()
    {
        return false;
    }
    ACTIVE_BATCH.with(|slot| {
        let mut active = slot.borrow_mut();
        let Some(scratch) = active.as_mut() else {
            return false;
        };
        let start = scratch.bytes.len();
        scratch.bytes.extend_from_slice(bytes);
        scratch.copies.push(PendingTextureCopy {
            texture: destination.texture.clone(),
            mip_level: destination.mip_level,
            origin: destination.origin,
            aspect: destination.aspect,
            bytes_per_row,
            rows_per_image,
            size,
            bytes: start..scratch.bytes.len(),
        });
        true
    })
}

/// Staged texture copies ready to share a submit with mesh uploads.
pub(crate) struct TextureUploadFlush {
    /// Copy commands. `None` when every region was replayed through `write_texture`.
    pub(crate) command_buffer: Option<wgpu::CommandBuffer>,
    /// Recycles the staging buffer after the submit completes.
    pub(crate) on_submitted_work_done: Option<Box<dyn FnOnce() + Send + 'static>>,
    /// Staging buffer that must stay alive until the driver calls `Queue::submit`.
    pub(crate) staging_buffer: Option<wgpu::Buffer>,
}

/// Takes the recorded regions and stages them into `upload_arena`.
///
/// Regions that cannot fit a staging buffer are replayed through `Queue::write_texture` while
/// `gate` is held. If the gate cannot be acquired, the bytes stay in the active scratch and the
/// next drain retries them.
pub(crate) fn flush_recorded(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    gate: &GpuQueueAccessGate,
    queue_access_mode: GpuQueueAccessMode,
    queue_gate_held: bool,
    max_buffer_size: u64,
    upload_arena: &mut PersistentUploadArena,
) -> Option<TextureUploadFlush> {
    profiling::scope!("texture_upload::flush");
    let recorded = take_recorded()?;
    let placements = plan_texture_staging(recorded.copies.iter().map(|copy| TextureStagingShape {
        bytes_per_row: copy.bytes_per_row,
        rows_per_image: copy.rows_per_image,
        layers: copy.size.depth_or_array_layers,
    }));
    let Some(placements) = placements else {
        replay_or_keep(queue, gate, queue_access_mode, queue_gate_held, recorded);
        return None;
    };
    if placements.total_bytes > max_buffer_size {
        replay_or_keep(queue, gate, queue_access_mode, queue_gate_held, recorded);
        return None;
    }
    let staging = upload_arena.prepare_staging_buffer(
        device,
        max_buffer_size,
        placements.total_bytes,
        recorded.copies.len(),
    );
    if staging.buffer().is_none() {
        let _ = staging.finish(upload_arena);
        replay_or_keep(queue, gate, queue_access_mode, queue_gate_held, recorded);
        return None;
    }
    fill_staging_buffer(
        staging.buffer().expect("buffer checked above"),
        &recorded,
        &placements.placements,
    );
    let staging = staging.finish(upload_arena);
    let command_buffer = record_texture_copies(
        device,
        &recorded,
        &placements.placements,
        staging.buffer.as_ref(),
    );
    let flush = TextureUploadFlush {
        command_buffer,
        on_submitted_work_done: staging.on_submitted_work_done,
        staging_buffer: staging.buffer,
    };
    restore_recorded_capacity(recorded);
    Some(flush)
}

struct RecordedTextureCopies {
    copies: Vec<PendingTextureCopy>,
    bytes: Vec<u8>,
}

fn take_recorded() -> Option<RecordedTextureCopies> {
    ACTIVE_BATCH.with(|slot| {
        let mut active = slot.borrow_mut();
        let scratch = active.as_mut()?;
        if scratch.copies.is_empty() {
            return None;
        }
        Some(RecordedTextureCopies {
            copies: std::mem::take(&mut scratch.copies),
            bytes: std::mem::take(&mut scratch.bytes),
        })
    })
}

fn restore_recorded_capacity(mut recorded: RecordedTextureCopies) {
    recorded.copies.clear();
    recorded.bytes.clear();
    ACTIVE_BATCH.with(|slot| {
        let mut active = slot.borrow_mut();
        if let Some(scratch) = active.as_mut() {
            scratch.copies = recorded.copies;
            scratch.bytes = recorded.bytes;
        }
    });
}

struct TextureStagingShape {
    bytes_per_row: u32,
    rows_per_image: u32,
    layers: u32,
}

struct TextureStagingPlan {
    placements: Vec<TextureStagingPlacement>,
    total_bytes: u64,
}

fn plan_texture_staging(
    shapes: impl IntoIterator<Item = TextureStagingShape>,
) -> Option<TextureStagingPlan> {
    profiling::scope!("texture_upload::plan_staging");
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let mut placements = Vec::new();
    let mut cursor = 0u64;
    for shape in shapes {
        let padded_bytes_per_row = align_up_u32(shape.bytes_per_row, align)?;
        let staging_offset = align_up_u64(cursor, u64::from(align))?;
        let row_span = u64::from(padded_bytes_per_row)
            .checked_mul(u64::from(shape.rows_per_image))?
            .checked_mul(u64::from(shape.layers.max(1)))?;
        cursor = staging_offset.checked_add(row_span)?;
        placements.push(TextureStagingPlacement {
            staging_offset,
            padded_bytes_per_row,
        });
    }
    Some(TextureStagingPlan {
        placements,
        total_bytes: cursor,
    })
}

fn align_up_u32(value: u32, align: u32) -> Option<u32> {
    let align = align.max(1);
    let remainder = value % align;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(align - remainder)
    }
}

fn align_up_u64(value: u64, align: u64) -> Option<u64> {
    let align = align.max(1);
    let remainder = value % align;
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(align - remainder)
    }
}

fn fill_staging_buffer(
    buffer: &wgpu::Buffer,
    recorded: &RecordedTextureCopies,
    placements: &[TextureStagingPlacement],
) {
    profiling::scope!("texture_upload::copy_to_staging");
    let mut mapped = buffer.slice(..).get_mapped_range_mut();
    for (copy, placement) in recorded.copies.iter().zip(placements.iter()) {
        let tight = copy.bytes_per_row as usize;
        let padded = placement.padded_bytes_per_row as usize;
        let rows = copy.rows_per_image as usize;
        let layers = copy.size.depth_or_array_layers.max(1) as usize;
        let src = &recorded.bytes[copy.bytes.clone()];
        for layer in 0..layers {
            for row in 0..rows {
                let src_offset = (layer * rows + row) * tight;
                let dst_offset = placement.staging_offset as usize + (layer * rows + row) * padded;
                let src_end = src_offset + tight;
                let dst_end = dst_offset + tight;
                debug_assert!(
                    src_end <= src.len() && dst_end <= mapped.len(),
                    "texture staging copy exceeded the recorded range"
                );
                if src_end > src.len() || dst_end > mapped.len() {
                    continue;
                }
                mapped[dst_offset..dst_end].copy_from_slice(&src[src_offset..src_end]);
            }
        }
    }
}

fn record_texture_copies(
    device: &wgpu::Device,
    recorded: &RecordedTextureCopies,
    placements: &[TextureStagingPlacement],
    staging: Option<&wgpu::Buffer>,
) -> Option<wgpu::CommandBuffer> {
    let staging = staging?;
    profiling::scope!("texture_upload::record_encoder");
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("texture_upload_staging_batch"),
    });
    for (copy, placement) in recorded.copies.iter().zip(placements.iter()) {
        encoder.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: placement.staging_offset,
                    bytes_per_row: Some(placement.padded_bytes_per_row),
                    rows_per_image: Some(copy.rows_per_image),
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture: &copy.texture,
                mip_level: copy.mip_level,
                origin: copy.origin,
                aspect: copy.aspect,
            },
            copy.size,
        );
    }
    Some(encoder.finish())
}

fn replay_or_keep(
    queue: &wgpu::Queue,
    gate: &GpuQueueAccessGate,
    queue_access_mode: GpuQueueAccessMode,
    queue_gate_held: bool,
    recorded: RecordedTextureCopies,
) {
    if !queue_gate_held && gate.lock_for(queue_access_mode).is_none() {
        restore_recorded_bytes(recorded);
        return;
    }
    profiling::scope!("texture_upload::queue_fallback");
    for copy in &recorded.copies {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &copy.texture,
                mip_level: copy.mip_level,
                origin: copy.origin,
                aspect: copy.aspect,
            },
            &recorded.bytes[copy.bytes.clone()],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(copy.bytes_per_row),
                rows_per_image: Some(copy.rows_per_image),
            },
            copy.size,
        );
    }
    restore_recorded_capacity(recorded);
}

fn restore_recorded_bytes(recorded: RecordedTextureCopies) {
    ACTIVE_BATCH.with(|slot| {
        let mut active = slot.borrow_mut();
        let Some(scratch) = active.as_mut() else {
            return;
        };
        let byte_base = scratch.bytes.len();
        scratch.bytes.extend_from_slice(&recorded.bytes);
        scratch
            .copies
            .extend(recorded.copies.into_iter().map(|mut copy| {
                copy.bytes = copy.bytes.start + byte_base..copy.bytes.end + byte_base;
                copy
            }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_plan_pads_rows_to_copy_alignment_and_packs_images() {
        let plan = plan_texture_staging([
            TextureStagingShape {
                bytes_per_row: 400,
                rows_per_image: 2,
                layers: 1,
            },
            TextureStagingShape {
                bytes_per_row: 256,
                rows_per_image: 1,
                layers: 2,
            },
        ])
        .expect("plan");

        assert_eq!(
            plan.placements,
            vec![
                TextureStagingPlacement {
                    staging_offset: 0,
                    padded_bytes_per_row: 512,
                },
                TextureStagingPlacement {
                    staging_offset: 1024,
                    padded_bytes_per_row: 256,
                },
            ]
        );
        assert_eq!(plan.total_bytes, 1536);
    }

    #[test]
    fn staging_plan_rejects_overflowing_row_padding() {
        let plan = plan_texture_staging([TextureStagingShape {
            bytes_per_row: u32::MAX - 10,
            rows_per_image: 1,
            layers: 1,
        }]);

        assert!(plan.is_none());
    }

    #[test]
    fn align_up_is_identity_when_already_aligned() {
        assert_eq!(align_up_u32(256, 256), Some(256));
        assert_eq!(align_up_u64(512, 256), Some(512));
    }
}
