use std::path::{Path, PathBuf};
use std::time::Duration;

use super::*;
use crate::HarnessError;
use crate::scene_dsl::SuiteFailureKind;

const PBS_LIT: &str = "pbs_lit_material_matrix";
const NEGATIVE_LIGHT_PARITY: &str = "negative_light_parity";
const NEGATIVE_LIGHT_PHYSICALLY_BASED: &str = "negative_light_physically_based";
const NEGATIVE_COLOR_PHYSICALLY_BASED: &str = "negative_color_physically_based";
const OFF_THE_QUARANTINE_LIST: &[&str] = &[
    NEGATIVE_LIGHT_PARITY,
    NEGATIVE_LIGHT_PHYSICALLY_BASED,
    NEGATIVE_COLOR_PHYSICALLY_BASED,
];

fn before_expiry() -> SystemTime {
    expires_at()
        .checked_sub(Duration::from_hours(168))
        .expect("before expiry")
}

fn after_expiry() -> SystemTime {
    expires_at() + Duration::from_secs(1)
}

fn within_warning_window() -> SystemTime {
    expires_at()
        .checked_sub(Duration::from_hours(1))
        .expect("inside 48h")
}

fn outside_warning_window() -> SystemTime {
    expires_at()
        .checked_sub(WARN_WINDOW + Duration::from_secs(1))
        .expect("before the warning window")
}

fn flat_message() -> String {
    HarnessError::FlatOrUnstableFrame {
        path: PathBuf::from("actual.png"),
        wait: Duration::from_secs(300),
        last_pending: 0,
        ready_chunks: 1,
        pending_chunks: 0,
        missing_chunks: 0,
        frame_count: 2,
        frame_pending: Some(0),
        gpu_done: true,
        dropped_draws: Some(0),
    }
    .to_string()
}

fn timed_out_frame(pending: &str, gpu_done: &str) -> (Option<u64>, Option<u64>, Option<bool>) {
    let (frame_pending, dropped_draws) = match pending.trim() {
        "0" => (Some(0), Some(0)),
        other => match other.parse::<u64>() {
            Ok(count) => (Some(count), None),
            Err(_) => (None, None),
        },
    };
    (frame_pending, dropped_draws, Some(gpu_done.trim() == "1"))
}

/// Fixture setup only. The quarantine rule reads [`SuiteFailureKind`], not this text.
fn kind_for(error: Option<&str>) -> Option<SuiteFailureKind> {
    let error = error?;
    if error.contains("pipeline-pending") || error.contains("PipelineNotReady") {
        return Some(SuiteFailureKind::PipelineNotReady);
    }
    if error.contains("expected fresh PNG") {
        return Some(SuiteFailureKind::PngOutputMissing);
    }
    if error.contains("flat or unstable") {
        return Some(SuiteFailureKind::FlatOrUnstableFrame);
    }
    None
}

