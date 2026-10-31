//! CPU and GPU snapshots of the packed light buffer for the #216 canary.
//!
//! Off unless `RENDERIDE_LIGHT_PACK_PROBE` is `1` or `true`. The canary CI run
//! sets that for Windows WARP and Linux llvmpipe. A normal run does not add
//! `COPY_SRC`, does not log, and does not map a buffer.
//!
//! The GPU copy uses `copy_buffer_to_buffer` into a `MAP_READ` staging buffer.
//! `get_mapped_range` returns a byte slice here, so this file has no `unsafe`.

use std::mem::size_of;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use bytemuck::Zeroable;
use parking_lot::Mutex;

use crate::gpu::frame_bindings::GpuLight;

const ENV: &str = "RENDERIDE_LIGHT_PACK_PROBE";
const MAX_LOGGED_LIGHTS: usize = 32;
const CLUSTER_Z: u32 = 16;
const TILE_PX: u32 = 32;
const HOLD_EVERY: u32 = 8;

const CASES: &[&str] = &[
    "negative_light_physically_based",
    "negative_color_physically_based",
    "negative_light_parity",
    "pbs_lit_material_matrix",
];

static ENABLED: OnceLock<bool> = OnceLock::new();
static ARMED: AtomicBool = AtomicBool::new(false);
static CASE: Mutex<ArmedCase> = Mutex::new(ArmedCase {
    name: "",
    file: String::new(),
});
static SNAPSHOT: Mutex<Option<Snapshot>> = Mutex::new(None);
static LAST_CPU: Mutex<String> = Mutex::new(String::new());
static LAST_SIG: Mutex<String> = Mutex::new(String::new());
static HOLD: AtomicU32 = AtomicU32::new(0);

struct ArmedCase {
    name: &'static str,
    file: String,
}

struct Snapshot {
    lights: Vec<GpuLight>,
    /// `true` when the upload wrote one all-zero row because the list was empty.
    zero_row: bool,
    lights_buffer: wgpu::Buffer,
    counts: Option<wgpu::Buffer>,
    indices: Option<wgpu::Buffer>,
    cluster_count_x: u32,
    cluster_count_y: u32,
    light_count: u32,
    viewport: (u32, u32),
    cleared: bool,
    cluster_seen: bool,
}

/// Storage usage for the light and cluster buffers.
///
/// `COPY_SRC` is added only while the probe env is set, so the copy can run
/// after the frame submit.
pub(crate) fn storage_usage() -> wgpu::BufferUsages {
    let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
    if enabled() {
        usage | wgpu::BufferUsages::COPY_SRC
    } else {
        usage
    }
}

/// `true` after [`arm`] matched one of the four lit cases.
pub(crate) fn armed() -> bool {
    ARMED.load(Ordering::Acquire)
}

/// Arms the probe when `path` is one of the four lit captures and the env is set.
pub(crate) fn arm(path: &Path) {
    if !enabled() {
        return;
    }
    let Some(name) = case_name(path) else {
        return;
    };
    let file = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "frame".to_owned());
    {
        let mut slot = CASE.lock();
        slot.name = name;
        slot.file = file;
    }
    ARMED.store(true, Ordering::Release);
    logger::info!("diag-light-8b armed case={name} path={}", path.display());
}

