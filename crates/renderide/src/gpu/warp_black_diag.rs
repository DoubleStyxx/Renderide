//! Temporary diagnostics for the WARP `pbs_lit` black readback.
//!
//! These logs answer whether wgpu reports device loss or an uncaptured error
//! around the stall, whether the capture target was cleared to a known color,
//! whether readback runs before that frame's GPU submit finishes, and whether
//! two extra WARP frames after the watchdog resumes change the pixels. On a
//! `pbs_lit` headless run they also log, once per change of state, the draws
//! encoded for the forward pass and the depth prepass, the world-view cull
//! counts, and the bound pipeline variants. Each of those draws is named
//! (mesh, index range, instance count, and the group-1 bind). The same run
//! logs the CPU camera, light, and material values that were uploaded, the
//! forward color and depth load ops, and one second readback after
//! `device.poll(Wait)` plus another copy submit. Naga info and warn records
//! already reach the renderer log; naga debug stays filtered. They do not
//! change the expected-failure rule.

use std::cell::Cell;
use std::collections::{BTreeSet, VecDeque};
use std::fmt::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use parking_lot::Mutex;

/// Microsoft WARP vendor id.
const WARP_VENDOR: u32 = 0x1414;
/// Microsoft Basic Render Driver device id. Numeric match: `0x8c` equals `0x008c`.
const WARP_DEVICE: u32 = 0x008c;
/// Ready frames withheld after the watchdog resumes, on WARP only.
const WARP_EXTRA_FRAMES: u32 = 2;
/// Linear magenta. sRGB and BGRA targets both store this as bytes `(255, 0, 255, 255)`.
const DIAG_CLEAR: wgpu::Color = wgpu::Color {
    r: 1.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

static DISTINCTIVE_CLEAR: AtomicBool = AtomicBool::new(false);
static CLEAR_LOGGED: AtomicBool = AtomicBool::new(false);
static RESUME_GENERATION: AtomicU64 = AtomicU64::new(0);
static WAIT: Mutex<WarpExtraWait> = Mutex::new(WarpExtraWait {
    seen_resume: 0,
    frames_after_quiet: 0,
});
static NOTES: Mutex<VecDeque<SubmitNote>> = Mutex::new(VecDeque::new());
static FRAME_LOG: AtomicBool = AtomicBool::new(false);
static FRAME: Mutex<FrameDiag> = Mutex::new(FrameDiag::new());

const PASS_FORWARD: u8 = 1;
const PASS_DEPTH: u8 = 2;
const MAX_LOGGED_PIPELINES: usize = 16;
const MAX_DRAW_NOTES: usize = 8;
const LOAD_UNKNOWN: u8 = 0;
const LOAD_CLEAR: u8 = 1;
const LOAD_LOAD: u8 = 2;
static LAST_PUBLISHED_FORWARD: AtomicU64 = AtomicU64::new(0);
static RETRY_BITS: AtomicU8 = AtomicU8::new(0);

thread_local! {
    static ENCODE_PASS: Cell<u8> = const { Cell::new(0) };
    static GROUP1: Cell<Group1Bind> = const { Cell::new(Group1Bind::EMPTY) };
}

#[derive(Clone, Copy)]
struct Group1Bind {
    embedded: bool,
    offset: Option<u32>,
}

impl Group1Bind {
    const EMPTY: Self = Self {
        embedded: false,
        offset: None,
    };
}

/// One indexed draw recorded while the `pbs_lit` frame log is armed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EncodedDrawNote {
    mesh_asset_id: i32,
    first_index: u32,
    index_count: u32,
    instances: u32,
    group1_embedded: bool,
    uniform_offset: Option<u32>,
}

struct SubmitNote {
    origin: &'static str,
    label: String,
    frame_seq: u64,
    command_buffers: usize,
    submitted_ms: u64,
    gpu_done_ms: Arc<AtomicU64>,
}

/// Keyword scan of a device-lost or uncaptured-error message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FaultFlags {
    /// DXGI device-removed, including HRESULT `0x887A0005`.
    pub dxgi_device_removed: bool,
    /// TDR / device-hung wording.
    pub tdr: bool,
    /// Timeout wording, including device-hung.
    pub timeout: bool,
}