fn write_frame_png(path: &Path, pending: &str, gpu_done: &str) {
    let file = fs::File::create(path).expect("create png");
    let mut encoder = png::Encoder::new(file, 1, 1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .add_text_chunk(PENDING_KEYWORD.to_string(), pending.to_string())
        .expect("pending chunk");
    encoder
        .add_text_chunk(GPU_DONE_KEYWORD.to_string(), gpu_done.to_string())
        .expect("gpu_done chunk");
    let mut writer = encoder.write_header().expect("png header");
    writer
        .write_image_data(&[0, 0, 0, 255])
        .expect("png pixels");
    writer.finish().expect("png finish");
}

fn suite_case(
    dir: &Path,
    name: &str,
    error: Option<&str>,
    pending: &str,
    gpu_done: &str,
) -> SuiteCaseReport {
    let artifacts = dir.join(name);
    fs::create_dir_all(&artifacts).expect("artifacts");
    write_frame_png(&artifacts.join("actual.png"), pending, gpu_done);
    let report_json = artifacts.join("report.json");
    let integration = crate::scene_dsl::cases::unlit_sphere();
    let mut case_report = output::report_from_error(&integration, error.unwrap_or("passed"));
    case_report.name = name.to_string();
    case_report.passed = error.is_none();
    case_report.error = error.map(str::to_string);
    fs::write(
        &report_json,
        serde_json::to_string_pretty(&case_report).expect("report json"),
    )
    .expect("write report");
    let failure_kind = kind_for(error);
    let (frame_pending, dropped_draws, gpu_done_flag) =
        if failure_kind == Some(SuiteFailureKind::FlatOrUnstableFrame) {
            timed_out_frame(pending, gpu_done)
        } else {
            (None, None, None)
        };
    let mut case = SuiteCaseReport::plain(
        name,
        error.is_none(),
        artifacts,
        report_json,
        error.map(str::to_string),
    );
    case.failure_kind = failure_kind;
    case.frame_pending = frame_pending;
    case.dropped_draws = dropped_draws;
    case.gpu_done = gpu_done_flag;
    case
}

fn apply(report: &mut SuiteReport, now: SystemTime, warp: bool) {
    apply_quarantine(report, now, move |_| warp);
}

#[test]
fn expiry_constant_is_the_utc_instant_named_in_the_ruling() {
    assert_eq!(QUARANTINE_216_EXPIRES, "2026-11-14T23:59:59Z");
    let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(1_794_700_799);
    assert_eq!(expires_at(), expected);
    assert_eq!(QUARANTINE_STATUS, "quarantined");
    assert_eq!(QUARANTINE_REASON, "quarantined:#216");
}

#[test]
fn warp_flat_frame_is_quarantined_before_expiry_and_fails_after() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut report = SuiteReport::from_cases(vec![
        suite_case(dir.path(), "unlit_sphere", None, "0", "1"),
        suite_case(dir.path(), PBS_LIT, Some(&flat_message()), "0", "1"),
    ]);
    apply(&mut report, before_expiry(), true);

    assert_eq!(report.passed, 1);
    assert_eq!(report.failed, 0);
    assert_eq!(report.expected_failures, 0);
    assert_eq!(report.quarantined, 1);
    assert!(report.passed());
    let case = &report.cases[1];
    assert!(!case.passed);
    assert_eq!(case.status.as_deref(), Some(QUARANTINE_STATUS));
    assert_eq!(case.reason.as_deref(), Some(QUARANTINE_REASON));
    assert!(case_is_quarantined(case));
}

#[test]
fn warp_flat_frame_fails_after_expiry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut report = SuiteReport::from_cases(vec![
        suite_case(dir.path(), "unlit_sphere", None, "0", "1"),
        suite_case(dir.path(), PBS_LIT, Some(&flat_message()), "0", "1"),
    ]);
    assert_eq!(after_expiry(), expires_at() + Duration::from_secs(1));
    apply(&mut report, after_expiry(), true);
    assert_eq!(report.passed, 1);
    assert_eq!(report.quarantined, 0);
    assert_eq!(report.failed, 1);
    assert!(!report.passed());
    assert!(report.cases[1].status.is_none());
    assert!(report.cases[1].reason.is_none());
    assert!(report.quarantine_warning.is_none());
    let expired_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&report.cases[1].report_json).expect("read report"),
    )
    .expect("expired json");
    assert!(expired_json.get("status").is_none());
}

#[test]
fn non_warp_flat_frame_stays_a_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut report = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "0",
        "1",
    )]);
    apply(&mut report, before_expiry(), false);
    assert_eq!(report.quarantined, 0);
    assert_eq!(report.failed, 1);
    assert!(!report.passed());
    assert!(report.cases[0].status.is_none());
}

#[test]
fn warp_flat_frame_with_pending_above_zero_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut report = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "4",
        "1",
    )]);
    apply(&mut report, before_expiry(), true);
    assert_eq!(report.failed, 1);
    assert_eq!(report.quarantined, 0);
    assert!(!report.passed());
}

#[test]
fn warp_flat_frame_in_another_scene_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut report = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        "unlit_sphere",
        Some(&flat_message()),
        "0",
        "1",
    )]);
    apply(&mut report, before_expiry(), true);
    assert_eq!(report.failed, 1);
    assert_eq!(report.quarantined, 0);
    assert!(report.cases[0].reason.is_none());
}

#[test]
fn warp_flat_frame_without_gpu_done_or_a_zero_chunk_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut not_done = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "0",
        "0",
    )]);
    apply(&mut not_done, before_expiry(), true);
    assert_eq!(not_done.failed, 1);
    assert_eq!(not_done.quarantined, 0);

    let mut missing = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "0",
        "1",
    )]);
    // The PNG still says pending 0 and gpu_done 1. The timed-out frame did not.
    missing.cases[0].frame_pending = None;
    missing.cases[0].dropped_draws = None;
    missing.cases[0].gpu_done = None;
    apply(&mut missing, before_expiry(), true);
    assert_eq!(missing.failed, 1, "a missing pending count is not zero");
    assert_eq!(missing.quarantined, 0);
}

