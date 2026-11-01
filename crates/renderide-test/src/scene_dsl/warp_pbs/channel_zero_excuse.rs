//! Gates on the channel-zero and flat-timeout arms of `excuse`.
//!
//! `pbs_specular_opaque` is the only name on `WARP_EXPECTED_FAILURE_CASES`.

use super::{CaseFault, CaseSnapshot, ImageSignature};

const LISTED: &str = "pbs_specular_opaque";

fn listed_channel_zero() -> CaseSnapshot<'static> {
    CaseSnapshot {
        name: LISTED,
        fault: CaseFault::ImageCheck,
        frame_accepted: true,
        pending: Some(0),
        dropped_draws: Some(0),
        signature: ImageSignature {
            channel_zero_failed: true,
            ..ImageSignature::default()
        },
    }
}

fn listed_flat() -> CaseSnapshot<'static> {
    CaseSnapshot {
        name: LISTED,
        fault: CaseFault::FlatOrUnstable,
        frame_accepted: false,
        pending: None,
        dropped_draws: None,
        signature: ImageSignature::default(),
    }
}

#[test]
fn clean_listed_channel_zero_frame_is_excused() {
    let case = listed_channel_zero();
    // `LISTED` is on the list on this base. An empty list does not excuse it.
    assert_eq!(
        super::excuse(true, &case),
        super::WARP_EXPECTED_FAILURE_CASES.contains(&LISTED)
    );
}

#[test]
fn listed_channel_zero_foreign_failure_is_not_excused() {
    let mut case = listed_channel_zero();
    case.signature.foreign_failure = true;
    assert!(
        !super::excuse(true, &case),
        "a foreign failure must not be excused"
    );
}

#[test]
fn listed_channel_zero_unaccepted_frame_is_not_excused() {
    let mut case = listed_channel_zero();
    case.frame_accepted = false;
    assert!(
        !super::excuse(true, &case),
        "an unaccepted frame must not be excused"
    );
}

#[test]
fn listed_channel_zero_dropped_draw_is_not_excused() {
    let mut case = listed_channel_zero();
    case.dropped_draws = Some(1);
    assert!(
        !super::excuse(true, &case),
        "a dropped draw must not be excused"
    );
}

#[test]
fn listed_channel_zero_nonzero_pending_is_not_excused() {
    let mut case = listed_channel_zero();
    case.pending = Some(1);
    assert!(
        !super::excuse(true, &case),
        "pending != 0 must not be excused"
    );
}

#[test]
fn non_warp_and_unlisted_channel_zero_are_not_excused() {
    let clean = listed_channel_zero();
    assert!(!super::excuse(false, &clean));
    let mut unlisted = clean;
    unlisted.name = super::PBS_LIT;
    assert!(!super::excuse(true, &unlisted));
    let mut other = clean;
    other.name = "unlit_sphere";
    assert!(!super::excuse(true, &other));
}

#[test]
fn listed_flat_timeout_without_an_accepted_frame_is_excused() {
    let case = listed_flat();
    assert!(!case.frame_accepted);
    assert_eq!(
        super::excuse(true, &case),
        super::WARP_EXPECTED_FAILURE_CASES.contains(&LISTED)
    );
    assert!(!super::excuse(false, &case));
}

#[test]
fn listed_flat_timeout_foreign_failure_is_not_excused() {
    let mut case = listed_flat();
    case.signature.foreign_failure = true;
    assert!(
        !super::excuse(true, &case),
        "a foreign failure on a flat timeout must not be excused"
    );
}

#[test]
fn listed_flat_timeout_dropped_draw_is_not_excused() {
    let mut case = listed_flat();
    case.pending = Some(0);
    case.dropped_draws = Some(1);
    assert!(
        !super::excuse(true, &case),
        "a dropped draw on a flat timeout must not be excused"
    );
}

