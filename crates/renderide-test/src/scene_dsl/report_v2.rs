//! Schema v2 fields stamped onto each case `report.json`.
//!
//! Existing fields stay. This module adds `schema`, `reason_text`, `artifacts`,
//! `capture`, and `runner` (`adapter` plus `icd`). `status` and `reason` stay
//! the quarantine fields; a v2 stamp fills them for the other outcomes too.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::HarnessError;

use super::host_info;
use super::output::{CaseReport, EarlyExit};
use super::quarantine_216::{QUARANTINE_REASON, QUARANTINE_STATUS, WARP_LIT_SHADING_UNTRUSTED};
use super::warp_pbs::{EXPECTED_FAILURE_LABEL, UNEXPECTED_PASS_LABEL};

/// `schema` value written into every stamped report.
pub(crate) const SCHEMA: &str = "renderide-test.v2";

/// Pass status.
pub(crate) const STATUS_PASS: &str = "pass";
/// Hard-failure status.
pub(crate) const STATUS_FAIL: &str = "fail";
/// Expected WARP failure (#212).
pub(crate) const STATUS_EXPECTED_FAILURE: &str = "expected-failure";
/// Unexpected WARP pass (#212).
pub(crate) const STATUS_UNEXPECTED_PASS: &str = "unexpected-pass";
/// Flat wait that ended before the full budget.
pub(crate) const STATUS_EARLY_EXIT: &str = "early-exit";

/// Reason when a case passed.
pub(crate) const REASON_OK: &str = "ok";
/// Reason for an expected WARP failure.
pub(crate) const REASON_EXPECTED_FAILURE: &str = "expected-failure:#212";
/// Reason for an unexpected WARP pass.
pub(crate) const REASON_UNEXPECTED_PASS: &str = "unexpected-pass:#212";
/// Reason for a #216 early flat exit.
pub(crate) const REASON_EARLY_EXIT: &str = "early-exit";

const VK_DRIVER_FILES: &str = "VK_DRIVER_FILES";
const VK_ICD_FILENAMES: &str = "VK_ICD_FILENAMES";
const FALLBACK_CODE: &str = "harness-error";
const FALLBACK_TEXT: &str = "The case failed before a more specific reason was recorded.";

/// One stable reason code and its non-empty text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Reason {
    /// Stable `reason` value.
    pub code: &'static str,
    /// Non-empty `reason_text`.
    pub text: &'static str,
}

/// Artifact filenames that were actually written. A missing file is omitted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct ArtifactNames {
    /// Captured PNG, when `actual.png` is on disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
    /// Golden copy, when `golden.png` is on disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub golden: Option<String>,
    /// Diff image, when `diff.png` is on disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    /// `golden | actual | diff`, when `side_by_side.png` is on disk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side_by_side: Option<String>,
}

/// Capture facts copied next to the flat top-level fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct CaptureBlock {
    /// Longest submission stall, in milliseconds.
    pub longest_submission_stall_ms: u64,
    /// True when the WARP readiness budget was applied.
    pub warp_readiness_budget_used: bool,
    /// Present when a #216 flat wait ended early.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub early_exit: Option<EarlyExit>,
}

/// Structured runner identity. The top-level `adapter` string stays.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct RunnerBlock {
    /// Same CPU model as the top-level field.
    pub cpu_model: String,
    /// Vendor, device, backend, and device type.
    pub adapter: AdapterBlock,
    /// Vulkan ICD the process inherited.
    pub icd: IcdBlock,
}

/// Adapter identity parsed from the renderer log.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct AdapterBlock {
    /// PCI vendor, or null when the log has no candidate line.
    pub vendor: Option<u32>,
    /// PCI device, or null when the log has no candidate line.
    pub device: Option<u32>,
    /// `backend=` value, or `unavailable`.
    pub backend: String,
    /// `type=` value, or `unavailable`.
    pub device_type: String,
}

/// Vulkan ICD selection. Same shape as the frame-bench schema-2 report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct IcdBlock {
    /// Manifest path, or null when the loader uses its default search.
    pub path: Option<String>,
    /// `env` or `loader_default`.
    pub source: String,
    /// `VK_DRIVER_FILES` or `VK_ICD_FILENAMES`, or null.
    pub var: Option<String>,
}