/// Pure counter for the WARP-only extra wait.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct WarpExtraWait {
    seen_resume: u64,
    frames_after_quiet: u32,
}

/// What to do with a frame that is about to be published.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HoldDecision {
    /// Selected adapter is not WARP.
    NotWarp,
    /// Material pipelines are still pending, so the extra wait has not started.
    PipelinesNotReady,
    /// No hang has resumed yet.
    WatchdogNotQuiet,
    /// This ready frame is one of the two withheld frames.
    Withhold {
        /// 1-based index within the extra wait.
        frame: u32,
    },
    /// The two extra frames have already been withheld.
    PublishAfterWait,
}

impl WarpExtraWait {
    /// Advances the wait when `warp` pipelines are ready after `resume_generation`.
    pub(crate) fn withhold(
        &mut self,
        warp: bool,
        pending: u64,
        resume_generation: u64,
    ) -> HoldDecision {
        if !warp {
            return HoldDecision::NotWarp;
        }
        if pending != 0 {
            return HoldDecision::PipelinesNotReady;
        }
        if resume_generation == 0 {
            return HoldDecision::WatchdogNotQuiet;
        }
        if self.seen_resume != resume_generation {
            self.seen_resume = resume_generation;
            self.frames_after_quiet = 0;
        }
        if self.frames_after_quiet < WARP_EXTRA_FRAMES {
            self.frames_after_quiet += 1;
            HoldDecision::Withhold {
                frame: self.frames_after_quiet,
            }
        } else {
            HoldDecision::PublishAfterWait
        }
    }
}

/// Scans `text` for DXGI device-removed, TDR, and timeout wording.
pub(crate) fn fault_flags(text: &str) -> FaultFlags {
    let lower = text.to_ascii_lowercase();
    let removed = lower.contains("device_removed")
        || lower.contains("device removed")
        || lower.contains("0x887a0005");
    let hung = lower.contains("device_hung") || lower.contains("device hung");
    let tdr = lower.contains("tdr") || lower.contains("timeout detection") || hung;
    let timeout = lower.contains("timeout") || lower.contains("timed out") || hung;
    FaultFlags {
        dxgi_device_removed: removed,
        tdr,
        timeout,
    }
}

/// Formats [`fault_flags`] for a single log line.
pub(crate) fn format_fault_flags(text: &str) -> String {
    let flags = fault_flags(text);
    format!(
        "dxgi_device_removed={} tdr={} timeout={}",
        flags.dxgi_device_removed, flags.tdr, flags.timeout
    )
}

/// `true` when the selected adapter is Microsoft Basic Render Driver (WARP).
pub(crate) fn adapter_is_warp(info: &wgpu::AdapterInfo) -> bool {
    info.vendor == WARP_VENDOR && info.device == WARP_DEVICE
}

/// Records the selected adapter and, on WARP, arms the distinctive capture clear.
pub(crate) fn configure_for_adapter(info: &wgpu::AdapterInfo) {
    let warp = adapter_is_warp(info);
    // The scene-color readback compares WARP to a black-clear golden. The magenta
    // probe would dominate those quadrant means, so that capture keeps production black.
    let debug_readback = crate::debug_scene_color::readback_enabled();
    if warp && !debug_readback {
        DISTINCTIVE_CLEAR.store(true, Ordering::Release);
    }
    logger::info!(
        "diag-adapter: name={} vendor=0x{:04x} device=0x{:04x} backend={:?} warp={} distinctive_clear={}",
        info.name,
        info.vendor,
        info.device,
        info.backend,
        warp,
        DISTINCTIVE_CLEAR.load(Ordering::Acquire),
    );
}

/// `true` after [`configure_for_adapter`] saw a WARP adapter.
pub(crate) fn distinctive_clear_enabled() -> bool {
    DISTINCTIVE_CLEAR.load(Ordering::Acquire)
}

/// Forward-pass clear. WARP uses magenta so an untouched target is not black.
pub(crate) fn forward_clear_color() -> wgpu::Color {
    if distinctive_clear_enabled() {
        log_clear_once();
        DIAG_CLEAR
    } else {
        crate::present::SWAPCHAIN_CLEAR_COLOR
    }
}