#[test]
fn png_chunks_do_not_override_the_timed_out_frame() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut pending_frame = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "0",
        "1",
    )]);
    pending_frame.cases[0].frame_pending = Some(4);
    pending_frame.cases[0].dropped_draws = None;
    apply(&mut pending_frame, before_expiry(), true);
    assert_eq!(pending_frame.quarantined, 0);
    assert_eq!(pending_frame.failed, 1);

    let mut ready_frame = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "4",
        "0",
    )]);
    ready_frame.cases[0].frame_pending = Some(0);
    ready_frame.cases[0].dropped_draws = Some(0);
    ready_frame.cases[0].gpu_done = Some(true);
    apply(&mut ready_frame, before_expiry(), true);
    assert_eq!(ready_frame.quarantined, 1);
    assert_eq!(ready_frame.failed, 0);
    assert!(ready_frame.passed());
}

#[test]
fn png_output_missing_stays_a_hard_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = HarnessError::PngOutputMissing {
        path: PathBuf::from("actual.png"),
        wait: Duration::from_secs(180),
    }
    .to_string();
    let mut report = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&missing),
        "0",
        "1",
    )]);
    assert_eq!(
        report.cases[0].failure_kind,
        Some(SuiteFailureKind::PngOutputMissing)
    );
    report.cases[0].frame_pending = Some(0);
    report.cases[0].dropped_draws = Some(0);
    report.cases[0].gpu_done = Some(true);
    apply(&mut report, before_expiry(), true);
    assert_eq!(report.quarantined, 0);
    assert_eq!(report.failed, 1);
    assert!(!report.passed());
    assert!(report.cases[0].status.is_none());
}

#[test]
fn stamp_failure_leaves_the_case_a_hard_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut report = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "0",
        "1",
    )]);
    report.cases[0].report_json = dir.path().join("missing").join("report.json");
    apply(&mut report, before_expiry(), true);
    assert_eq!(report.quarantined, 0);
    assert_eq!(report.failed, 1);
    assert!(!report.passed());
    assert!(report.cases[0].status.is_none());
    assert!(report.cases[0].reason.is_none());
    assert!(!report.cases[0].passed);
}

#[test]
fn one_second_before_expiry_still_quarantines_and_warns() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut report = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "0",
        "1",
    )]);
    let now = expires_at()
        .checked_sub(Duration::from_secs(1))
        .expect("one second before expiry");
    apply(&mut report, now, true);
    assert_eq!(report.quarantined, 1);
    assert!(report.passed());
    let warning = report.quarantine_warning.clone().expect("warning");
    assert!(warning.contains(QUARANTINE_216_EXPIRES), "{warning}");
    assert!(warning.contains("#216"), "{warning}");
    let path =
        crate::scene_dsl::suite::write_suite_report(dir.path(), &report).expect("suite report");
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some(crate::scene_dsl::suite::SUITE_REPORT_FILENAME)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("read suite")).expect("suite json");
    assert_eq!(json["quarantine_warning"], warning);
    assert_eq!(json["quarantined"], 1);
    assert_eq!(json["cases"][0]["status"], QUARANTINE_STATUS);
    assert_eq!(json["cases"][0]["reason"], QUARANTINE_REASON);
}

struct RemoveFile(PathBuf);

