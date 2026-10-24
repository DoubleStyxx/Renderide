//! WARP-only expected failure for `pbs_lit_material_matrix`.
//!
//! The known mismatch is the #208 color or band check failing on a frame captured
//! with no pending pipelines and no dropped draws
//! (https://github.com/DoubleStyxx/Renderide/issues/212). A lavapipe or real-GPU
//! run does not use this path. A PipelineNotReady timeout, a positive pending
//! count, any dropped draw, a missing case, or a crash stays a suite failure.

use std::fs;
use std::path::Path;

use crate::scene_dsl::tolerance::{CriterionResult, ToleranceEvaluation};

/// Case whose WARP mismatch is expected.
pub(crate) const PBS_LIT: &str = "pbs_lit_material_matrix";

/// PNG `tEXt` keyword written when the renderer counts pending and dropped draws.
const PIPELINE_PENDING_TEXT_KEYWORD: &str = "renderide_pipeline_pending";

/// What a check-suite should do with `pbs_lit` after the other cases are known.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Ruling {
    /// Leave the suite result alone. A failed suite stays failed.
    Unchanged,
    /// The known WARP color or band miss on a finished frame.
    ExpectedFailure,
    /// `pbs_lit` passed on WARP. The known mismatch went away, or the gate did not run.
    UnexpectedPass,
}

/// Why a case failed, for the WARP exception.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CaseFault {
    /// The case passed, or the caller did not name a fault.
    None,
    /// A region color or band-floor criterion failed.
    ColorOrBand,
    /// The capture timed out while material draws were still compiling or skipped.
    PipelineNotReady,
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
pub(crate) struct CaseSnapshot<'a> {
    /// Case identifier.
    pub name: &'a str,
    /// Whether the case passed.
    pub passed: bool,
    /// Why it failed. [`CaseFault::None`] when it passed.
    pub fault: CaseFault,
    /// Pipelines still pending when the frame was captured.
    pub pending: Option<u64>,
    /// Draws dropped before the capture.
    pub dropped_draws: Option<u64>,
}

