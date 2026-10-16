//! Pass-level GPU timestamp queries for the frame benchmark.
//!
//! Installed only when `--frame-bench-out` is set and the device enables
//! [`wgpu::Features::TIMESTAMP_QUERY`]. Raster passes use pass timestamp writes.
//! Compute and encoder passes use `write_timestamp` when
//! [`wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS`] is also enabled.
//! When the feature is missing, callers record null and keep running.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use super::GpuContext;

/// Timestamp slots reserved for one bench frame. Two queries per pass.
const QUERY_CAPACITY: u32 = 256;
/// Bytes per timestamp query (`u64`).
const TIMESTAMP_STRIDE: u64 = 8;

struct Slot {
    name: String,
    begin: u32,
}

struct Installed {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    period_ns: f32,
    inside_encoders: bool,
}

static INSTALLED: OnceLock<Installed> = OnceLock::new();
static SLOTS: Mutex<Vec<Slot>> = Mutex::new(Vec::new());
static ENABLED: AtomicBool = AtomicBool::new(false);
static POISONED: AtomicBool = AtomicBool::new(false);
static TRUNCATION_LOGGED: AtomicBool = AtomicBool::new(false);

/// Encoder-level timestamp bracket opened around a compute or encoder pass.
pub(crate) struct EncoderScope {
    end: u32,
}

/// Creates the query set when the device supports timestamp queries.
///
/// Later calls are no-ops. A missing feature leaves pass times null.
pub(crate) fn install(device: &Arc<wgpu::Device>, queue: &Arc<wgpu::Queue>) {
    if INSTALLED.get().is_some() || POISONED.load(Ordering::Relaxed) {
        return;
    }
    let features = device.features();
    if !features.contains(wgpu::Features::TIMESTAMP_QUERY) {
        logger::info!("frame bench: adapter lacks TIMESTAMP_QUERY; per-pass GPU times stay null");
        return;
    }
    let period_ns = queue.get_timestamp_period();
    if !period_ns.is_finite() || period_ns <= 0.0 {
        logger::info!("frame bench: timestamp period is unusable; per-pass GPU times stay null");
        return;
    }
    let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("frame_bench_pass_timestamps"),
        ty: wgpu::QueryType::Timestamp,
        count: QUERY_CAPACITY,
    });
    let bytes = u64::from(QUERY_CAPACITY) * TIMESTAMP_STRIDE;
    let resolve = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frame_bench_timestamp_resolve"),
        size: bytes,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    crate::profiling::note_resource_churn!(Buffer, "gpu::frame_bench_timestamp_resolve");
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frame_bench_timestamp_readback"),
        size: bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    crate::profiling::note_resource_churn!(Buffer, "gpu::frame_bench_timestamp_readback");
    let inside_encoders = features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS);
    let installed = Installed {
        device: Arc::clone(device),
        queue: Arc::clone(queue),
        query_set,
        resolve,
        readback,
        period_ns,
        inside_encoders,
    };
    if INSTALLED.set(installed).is_err() {
        return;
    }
    ENABLED.store(true, Ordering::Release);
    logger::info!(
        "frame bench: TIMESTAMP_QUERY enabled (period_ns={period_ns}, inside_encoders={inside_encoders})"
    );
}

/// Whether pass timestamps were installed and have not faulted.
pub(crate) fn timestamps_usable() -> bool {
    ENABLED.load(Ordering::Acquire) && !POISONED.load(Ordering::Acquire)
}

/// Pass timestamp writes for one raster pass, or [`None`] when bench queries are off.
pub(crate) fn raster_timestamp_writes(
    name: &str,
) -> Option<wgpu::RenderPassTimestampWrites<'static>> {
    if !timestamps_usable() {
        return None;
    }
    let installed = INSTALLED.get()?;
    let (begin, end) = alloc_pair(name)?;
    Some(wgpu::RenderPassTimestampWrites {
        query_set: &installed.query_set,
        beginning_of_pass_write_index: Some(begin),
        end_of_pass_write_index: Some(end),
    })
}

/// Opens an encoder timestamp when `TIMESTAMP_QUERY_INSIDE_ENCODERS` is available.
pub(crate) fn begin_encoder_scope(
    encoder: &mut wgpu::CommandEncoder,
    name: &str,
) -> Option<EncoderScope> {
    if !timestamps_usable() {
        return None;
    }
    let installed = INSTALLED.get()?;
    if !installed.inside_encoders {
        return None;
    }
    let (begin, end) = alloc_pair(name)?;
    encoder.write_timestamp(&installed.query_set, begin);
    Some(EncoderScope { end })
}

/// Closes a scope opened by [`begin_encoder_scope`].
pub(crate) fn end_encoder_scope(encoder: &mut wgpu::CommandEncoder, scope: EncoderScope) {
    let Some(installed) = INSTALLED.get() else {
        return;
    };
    if !timestamps_usable() {
        return;
    }
    encoder.write_timestamp(&installed.query_set, scope.end);
}