/// Skybox fallback color. WARP replaces the authored black clear.
pub(crate) fn skybox_clear_color(authored: [f32; 4]) -> [f32; 4] {
    if distinctive_clear_enabled() {
        log_clear_once();
        [1.0, 0.0, 1.0, 1.0]
    } else {
        authored
    }
}

fn log_clear_once() {
    if CLEAR_LOGGED.swap(true, Ordering::AcqRel) {
        return;
    }
    logger::info!(
        "diag-capture-clear: WARP clear is linear magenta (255,0,255,255) on the forward load op, skybox fallback, and scene-color compose; copy source is renderide-primary-offscreen-color sample_count=1, not an MSAA resolve"
    );
}

/// Logs one primary offscreen allocation. `sample_count` is always 1 for this target.
pub(crate) fn log_capture_target(format: wgpu::TextureFormat, width: u32, height: u32) {
    logger::info!(
        "diag-capture-target: label=renderide-primary-offscreen-color sample_count=1 format={format:?} extent={width}x{height}"
    );
}

/// First pet after a hang report. That is the watchdog going quiet because the thread moved.
pub(crate) fn note_watchdog_resumed(thread: &str) {
    let generation = RESUME_GENERATION.fetch_add(1, Ordering::AcqRel) + 1;
    logger::info!(
        "diag-watchdog-quiet: thread '{thread}' petted after a hang report; quiet_generation={generation}"
    );
}

/// Stamps `pending` into the PNG, or `1` while WARP is withholding two post-quiet frames.
pub(crate) fn pending_for_publish(info: &wgpu::AdapterInfo, pending: u64) -> u64 {
    let warp = adapter_is_warp(info);
    let resume = RESUME_GENERATION.load(Ordering::Acquire);
    let decision = WAIT.lock().withhold(warp, pending, resume);
    match decision {
        HoldDecision::Withhold { frame } => {
            logger::info!(
                "diag-warp-hold: WARP pipelines ready (pending={pending}); watchdog quiet generation={resume}; withholding frame {frame} of {WARP_EXTRA_FRAMES}"
            );
            1
        }
        HoldDecision::PublishAfterWait => {
            logger::info!(
                "diag-warp-hold: WARP pipelines ready (pending=0); watchdog quiet generation={resume}; extra {WARP_EXTRA_FRAMES} frames elapsed, publishing this frame"
            );
            pending
        }
        HoldDecision::NotWarp
        | HoldDecision::PipelinesNotReady
        | HoldDecision::WatchdogNotQuiet => pending,
    }
}

/// Records `submission` and logs when `on_submitted_work_done` fires for it.
pub(crate) fn watch_submission(
    queue: &wgpu::Queue,
    submission: &wgpu::SubmissionIndex,
    origin: &'static str,
    frame_seq: u64,
    command_buffers: usize,
) {
    let label = format!("{submission:?}");
    let submitted_ms = monotonic_ms();
    let done = Arc::new(AtomicU64::new(0));
    {
        let mut notes = NOTES.lock();
        if notes.len() == 16 {
            notes.pop_front();
        }
        notes.push_back(SubmitNote {
            origin,
            label: label.clone(),
            frame_seq,
            command_buffers,
            submitted_ms,
            gpu_done_ms: Arc::clone(&done),
        });
    }
    logger::info!(
        "diag-queue-submit: origin={origin} submission={label} frame_seq={frame_seq} command_buffers={command_buffers} submitted_ms={submitted_ms}"
    );
    queue.on_submitted_work_done(move || {
        let done_ms = monotonic_ms().max(1);
        done.store(done_ms, Ordering::Release);
        logger::info!(
            "diag-queue-gpu-done: origin={origin} submission={label} frame_seq={frame_seq} submitted_ms={submitted_ms} done_ms={done_ms}"
        );
    });
}