/// Fills the v2 fields from the report already stamped with host and capture stats.
///
/// `capture.early_exit` is the same record as the top-level field, including
/// settle time and K-window time. Artifact names are included only for files
/// that exist in `layout`.
pub(crate) fn stamp(report: &mut CaseReport, layout: &super::output::CaseOutputLayout) {
    report.schema = SCHEMA.to_string();
    assign_outcome(report);
    report.artifacts = written_artifacts(layout);
    report.capture = Some(CaptureBlock {
        longest_submission_stall_ms: report.longest_submission_stall_ms,
        warp_readiness_budget_used: report.warp_readiness_budget_used,
        early_exit: report.early_exit.clone(),
    });
    report.runner = Some(RunnerBlock {
        cpu_model: report.cpu_model.clone(),
        adapter: adapter_block(&layout.actual_png),
        icd: icd_from_env(),
    });
}

/// Sets `status`, `reason`, and `reason_text` without touching the other fields.
///
/// A quarantine stamp wins over an early exit so `quarantined:#216` stays the
/// suite reason. The early exit remains on `early_exit` and `capture.early_exit`.
pub(crate) fn assign_outcome(report: &mut CaseReport) {
    if is_quarantine(report) {
        report.status = Some(QUARANTINE_STATUS.to_string());
        report.reason_text = Some(quarantine_text(report).to_string());
        return;
    }
    let (status, reason) = classify(report);
    report.status = Some(status.to_string());
    report.reason = Some(reason.code.to_string());
    report.reason_text = Some(reason.text.to_string());
}

/// Maps one [`HarnessError`] variant to one reason. The text is never empty.
pub(crate) fn reason_for_error(err: &HarnessError) -> Reason {
    reason_from_code(match err {
        HarnessError::RendererBinaryMissing(_) => "renderer-binary-missing",
        HarnessError::SpawnRenderer(_) => "spawn-renderer",
        HarnessError::QueueOptions(_) => "queue-options",
        HarnessError::HandshakeTimeout(_) => "handshake-timeout",
        HarnessError::AssetAckTimeout(_, _) => "asset-ack-timeout",
        HarnessError::PngOutputMissing { .. } => "png-output-missing",
        HarnessError::PipelineNotReady { .. } => "pipeline-not-ready",
        HarnessError::FlatOrUnstableFrame { .. } => "flat-or-unstable-frame",
        HarnessError::PngRead { .. } => "png-read",
        HarnessError::PngWrite { .. } => "png-write",
        HarnessError::FlatImage { .. } => "flat-image",
        HarnessError::GoldenMissing(_) => "golden-missing",
        HarnessError::GoldenMismatch { .. } => "golden-mismatch",
        HarnessError::Io(_) => "io",
        HarnessError::GltfFixture { .. } => "gltf-fixture",
        HarnessError::ImageCompare(_) => "image-compare",
        HarnessError::UnknownCase(_) => "unknown-case",
        HarnessError::BenchReportMissing { .. } => "bench-report-missing",
        HarnessError::UnknownBenchScenario(_) => "unknown-bench-scenario",
        HarnessError::SuiteFailed { .. } => "suite-failed",
    })
}

fn classify(report: &CaseReport) -> (&'static str, Reason) {
    if report.result.as_deref() == Some(UNEXPECTED_PASS_LABEL) {
        return (
            STATUS_UNEXPECTED_PASS,
            reason_from_code(REASON_UNEXPECTED_PASS),
        );
    }
    if report.result.as_deref() == Some(EXPECTED_FAILURE_LABEL) {
        return (
            STATUS_EXPECTED_FAILURE,
            reason_from_code(REASON_EXPECTED_FAILURE),
        );
    }
    if report.early_exit.is_some() {
        return (STATUS_EARLY_EXIT, reason_from_code(REASON_EARLY_EXIT));
    }
    if report.passed {
        return (STATUS_PASS, reason_from_code(REASON_OK));
    }
    if let Some(code) = report
        .reason
        .as_deref()
        .filter(|code| is_variant_reason(code))
    {
        return (STATUS_FAIL, reason_from_code(code));
    }
    (
        STATUS_FAIL,
        reason_for_message(report.error.as_deref().unwrap_or("")),
    )
}

fn is_variant_reason(code: &str) -> bool {
    !matches!(
        code,
        REASON_OK
            | REASON_EXPECTED_FAILURE
            | REASON_UNEXPECTED_PASS
            | REASON_EARLY_EXIT
            | QUARANTINE_REASON
            | WARP_LIT_SHADING_UNTRUSTED
    ) && REASONS.iter().any(|(known, _)| *known == code)
}

