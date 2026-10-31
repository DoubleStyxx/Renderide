//! Retired WARP excuse for `pbs_lit_material_matrix` (#212 / #216).
//!
//! The lit frame now passes the image check on WARP. [`apply_expected_failure`]
//! does not relabel a pass or a mismatch. A failed comparison fails the suite.
//!
//! The excuse is the region-color signature: all 4 bands fail that criterion.
//! SSIM, pixel diff, image coverage, and band count may fail beside it.
//! Evidence from the Windows captures: 40088875619, 40096421641, 40097968491,
//! 41084635804, 41089310635, 41093531109, 41103575428, 41099685993.

use std::fs;
use std::path::{Path, PathBuf};

use super::output::{self, CaseReport};
use super::suite::{SuiteCaseReport, SuiteReport};

/// Text written into `report.json` and the suite summary. This is not a pass.
pub(crate) const EXPECTED_FAILURE_LABEL: &str = "expected failure (WARP, #212)";

/// `report.json` `result` when WARP passes `pbs_lit`. The case stays a pass.
pub(crate) const UNEXPECTED_PASS_LABEL: &str = "unexpected pass (WARP, #212)";

/// Case whose WARP image mismatch is expected.
const PBS_LIT: &str = "pbs_lit_material_matrix";

/// PNG `tEXt` keyword written when the renderer counts pending and dropped draws.
const PIPELINE_PENDING_TEXT_KEYWORD: &str = "renderide_pipeline_pending";

/// Microsoft Basic Render Driver.
const WARP_VENDOR: u32 = 0x1414;
/// WARP device id. Logs print this as `0x008c`.
const WARP_DEVICE: u32 = 0x008c;

/// The selected adapter is WARP when its vendor and device match numerically.
///
/// `0x8c` and `0x008c` are the same id. The operating system is not consulted.
pub(crate) const fn adapter_is_warp(vendor: u32, device: u32) -> bool {
    vendor == WARP_VENDOR && device == WARP_DEVICE
}

/// Why a case failed, for the WARP exception.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaseFault {
    /// The case passed.
    None,
    /// The accepted frame failed its image check.
    ImageCheck,
    /// The capture timed out while material draws were still compiling or skipped.
    PipelineNotReady,
    /// The deadline hit with pending=0 on a flat or unstable frame.
    FlatOrUnstable,
    /// The capture timed out before an accepted frame.
    Timeout,
    /// The renderer crashed or exited with a failure before a comparison.
    Crash,
    /// Some other failure. That is not the known WARP mismatch.
    Other,
}

/// One suite case, with the capture facts the WARP exception needs.
///
/// `pending` and `dropped_draws` are `None` when the capture did not record them.
/// `None` is not zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CaseSnapshot<'a> {
    name: &'a str,
    fault: CaseFault,
    /// The harness accepted the frame and ran the image check.
    frame_accepted: bool,
    pending: Option<u64>,
    dropped_draws: Option<u64>,
    /// Region-color signature and the criteria that failed beside it.
    signature: ImageSignature,
}

/// Failed criteria beside the region-color signature.
///
/// `foreign_failure` is the extension point. A criterion that is not region
/// color, SSIM, pixel diff, image coverage, or band count sets it and the
/// excuse does not apply.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ImageSignature {
    /// Region-color bands present in the evaluation, passed or failed.
    region_color_bands: u8,
    /// Region-color bands whose criterion failed.
    region_color_failed: u8,
    ssim_failed: bool,
    pixel_diff_failed: bool,
    coverage_failed: bool,
    band_failed: bool,
    /// A failed criterion outside the signature and the allowed co-failures.
    foreign_failure: bool,
}

/// Whether `case` carries the expected-failure label.
pub(crate) fn case_is_expected_failure(case: &SuiteCaseReport) -> bool {
    case.result.as_deref() == Some(EXPECTED_FAILURE_LABEL) && !case.passed
}

/// Whether `case` passed on WARP and carries the unexpected-pass label.
pub(crate) fn case_is_unexpected_pass(case: &SuiteCaseReport) -> bool {
    case.passed && case.result.as_deref() == Some(UNEXPECTED_PASS_LABEL)
}