/// Logs driver-submit completion after `flush_driver`, then polls without blocking.
///
/// `flush_driver` returns when the driver thread has returned from `Queue::submit`.
/// It does not wait for GPU completion. The non-blocking poll lets an already
/// finished `on_submitted_work_done` callback run before the readback copy.
pub(crate) fn log_after_flush(device: &wgpu::Device) {
    let poll = match device.poll(wgpu::PollType::Poll) {
        Ok(status) => format!("{status:?}"),
        Err(err) => format!("error:{err}"),
    };
    logger::info!(
        "diag-readback-timing: flush_driver returned (CPU Queue::submit done, not GPU idle) nonblocking_poll={poll} {}",
        driver_status()
    );
}

/// Logs the readback copy's submission index against the latest driver submit.
pub(crate) fn log_copy_submitted(submission: &wgpu::SubmissionIndex) {
    logger::info!(
        "diag-readback-timing: copy_texture_to_buffer submitted submission={submission:?} {}",
        driver_status()
    );
}

/// Logs `map_async` entry and, on drop, whether the driver submit's GPU callback has fired.
#[must_use = "the span logs map_async completion when dropped"]
pub(crate) struct MapAsyncSpan;

impl MapAsyncSpan {
    /// Starts the map span.
    pub(crate) fn start() -> Self {
        logger::info!("diag-readback-timing: map_async start {}", driver_status());
        Self
    }
}

impl Drop for MapAsyncSpan {
    fn drop(&mut self) {
        logger::info!(
            "diag-readback-timing: map_async finished {}",
            driver_status()
        );
    }
}

/// Counts black, magenta, and other pixels in a tight RGBA readback.
pub(crate) fn log_readback_colors(bytes: &[u8]) {
    let colors = super::warp_black_diag_probe::count_readback_colors(bytes);
    logger::info!(
        "diag-readback-colors: pixels={} class={} black_0_0_0_255={} distinctive_255_0_255_255={} other={} first=({},{},{},{}) center=({},{},{},{}) min=({},{},{},{}) max=({},{},{},{}) distinctive_clear_armed={}",
        colors.pixels,
        super::warp_black_diag_probe::classify_readback(&colors),
        colors.black,
        colors.distinctive,
        colors.other,
        colors.first[0],
        colors.first[1],
        colors.first[2],
        colors.first[3],
        colors.center[0],
        colors.center[1],
        colors.center[2],
        colors.center[3],
        colors.min[0],
        colors.min[1],
        colors.min[2],
        colors.min[3],
        colors.max[0],
        colors.max[1],
        colors.max[2],
        colors.max[3],
        distinctive_clear_enabled(),
    );
}

/// `true` once for the first drawn frame, and once for the first black frame.
///
/// A second copy is expensive on WARP, so a run probes at most those two
/// readbacks. Magenta frames that encoded no draws are not probed.
pub(crate) fn claim_second_readback(bytes: &[u8]) -> bool {
    if !frame_log_armed() {
        return false;
    }
    let bits = super::warp_black_diag_probe::retry_bits(
        super::warp_black_diag_probe::classify_readback(
            &super::warp_black_diag_probe::count_readback_colors(bytes),
        ),
        LAST_PUBLISHED_FORWARD.load(Ordering::Acquire),
    );
    if bits == 0 {
        return false;
    }
    let previous = RETRY_BITS.fetch_or(bits, Ordering::AcqRel);
    super::warp_black_diag_probe::claim_retry_bits(previous, bits)
}

/// Logs the explicit `PollType::Wait` and the second copy against `first`.
pub(crate) fn log_readback_retry(poll: &str, poll_ms: u128, first: &[u8], second: &[u8]) {
    let before = super::warp_black_diag_probe::count_readback_colors(first);
    let after = super::warp_black_diag_probe::count_readback_colors(second);
    logger::info!(
        "diag-readback-retry: poll={poll} poll_ms={poll_ms} same_bytes={} first_class={} first_black={} first_magenta={} first_other={} second_class={} second_black={} second_magenta={} second_other={}",
        first == second,
        super::warp_black_diag_probe::classify_readback(&before),
        before.black,
        before.distinctive,
        before.other,
        super::warp_black_diag_probe::classify_readback(&after),
        after.black,
        after.distinctive,
        after.other,
    );
}