/// Decides the WARP exception from the adapter and the case results.
pub(crate) fn ruling(is_warp: bool, cases: &[CaseSnapshot<'_>]) -> Ruling {
    if !is_warp {
        return Ruling::Unchanged;
    }
    let Some(pbs) = cases.iter().find(|case| case.name == PBS_LIT) else {
        return Ruling::Unchanged;
    };
    if cases
        .iter()
        .any(|case| case.name != PBS_LIT && !case.passed)
    {
        return Ruling::Unchanged;
    }
    if pbs.passed {
        return Ruling::UnexpectedPass;
    }
    if known_mismatch(pbs) {
        Ruling::ExpectedFailure
    } else {
        Ruling::Unchanged
    }
}

fn known_mismatch(case: &CaseSnapshot<'_>) -> bool {
    case.fault == CaseFault::ColorOrBand && case.pending == Some(0) && case.dropped_draws == Some(0)
}

/// Classifies a suite error string.
///
/// A PipelineNotReady timeout wins over a color or band flag, so a capture that
/// never became ready cannot be relabeled as the known mismatch.
pub(crate) fn fault_from_failure(error: Option<&str>, color_or_band_failed: bool) -> CaseFault {
    let error = error.unwrap_or("");
    if error.contains("pipeline-pending") || error.contains("PipelineNotReady") {
        return CaseFault::PipelineNotReady;
    }
    if error.contains("renderer exited with failure") || error.contains("spawn renderer process") {
        return CaseFault::Crash;
    }
    if color_or_band_failed {
        return CaseFault::ColorOrBand;
    }
    if error.is_empty() {
        CaseFault::None
    } else {
        CaseFault::Other
    }
}

/// True when a region color or band-floor criterion failed.
pub(crate) fn evaluation_failed_color_or_band(eval: &ToleranceEvaluation) -> bool {
    eval.criteria.iter().any(|criterion| {
        matches!(
            criterion,
            CriterionResult::RegionColor { passed: false, .. }
                | CriterionResult::RegionNonDominant { passed: false, .. }
        )
    })
}

/// Pending and dropped counts for a frame that reached image comparison.
///
/// `chunk` is the `renderide_pipeline_pending` text. That one number is the sum
/// of still-compiling lookups and dropped draws, so only `0` records both as
/// zero. A missing chunk means this renderer does not count either, and the
/// frame was still compared, so both are zero. A positive or unreadable chunk
/// does not record zero dropped draws.
pub(crate) fn counts_for_compared_frame(chunk: Option<&str>) -> (Option<u64>, Option<u64>) {
    let Some(chunk) = chunk else {
        return (Some(0), Some(0));
    };
    match chunk.trim().parse::<u64>() {
        Ok(0) => (Some(0), Some(0)),
        Ok(pending) => (Some(pending), None),
        Err(_) => (None, None),
    }
}

/// Reads `renderide_pipeline_pending` from a PNG. A missing file or chunk is `None`.
pub(crate) fn read_pipeline_pending_chunk(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    pipeline_pending_text(&bytes)
}

pub(crate) fn pipeline_pending_text(bytes: &[u8]) -> Option<String> {
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

/// True when every selected adapter is WARP and a candidate line carries its id.
///
/// WARP is `Microsoft Basic Render Driver`, device type `Cpu`, vendor `0x1414`,
/// device `0x008c`, backend `Dx12`. A missing id is not WARP.
pub(crate) fn logs_are_warp(logs: &[&str]) -> bool {
    let mut saw_selected = false;
    let mut saw_identity = false;
    for log in logs {
        for line in log.lines() {
            if line.contains("wgpu adapter selected") {
                saw_selected = true;
                if !selected_line_is_warp(line) {
                    return false;
                }
            }
            if candidate_line_is_warp(line) {
                saw_identity = true;
            }
        }
    }
    saw_selected && saw_identity
}

fn selected_line_is_warp(line: &str) -> bool {
    line.contains("Microsoft Basic Render Driver")
        && line.contains("type=Cpu")
        && line.contains("backend=Dx12")
}

fn candidate_line_is_warp(line: &str) -> bool {
    line.contains("wgpu adapter candidate: Microsoft Basic Render Driver")
        && line.contains("type=Cpu")
        && line.contains("vendor=0x1414")
        && line.contains("device=0x008c")
}

/// Reads `*.log` files under `dir`. An unreadable directory is not WARP.
pub(crate) fn directory_is_warp(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    let mut logs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            logs.push(text);
        }
    }
    let borrowed: Vec<&str> = logs.iter().map(String::as_str).collect();
    logs_are_warp(&borrowed)
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

    fn finished(name: &str, passed: bool) -> CaseSnapshot<'_> {
        CaseSnapshot {
            name,
            passed,
            fault: if passed {
                CaseFault::None
            } else {
                CaseFault::ColorOrBand
            },
            pending: Some(0),
            dropped_draws: Some(0),
        }
    }

    fn known_failure() -> CaseSnapshot<'static> {
        finished(PBS_LIT, false)
    }

    #[test]
    fn warp_identity_requires_the_selected_adapter_and_the_device_id() {
        assert!(logs_are_warp(&[WARP_LOG]));
        assert!(!logs_are_warp(&[LAVAPIPE_LOG]));
        assert!(!logs_are_warp(&[
            "wgpu adapter selected (headless): Microsoft Basic Render Driver type=Cpu backend=Dx12"
        ]));
        assert!(!logs_are_warp(&[]));
    }

    #[test]
    fn a_real_gpu_selected_next_to_a_warp_candidate_is_not_warp() {
        let mixed = "\
wgpu adapter candidate: Microsoft Basic Render Driver type=Cpu backend=Dx12 vendor=0x1414 device=0x008c active_backends=DX12
wgpu adapter selected (headless): NVIDIA GeForce RTX 4090 type=DiscreteGpu backend=Dx12 (preference=HighPerformance)
";
        assert!(!logs_are_warp(&[mixed]));
    }

    #[test]
    fn color_or_band_on_a_finished_frame_is_the_only_expected_failure() {
        assert_eq!(
            ruling(true, &[finished("unlit_sphere", true), known_failure()]),
            Ruling::ExpectedFailure
        );
        assert_eq!(
            ruling(false, &[finished("unlit_sphere", true), known_failure()]),
            Ruling::Unchanged
        );
    }

    #[test]
    fn an_unexpected_pass_is_reported() {
        assert_eq!(
            ruling(
                true,
                &[finished("unlit_sphere", true), finished(PBS_LIT, true)]
            ),
            Ruling::UnexpectedPass
        );
    }

    #[test]
    fn another_case_failing_stays_a_hard_failure() {
        assert_eq!(
            ruling(true, &[finished("unlit_sphere", false), known_failure()]),
            Ruling::Unchanged
        );
        assert_eq!(
            ruling(
                true,
                &[finished("unlit_sphere", false), finished(PBS_LIT, true)]
            ),
            Ruling::Unchanged
        );
    }

    #[test]
    fn a_pipeline_not_ready_timeout_stays_a_hard_failure() {
        let mut timed_out = known_failure();
        timed_out.fault = CaseFault::PipelineNotReady;
        assert_eq!(
            ruling(true, &[finished("unlit_sphere", true), timed_out]),
            Ruling::Unchanged
        );
        assert_eq!(
            fault_from_failure(
                Some(
                    "headless PNG at actual.png stayed pipeline-pending for 180s; capture refused frames whose material draws were still compiling or were skipped because no pipeline was available"
                ),
                true,
            ),
            CaseFault::PipelineNotReady
        );
    }

    #[test]
    fn pending_draws_stay_a_hard_failure() {
        let mut pending = known_failure();
        pending.pending = Some(1);
        assert_eq!(
            ruling(true, &[finished("unlit_sphere", true), pending]),
            Ruling::Unchanged
        );
        let mut unrecorded = known_failure();
        unrecorded.pending = None;
        assert_eq!(
            ruling(true, &[finished("unlit_sphere", true), unrecorded]),
            Ruling::Unchanged
        );
    }

    #[test]
    fn dropped_draws_stay_a_hard_failure() {
        let mut dropped = known_failure();
        dropped.dropped_draws = Some(1);
        assert_eq!(
            ruling(true, &[finished("unlit_sphere", true), dropped]),
            Ruling::Unchanged
        );
        let mut unrecorded = known_failure();
        unrecorded.dropped_draws = None;
        assert_eq!(
            ruling(true, &[finished("unlit_sphere", true), unrecorded]),
            Ruling::Unchanged
        );
    }

    #[test]
    fn a_missing_case_stays_a_hard_failure() {
        assert_eq!(
            ruling(true, &[finished("unlit_sphere", true)]),
            Ruling::Unchanged
        );
    }

    #[test]
    fn a_crash_stays_a_hard_failure() {
        let mut crashed = known_failure();
        crashed.fault = CaseFault::Crash;
        crashed.pending = None;
        crashed.dropped_draws = None;
        assert_eq!(
            ruling(true, &[finished("unlit_sphere", true), crashed]),
            Ruling::Unchanged
        );
        assert_eq!(
            fault_from_failure(
                Some("renderer exited with failure before producing PNG"),
                false,
            ),
            CaseFault::Crash
        );
        assert_eq!(
            fault_from_failure(Some("spawn renderer process: entity not found"), false),
            CaseFault::Crash
        );
    }

    #[test]
    fn a_tolerance_miss_without_color_or_band_stays_a_hard_failure() {
        assert_eq!(
            fault_from_failure(Some("case failed tolerance"), false),
            CaseFault::Other
        );
        let mut other = known_failure();
        other.fault = CaseFault::Other;
        assert_eq!(
            ruling(true, &[finished("unlit_sphere", true), other]),
            Ruling::Unchanged
        );
    }

    #[test]
    fn compared_frame_counts_trust_only_a_zero_pending_chunk() {
        assert_eq!(counts_for_compared_frame(None), (Some(0), Some(0)));
        assert_eq!(counts_for_compared_frame(Some("0")), (Some(0), Some(0)));
        assert_eq!(counts_for_compared_frame(Some("4")), (Some(4), None));
        assert_eq!(counts_for_compared_frame(Some("nope")), (None, None));
    }

    #[test]
    fn pending_text_reads_the_png_chunk_and_ignores_other_keywords() {
        let png = png_with_text(&[("comment", "0"), (PIPELINE_PENDING_TEXT_KEYWORD, "0")]);
        assert_eq!(pipeline_pending_text(&png).as_deref(), Some("0"));
        assert_eq!(
            pipeline_pending_text(&png_with_text(&[("comment", "0")])),
            None
        );
        assert_eq!(pipeline_pending_text(&[1, 2, 3]), None);
    }

    fn png_with_text(chunks: &[(&str, &str)]) -> Vec<u8> {
        let mut png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        for (keyword, text) in chunks {
            let mut data = Vec::new();
            data.extend(keyword.as_bytes());
            data.push(0);
            data.extend(text.as_bytes());
            let len = u32::try_from(data.len()).expect("chunk length");
            png.extend(len.to_be_bytes());
            png.extend(b"tEXt");
            png.extend(data);
            png.extend([0, 0, 0, 0]);
        }
        png.extend(0u32.to_be_bytes());
        png.extend(b"IEND");
        png.extend([0, 0, 0, 0]);
        png
    }
}