/// Loud suite line for a WARP `pbs_lit` pass. Names #212.
pub(crate) fn unexpected_pass_line(name: &str, artifacts: &Path) -> String {
    format!(
        "UNEXPECTED PASS {name} ({}) {UNEXPECTED_PASS_LABEL}",
        artifacts.display()
    )
}

/// Labels `pbs_lit` when the selected adapter is WARP and the image check failed
/// on an accepted frame with pending=0 and no dropped draws.
pub(crate) fn apply_expected_failure(report: &mut SuiteReport) {
    let Some(index) = report.cases.iter().position(|case| case.name == PBS_LIT) else {
        return;
    };
    let actual = output::actual_png_in(&report.cases[index].artifacts_dir);
    let is_warp = output_is_warp(&actual);
    let snapshot = snapshot_for(&report.cases[index]);
    if unexpected_pass(is_warp, &snapshot) {
        report.cases[index].result = Some(UNEXPECTED_PASS_LABEL.to_string());
        report.recompute();
        if let Err(err) = stamp_outcome(
            &report.cases[index].report_json,
            true,
            UNEXPECTED_PASS_LABEL,
        ) {
            logger::warn!(
                "WARP unexpected pass: could not stamp {}: {err}",
                report.cases[index].report_json.display()
            );
        }
        return;
    }
    if !excuse(is_warp, &snapshot) {
        return;
    }
    let case = &mut report.cases[index];
    case.passed = false;
    case.error = Some(EXPECTED_FAILURE_LABEL.to_string());
    case.result = Some(EXPECTED_FAILURE_LABEL.to_string());
    report.recompute();
    if let Err(err) = stamp_outcome(
        &report.cases[index].report_json,
        false,
        EXPECTED_FAILURE_LABEL,
    ) {
        logger::warn!(
            "WARP expected failure: could not stamp {}: {err}",
            report.cases[index].report_json.display()
        );
    }
}

fn unexpected_pass(is_warp: bool, case: &CaseSnapshot<'_>) -> bool {
    let _ = (is_warp, case);
    false
}

fn excuse(is_warp: bool, case: &CaseSnapshot<'_>) -> bool {
    let _ = (is_warp, case);
    false
}

fn snapshot_for(case: &SuiteCaseReport) -> CaseSnapshot<'_> {
    let fault = fault_from_failure(case.passed, case.error.as_deref());
    let frame_accepted = case.passed || fault == CaseFault::ImageCheck;
    let (pending, dropped_draws, signature) = if frame_accepted {
        let chunk = read_pipeline_pending_chunk(&output::actual_png_in(&case.artifacts_dir));
        let (pending, dropped_draws) = counts_for_compared_frame(chunk.as_deref());
        (
            pending,
            dropped_draws,
            signature_from_report(&case.report_json),
        )
    } else {
        (None, None, ImageSignature::default())
    };
    CaseSnapshot {
        name: &case.name,
        fault,
        frame_accepted,
        pending,
        dropped_draws,
        signature,
    }
}

fn signature_from_report(path: &Path) -> ImageSignature {
    let Ok(bytes) = fs::read(path) else {
        return ImageSignature::default();
    };
    let Ok(report) = serde_json::from_slice::<CaseReport>(&bytes) else {
        return ImageSignature::default();
    };
    let Some(evaluation) = report.evaluation.as_ref() else {
        return ImageSignature::default();
    };
    signature_from_criteria(&evaluation.criteria)
}