/// Drops recorded slot names without reading the GPU.
///
/// Used when a tick skipped rendering and no timestamp writes were submitted.
pub(crate) fn discard_slots() {
    let mut slots = SLOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    slots.clear();
}

/// Resolves this frame's pass timestamps into milliseconds.
///
/// Returns [`None`] when queries are unavailable, the readback fails, or no pass
/// was instrumented. The bench keeps going in every case.
pub(crate) fn resolve_pass_ms(gpu: &GpuContext) -> Option<BTreeMap<String, f64>> {
    if !timestamps_usable() {
        discard_slots();
        return None;
    }
    let installed = INSTALLED.get()?;
    let slots = take_slots();
    if slots.is_empty() {
        return None;
    }
    let count = (slots.len() as u32).saturating_mul(2).min(QUERY_CAPACITY);
    gpu.flush_driver();
    if !submit_resolve(installed, count) {
        poison();
        return None;
    }
    let bytes = read_timestamps(installed, count)?;
    let period_ns = installed.period_ns;
    Some(durations_ms(&slots, &bytes, period_ns)).filter(|map| !map.is_empty())
}

fn take_slots() -> Vec<Slot> {
    let mut slots = SLOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::mem::take(&mut *slots)
}

fn alloc_pair(name: &str) -> Option<(u32, u32)> {
    let mut slots = SLOTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let begin = (slots.len() as u32).saturating_mul(2);
    if begin.saturating_add(2) > QUERY_CAPACITY {
        drop(slots);
        if !TRUNCATION_LOGGED.swap(true, Ordering::Relaxed) {
            logger::warn!(
                "frame bench: pass timestamp capacity ({QUERY_CAPACITY}) exceeded; extra passes omitted"
            );
        }
        return None;
    }
    slots.push(Slot {
        name: name.to_string(),
        begin,
    });
    drop(slots);
    Some((begin, begin + 1))
}

fn submit_resolve(installed: &Installed, count: u32) -> bool {
    let byte_len = u64::from(count) * TIMESTAMP_STRIDE;
    let mut encoder = installed
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame_bench_timestamp_resolve"),
        });
    encoder.resolve_query_set(&installed.query_set, 0..count, &installed.resolve, 0);
    encoder.copy_buffer_to_buffer(&installed.resolve, 0, &installed.readback, 0, byte_len);
    let _ = installed.queue.submit([encoder.finish()]);
    true
}

fn read_timestamps(installed: &Installed, count: u32) -> Option<Vec<u8>> {
    let byte_len = u64::from(count) * TIMESTAMP_STRIDE;
    let slice = installed.readback.slice(..byte_len);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    if let Err(err) = installed.device.poll(wgpu::PollType::wait_indefinitely()) {
        logger::warn!("frame bench: timestamp poll failed: {err:?}");
        poison();
        return None;
    }
    let mapped = match receiver.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => true,
        Ok(Err(err)) => {
            logger::warn!("frame bench: timestamp map failed: {err:?}");
            false
        }
        Err(err) => {
            logger::warn!("frame bench: timestamp map timed out: {err}");
            false
        }
    };
    if !mapped {
        poison();
        return None;
    }
    let bytes = slice.get_mapped_range().to_vec();
    installed.readback.unmap();
    Some(bytes)
}

fn durations_ms(slots: &[Slot], bytes: &[u8], period_ns: f32) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    for slot in slots {
        let Some(ms) = pass_ms(bytes, slot.begin, period_ns) else {
            continue;
        };
        out.entry(slot.name.clone())
            .and_modify(|total: &mut f64| *total += ms)
            .or_insert(ms);
    }
    out
}

fn pass_ms(bytes: &[u8], begin: u32, period_ns: f32) -> Option<f64> {
    let start = read_u64(bytes, begin)?;
    let end = read_u64(bytes, begin.saturating_add(1))?;
    let delta = end.checked_sub(start)?;
    let ms = delta as f64 * f64::from(period_ns) / 1_000_000.0;
    (ms.is_finite() && ms >= 0.0).then_some(ms)
}

fn read_u64(bytes: &[u8], index: u32) -> Option<u64> {
    let start = usize::try_from(index).ok()?.checked_mul(8)?;
    let raw: [u8; 8] = bytes.get(start..start + 8)?.try_into().ok()?;
    Some(u64::from_le_bytes(raw))
}

fn poison() {
    POISONED.store(true, Ordering::Release);
    ENABLED.store(false, Ordering::Release);
    discard_slots();
}

#[cfg(test)]
mod tests {
    use super::read_u64;

    #[test]
    fn read_u64_decodes_little_endian_pair() {
        let mut bytes = vec![0u8; 16];
        bytes[..8].copy_from_slice(&1u64.to_le_bytes());
        bytes[8..].copy_from_slice(&4u64.to_le_bytes());
        assert_eq!(read_u64(&bytes, 0), Some(1));
        assert_eq!(read_u64(&bytes, 1), Some(4));
        assert_eq!(read_u64(&bytes, 2), None);
    }
}