fn is_quarantine(report: &CaseReport) -> bool {
    report.status.as_deref() == Some(QUARANTINE_STATUS)
        || matches!(
            report.reason.as_deref(),
            Some(QUARANTINE_REASON | WARP_LIT_SHADING_UNTRUSTED)
        )
}

fn quarantine_text(report: &CaseReport) -> &'static str {
    if report.early_exit.is_some() {
        return "Time-limited WARP quarantine #216; the flat wait ended early.";
    }
    if report.reason.as_deref() == Some(WARP_LIT_SHADING_UNTRUSTED) {
        return reason_from_code(WARP_LIT_SHADING_UNTRUSTED).text;
    }
    reason_from_code(QUARANTINE_REASON).text
}

fn reason_for_message(message: &str) -> Reason {
    let code = MESSAGE_NEEDLES
        .iter()
        .find(|(needle, _)| message.contains(needle))
        .map(|(_, code)| *code)
        .unwrap_or(FALLBACK_CODE);
    reason_from_code(code)
}

const MESSAGE_NEEDLES: &[(&str, &str)] = &[
    ("renderer binary not found", "renderer-binary-missing"),
    ("spawn renderer process", "spawn-renderer"),
    ("queue options invalid", "queue-options"),
    ("handshake timed out", "handshake-timeout"),
    ("asset ack timed out", "asset-ack-timeout"),
    ("expected fresh PNG", "png-output-missing"),
    ("pipeline-pending", "pipeline-not-ready"),
    ("flat or unstable", "flat-or-unstable-frame"),
    ("read png", "png-read"),
    ("write png", "png-write"),
    ("flat single color", "flat-image"),
    ("golden image not found", "golden-missing"),
    ("perceptual diff failed", "golden-mismatch"),
    ("io: ", "io"),
    ("load GLB fixture", "gltf-fixture"),
    ("image-compare", "image-compare"),
    ("unknown integration case", "unknown-case"),
    ("frame bench report missing", "bench-report-missing"),
    ("unknown frame bench scenario", "unknown-bench-scenario"),
    ("headless suite failed", "suite-failed"),
];

fn reason_from_code(code: &str) -> Reason {
    REASONS
        .iter()
        .find(|pair| pair.0 == code)
        .map(|pair| Reason {
            code: pair.0,
            text: pair.1,
        })
        .unwrap_or(Reason {
            code: FALLBACK_CODE,
            text: FALLBACK_TEXT,
        })
}

const REASONS: &[(&str, &str)] = &[
    (
        "renderer-binary-missing",
        "The renderer binary was not on disk.",
    ),
    (
        "spawn-renderer",
        "The renderer process could not be spawned.",
    ),
    (
        "queue-options",
        "The interprocess queue options were invalid.",
    ),
    ("handshake-timeout", "The renderer handshake timed out."),
    (
        "asset-ack-timeout",
        "An asset upload acknowledgement timed out.",
    ),
    (
        "png-output-missing",
        "The headless PNG was not written in time.",
    ),
    (
        "pipeline-not-ready",
        "The capture stayed pipeline-pending until the deadline.",
    ),
    (
        "flat-or-unstable-frame",
        "The capture timed out on a flat or unstable frame.",
    ),
    ("png-read", "The PNG could not be read."),
    ("png-write", "The PNG could not be written."),
    ("flat-image", "The image was a single flat color."),
    ("golden-missing", "The golden image is not on disk."),
    (
        "golden-mismatch",
        "The perceptual diff was below its threshold.",
    ),
    ("io", "A filesystem operation failed."),
    ("gltf-fixture", "A GLB fixture could not be loaded."),
    ("image-compare", "image-compare could not score the pair."),
    (
        "unknown-case",
        "The named integration case is not in the suite.",
    ),
    (
        "bench-report-missing",
        "The frame benchmark did not write its report.",
    ),
    (
        "unknown-bench-scenario",
        "The frame benchmark scenario name is unknown.",
    ),
    ("suite-failed", "One or more headless suite cases failed."),
    (REASON_OK, "The case passed."),
    (REASON_EXPECTED_FAILURE, EXPECTED_FAILURE_LABEL),
    (REASON_UNEXPECTED_PASS, UNEXPECTED_PASS_LABEL),
    (
        REASON_EARLY_EXIT,
        "The #216 flat wait ended after K identical settled frames.",
    ),
    (
        QUARANTINE_REASON,
        "Time-limited WARP quarantine for a flat #216 frame.",
    ),
    (
        WARP_LIT_SHADING_UNTRUSTED,
        "WARP lit shading is untrusted until #216 is fixed.",
    ),
    (FALLBACK_CODE, FALLBACK_TEXT),
];