fn driver_status() -> String {
    let snapshot = {
        let notes = NOTES.lock();
        notes.iter().rev().find_map(|note| {
            (note.origin == "driver").then(|| DriverSnapshot {
                label: note.label.clone(),
                frame_seq: note.frame_seq,
                command_buffers: note.command_buffers,
                submitted_ms: note.submitted_ms,
                done_ms: note.gpu_done_ms.load(Ordering::Acquire),
            })
        })
    };
    let Some(note) = snapshot else {
        return "driver_submission=none gpu_done=false".to_owned();
    };
    if note.done_ms == 0 {
        format!(
            "driver_submission={} frame_seq={} command_buffers={} submitted_ms={} gpu_done=false",
            note.label, note.frame_seq, note.command_buffers, note.submitted_ms
        )
    } else {
        format!(
            "driver_submission={} frame_seq={} command_buffers={} submitted_ms={} gpu_done=true done_ms={}",
            note.label, note.frame_seq, note.command_buffers, note.submitted_ms, note.done_ms
        )
    }
}

struct DriverSnapshot {
    label: String,
    frame_seq: u64,
    command_buffers: usize,
    submitted_ms: u64,
    done_ms: u64,
}

/// `true` when this process's headless output path is a `pbs_lit` case.
pub(crate) fn frame_log_armed() -> bool {
    FRAME_LOG.load(Ordering::Acquire)
}

/// Arms the per-frame draw log when `path` names a `pbs_lit` capture.
pub(crate) fn arm_frame_log(path: &Path) {
    let armed = path_requests_frame_log(path);
    FRAME_LOG.store(armed, Ordering::Release);
    if armed {
        logger::info!(
            "diag-pbs-frame: armed path={} naga_info_and_warn_visible=true depth_clear={} depth_compare={:?} skybox_zwrite=off skybox_after_opaque=true",
            path.display(),
            super::MAIN_FORWARD_DEPTH_CLEAR,
            super::MAIN_FORWARD_DEPTH_COMPARE,
        );
    }
}

/// Restores the previous encode pass when dropped.
#[must_use = "the guard restores the previous encode pass when dropped"]
pub(crate) struct EncodePassGuard {
    previous: u8,
}

impl EncodePassGuard {
    /// Counts `draw_indexed` calls until dropped as forward-pass draws.
    pub(crate) fn forward() -> Self {
        Self::enter(PASS_FORWARD)
    }

    /// Counts `draw_indexed` calls until dropped as depth-prepass draws.
    pub(crate) fn depth_prepass() -> Self {
        Self::enter(PASS_DEPTH)
    }

    fn enter(pass: u8) -> Self {
        let previous = ENCODE_PASS.with(|cell| cell.replace(pass));
        Self { previous }
    }
}

impl Drop for EncodePassGuard {
    fn drop(&mut self) {
        ENCODE_PASS.with(|cell| cell.set(self.previous));
    }
}

/// Records the group-1 bind used by the next forward draws.
pub(crate) fn note_group1_binding(embedded: bool, uniform_offset: Option<u32>) {
    if !frame_log_armed() {
        return;
    }
    GROUP1.with(|cell| {
        cell.set(Group1Bind {
            embedded,
            offset: uniform_offset,
        });
    });
}

/// Records one encoded draw when a pass guard is active.
pub(crate) fn note_encoded_draw(
    mesh_asset_id: i32,
    first_index: u32,
    index_count: u32,
    instances: u32,
) {
    if !frame_log_armed() {
        return;
    }
    let pass = ENCODE_PASS.with(|cell| cell.get());
    let group1 = GROUP1.with(|cell| cell.get());
    let forward = pass == PASS_FORWARD;
    let note = EncodedDrawNote {
        mesh_asset_id,
        first_index,
        index_count,
        instances,
        group1_embedded: forward && group1.embedded,
        uniform_offset: if forward { group1.offset } else { None },
    };
    let mut diag = FRAME.lock();
    let count = u64::from(instances);
    if forward {
        diag.accum.forward_draws = diag.accum.forward_draws.saturating_add(1);
        diag.accum.forward_instances = diag.accum.forward_instances.saturating_add(count);
        diag.accum.push_forward_draw(note);
    } else if pass == PASS_DEPTH {
        diag.accum.depth_draws = diag.accum.depth_draws.saturating_add(1);
        diag.accum.depth_instances = diag.accum.depth_instances.saturating_add(count);
        diag.accum.push_depth_draw(note);
    }
}