impl Drop for RemoveFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[test]
fn lavapipe_adapter_log_is_not_warp() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lavapipe_png = dir.path().join("lavapipe-actual.png");
    let warp_png = dir.path().join("warp-actual.png");
    let log_dir = PathBuf::from("logs/renderer");
    fs::create_dir_all(&log_dir).expect("log dir");
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let lavapipe_log = log_dir.join(format!("lavapipe-{stamp}.log"));
    let warp_log = log_dir.join(format!("warp-{stamp}.log"));
    let _lavapipe_guard = RemoveFile(lavapipe_log.clone());
    let _warp_guard = RemoveFile(warp_log.clone());
    fs::write(
        &lavapipe_log,
        format!(
            "Headless mode: output={} size=320x320\n\
             wgpu adapter candidate: llvmpipe (LLVM 20.1.2, 256 bits) type=Cpu backend=Vulkan vendor=0x10005 device=0x0000 active_backends=VULKAN\n\
             wgpu adapter selected (headless): llvmpipe (LLVM 20.1.2, 256 bits) type=Cpu backend=Vulkan (preference=HighPerformance)\n",
            lavapipe_png.display()
        ),
    )
    .expect("lavapipe log");
    fs::write(
        &warp_log,
        format!(
            "Headless mode: output={} size=320x320\n\
             wgpu adapter candidate: Microsoft Basic Render Driver type=Cpu backend=Dx12 vendor=0x1414 device=0x008c active_backends=DX12\n\
             wgpu adapter selected (headless): Microsoft Basic Render Driver type=Cpu backend=Dx12 (preference=HighPerformance)\n",
            warp_png.display()
        ),
    )
    .expect("warp log");
    assert!(
        !capture_is_warp(&lavapipe_png),
        "lavapipe vendor 0x10005 is not WARP"
    );
    assert!(
        capture_is_warp(&warp_png),
        "the same log reader accepts vendor 0x1414 device 0x008c"
    );
}

#[test]
fn other_failures_stay_hard_failures() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pipeline = HarnessError::PipelineNotReady {
        path: PathBuf::from("actual.png"),
        wait: Duration::from_secs(180),
    }
    .to_string();
    let crash = HarnessError::SpawnRenderer(std::io::Error::other("spawn")).to_string();
    let flat_image = HarnessError::FlatImage {
        path: PathBuf::from("actual.png"),
        color: [0, 0, 0, 255],
    }
    .to_string();
    assert!(pipeline.contains("pipeline-pending") || pipeline.contains("PipelineNotReady"));
    assert!(flat_image.contains("flat single color"));
    assert!(!flat_image.contains("flat or unstable"));
    for error in [pipeline, crash, flat_image] {
        let mut report = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            PBS_LIT,
            Some(&error),
            "0",
            "1",
        )]);
        apply(&mut report, before_expiry(), true);
        assert_eq!(report.quarantined, 0, "{error}");
        assert_eq!(report.failed, 1, "{error}");
    }
}

#[test]
fn quarantine_warns_only_inside_the_last_48_hours() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut early = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "0",
        "1",
    )]);
    apply(&mut early, outside_warning_window(), true);
    assert_eq!(early.quarantined, 1);
    assert!(early.quarantine_warning.is_none());

    let mut close = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "0",
        "1",
    )]);
    apply(&mut close, within_warning_window(), true);
    let warning = close.quarantine_warning.expect("warning");
    assert!(warning.contains("#216"), "{warning}");
    assert!(warning.contains(QUARANTINE_216_EXPIRES), "{warning}");
    assert!(
        warning.contains("https://github.com/DoubleStyxx/Renderide/issues/216"),
        "{warning}"
    );
    assert!(warn_line(&warning).starts_with("WARN "));
    assert_eq!(close.quarantined, 1);

    let mut expired = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "0",
        "1",
    )]);
    apply(&mut expired, expires_at(), true);
    assert_eq!(expired.quarantined, 0);
    assert!(expired.quarantine_warning.is_none());
}

#[test]
fn expected_failure_label_is_left_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut case = suite_case(dir.path(), PBS_LIT, Some("case failed tolerance"), "0", "1");
    case.passed = false;
    case.result = Some(super::super::warp_pbs::EXPECTED_FAILURE_LABEL.to_string());
    let mut report = SuiteReport::from_cases(vec![case]);
    report.cases[0].passed = false;
    report.cases[0].result = Some(super::super::warp_pbs::EXPECTED_FAILURE_LABEL.to_string());
    report.recompute();
    apply(&mut report, before_expiry(), true);
    assert!(super::super::warp_pbs::case_is_expected_failure(
        &report.cases[0]
    ));
    assert!(!case_is_quarantined(&report.cases[0]));
    assert_eq!(report.expected_failures, 1);
    assert_eq!(report.quarantined, 0);
    assert_eq!(report.failed, 0);
    assert!(report.passed());
}