#[test]
fn listed_flat_timeout_nonzero_pending_is_not_excused() {
    let mut case = listed_flat();
    case.pending = Some(1);
    case.dropped_draws = Some(0);
    assert!(
        !super::excuse(true, &case),
        "pending != 0 on a flat timeout must not be excused"
    );
}

#[test]
fn listed_four_band_mismatch_is_still_excused_only_when_clean() {
    let mut case = listed_channel_zero();
    case.signature = ImageSignature {
        region_color_bands: 4,
        region_color_failed: 4,
        ..ImageSignature::default()
    };
    assert_eq!(
        super::excuse(true, &case),
        super::WARP_EXPECTED_FAILURE_CASES.contains(&LISTED)
    );
    case.pending = Some(1);
    assert!(!super::excuse(true, &case));
    case.pending = Some(0);
    case.dropped_draws = None;
    assert!(
        !super::excuse(true, &case),
        "the four-band arm still requires a recorded zero drop count"
    );
}

#[test]
fn flat_suite_dropped_draws_reach_the_excuse() {
    let mut case = crate::scene_dsl::suite::SuiteCaseReport::plain(
        LISTED,
        false,
        std::path::PathBuf::from("artifacts"),
        std::path::PathBuf::from("report.json"),
        Some("flat or unstable frame".to_string()),
    );
    case.frame_pending = Some(0);
    case.dropped_draws = Some(1);
    let snapshot = super::snapshot_for(&case);
    assert!(
        !super::excuse(true, &snapshot),
        "a suite flat timeout with a dropped draw must not be excused"
    );
    assert_eq!(snapshot.pending, Some(0));
    assert_eq!(snapshot.dropped_draws, Some(1));
    assert!(!snapshot.frame_accepted);
}

#[test]
fn a_warp_pass_is_labeled_and_does_not_fail_the_suite() {
    let pass = CaseSnapshot {
        name: super::PBS_LIT,
        fault: CaseFault::None,
        frame_accepted: true,
        pending: Some(0),
        dropped_draws: Some(0),
        signature: ImageSignature::default(),
    };
    assert!(!super::unexpected_pass(true, &pass));
    assert!(!super::unexpected_pass(false, &pass));
    assert!(!super::excuse(true, &pass));
    let mut other = pass;
    other.name = "unlit_sphere";
    assert!(!super::unexpected_pass(true, &other));

    let dir = tempfile::tempdir().expect("tempdir");
    let report_path = dir.path().join("report.json");
    let integration = crate::scene_dsl::cases::pbs_lit_material_matrix();
    let mut case_report = super::output::report_from_evaluation(
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
    std::fs::write(
        &report_path,
        serde_json::to_string_pretty(&case_report).expect("json"),
    )
    .expect("write");
    super::stamp_outcome(&report_path, true, super::UNEXPECTED_PASS_LABEL).expect("stamp");
    let text = std::fs::read_to_string(&report_path).expect("read");
    assert!(text.contains(super::UNEXPECTED_PASS_LABEL), "{text}");
    assert!(text.contains("\"passed\": true"), "{text}");

    let mut case = crate::scene_dsl::suite::SuiteCaseReport::plain(
        super::PBS_LIT,
        true,
        dir.path().to_path_buf(),
        report_path,
        None,
    );
    case.result = Some(super::UNEXPECTED_PASS_LABEL.to_string());
    let suite = crate::scene_dsl::suite::SuiteReport::from_cases(vec![case]);
    assert!(suite.passed());
    assert_eq!(suite.failed, 0);
    assert_eq!(suite.passed, 1);
    assert_eq!(suite.unexpected_passes, 1);
    assert_eq!(suite.expected_failures, 0);
    assert!(super::case_is_unexpected_pass(&suite.cases[0]));
    let line = super::unexpected_pass_line(super::PBS_LIT, dir.path());
    assert!(line.starts_with("UNEXPECTED PASS pbs_lit_material_matrix"));
    assert!(line.contains("#212"));
}