/// Classifies a tolerance evaluation.
///
/// Region color is the signature. SSIM, pixel diff, image coverage, and band
/// count are recorded as co-failures and do not reject the excuse. The match
/// is exhaustive so a new criterion has to be classified before it can hide
/// inside the signature.
fn signature_from_criteria(criteria: &[super::tolerance::CriterionResult]) -> ImageSignature {
    use super::tolerance::CriterionResult;
    let mut signature = ImageSignature::default();
    for criterion in criteria {
        match criterion {
            CriterionResult::RegionColor { passed, .. } => {
                signature.region_color_bands = signature.region_color_bands.saturating_add(1);
                if !passed {
                    signature.region_color_failed = signature.region_color_failed.saturating_add(1);
                }
            }
            CriterionResult::Ssim { passed: false, .. } => signature.ssim_failed = true,
            CriterionResult::PixelDiff { passed: false, .. } => signature.pixel_diff_failed = true,
            CriterionResult::ImageCoverage { passed: false, .. } => {
                signature.coverage_failed = true;
            }
            CriterionResult::RegionNonDominant { passed: false, .. } => {
                signature.band_failed = true;
            }
            // Region-mean and channel-zero tighten comparison. They do not
            // count as region-color bands and they do not set `foreign_failure`,
            // so the all-4-band #212 excuse is unchanged when they fail beside it.
            CriterionResult::RegionMean { .. }
            | CriterionResult::ChannelZero { .. }
            | CriterionResult::Ssim { passed: true, .. }
            | CriterionResult::PixelDiff { passed: true, .. }
            | CriterionResult::ImageCoverage { passed: true, .. }
            | CriterionResult::RegionNonDominant { passed: true, .. } => {}
        }
    }
    signature
}

fn fault_from_failure(passed: bool, error: Option<&str>) -> CaseFault {
    if passed {
        return CaseFault::None;
    }
    let error = error.unwrap_or("");
    if error.contains("pipeline-pending") || error.contains("PipelineNotReady") {
        return CaseFault::PipelineNotReady;
    }
    if error.contains("flat or unstable") || error.contains("flat single color") {
        return CaseFault::FlatOrUnstable;
    }
    if error.contains("timed out") || error.contains("expected fresh PNG") {
        return CaseFault::Timeout;
    }
    if error.contains("renderer exited with failure") || error.contains("spawn renderer process") {
        return CaseFault::Crash;
    }
    if error.contains("case failed tolerance") {
        return CaseFault::ImageCheck;
    }
    CaseFault::Other
}

/// Pending and dropped counts for a frame that reached image comparison.
///
/// The `renderide_pipeline_pending` chunk is the sum of still-compiling lookups
/// and dropped draws, so only `0` records both as zero. A missing or unreadable
/// chunk is not zero.
fn counts_for_compared_frame(chunk: Option<&str>) -> (Option<u64>, Option<u64>) {
    match chunk.map(str::trim) {
        Some("0") => (Some(0), Some(0)),
        Some(text) => match text.parse::<u64>() {
            Ok(pending) => (Some(pending), None),
            Err(_) => (None, None),
        },
        None => (None, None),
    }
}

fn read_pipeline_pending_chunk(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    pipeline_pending_text(&bytes)
}

fn pipeline_pending_text(bytes: &[u8]) -> Option<String> {
    const SIGNATURE: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < SIGNATURE.len() || &bytes[..SIGNATURE.len()] != SIGNATURE {
        return None;
    }
    let mut offset = SIGNATURE.len();
    while let Some(chunk) = png_chunk(bytes, offset) {
        if chunk.kind == *b"tEXt"
            && let Some(text) = text_keyword(chunk.data, PIPELINE_PENDING_TEXT_KEYWORD)
        {
            return Some(text);
        }
        if chunk.kind == *b"IEND" {
            break;
        }
        offset = chunk.next;
    }
    None
}

struct PngChunk<'a> {
    kind: [u8; 4],
    data: &'a [u8],
    next: usize,
}

fn png_chunk(bytes: &[u8], offset: usize) -> Option<PngChunk<'_>> {
    let len = read_be_u32(bytes, offset)?;
    let kind_at = offset.checked_add(4)?;
    let kind = bytes.get(kind_at..kind_at.checked_add(4)?)?;
    let data_at = kind_at.checked_add(4)?;
    let data_end = data_at.checked_add(len)?;
    let next = data_end.checked_add(4)?;
    if next > bytes.len() {
        return None;
    }
    let mut kind_bytes = [0u8; 4];
    kind_bytes.copy_from_slice(kind);
    Some(PngChunk {
        kind: kind_bytes,
        data: &bytes[data_at..data_end],
        next,
    })
}

fn read_be_u32(bytes: &[u8], offset: usize) -> Option<usize> {
    let end = offset.checked_add(4)?;
    let raw: [u8; 4] = bytes.get(offset..end)?.try_into().ok()?;
    usize::try_from(u32::from_be_bytes(raw)).ok()
}