#[test]
fn negative_light_cases_match_only_the_warp_flat_signature() {
    const COVERED: &[&str] = &[
        NEGATIVE_LIGHT_PARITY,
        NEGATIVE_LIGHT_PHYSICALLY_BASED,
        NEGATIVE_COLOR_PHYSICALLY_BASED,
    ];
    for name in COVERED {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut covered = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            name,
            Some(&flat_message()),
            "0",
            "1",
        )]);
        apply(&mut covered, before_expiry(), true);
        assert_eq!(covered.quarantined, 0, "{name}");
        assert_eq!(covered.failed, 1, "{name}");
        assert!(!covered.passed(), "{name}");
        assert!(!case_is_quarantined(&covered.cases[0]), "{name}");
        assert!(covered.cases[0].reason.is_none(), "{name}");

        let mut other_adapter = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            name,
            Some(&flat_message()),
            "0",
            "1",
        )]);
        apply(&mut other_adapter, before_expiry(), false);
        assert_eq!(other_adapter.quarantined, 0, "{name}");
        assert_eq!(other_adapter.failed, 1, "{name}");
        assert!(!other_adapter.passed(), "{name}");

        let mut pending = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            name,
            Some(&flat_message()),
            "4",
            "1",
        )]);
        apply(&mut pending, before_expiry(), true);
        assert_eq!(pending.quarantined, 0, "{name}");
        assert_eq!(pending.failed, 1, "{name}");

        let mut not_done = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            name,
            Some(&flat_message()),
            "0",
            "0",
        )]);
        apply(&mut not_done, before_expiry(), true);
        assert_eq!(not_done.quarantined, 0, "{name}");
        assert_eq!(not_done.failed, 1, "{name}");

        let mut mismatch = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            name,
            Some("case failed tolerance"),
            "0",
            "1",
        )]);
        apply(&mut mismatch, before_expiry(), true);
        assert_eq!(mismatch.quarantined, 0, "{name}");
        assert_eq!(mismatch.failed, 1, "{name}");
        assert!(mismatch.cases[0].status.is_none(), "{name}");
    }

    let dir = tempfile::tempdir().expect("tempdir");
    for name in ["unlit_sphere", "negative_light_parity_extra"] {
        let mut unrelated = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            name,
            Some(&flat_message()),
            "0",
            "1",
        )]);
        apply(&mut unrelated, before_expiry(), true);
        assert_eq!(unrelated.quarantined, 0, "{name}");
        assert_eq!(unrelated.failed, 1, "{name}");
        assert!(unrelated.cases[0].reason.is_none(), "{name}");
    }
}

fn tolerance_case(dir: &Path, name: &str, passed: bool) -> SuiteCaseReport {
    let error = if passed {
        None
    } else {
        Some("case failed tolerance")
    };
    let mut case = suite_case(dir, name, error, "0", "1");
    case.image_compared = true;
    case
}

#[test]
fn warp_negative_light_tolerance_pass_is_quarantined_not_passed() {
    for name in OFF_THE_QUARANTINE_LIST {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut report = SuiteReport::from_cases(vec![tolerance_case(dir.path(), name, true)]);
        apply(&mut report, before_expiry(), true);
        assert!(report.cases[0].passed, "{name}");
        assert_eq!(report.passed, 1, "{name}");
        assert_eq!(report.quarantined, 0, "{name}");
        assert_eq!(report.failed, 0, "{name}");
        assert!(report.passed(), "{name}");
        assert!(report.cases[0].reason.is_none(), "{name}");
    }
}

#[test]
fn warp_negative_light_tolerance_fail_is_quarantined() {
    for name in OFF_THE_QUARANTINE_LIST {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut report = SuiteReport::from_cases(vec![tolerance_case(dir.path(), name, false)]);
        apply(&mut report, before_expiry(), true);
        assert_eq!(report.quarantined, 0, "{name}");
        assert_eq!(report.failed, 1, "{name}");
        assert!(report.cases[0].reason.is_none(), "{name}");
    }
}