/// Remembers the packed rows just written into `buffer`.
pub(crate) fn note_cpu_upload(buffer: &wgpu::Buffer, lights: &[GpuLight]) {
    if !armed() {
        return;
    }
    let logged = lights.len().min(MAX_LOGGED_LIGHTS);
    let line = format_cpu_lights(&lights[..logged]);
    let changed = {
        let mut last = LAST_CPU.lock();
        if *last == line {
            false
        } else {
            last.clone_from(&line);
            true
        }
    };
    if changed {
        log_cpu_lights(&lights[..logged]);
    }
    let mut slot = SNAPSHOT.lock();
    let previous = slot.take();
    *slot = Some(Snapshot {
        lights: lights[..logged].to_vec(),
        zero_row: lights.is_empty(),
        lights_buffer: buffer.clone(),
        counts: previous.as_ref().and_then(|prev| prev.counts.clone()),
        indices: previous.as_ref().and_then(|prev| prev.indices.clone()),
        cluster_count_x: previous
            .as_ref()
            .map(|prev| prev.cluster_count_x)
            .unwrap_or(0),
        cluster_count_y: previous
            .as_ref()
            .map(|prev| prev.cluster_count_y)
            .unwrap_or(0),
        light_count: previous
            .as_ref()
            .map(|prev| prev.light_count)
            .unwrap_or(lights.len() as u32),
        viewport: previous
            .as_ref()
            .map(|prev| prev.viewport)
            .unwrap_or((0, 0)),
        cleared: previous.as_ref().is_some_and(|prev| prev.cleared),
        cluster_seen: previous.as_ref().is_some_and(|prev| prev.cluster_seen),
    });
}

/// One frame's cluster buffers and the grid the center tile is indexed with.
pub(crate) struct ClusterNote {
    /// Per-cluster range buffer (`vec2<u32>` offset, count).
    pub counts: wgpu::Buffer,
    /// Compact index buffer.
    pub indices: wgpu::Buffer,
    /// Cluster grid width in tiles.
    pub cluster_count_x: u32,
    /// Cluster grid height in tiles.
    pub cluster_count_y: u32,
    /// Light count the compute pass used as the index stride.
    pub light_count: u32,
    /// Viewport in pixels.
    pub viewport: (u32, u32),
    /// The pass cleared ranges because the CPU light count was 0.
    pub cleared: bool,
}

/// Remembers the cluster buffers written by this frame's compute pass.
pub(crate) fn note_cluster(note: ClusterNote) {
    if !armed() {
        return;
    }
    if let Some(snapshot) = SNAPSHOT.lock().as_mut() {
        snapshot.counts = Some(note.counts);
        snapshot.indices = Some(note.indices);
        snapshot.cluster_count_x = note.cluster_count_x;
        snapshot.cluster_count_y = note.cluster_count_y;
        snapshot.light_count = note.light_count;
        snapshot.viewport = note.viewport;
        snapshot.cleared = note.cleared;
        snapshot.cluster_seen = true;
    }
}

/// Copies the stashed light buffer and the center-tile cluster column back to the CPU.
pub(crate) fn roundtrip(gpu: &crate::gpu::GpuContext, adapter: &wgpu::AdapterInfo) {
    if !armed() {
        return;
    }
    let Some(snapshot) = SNAPSHOT.lock().as_ref().map(Snapshot::clone_handles) else {
        return;
    };
    let (case, file) = {
        let armed = CASE.lock();
        (armed.name, armed.file.clone())
    };
    let adapter_label = format!(
        "{} backend={:?} vendor={:#06x} device={:#06x}",
        adapter.name, adapter.backend, adapter.vendor, adapter.device
    );
    match read_gpu(gpu, &snapshot) {
        Ok(gpu) => log_roundtrip(case, &file, &adapter_label, &snapshot, &gpu),
        Err(err) => {
            logger::info!(
                "diag-light-8b summary case={case} file={file} adapter={adapter_label} gpu_match=error error={err}"
            );
        }
    }
}