/// Records the forward pass load ops. `true` means `LoadOp::Clear`.
pub(crate) fn note_forward_load_ops(color_clear: bool, depth_clear: bool) {
    if !frame_log_armed() {
        return;
    }
    let mut diag = FRAME.lock();
    diag.accum.color_load = if color_clear { LOAD_CLEAR } else { LOAD_LOAD };
    diag.accum.depth_load = if depth_clear { LOAD_CLEAR } else { LOAD_LOAD };
}

/// Records one skybox or solid-background draw. `kind` is `clear_color` or `material`.
pub(crate) fn note_skybox_draw(kind: &'static str) {
    if !frame_log_armed() {
        return;
    }
    let mut diag = FRAME.lock();
    diag.accum.skybox_draws = diag.accum.skybox_draws.saturating_add(1);
    if !diag.accum.skybox_kinds.is_empty() {
        diag.accum.skybox_kinds.push('+');
    }
    if diag.accum.skybox_kinds.len() < 48 {
        diag.accum.skybox_kinds.push_str(kind);
    }
}

/// Records one bound forward pipeline. Repeated binds of the same variant collapse.
pub(crate) fn note_bound_pipeline(kind: &str, shader_asset_id: i32, variant_bits: u32) {
    if !frame_log_armed() {
        return;
    }
    let mut diag = FRAME.lock();
    diag.accum.pipelines.insert(PipelineNote {
        kind: kind.to_owned(),
        shader_asset_id,
        variant_bits,
    });
}

/// Adds world-view cull totals for this frame. `counts` is pre-cull, frustum, Hi-Z, visible.
pub(crate) fn note_world_cull(counts: [u64; 4], views: usize) {
    if !frame_log_armed() {
        return;
    }
    let mut diag = FRAME.lock();
    diag.accum.pre_cull = diag.accum.pre_cull.saturating_add(counts[0]);
    diag.accum.frustum = diag.accum.frustum.saturating_add(counts[1]);
    diag.accum.hiz = diag.accum.hiz.saturating_add(counts[2]);
    diag.accum.visible = diag.accum.visible.saturating_add(counts[3]);
    diag.accum.views = diag
        .accum
        .views
        .saturating_add(u64::try_from(views).unwrap_or(u64::MAX));
}

/// Logs one line the first time `kind` is compiled at `variant_bits`.
pub(crate) fn note_shader_compile(kind: &str, variant_bits: u32, wgsl_len: usize) {
    if !frame_log_armed() {
        return;
    }
    let key = (kind.to_owned(), variant_bits);
    let fresh = FRAME.lock().seen_compiles.insert(key);
    if fresh {
        logger::info!(
            "diag-shader-compile: kind={kind} variant_bits={variant_bits:#x} wgsl_len={wgsl_len}"
        );
    }
}

/// Logs a material pipeline build that failed while the frame log is armed.
pub(crate) fn note_pipeline_build_failed(kind: &str) {
    if !frame_log_armed() {
        return;
    }
    logger::info!("diag-shader-compile: failed kind={kind}");
}

/// Logs this frame when its draw, cull, or pipeline tuple differs from the last one.
pub(crate) fn flush_frame_log() {
    publish_frame_log(true);
}

/// Drops this frame's counters without logging. Used when the frame was not published.
pub(crate) fn discard_frame_log() {
    publish_frame_log(false);
}

fn publish_frame_log(record: bool) {
    if !frame_log_armed() {
        return;
    }
    let event = {
        let mut diag = FRAME.lock();
        let signature = diag.accum.signature();
        diag.accum = FrameAccum::new();
        if record {
            LAST_PUBLISHED_FORWARD.store(signature.forward_draws, Ordering::Release);
            diag.log.observe(signature)
        } else {
            None
        }
    };
    if let Some(event) = event {
        logger::info!("{}", format_frame_event(&event));
    }
}