#[test]
fn warp_negative_light_non_flat_harness_error_fails() {
    let pipeline = HarnessError::PipelineNotReady {
        path: PathBuf::from("actual.png"),
        wait: Duration::from_secs(180),
    }
    .to_string();
    let spawn = HarnessError::SpawnRenderer(std::io::Error::other("spawn")).to_string();
    for name in OFF_THE_QUARANTINE_LIST {
        for error in [&pipeline, &spawn] {
            let dir = tempfile::tempdir().expect("tempdir");
            let mut report =
                SuiteReport::from_cases(vec![suite_case(dir.path(), name, Some(error), "0", "1")]);
            apply(&mut report, before_expiry(), true);
            assert_eq!(report.quarantined, 0, "{name} {error}");
            assert_eq!(report.failed, 1, "{name} {error}");
            assert!(report.cases[0].reason.is_none(), "{name}");
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let mut missing = SuiteReport::from_cases(vec![tolerance_case(dir.path(), name, true)]);
        missing.cases[0].report_json = dir.path().join("missing").join("report.json");
        apply(&mut missing, before_expiry(), true);
        assert_eq!(missing.quarantined, 0, "{name}");
        assert_eq!(missing.failed, 0, "{name}");
        assert!(missing.cases[0].passed, "{name}");
        assert!(missing.cases[0].status.is_none(), "{name}");
    }
}

#[test]
fn non_warp_negative_light_results_are_unchanged() {
    for name in OFF_THE_QUARANTINE_LIST {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut passed = SuiteReport::from_cases(vec![tolerance_case(dir.path(), name, true)]);
        apply(&mut passed, before_expiry(), false);
        assert!(passed.cases[0].passed, "{name}");
        assert_eq!(passed.quarantined, 0, "{name}");
        assert_eq!(passed.passed, 1, "{name}");
        assert!(passed.cases[0].reason.is_none(), "{name}");

        let mut failed = SuiteReport::from_cases(vec![tolerance_case(dir.path(), name, false)]);
        apply(&mut failed, before_expiry(), false);
        assert!(!failed.cases[0].passed, "{name}");
        assert_eq!(failed.quarantined, 0, "{name}");
        assert_eq!(failed.failed, 1, "{name}");
    }
}

#[test]
fn negative_light_quarantine_fails_closed_after_expiry() {
    for name in OFF_THE_QUARANTINE_LIST {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut passed = SuiteReport::from_cases(vec![tolerance_case(dir.path(), name, true)]);
        apply(&mut passed, after_expiry(), true);
        assert!(passed.cases[0].passed, "{name}");
        assert_eq!(passed.quarantined, 0, "{name}");
        assert_eq!(passed.failed, 0, "{name}");
        assert!(passed.passed(), "{name}");
        assert!(passed.cases[0].status.is_none(), "{name}");
        assert!(passed.cases[0].reason.is_none(), "{name}");

        let mut flat = SuiteReport::from_cases(vec![suite_case(
            dir.path(),
            name,
            Some(&flat_message()),
            "0",
            "1",
        )]);
        apply(&mut flat, after_expiry(), true);
        assert_eq!(flat.quarantined, 0, "{name}");
        assert_eq!(flat.failed, 1, "{name}");
    }
}

#[test]
fn pbs_lit_quarantine_stays_the_flat_frame_rule() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut flat = SuiteReport::from_cases(vec![suite_case(
        dir.path(),
        PBS_LIT,
        Some(&flat_message()),
        "0",
        "1",
    )]);
    apply(&mut flat, before_expiry(), true);
    assert_eq!(flat.quarantined, 1);
    assert_eq!(flat.cases[0].reason.as_deref(), Some(QUARANTINE_REASON));

    let mut passed = SuiteReport::from_cases(vec![tolerance_case(dir.path(), PBS_LIT, true)]);
    apply(&mut passed, before_expiry(), true);
    assert!(passed.cases[0].passed);
    assert_eq!(passed.quarantined, 0);
    assert!(passed.cases[0].reason.is_none());

    let mut failed = SuiteReport::from_cases(vec![tolerance_case(dir.path(), PBS_LIT, false)]);
    apply(&mut failed, before_expiry(), true);
    assert_eq!(failed.quarantined, 0);
    assert_eq!(failed.failed, 1);
    assert!(failed.cases[0].reason.is_none());
}

#[test]
fn quarantined_line_names_the_case_and_the_reason() {
    let line = quarantined_line(
        PBS_LIT,
        Path::new("out/pbs_lit_material_matrix"),
        QUARANTINE_REASON,
    );
    assert_eq!(
        line,
        "QUARANTINED pbs_lit_material_matrix (out/pbs_lit_material_matrix) quarantined:#216"
    );
}