fn enabled() -> bool {
    *ENABLED.get_or_init(|| {
        std::env::var(ENV).is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn case_name(path: &Path) -> Option<&'static str> {
    let text = path.to_string_lossy();
    CASES.iter().copied().find(|name| text.contains(name))
}

struct GpuRead {
    lights: Vec<GpuLight>,
    /// Range `(offset, count)` for each Z slice of the center tile. Empty when the grid was missing.
    ranges: Vec<(u32, u32)>,
    /// First indices of each Z slice, parallel to `ranges`.
    indices: Vec<Vec<u32>>,
    tile: (u32, u32),
}

fn read_gpu(gpu: &crate::gpu::GpuContext, snapshot: &Snapshot) -> Result<GpuRead, String> {
    let device = gpu.device();
    let light_bytes = light_copy_bytes(snapshot);
    let tile = center_tile(
        snapshot.viewport,
        snapshot.cluster_count_x,
        snapshot.cluster_count_y,
    );
    let z_count = z_slices(snapshot);
    let index_words = index_words_per_slice(snapshot.light_count);
    let range_bytes = z_count.saturating_mul(8);
    let index_bytes = z_count.saturating_mul(index_words).saturating_mul(4);
    let total = light_bytes
        .saturating_add(range_bytes)
        .saturating_add(index_bytes);
    if total == 0 {
        return Err("nothing to copy".to_owned());
    }
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("diag-light-8b-readback"),
        size: total,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("diag-light-8b-readback"),
    });
    if light_bytes > 0 && light_bytes <= snapshot.lights_buffer.size() {
        let copy_scope = crate::profiling::GpuEncoderScope::begin(
            None,
            "diag-light-8b::light-copy",
            &mut encoder,
        );
        encoder.copy_buffer_to_buffer(&snapshot.lights_buffer, 0, &staging, 0, light_bytes);
        copy_scope.end(&mut encoder);
    }
    copy_center_column(
        &mut encoder,
        snapshot,
        tile,
        z_count,
        index_words,
        &staging,
        light_bytes,
    );
    gpu.queue().submit(std::iter::once(encoder.finish()));
    let bytes = map_staging(device, &staging, total)?;
    let lights = decode_lights(&bytes[..light_bytes as usize]);
    let (ranges, indices) = decode_column(&bytes, light_bytes, z_count, index_words);
    staging.unmap();
    Ok(GpuRead {
        lights,
        ranges,
        indices,
        tile,
    })
}

fn light_copy_bytes(snapshot: &Snapshot) -> u64 {
    let rows = if snapshot.zero_row {
        1
    } else {
        snapshot.lights.len()
    };
    (rows * size_of::<GpuLight>()) as u64
}

fn z_slices(snapshot: &Snapshot) -> u64 {
    if snapshot.cluster_seen && snapshot.cluster_count_x > 0 && snapshot.cluster_count_y > 0 {
        u64::from(CLUSTER_Z)
    } else {
        0
    }
}

fn index_words_per_slice(light_count: u32) -> u64 {
    u64::from(light_count.min(MAX_LOGGED_LIGHTS as u32))
}

fn center_tile(viewport: (u32, u32), count_x: u32, count_y: u32) -> (u32, u32) {
    let tile = |pixels: u32, count: u32| {
        if count == 0 {
            return 0;
        }
        (pixels / 2 / TILE_PX).min(count - 1)
    };
    (tile(viewport.0, count_x), tile(viewport.1, count_y))
}

fn local_cluster_id(tile: (u32, u32), z: u32, count_x: u32, count_y: u32) -> Option<u32> {
    let row = tile.1.checked_add(count_y.checked_mul(z)?)?;
    tile.0.checked_add(count_x.checked_mul(row)?)
}

fn copy_center_column(
    encoder: &mut wgpu::CommandEncoder,
    snapshot: &Snapshot,
    tile: (u32, u32),
    z_count: u64,
    index_words: u64,
    staging: &wgpu::Buffer,
    light_bytes: u64,
) {
    let (Some(counts), Some(indices)) = (snapshot.counts.as_ref(), snapshot.indices.as_ref())
    else {
        return;
    };
    let copy_scope =
        crate::profiling::GpuEncoderScope::begin(None, "diag-light-8b::cluster-copy", encoder);
    for z in 0..z_count {
        let Some(id) = local_cluster_id(
            tile,
            z as u32,
            snapshot.cluster_count_x,
            snapshot.cluster_count_y,
        ) else {
            continue;
        };
        let range_off = u64::from(id).saturating_mul(8);
        let dst = light_bytes.saturating_add(z.saturating_mul(8));
        if range_off.saturating_add(8) <= counts.size() {
            encoder.copy_buffer_to_buffer(counts, range_off, staging, dst, 8);
        }
        if index_words == 0 {
            continue;
        }
        let index_off = u64::from(id)
            .saturating_mul(u64::from(snapshot.light_count))
            .saturating_mul(4);
        let index_size = index_words.saturating_mul(4);
        let index_dst = light_bytes
            .saturating_add(z_count.saturating_mul(8))
            .saturating_add(z.saturating_mul(index_size));
        if index_off.saturating_add(index_size) <= indices.size() {
            encoder.copy_buffer_to_buffer(indices, index_off, staging, index_dst, index_size);
        }
    }
    copy_scope.end(encoder);
}