fn path_requests_frame_log(path: &Path) -> bool {
    path.to_string_lossy().contains("pbs_lit")
}

struct FrameDiag {
    accum: FrameAccum,
    log: FrameLogState,
    seen_compiles: BTreeSet<(String, u32)>,
}

impl FrameDiag {
    const fn new() -> Self {
        Self {
            accum: FrameAccum::new(),
            log: FrameLogState::new(),
            seen_compiles: BTreeSet::new(),
        }
    }
}

struct FrameAccum {
    forward_draws: u64,
    forward_instances: u64,
    depth_draws: u64,
    depth_instances: u64,
    pre_cull: u64,
    frustum: u64,
    hiz: u64,
    visible: u64,
    views: u64,
    pipelines: BTreeSet<PipelineNote>,
    forward_draw_notes: Vec<EncodedDrawNote>,
    depth_draw_notes: Vec<EncodedDrawNote>,
    forward_draw_overflow: u64,
    depth_draw_overflow: u64,
    skybox_draws: u64,
    skybox_kinds: String,
    color_load: u8,
    depth_load: u8,
}

impl FrameAccum {
    const fn new() -> Self {
        Self {
            forward_draws: 0,
            forward_instances: 0,
            depth_draws: 0,
            depth_instances: 0,
            pre_cull: 0,
            frustum: 0,
            hiz: 0,
            visible: 0,
            views: 0,
            pipelines: BTreeSet::new(),
            forward_draw_notes: Vec::new(),
            depth_draw_notes: Vec::new(),
            forward_draw_overflow: 0,
            depth_draw_overflow: 0,
            skybox_draws: 0,
            skybox_kinds: String::new(),
            color_load: LOAD_UNKNOWN,
            depth_load: LOAD_UNKNOWN,
        }
    }

    fn push_forward_draw(&mut self, note: EncodedDrawNote) {
        if self.forward_draw_notes.len() < MAX_DRAW_NOTES {
            self.forward_draw_notes.push(note);
        } else {
            self.forward_draw_overflow = self.forward_draw_overflow.saturating_add(1);
        }
    }

    fn push_depth_draw(&mut self, note: EncodedDrawNote) {
        if self.depth_draw_notes.len() < MAX_DRAW_NOTES {
            self.depth_draw_notes.push(note);
        } else {
            self.depth_draw_overflow = self.depth_draw_overflow.saturating_add(1);
        }
    }

