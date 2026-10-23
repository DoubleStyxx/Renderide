//! End-to-end integration test for sibling-step bias on a tilted canvas at distance.
//!
//! Skipped unless the renderer binary sits next to the test executable. The suite entry point
//! is `renderide-test check-suite --case tilted_canvas_distance_bias`.

use renderide_test::scene_dsl::cases::tilted_canvas_distance_bias;
use renderide_test::scene_dsl::runner::{RunnerConfig, run_integration_case};

#[test]
#[ignore = "GPU headless case; run `renderide-test check-suite`"]
fn tilted_canvas_distance_bias_passes_through_runner() {
    let Some(runner_cfg) = RunnerConfig::for_cargo_test() else {
        eprintln!(
            "skipping integration_tilted_canvas_distance_bias: renderer binary not found next to \
             the test binary; run `cargo build -p renderide` first"
        );
        return;
    };

    let case = tilted_canvas_distance_bias();
    let outcome = run_integration_case(&case, &runner_cfg)
        .unwrap_or_else(|e| panic!("run_integration_case({}) failed: {e}", case.name));

    assert!(
        outcome.report.passed,
        "tilted_canvas_distance_bias did not pass tolerance: {:?}",
        outcome.report
    );
    assert!(outcome.layout.actual_png.exists());
    assert!(outcome.layout.report_json.exists());
}