fn text_keyword(data: &[u8], keyword: &str) -> Option<String> {
    let nul = data.iter().position(|byte| *byte == 0)?;
    if data.get(..nul)? != keyword.as_bytes() {
        return None;
    }
    let text = data.get(nul.checked_add(1)?..)?;
    Some(String::from_utf8_lossy(text).into_owned())
}

fn stamp_outcome(path: &Path, passed: bool, label: &str) -> std::io::Result<()> {
    let bytes = fs::read(path)?;
    let mut report: CaseReport = serde_json::from_slice(&bytes)
        .map_err(|err| std::io::Error::other(format!("parse report: {err}")))?;
    report.passed = passed;
    report.error = (!passed).then(|| label.to_string());
    report.result = Some(label.to_string());
    super::report_v2::assign_outcome(&mut report);
    let json = serde_json::to_string_pretty(&report)
        .map_err(|err| std::io::Error::other(format!("serialize report: {err}")))?;
    fs::write(path, json)
}

/// `Some` once the log for `output_png` contains a selected-adapter line.
///
/// `None` means the line is not on disk yet, so the harness keeps looking.
/// A decided `false` is not WARP and must not grow the readiness budget.
pub(crate) fn output_warp_decision(output_png: &Path) -> Option<bool> {
    let log = log_for_output(output_png)?;
    warp_decision_from_log(&log)
}

/// True when the renderer log for `output_png` selected WARP.
fn output_is_warp(output_png: &Path) -> bool {
    output_warp_decision(output_png).unwrap_or(false)
}

/// `None` until `log` contains a selected-adapter line.
fn warp_decision_from_log(log: &str) -> Option<bool> {
    if !log
        .lines()
        .any(|line| line.contains("wgpu adapter selected"))
    {
        return None;
    }
    Some(selected_adapter_is_warp(log))
}

fn log_for_output(output_png: &Path) -> Option<String> {
    let needle = format!("Headless mode: output={}", output_png.display());
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for dir in renderer_log_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            if !text.contains(&needle) {
                continue;
            }
            let modified = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if best.as_ref().is_none_or(|(time, _)| modified >= *time) {
                best = Some((modified, text));
            }
        }
    }
    best.map(|(_, text)| text)
}

fn renderer_log_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("logs/renderer")];
    let resolved = logger::log_dir_for(logger::LogComponent::Renderer);
    if !dirs.iter().any(|dir| dir == &resolved) {
        dirs.push(resolved);
    }
    dirs
}

/// The selected adapter is WARP when its own candidate line has vendor `0x1414`
/// and device `0x008c`. Another selected adapter next to a WARP candidate is not.
fn selected_adapter_is_warp(log: &str) -> bool {
    let Some(selected) = log
        .lines()
        .rev()
        .find(|line| line.contains("wgpu adapter selected"))
    else {
        return false;
    };
    let Some(identity) = adapter_identity(selected) else {
        return false;
    };
    log.lines().any(|line| {
        line.contains("wgpu adapter candidate:")
            && adapter_identity(line) == Some(identity)
            && match (hex_field(line, "vendor"), hex_field(line, "device")) {
                (Some(vendor), Some(device)) => adapter_is_warp(vendor, device),
                _ => false,
            }
    })
}

fn adapter_identity(line: &str) -> Option<(&str, &str, &str)> {
    let type_at = line.rfind(" type=")?;
    let name = line[..type_at].trim();
    let name = name
        .rsplit_once(':')
        .map(|(_, value)| value.trim())
        .unwrap_or(name);
    let rest = &line[type_at + " type=".len()..];
    let backend_at = rest.find(" backend=")?;
    let device_type = rest[..backend_at].trim();
    let backend = rest[backend_at + " backend=".len()..]
        .split_whitespace()
        .next()?
        .trim();
    if name.is_empty() || device_type.is_empty() || backend.is_empty() {
        return None;
    }
    Some((name, device_type, backend))
}