fn written_artifacts(layout: &super::output::CaseOutputLayout) -> Option<ArtifactNames> {
    let names = ArtifactNames {
        actual: name_if_present(&layout.actual_png, "actual.png"),
        golden: name_if_present(&layout.golden_png_copy, "golden.png"),
        diff: name_if_present(&layout.diff_png, "diff.png"),
        side_by_side: name_if_present(
            &layout.side_by_side_png,
            super::side_by_side::SIDE_BY_SIDE_FILENAME,
        ),
    };
    let any = names.actual.is_some()
        || names.golden.is_some()
        || names.diff.is_some()
        || names.side_by_side.is_some();
    any.then_some(names)
}

fn name_if_present(path: &Path, name: &str) -> Option<String> {
    path.is_file().then(|| name.to_string())
}

fn adapter_block(actual_png: &Path) -> AdapterBlock {
    let Some(log) = host_info::renderer_log_for_output(actual_png) else {
        return unavailable_adapter();
    };
    let Some(adapter) = crate::adapter_gate::adapter_from_logs(&[&log]) else {
        return unavailable_adapter();
    };
    AdapterBlock {
        vendor: adapter.vendor,
        device: adapter.device,
        backend: adapter.backend,
        device_type: adapter.device_type,
    }
}

fn unavailable_adapter() -> AdapterBlock {
    AdapterBlock {
        vendor: None,
        device: None,
        backend: "unavailable".to_string(),
        device_type: "unavailable".to_string(),
    }
}

fn icd_from_env() -> IcdBlock {
    icd_from_lookup(|key| std::env::var(key).ok())
}