fn map_staging(
    device: &wgpu::Device,
    staging: &wgpu::Buffer,
    total: u64,
) -> Result<Vec<u8>, String> {
    let slice = staging.slice(..total);
    let (sender, receiver) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|err| format!("poll {err}"))?;
    match receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(())) => {}
        Ok(Err(err)) => return Err(format!("map {err:?}")),
        Err(_) => return Err("map timed out".to_owned()),
    }
    let view = slice
        .get_mapped_range()
        .map_err(|err| format!("mapped range {err}"))?;
    Ok(view.to_vec())
}

fn decode_lights(bytes: &[u8]) -> Vec<GpuLight> {
    let row = size_of::<GpuLight>();
    bytes
        .chunks_exact(row)
        .map(bytemuck::pod_read_unaligned)
        .collect()
}

fn decode_column(
    bytes: &[u8],
    light_bytes: u64,
    z_count: u64,
    index_words: u64,
) -> (Vec<(u32, u32)>, Vec<Vec<u32>>) {
    let mut ranges = Vec::new();
    let mut indices = Vec::new();
    let start = light_bytes as usize;
    for z in 0..z_count as usize {
        let at = start + z * 8;
        let range = if at + 8 <= bytes.len() {
            let offset = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap_or([0; 4]));
            let count = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap_or([0; 4]));
            (offset, count)
        } else {
            (0, 0)
        };
        ranges.push(range);
        let index_size = index_words as usize * 4;
        let index_at = start + z_count as usize * 8 + z * index_size;
        let mut row = Vec::new();
        let keep = range.1.min(index_words as u32) as usize;
        for word in 0..keep {
            let word_at = index_at + word * 4;
            if word_at + 4 <= bytes.len() {
                row.push(u32::from_le_bytes(
                    bytes[word_at..word_at + 4].try_into().unwrap_or([0; 4]),
                ));
            }
        }
        indices.push(row);
    }
    (ranges, indices)
}