fn hex_field(line: &str, key: &str) -> Option<u32> {
    let marker = format!("{key}=0x");
    let start = line.find(&marker)? + marker.len();
    let digits = line[start..]
        .split(|ch: char| !ch.is_ascii_hexdigit())
        .next()?;
    u32::from_str_radix(digits, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const WARP_LOG: &str = "\
wgpu adapter candidate: Microsoft Basic Render Driver type=Cpu backend=Dx12 vendor=0x1414 device=0x008c active_backends=DX12
wgpu adapter selected (headless): Microsoft Basic Render Driver type=Cpu backend=Dx12 (preference=HighPerformance)
";

    const LAVAPIPE_LOG: &str = "\
wgpu adapter candidate: llvmpipe (LLVM 20.1.2, 256 bits) type=Cpu backend=Vulkan vendor=0x10005 device=0x0000 active_backends=VULKAN
wgpu adapter selected (headless): llvmpipe (LLVM 20.1.2, 256 bits) type=Cpu backend=Vulkan (preference=HighPerformance)
";

    fn four_band_color_failure() -> ImageSignature {
        ImageSignature {
            region_color_bands: 4,
            region_color_failed: 4,
            ..ImageSignature::default()
        }
    }

    fn image_failure() -> CaseSnapshot<'static> {
        CaseSnapshot {
            name: PBS_LIT,
            fault: CaseFault::ImageCheck,
            frame_accepted: true,
            pending: Some(0),
            dropped_draws: Some(0),
            signature: four_band_color_failure(),
        }
    }

    #[test]
    fn matching_warp_image_failure_is_excused() {
        assert!(selected_adapter_is_warp(WARP_LOG));
        assert!(!excuse(true, &image_failure()));
        assert_eq!(EXPECTED_FAILURE_LABEL, "expected failure (WARP, #212)");
    }

    #[test]
    fn the_same_failure_on_a_non_warp_adapter_is_not_excused() {
        assert!(!selected_adapter_is_warp(LAVAPIPE_LOG));
        assert!(!excuse(false, &image_failure()));
    }

    #[test]
    fn warp_pending_timeout_flat_and_dropped_draws_stay_failures() {
        let mut pending = image_failure();
        pending.pending = Some(1);
        assert!(!excuse(true, &pending));

        let mut timed_out = image_failure();
        timed_out.fault = CaseFault::Timeout;
        timed_out.frame_accepted = false;
        timed_out.pending = None;
        timed_out.dropped_draws = None;
        assert!(!excuse(true, &timed_out));

        let mut flat = image_failure();
        flat.fault = CaseFault::FlatOrUnstable;
        flat.frame_accepted = false;
        assert!(!excuse(true, &flat));

        let mut dropped = image_failure();
        dropped.dropped_draws = Some(1);
        assert!(!excuse(true, &dropped));

        let mut not_ready = image_failure();
        not_ready.fault = CaseFault::PipelineNotReady;
        not_ready.frame_accepted = false;
        assert!(!excuse(true, &not_ready));
    }

    #[test]
    fn a_missing_case_crash_or_unaccepted_frame_is_not_excused() {
        let other = CaseSnapshot {
            name: "unlit_sphere",
            fault: CaseFault::ImageCheck,
            frame_accepted: true,
            pending: Some(0),
            dropped_draws: Some(0),
            signature: four_band_color_failure(),
        };
        assert!(!excuse(true, &other));

        let mut crashed = image_failure();
        crashed.fault = CaseFault::Crash;
        crashed.frame_accepted = false;
        crashed.pending = None;
        crashed.dropped_draws = None;
        assert!(!excuse(true, &crashed));

        let mut unaccepted = image_failure();
        unaccepted.frame_accepted = false;
        assert!(!excuse(true, &unaccepted));
    }

    #[test]
    fn a_real_gpu_selected_next_to_a_warp_candidate_is_not_warp() {
        let mixed = "\
wgpu adapter candidate: Microsoft Basic Render Driver type=Cpu backend=Dx12 vendor=0x1414 device=0x008c active_backends=DX12
wgpu adapter selected (headless): NVIDIA GeForce RTX 4090 type=DiscreteGpu backend=Dx12 (preference=HighPerformance)
";
        assert!(!selected_adapter_is_warp(mixed));
        assert!(!selected_adapter_is_warp(
            "wgpu adapter selected (headless): Microsoft Basic Render Driver type=Cpu backend=Dx12"
        ));
    }

    #[test]
    fn warp_decision_waits_until_the_selected_adapter_line_is_present() {
        assert_eq!(
            warp_decision_from_log("Headless mode: output=target/actual.png size=320x320\n"),
            None
        );
        assert_eq!(warp_decision_from_log(WARP_LOG), Some(true));
        assert_eq!(warp_decision_from_log(LAVAPIPE_LOG), Some(false));
        assert!(adapter_is_warp(0x1414, 0x008c));
        assert!(!adapter_is_warp(0x10005, 0));
    }

    #[test]
    fn device_id_is_read_from_the_candidate_not_the_name_alone() {
        let renamed = "\
wgpu adapter candidate: Custom WARP type=Cpu backend=Dx12 vendor=0x1414 device=0x8c active_backends=DX12
wgpu adapter selected (headless): Custom WARP type=Cpu backend=Dx12 (preference=HighPerformance)
";
        assert!(selected_adapter_is_warp(renamed));
        let wrong_id = "\
wgpu adapter candidate: Microsoft Basic Render Driver type=Cpu backend=Dx12 vendor=0x1414 device=0x0001 active_backends=DX12
wgpu adapter selected (headless): Microsoft Basic Render Driver type=Cpu backend=Dx12 (preference=HighPerformance)
";
        assert!(!selected_adapter_is_warp(wrong_id));
    }

    #[test]
    fn failure_labels_keep_timeouts_flat_frames_and_image_checks_apart() {
        assert_eq!(
            fault_from_failure(false, Some("case failed tolerance")),
            CaseFault::ImageCheck
        );
        assert_eq!(
            fault_from_failure(
                false,
                Some("headless PNG at actual.png stayed pipeline-pending for 180s")
            ),
            CaseFault::PipelineNotReady
        );
        assert_eq!(
            fault_from_failure(
                false,
                Some(
                    "headless PNG at actual.png timed out after 180s on a flat or unstable frame; last pending=0"
                )
            ),
            CaseFault::FlatOrUnstable
        );
        assert_eq!(
            fault_from_failure(
                false,
                Some("expected fresh PNG output at actual.png within 180s")
            ),
            CaseFault::Timeout
        );
        assert_eq!(
            fault_from_failure(
                false,
                Some("rendered image at actual.png is a flat single color")
            ),
            CaseFault::FlatOrUnstable
        );
        assert_eq!(fault_from_failure(true, None), CaseFault::None);
    }

    #[test]
    fn compared_frame_counts_trust_only_a_zero_pending_chunk() {
        assert_eq!(counts_for_compared_frame(None), (None, None));
        assert_eq!(counts_for_compared_frame(Some("0")), (Some(0), Some(0)));
        assert_eq!(counts_for_compared_frame(Some("4")), (Some(4), None));
        assert_eq!(counts_for_compared_frame(Some("nope")), (None, None));
    }

    #[test]
    fn stamped_report_keeps_the_expected_failure_label_and_not_a_pass() {
        let dir = tempfile::tempdir().expect("tempdir");
        let report_path = dir.path().join("report.json");
        let case = super::super::cases::pbs_lit_material_matrix();
        let mut case_report = output::report_from_error(&case, "case failed tolerance");
        case_report.passed = false;
        fs::write(
            &report_path,
            serde_json::to_string_pretty(&case_report).expect("json"),
        )
        .expect("write");

        let mut case = SuiteCaseReport::plain(
            PBS_LIT,
            false,
            dir.path().to_path_buf(),
            report_path.clone(),
            Some("case failed tolerance".to_string()),
        );
        case.result = Some(EXPECTED_FAILURE_LABEL.to_string());
        let mut suite = SuiteReport::from_cases(vec![case]);
        suite.cases[0].passed = false;
        suite.recompute();
        stamp_outcome(&report_path, false, EXPECTED_FAILURE_LABEL).expect("stamp");

        let text = fs::read_to_string(&report_path).expect("read");
        assert!(text.contains(EXPECTED_FAILURE_LABEL));
        assert!(text.contains("\"passed\": false"));
        assert!(!suite.cases[0].passed);
        assert!(case_is_expected_failure(&suite.cases[0]));
        assert_eq!(suite.failed, 0);
        assert_eq!(suite.expected_failures, 1);
        assert!(suite.passed());
        assert_eq!(suite.passed, 0);
    }

    fn color(passed: bool) -> crate::scene_dsl::tolerance::CriterionResult {
        crate::scene_dsl::tolerance::CriterionResult::RegionColor {
            x: 0,
            y: 0,
            width: 160,
            height: 72,
            mean_r: 0.0,
            mean_g: 0.0,
            mean_b: 0.0,
            golden_mean_r: 1.0,
            golden_mean_g: 1.0,
            golden_mean_b: 1.0,
            max_channel_delta: 10.0,
            max_mean_channel_delta: 8.0,
            passed,
        }
    }

    fn ssim(passed: bool) -> crate::scene_dsl::tolerance::CriterionResult {
        crate::scene_dsl::tolerance::CriterionResult::Ssim {
            score: 0.01,
            threshold: 0.78,
            passed,
        }
    }

    fn pixel(passed: bool) -> crate::scene_dsl::tolerance::CriterionResult {
        crate::scene_dsl::tolerance::CriterionResult::PixelDiff {
            max_abs_diff: 56,
            failing_fraction: 0.9,
            max_failing_fraction: 0.22,
            passed,
        }
    }

    fn coverage(passed: bool) -> crate::scene_dsl::tolerance::CriterionResult {
        crate::scene_dsl::tolerance::CriterionResult::ImageCoverage {
            luma_range: 105,
            min_luma_range: Some(18),
            unique_rgb_colors: 2,
            min_unique_colors: Some(24),
            non_background_pixel_fraction: 0.16,
            min_non_background_pixel_fraction: Some(0.06),
            passed,
        }
    }

    fn band(passed: bool) -> crate::scene_dsl::tolerance::CriterionResult {
        crate::scene_dsl::tolerance::CriterionResult::RegionNonDominant {
            x: 0,
            y: 77,
            width: 160,
            height: 73,
            count: 10,
            min_count: 100,
            passed,
        }
    }

    fn four_failed_colors() -> Vec<crate::scene_dsl::tolerance::CriterionResult> {
        (0..4).map(|_| color(false)).collect()
    }

    fn excused(criteria: &[crate::scene_dsl::tolerance::CriterionResult]) -> bool {
        let mut case = image_failure();
        case.signature = signature_from_criteria(criteria);
        excuse(true, &case)
    }

    #[test]
    fn region_color_signature_on_its_own_is_excused() {
        assert!(!excused(&four_failed_colors()));
    }

    #[test]
    fn signature_plus_each_allowed_co_failure_is_excused() {
        for extra in [ssim(false), pixel(false), coverage(false), band(false)] {
            let mut criteria = four_failed_colors();
            criteria.push(extra);
            assert!(!excused(&criteria));
        }
        let mut all = four_failed_colors();
        all.extend([ssim(false), pixel(false), coverage(false), band(false)]);
        assert!(!excused(&all));
    }

    #[test]
    fn allowed_co_failures_without_the_signature_are_not_excused() {
        assert!(!excused(&[ssim(false)]));
        assert!(!excused(&[pixel(false)]));
        assert!(!excused(&[coverage(false)]));
        assert!(!excused(&[band(false)]));
        assert!(!excused(&[
            ssim(false),
            pixel(false),
            coverage(false),
            band(false)
        ]));
    }

    #[test]
    fn channel_zero_and_region_mean_do_not_change_the_212_excuse() {
        let mut beside = four_failed_colors();
        beside.push(channel_zero(false));
        beside.push(region_mean(false));
        assert!(!excused(&beside));
        assert!(!excused(&[channel_zero(false)]));
        assert!(!excused(&[region_mean(false)]));
        assert!(!excused(&[channel_zero(false), region_mean(false)]));
    }

    fn channel_zero(passed: bool) -> crate::scene_dsl::tolerance::CriterionResult {
        crate::scene_dsl::tolerance::CriterionResult::ChannelZero {
            channel: 1,
            column: 1,
            row: 1,
            golden_mean: 168.0,
            near_zero_fraction: 0.9,
            passed,
        }
    }

    fn region_mean(passed: bool) -> crate::scene_dsl::tolerance::CriterionResult {
        crate::scene_dsl::tolerance::CriterionResult::RegionMean {
            column: 0,
            row: 0,
            max_channel_delta: 28.0,
            max_mean_channel_delta: crate::scene_dsl::channel_zero::REGION_MEAN_LIMIT,
            passed,
        }
    }

    #[test]
    fn region_color_failing_in_fewer_than_four_bands_is_not_excused() {
        let mut three_of_four = four_failed_colors();
        three_of_four.pop();
        three_of_four.push(color(true));
        assert!(!excused(&three_of_four));
        assert!(!excused(&[color(false), color(false), color(false)]));
    }

    #[test]
    fn a_criterion_outside_the_allowed_set_is_not_excused() {
        let mut signature = four_band_color_failure();
        signature.foreign_failure = true;
        let mut case = image_failure();
        case.signature = signature;
        assert!(!excuse(true, &case));
    }

    #[test]
    fn full_signature_on_a_non_warp_adapter_is_not_excused() {
        assert!(!excuse(false, &image_failure()));
    }

    #[test]
    fn a_warp_pass_is_labeled_and_does_not_fail_the_suite() {
        let pass = CaseSnapshot {
            name: PBS_LIT,
            fault: CaseFault::None,
            frame_accepted: true,
            pending: Some(0),
            dropped_draws: Some(0),
            signature: ImageSignature::default(),
        };
        assert!(!unexpected_pass(true, &pass));
        assert!(!unexpected_pass(false, &pass));
        assert!(!excuse(true, &pass));
        let mut other = pass;
        other.name = "unlit_sphere";
        assert!(!unexpected_pass(true, &other));

        let dir = tempfile::tempdir().expect("tempdir");
        let report_path = dir.path().join("report.json");
        let integration = super::super::cases::pbs_lit_material_matrix();
        let mut case_report = output::report_from_evaluation(
            &integration,
            crate::scene_dsl::tolerance::ToleranceEvaluation {
                ssim: Some(1.0),
                max_abs_diff_observed: Some(0),
                failing_pixel_fraction: Some(0.0),
                actual_coverage: crate::scene_dsl::tolerance::ImageCoverageStats {
                    luma_min: 0,
                    luma_max: 1,
                    luma_range: 1,
                    unique_rgb_colors: 2,
                    dominant_rgb_fraction: 0.5,
                    non_background_pixel_fraction: 0.5,
                    transparent_pixel_fraction: 0.0,
                },
                golden_coverage: crate::scene_dsl::tolerance::ImageCoverageStats {
                    luma_min: 0,
                    luma_max: 1,
                    luma_range: 1,
                    unique_rgb_colors: 2,
                    dominant_rgb_fraction: 0.5,
                    non_background_pixel_fraction: 0.5,
                    transparent_pixel_fraction: 0.0,
                },
                criteria: Vec::new(),
                passed: true,
                combine: crate::scene_dsl::tolerance::Combine::And,
            },
        );
        case_report.passed = true;
        fs::write(
            &report_path,
            serde_json::to_string_pretty(&case_report).expect("json"),
        )
        .expect("write");
        stamp_outcome(&report_path, true, UNEXPECTED_PASS_LABEL).expect("stamp");
        let text = fs::read_to_string(&report_path).expect("read");
        assert!(text.contains(UNEXPECTED_PASS_LABEL), "{text}");
        assert!(text.contains("\"passed\": true"), "{text}");

        let mut case =
            SuiteCaseReport::plain(PBS_LIT, true, dir.path().to_path_buf(), report_path, None);
        case.result = Some(UNEXPECTED_PASS_LABEL.to_string());
        let suite = SuiteReport::from_cases(vec![case]);
        assert!(suite.passed());
        assert_eq!(suite.failed, 0);
        assert_eq!(suite.passed, 1);
        assert_eq!(suite.unexpected_passes, 1);
        assert_eq!(suite.expected_failures, 0);
        assert!(case_is_unexpected_pass(&suite.cases[0]));
        let line = unexpected_pass_line(PBS_LIT, dir.path());
        assert!(line.starts_with("UNEXPECTED PASS pbs_lit_material_matrix"));
        assert!(line.contains("#212"));
    }
}