fn icd_from_lookup(lookup: impl Fn(&str) -> Option<String>) -> IcdBlock {
    for var in [VK_DRIVER_FILES, VK_ICD_FILENAMES] {
        if let Some(path) = lookup(var).filter(|value| !value.trim().is_empty()) {
            return IcdBlock {
                path: Some(path),
                source: "env".to_string(),
                var: Some(var.to_string()),
            };
        }
    }
    IcdBlock {
        path: None,
        source: "loader_default".to_string(),
        var: None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::*;
    use crate::scene_dsl::output::{CaseOutputLayout, report_from_error, write_report};

    #[test]
    fn every_harness_error_maps_to_one_reason_with_text() {
        let samples = sample_errors();
        let mut codes = Vec::new();
        for err in &samples {
            let reason = reason_for_error(err);
            assert!(!reason.text.is_empty(), "{}", reason.code);
            assert_ne!(reason.code, FALLBACK_CODE, "{}", reason.code);
            assert_eq!(reason, reason_for_message(&err.to_string()), "{err}");
            assert!(!codes.contains(&reason.code), "duplicate {}", reason.code);
            codes.push(reason.code);
        }
        assert_eq!(codes.len(), 20, "one sample per HarnessError variant");
        for (_, text) in REASONS {
            assert!(!text.is_empty());
        }
    }

    #[test]
    fn outcome_status_and_reason_cover_the_four_rulings() {
        let case = crate::scene_dsl::cases::unlit_sphere();
        let expected = labeled(false, EXPECTED_FAILURE_LABEL);
        assert_eq!(expected.status.as_deref(), Some(STATUS_EXPECTED_FAILURE));
        assert_eq!(expected.reason.as_deref(), Some(REASON_EXPECTED_FAILURE));
        assert!(!expected.reason_text.as_deref().unwrap_or("").is_empty());

        let unexpected = labeled(true, UNEXPECTED_PASS_LABEL);
        assert_eq!(unexpected.status.as_deref(), Some(STATUS_UNEXPECTED_PASS));
        assert_eq!(unexpected.reason.as_deref(), Some(REASON_UNEXPECTED_PASS));
        assert!(!unexpected.reason_text.as_deref().unwrap_or("").is_empty());

        let mut quarantined = report_from_error(&case, "flat or unstable frame");
        quarantined.status = Some(QUARANTINE_STATUS.to_string());
        quarantined.reason = Some(QUARANTINE_REASON.to_string());
        assign_outcome(&mut quarantined);
        assert_eq!(quarantined.status.as_deref(), Some(QUARANTINE_STATUS));
        assert_eq!(quarantined.reason.as_deref(), Some(QUARANTINE_REASON));
        assert!(!quarantined.reason_text.as_deref().unwrap_or("").is_empty());

        let mut early = report_from_error(&case, "flat or unstable frame");
        early.early_exit = Some(sample_exit());
        assign_outcome(&mut early);
        assert_eq!(early.status.as_deref(), Some(STATUS_EARLY_EXIT));
        assert_eq!(early.reason.as_deref(), Some(REASON_EARLY_EXIT));
        assert!(!early.reason_text.as_deref().unwrap_or("").is_empty());
    }

    #[test]
    fn emitted_json_round_trips_and_has_schema_required_keys() {
        let dir = tempfile::tempdir().expect("tempdir");
        let case = crate::scene_dsl::cases::unlit_sphere();
        let layout = CaseOutputLayout::for_case(dir.path(), &case.name);
        layout.prepare().expect("prepare");
        let mut report = report_from_error(&case, "golden image not found at missing.png");
        report.region_stats = Some(vec![sample_region()]);
        write_report(&layout, &mut report).expect("write");
        let bytes = std::fs::read(&layout.report_json).expect("read");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        let parsed: CaseReport = serde_json::from_slice(&bytes).expect("round trip parse");
        let again = serde_json::to_value(&parsed).expect("round trip write");
        assert_eq!(value, again);
        let schema = schema_document();
        assert_required(&schema, &value);
        assert_eq!(value["schema"], SCHEMA);
        assert_eq!(value["reason"], "golden-missing");
        assert!(!value["reason_text"].as_str().unwrap_or("").is_empty());
        assert!(value.get("artifacts").is_none(), "no image was written");
        assert_eq!(value["region_stats"][0]["column"], 0);
        assert_eq!(value["region_stats"][0]["non_dominant"], 3);
        assert!(schema["properties"].get("region_stats").is_some());
        assert!(value["runner"]["adapter"]["vendor"].is_null());
        assert!(value["runner"]["adapter"]["device"].is_null());
        assert!(value["runner"]["icd"]["source"].is_string());
        for status in [
            STATUS_EXPECTED_FAILURE,
            STATUS_UNEXPECTED_PASS,
            QUARANTINE_STATUS,
            STATUS_EARLY_EXIT,
        ] {
            assert!(
                schema["properties"]["status"]["enum"]
                    .as_array()
                    .is_some_and(|values| values.iter().any(|item| item == status)),
                "{status}"
            );
        }
        for reason in [
            REASON_EXPECTED_FAILURE,
            REASON_UNEXPECTED_PASS,
            QUARANTINE_REASON,
            REASON_EARLY_EXIT,
        ] {
            assert!(
                schema["properties"]["reason"]["enum"]
                    .as_array()
                    .is_some_and(|values| values.iter().any(|item| item == reason)),
                "{reason}"
            );
        }
    }

    fn labeled(passed: bool, label: &str) -> CaseReport {
        let case = crate::scene_dsl::cases::unlit_sphere();
        let mut report = report_from_error(&case, label);
        report.passed = passed;
        report.result = Some(label.to_string());
        assign_outcome(&mut report);
        report
    }

    #[test]
    fn v2_early_exit_keeps_settle_time_and_the_k_window() {
        let dir = tempfile::tempdir().expect("tempdir");
        let case = crate::scene_dsl::cases::unlit_sphere();
        let layout = CaseOutputLayout::for_case(dir.path(), &case.name);
        layout.prepare().expect("prepare");
        let mut report = report_from_error(&case, "flat or unstable frame");
        report.early_exit = Some(sample_exit());
        write_report(&layout, &mut report).expect("write");
        let bytes = std::fs::read(&layout.report_json).expect("read");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        let parsed: CaseReport = serde_json::from_slice(&bytes).expect("round trip parse");
        let again = serde_json::to_value(&parsed).expect("round trip write");
        assert_eq!(value, again);
        assert_required(&schema_document(), &value);
        for slot in ["early_exit", "capture"] {
            let exit = if slot == "capture" {
                &value["capture"]["early_exit"]
            } else {
                &value[slot]
            };
            assert_eq!(exit["settle_ms"], 4_000, "{slot}");
            assert_eq!(exit["k_window_ms"], 8_500, "{slot}");
            assert_eq!(exit["elapsed_ms"], 12_500, "{slot}");
        }
    }

    fn sample_exit() -> EarlyExit {
        EarlyExit {
            case: "pbs_lit_material_matrix".to_string(),
            k: 3,
            elapsed_ms: 12_500,
            settle_ms: 4_000,
            k_window_ms: 8_500,
            reason: "flat_stable".to_string(),
        }
    }

    fn sample_region() -> super::super::region_stats::RegionStatRow {
        super::super::region_stats::RegionStatRow {
            column: 0,
            row: 1,
            x: 0,
            y: 2,
            width: 4,
            height: 4,
            mean: [1.0, 2.0, 3.0, 4.0],
            variance: [0.0, 0.0, 0.0, 0.0],
            non_dominant: 3,
            mean_delta: 0.0,
            variance_delta: 0.0,
            non_dominant_delta: 0,
        }
    }

    fn schema_document() -> serde_json::Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/schemas/renderide-test.v2.json");
        let bytes =
            std::fs::read(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        serde_json::from_slice(&bytes).expect("schema json")
    }

    fn assert_required(schema: &serde_json::Value, instance: &serde_json::Value) {
        let Some(required) = schema.get("required").and_then(|value| value.as_array()) else {
            return;
        };
        for key in required {
            let Some(name) = key.as_str() else {
                continue;
            };
            assert!(instance.get(name).is_some(), "missing required key {name}");
        }
        let Some(properties) = schema.get("properties").and_then(|value| value.as_object()) else {
            return;
        };
        for (name, child_schema) in properties {
            let Some(child) = instance.get(name) else {
                continue;
            };
            if child.is_object() {
                assert_required(child_schema, child);
            } else if let Some(items) = child.as_array()
                && let Some(item_schema) = child_schema.get("items")
            {
                for item in items {
                    if item.is_object() {
                        assert_required(item_schema, item);
                    }
                }
            }
        }
    }

    fn sample_errors() -> Vec<HarnessError> {
        let path = PathBuf::from("x");
        let wait = Duration::from_secs(1);
        let image_err = || image::ImageError::IoError(std::io::Error::other("io"));
        vec![
            HarnessError::RendererBinaryMissing(path.clone()),
            HarnessError::SpawnRenderer(std::io::Error::other("spawn")),
            HarnessError::QueueOptions("bad".to_string()),
            HarnessError::HandshakeTimeout(wait),
            HarnessError::AssetAckTimeout(wait, "mesh"),
            HarnessError::PngOutputMissing {
                path: path.clone(),
                wait,
            },
            HarnessError::PipelineNotReady {
                path: path.clone(),
                wait,
            },
            HarnessError::FlatOrUnstableFrame {
                path: path.clone(),
                wait,
                last_pending: 0,
                ready_chunks: 0,
                pending_chunks: 0,
                missing_chunks: 0,
                frame_count: 0,
                frame_pending: None,
                gpu_done: false,
                dropped_draws: None,
            },
            HarnessError::PngRead {
                path: path.clone(),
                source: image_err(),
            },
            HarnessError::PngWrite {
                path: path.clone(),
                source: image_err(),
            },
            HarnessError::FlatImage {
                path: path.clone(),
                color: [0, 0, 0, 255],
            },
            HarnessError::GoldenMissing(path.clone()),
            HarnessError::GoldenMismatch {
                score: 0.1,
                threshold: 0.9,
                diff_path: path.clone(),
            },
            HarnessError::Io(std::io::Error::other("disk")),
            HarnessError::GltfFixture {
                path: path.clone(),
                message: "missing".to_string(),
            },
            HarnessError::ImageCompare("rank".to_string()),
            HarnessError::UnknownCase("missing".to_string()),
            HarnessError::BenchReportMissing {
                path: path.clone(),
                wait,
                renderer_state: "exited".to_string(),
            },
            HarnessError::UnknownBenchScenario("nope".to_string()),
            HarnessError::SuiteFailed {
                failed: 1,
                total: 2,
                report_path: path,
            },
        ]
    }
}