fn log_roundtrip(case: &str, file: &str, adapter: &str, snapshot: &Snapshot, gpu: &GpuRead) {
    let cpu = cpu_rows(snapshot);
    let gpu_match = lights_match(&cpu, &gpu.lights, snapshot.zero_row);
    let reordered = !gpu_match && same_lights_reordered(&cpu, &gpu.lights);
    let (nan, inf, neg) = float_flags(&cpu);
    let center_empty = gpu.ranges.iter().all(|(_, count)| *count == 0);
    let offset_mismatch = offset_mismatches(snapshot, gpu);
    let z_counts = gpu
        .ranges
        .iter()
        .map(|(_, count)| count.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let signature = format!(
        "{gpu_match}|{reordered}|{nan}|{inf}|{neg}|{center_empty}|{offset_mismatch}|{z_counts}|{}",
        cpu.len()
    );
    let changed = {
        let mut last = LAST_SIG.lock();
        if *last == signature {
            false
        } else {
            last.clone_from(&signature);
            true
        }
    };
    if changed {
        HOLD.store(0, Ordering::Relaxed);
        log_gpu_lights(case, file, snapshot, gpu, &cpu);
        log_cluster(case, file, gpu);
    } else {
        let samples = HOLD.fetch_add(1, Ordering::Relaxed).saturating_add(1);
        if !samples.is_multiple_of(HOLD_EVERY) {
            return;
        }
    }
    logger::info!(
        "diag-light-8b summary case={case} file={file} adapter={adapter} n={} cluster_lights={} gpu_match={} reordered={} nan={nan} inf={inf} neg={neg} cleared={} cluster_seen={} center_empty={} offset_mismatch={offset_mismatch} tile={},{} z_counts={z_counts}",
        cpu.len(),
        snapshot.light_count,
        bit(gpu_match),
        bit(reordered),
        bit(snapshot.cleared),
        bit(snapshot.cluster_seen),
        bit(center_empty),
        gpu.tile.0,
        gpu.tile.1,
    );
}

fn cpu_rows(snapshot: &Snapshot) -> Vec<GpuLight> {
    if snapshot.zero_row {
        vec![GpuLight::zeroed()]
    } else {
        snapshot.lights.clone()
    }
}

fn lights_match(cpu: &[GpuLight], gpu: &[GpuLight], zero_row: bool) -> bool {
    if zero_row {
        return gpu
            .first()
            .is_some_and(|row| bytemuck::bytes_of(row).iter().all(|byte| *byte == 0));
    }
    cpu.len() == gpu.len()
        && cpu
            .iter()
            .zip(gpu)
            .all(|(left, right)| bytemuck::bytes_of(left) == bytemuck::bytes_of(right))
}

fn same_lights_reordered(cpu: &[GpuLight], gpu: &[GpuLight]) -> bool {
    if cpu.len() != gpu.len() || cpu.is_empty() {
        return false;
    }
    let mut left = cpu.iter().map(light_bits).collect::<Vec<_>>();
    let mut right = gpu.iter().map(light_bits).collect::<Vec<_>>();
    left.sort_unstable();
    right.sort_unstable();
    left == right
}

fn light_bits(light: &GpuLight) -> Vec<u8> {
    bytemuck::bytes_of(light).to_vec()
}

fn float_flags(lights: &[GpuLight]) -> (u32, u32, u32) {
    let mut nan = 0u32;
    let mut inf = 0u32;
    let mut neg = 0u32;
    for light in lights {
        let mut saw_nan = false;
        let mut saw_inf = false;
        let mut saw_neg = false;
        for value in light_floats(light) {
            if value.is_nan() {
                saw_nan = true;
            } else if value.is_infinite() {
                saw_inf = true;
            } else if value < 0.0 {
                saw_neg = true;
            }
        }
        nan += u32::from(saw_nan);
        inf += u32::from(saw_inf);
        neg += u32::from(saw_neg);
    }
    (nan, inf, neg)
}

fn light_floats(light: &GpuLight) -> [f32; 11] {
    [
        light.position[0],
        light.position[1],
        light.position[2],
        light.direction[0],
        light.direction[1],
        light.direction[2],
        light.color[0],
        light.color[1],
        light.color[2],
        light.intensity,
        light.range,
    ]
}

fn offset_mismatches(snapshot: &Snapshot, gpu: &GpuRead) -> u32 {
    if !snapshot.cluster_seen || snapshot.light_count == 0 {
        return 0;
    }
    let mut mismatches = 0u32;
    for (z, (offset, count)) in gpu.ranges.iter().copied().enumerate() {
        if count == 0 {
            continue;
        }
        let Some(id) = local_cluster_id(
            gpu.tile,
            z as u32,
            snapshot.cluster_count_x,
            snapshot.cluster_count_y,
        ) else {
            continue;
        };
        let expected = id.saturating_mul(snapshot.light_count);
        if offset != expected {
            mismatches = mismatches.saturating_add(1);
        }
    }
    mismatches
}

fn log_cpu_lights(lights: &[GpuLight]) {
    let (name, file) = case_file();
    for (index, light) in lights.iter().enumerate() {
        logger::info!(
            "diag-light-8b cpu case={name} file={file} {}",
            format_light(index, light)
        );
    }
    if lights.is_empty() {
        logger::info!("diag-light-8b cpu case={name} file={file} n=0");
    }
}

fn log_gpu_lights(case: &str, file: &str, snapshot: &Snapshot, gpu: &GpuRead, cpu: &[GpuLight]) {
    let rows = cpu.len().max(gpu.lights.len());
    for index in 0..rows {
        match (cpu.get(index), gpu.lights.get(index)) {
            (Some(left), Some(right)) => {
                logger::info!(
                    "diag-light-8b gpu case={case} file={file} match={} {}",
                    bit(bytemuck::bytes_of(left) == bytemuck::bytes_of(right)),
                    format_light(index, right)
                );
                log_field_diffs(case, file, index, left, right);
            }
            (None, Some(right)) => {
                logger::info!(
                    "diag-light-8b gpu case={case} file={file} match=0 extra {}",
                    format_light(index, right)
                );
            }
            (Some(left), None) => {
                logger::info!(
                    "diag-light-8b gpu case={case} file={file} match=0 missing {}",
                    format_light(index, left)
                );
            }
            (None, None) => {}
        }
    }
    if snapshot.zero_row {
        logger::info!(
            "diag-light-8b gpu case={case} file={file} zero_row={}",
            bit(gpu
                .lights
                .first()
                .is_some_and(|row| { bytemuck::bytes_of(row).iter().all(|byte| *byte == 0) }))
        );
    }
}

fn log_field_diffs(case: &str, file: &str, index: usize, cpu: &GpuLight, gpu: &GpuLight) {
    let left = bytemuck::bytes_of(cpu);
    let right = bytemuck::bytes_of(gpu);
    for word in 0..(left.len() / 4) {
        let at = word * 4;
        if left[at..at + 4] != right[at..at + 4] {
            logger::info!(
                "diag-light-8b diff case={case} file={file} i={index} field={} cpu_bits={} gpu_bits={}",
                field_name(at),
                hex_word(&left[at..at + 4]),
                hex_word(&right[at..at + 4])
            );
        }
    }
}

fn log_cluster(case: &str, file: &str, gpu: &GpuRead) {
    for (z, ((offset, count), indices)) in gpu.ranges.iter().zip(gpu.indices.iter()).enumerate() {
        if *count == 0 {
            continue;
        }
        let listed = indices
            .iter()
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(",");
        logger::info!(
            "diag-light-8b cluster case={case} file={file} tile={},{} z={z} offset={offset} count={count} indices={listed}",
            gpu.tile.0,
            gpu.tile.1
        );
    }
}

fn format_cpu_lights(lights: &[GpuLight]) -> String {
    lights
        .iter()
        .enumerate()
        .map(|(index, light)| format_light(index, light))
        .collect::<Vec<_>>()
        .join(";")
}

fn format_light(index: usize, light: &GpuLight) -> String {
    format!(
        "i={index} type={} pos={} pos_bits={} dir={} dir_bits={} color={} color_bits={} intensity={} intensity_bits={} range={} range_bits={}",
        light.light_type,
        fmt_vec3(light.position),
        bits_vec3(light.position),
        fmt_vec3(light.direction),
        bits_vec3(light.direction),
        fmt_vec3(light.color),
        bits_vec3(light.color),
        fmt_f32(light.intensity),
        hex_bits(light.intensity.to_bits()),
        fmt_f32(light.range),
        hex_bits(light.range.to_bits()),
    )
}

fn case_file() -> (&'static str, String) {
    let armed = CASE.lock();
    (armed.name, armed.file.clone())
}

fn fmt_vec3(value: [f32; 3]) -> String {
    format!(
        "{},{},{}",
        fmt_f32(value[0]),
        fmt_f32(value[1]),
        fmt_f32(value[2])
    )
}

fn bits_vec3(value: [f32; 3]) -> String {
    format!(
        "{},{},{}",
        hex_bits(value[0].to_bits()),
        hex_bits(value[1].to_bits()),
        hex_bits(value[2].to_bits())
    )
}

fn fmt_f32(value: f32) -> String {
    if value.is_nan() {
        "nan".to_owned()
    } else if value.is_infinite() {
        if value.is_sign_negative() {
            "-inf".to_owned()
        } else {
            "inf".to_owned()
        }
    } else {
        format!("{value:.8}")
    }
}

fn hex_bits(bits: u32) -> String {
    format!("{bits:08x}")
}

fn hex_word(bytes: &[u8]) -> String {
    let word = u32::from_le_bytes(bytes.try_into().unwrap_or([0; 4]));
    hex_bits(word)
}

fn field_name(byte: usize) -> &'static str {
    match byte {
        0 => "pos.x",
        4 => "pos.y",
        8 => "pos.z",
        12 => "pad0",
        16 => "dir.x",
        20 => "dir.y",
        24 => "dir.z",
        28 => "pad1",
        32 => "color.x",
        36 => "color.y",
        40 => "color.z",
        44 => "intensity",
        48 => "range",
        52 => "spot_cos",
        56 => "light_type",
        _ => "other",
    }
}

