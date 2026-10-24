//! Temporary diagnostics for the WARP `pbs_lit` black readback.
//!
//! These logs answer four questions on the canary: whether wgpu reports device
//! loss or an uncaptured error around the stall, whether the capture target was
//! cleared to a known color, whether readback runs before that frame's GPU
//! submit finishes, and whether two extra WARP frames after the watchdog
//! resumes change the pixels. They do not change the expected-failure rule.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
const DISTINCTIVE_BYTES: [u8; 4] = [255, 0, 255, 255];
const BLACK_BYTES: [u8; 4] = [0, 0, 0, 255];

static DISTINCTIVE_CLEAR: AtomicBool = AtomicBool::new(false);
static CLEAR_LOGGED: AtomicBool = AtomicBool::new(false);
static RESUME_GENERATION: AtomicU64 = AtomicU64::new(0);
static WAIT: Mutex<WarpExtraWait> = Mutex::new(WarpExtraWait {
    seen_resume: 0,
    frames_after_quiet: 0,
});
static NOTES: Mutex<VecDeque<SubmitNote>> = Mutex::new(VecDeque::new());

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
    if warp {
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
    let colors = count_readback_colors(bytes);
    logger::info!(
        "diag-readback-colors: pixels={} black_0_0_0_255={} distinctive_255_0_255_255={} other={} first=({},{},{},{}) center=({},{},{},{}) distinctive_clear_armed={}",
        colors.pixels,
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
        distinctive_clear_enabled(),
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReadbackColors {
    pixels: u64,
    black: u64,
    distinctive: u64,
    other: u64,
    first: [u8; 4],
    center: [u8; 4],
}

fn count_readback_colors(bytes: &[u8]) -> ReadbackColors {
    let mut colors = ReadbackColors {
        pixels: 0,
        black: 0,
        distinctive: 0,
        other: 0,
        first: [0, 0, 0, 0],
        center: [0, 0, 0, 0],
    };
    let (pixels, _) = bytes.as_chunks::<4>();
    colors.pixels = u64::try_from(pixels.len()).unwrap_or(u64::MAX);
    if let Some(first) = pixels.first() {
        colors.first = *first;
    }
    if let Some(center) = pixels.get(pixels.len() / 2) {
        colors.center = *center;
    }
    for px in pixels {
        if *px == BLACK_BYTES {
            colors.black += 1;
        } else if *px == DISTINCTIVE_BYTES {
            colors.distinctive += 1;
        } else {
            colors.other += 1;
        }
    }
    colors
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

fn monotonic_ms() -> u64 {
    static CLOCK: OnceLock<Instant> = OnceLock::new();
    let start = CLOCK.get_or_init(Instant::now);
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{HoldDecision, WarpExtraWait, count_readback_colors, fault_flags};

    #[test]
    fn fault_flags_detect_device_removed_tdr_and_timeout() {
        let removed = fault_flags("DXGI_ERROR_DEVICE_REMOVED (0x887A0005)");
        assert!(removed.dxgi_device_removed);
        assert!(!removed.tdr);

        let hung = fault_flags("DXGI_ERROR_DEVICE_HUNG");
        assert!(!hung.dxgi_device_removed);
        assert!(hung.tdr);
        assert!(hung.timeout);

        let tdr = fault_flags("Timeout Detection and Recovery");
        assert!(tdr.tdr);
        assert!(tdr.timeout);

        let plain = fault_flags("wgpu validation error: binding mismatch");
        assert!(!plain.dxgi_device_removed);
        assert!(!plain.tdr);
        assert!(!plain.timeout);
    }

    #[test]
    fn warp_hold_waits_two_ready_frames_after_the_watchdog_resumes() {
        let mut wait = WarpExtraWait::default();
        assert_eq!(wait.withhold(false, 0, 1), HoldDecision::NotWarp);
        assert_eq!(wait.withhold(true, 4, 1), HoldDecision::PipelinesNotReady);
        assert_eq!(wait.withhold(true, 0, 0), HoldDecision::WatchdogNotQuiet);
        assert_eq!(
            wait.withhold(true, 0, 1),
            HoldDecision::Withhold { frame: 1 }
        );
        assert_eq!(
            wait.withhold(true, 0, 1),
            HoldDecision::Withhold { frame: 2 }
        );
        assert_eq!(wait.withhold(true, 0, 1), HoldDecision::PublishAfterWait);
        assert_eq!(
            wait.withhold(true, 0, 2),
            HoldDecision::Withhold { frame: 1 }
        );
    }

    #[test]
    fn readback_counts_separate_black_magenta_and_other() {
        let mut bytes = vec![0, 0, 0, 255, 255, 0, 255, 255, 1, 2, 3, 255];
        bytes.extend_from_slice(&[9, 9, 9, 255]);
        let colors = count_readback_colors(&bytes);
        assert_eq!(colors.pixels, 4);
        assert_eq!(colors.black, 1);
        assert_eq!(colors.distinctive, 1);
        assert_eq!(colors.other, 2);
        assert_eq!(colors.first, [0, 0, 0, 255]);
        assert_eq!(colors.center, [1, 2, 3, 255]);
    }
}