    fn signature(&self) -> FrameSignature {
        let overflow = u64::try_from(self.pipelines.len())
            .unwrap_or(u64::MAX)
            .saturating_sub(u64::try_from(MAX_LOGGED_PIPELINES).unwrap_or(u64::MAX));
        let mut pipelines = String::new();
        for (index, note) in self.pipelines.iter().take(MAX_LOGGED_PIPELINES).enumerate() {
            if index > 0 {
                pipelines.push(';');
            }
            pipelines.push_str(&log_pipeline_kind(&note.kind));
            let _ = write!(
                pipelines,
                "@{}:{:#x}",
                note.shader_asset_id, note.variant_bits
            );
        }
        FrameSignature {
            forward_draws: self.forward_draws,
            forward_instances: self.forward_instances,
            depth_draws: self.depth_draws,
            depth_instances: self.depth_instances,
            pre_cull: self.pre_cull,
            frustum: self.frustum,
            hiz: self.hiz,
            visible: self.visible,
            views: self.views,
            pipelines,
            pipeline_overflow: overflow,
            color_load: self.color_load,
            depth_load: self.depth_load,
            skybox_draws: self.skybox_draws,
            skybox_kinds: self.skybox_kinds.clone(),
            forward_draws_detail: format_draw_notes(&self.forward_draw_notes, false),
            depth_draws_detail: format_draw_notes(&self.depth_draw_notes, true),
            forward_draw_overflow: self.forward_draw_overflow,
            depth_draw_overflow: self.depth_draw_overflow,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct PipelineNote {
    kind: String,
    shader_asset_id: i32,
    variant_bits: u32,
}

fn log_pipeline_kind(kind: &str) -> String {
    kind.chars()
        .map(|ch| if ch == ';' || ch == '@' { '_' } else { ch })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FrameSignature {
    forward_draws: u64,
    forward_instances: u64,
    depth_draws: u64,
    depth_instances: u64,
    pre_cull: u64,
    frustum: u64,
    hiz: u64,
    visible: u64,
    views: u64,
    pipelines: String,
    pipeline_overflow: u64,
    color_load: u8,
    depth_load: u8,
    skybox_draws: u64,
    skybox_kinds: String,
    forward_draws_detail: String,
    depth_draws_detail: String,
    forward_draw_overflow: u64,
    depth_draw_overflow: u64,
}

struct FrameLogState {
    last: Option<FrameSignature>,
    streak: u64,
    frame: u64,
}

struct FrameLogEvent {
    frame: u64,
    prev_streak: u64,
    signature: FrameSignature,
}

impl FrameLogState {
    const fn new() -> Self {
        Self {
            last: None,
            streak: 0,
            frame: 0,
        }
    }

    fn observe(&mut self, signature: FrameSignature) -> Option<FrameLogEvent> {
        self.frame = self.frame.saturating_add(1);
        if self.last.as_ref() == Some(&signature) {
            self.streak = self.streak.saturating_add(1);
            return None;
        }
        let prev_streak = self.streak;
        self.streak = 1;
        self.last = Some(signature.clone());
        Some(FrameLogEvent {
            frame: self.frame,
            prev_streak,
            signature,
        })
    }
}

fn format_frame_event(event: &FrameLogEvent) -> String {
    let signature = &event.signature;
    let pipelines = if signature.pipelines.is_empty() {
        "-"
    } else {
        signature.pipelines.as_str()
    };
    let skybox = if signature.skybox_kinds.is_empty() {
        "-"
    } else {
        signature.skybox_kinds.as_str()
    };
    format!(
        "diag-pbs-frame: frame={} prev_streak={} forward_draws={} forward_instances={} depth_draws={} depth_instances={} pre_cull={} frustum={} hiz={} visible={} views={} pipelines={} pipeline_overflow={} color_load={} depth_load={} skybox={} skybox_kind={} draws={} depth_draws_detail={} draw_overflow={} depth_draw_overflow={}",
        event.frame,
        event.prev_streak,
        signature.forward_draws,
        signature.forward_instances,
        signature.depth_draws,
        signature.depth_instances,
        signature.pre_cull,
        signature.frustum,
        signature.hiz,
        signature.visible,
        signature.views,
        pipelines,
        signature.pipeline_overflow,
        load_label(signature.color_load),
        load_label(signature.depth_load),
        signature.skybox_draws,
        skybox,
        signature.forward_draws_detail,
        signature.depth_draws_detail,
        signature.forward_draw_overflow,
        signature.depth_draw_overflow,
    )
}

fn load_label(kind: u8) -> &'static str {
    match kind {
        LOAD_CLEAR => "clear",
        LOAD_LOAD => "load",
        _ => "unknown",
    }
}

fn format_draw_notes(notes: &[EncodedDrawNote], depth: bool) -> String {
    if notes.is_empty() {
        return "-".to_owned();
    }
    let mut out = String::new();
    for (index, note) in notes.iter().enumerate() {
        if index > 0 {
            out.push(';');
        }
        out.push_str(&format_draw_note(note, depth));
    }
    out
}

fn format_draw_note(note: &EncodedDrawNote, depth: bool) -> String {
    let pass = if depth { "d" } else { "f" };
    let bind = if depth {
        String::new()
    } else if note.group1_embedded {
        match note.uniform_offset {
            Some(offset) => format!("/e{offset}"),
            None => "/e".to_owned(),
        }
    } else {
        "/empty".to_owned()
    };
    format!(
        "{pass}:m{}:{}+{}x{}{bind}",
        note.mesh_asset_id, note.first_index, note.index_count, note.instances
    )
}

fn monotonic_ms() -> u64 {
    static CLOCK: OnceLock<Instant> = OnceLock::new();
    let start = CLOCK.get_or_init(Instant::now);
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "warp_black_diag_tests.rs"]
mod tests;