fn bit(value: bool) -> u8 {
    u8::from(value)
}

impl Snapshot {
    fn clone_handles(&self) -> Self {
        Self {
            lights: self.lights.clone(),
            zero_row: self.zero_row,
            lights_buffer: self.lights_buffer.clone(),
            counts: self.counts.clone(),
            indices: self.indices.clone(),
            cluster_count_x: self.cluster_count_x,
            cluster_count_y: self.cluster_count_y,
            light_count: self.light_count,
            viewport: self.viewport,
            cleared: self.cleared,
            cluster_seen: self.cluster_seen,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_name_matches_the_four_lit_captures() {
        assert_eq!(
            case_name(Path::new("out/negative_light_physically_based/actual.png")),
            Some("negative_light_physically_based")
        );
        assert_eq!(
            case_name(Path::new("out/negative_light_parity/actual.png")),
            Some("negative_light_parity")
        );
        assert_eq!(
            case_name(Path::new(
                "out/negative_color_physically_based/positive_only.png"
            )),
            Some("negative_color_physically_based")
        );
        assert_eq!(
            case_name(Path::new("out/pbs_lit_material_matrix/actual.png")),
            Some("pbs_lit_material_matrix")
        );
        assert_eq!(case_name(Path::new("out/unlit_sphere/actual.png")), None);
    }

    #[test]
    fn center_tile_uses_the_shader_index() {
        let tile = center_tile((256, 256), 8, 8);
        assert_eq!(tile, (4, 4));
        let id = local_cluster_id(tile, 3, 8, 8).unwrap();
        assert_eq!(id, 4 + 8 * (4 + 8 * 3));
    }

    #[test]
    fn field_diff_names_a_green_bit_flip() {
        let cpu = GpuLight {
            color: [1.0, 0.5, 0.25],
            ..GpuLight::default()
        };
        let gpu = GpuLight {
            color: [1.0, 0.0, 0.25],
            ..cpu
        };
        let left = bytemuck::bytes_of(&cpu);
        let right = bytemuck::bytes_of(&gpu);
        let diff = (0..left.len() / 4).find(|word| {
            let at = word * 4;
            left[at..at + 4] != right[at..at + 4]
        });
        assert_eq!(diff.map(|word| field_name(word * 4)), Some("color.y"));
    }

    #[test]
    fn reorder_is_the_same_rows_in_a_different_order() {
        let first = GpuLight {
            intensity: 1.0,
            ..GpuLight::default()
        };
        let second = GpuLight {
            intensity: 2.0,
            ..GpuLight::default()
        };
        assert!(same_lights_reordered(&[first, second], &[second, first]));
        assert!(!lights_match(&[first, second], &[second, first], false));
    }

    #[test]
    fn float_flags_count_nan_inf_and_negative() {
        let nan_light = GpuLight {
            color: [f32::NAN, 1.0, 1.0],
            ..GpuLight::default()
        };
        let inf_light = GpuLight {
            intensity: f32::INFINITY,
            ..GpuLight::default()
        };
        let neg_light = GpuLight {
            intensity: -1.5,
            ..GpuLight::default()
        };
        let (nan, inf, neg) = float_flags(&[nan_light, inf_light, neg_light]);
        assert_eq!((nan, inf, neg), (1, 1, 1));
    }
}
