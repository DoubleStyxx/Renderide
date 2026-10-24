//! End-to-end integration test for the canvas Offset NDC bias.
//!
//! Skipped unless the renderer binary sits next to the test executable. The suite entry point
//! is `renderide-test check-suite --case canvas_offset_bias_quad`.

use renderide_test::scene_dsl::cases::canvas_offset_bias_quad;
use renderide_test::scene_dsl::runner::{RunnerConfig, run_integration_case};

#[test]
#[ignore = "GPU headless case; run `renderide-test check-suite`"]
fn canvas_offset_bias_quad_passes_through_runner() {
    let Some(runner_cfg) = RunnerConfig::for_cargo_test() else {
        eprintln!(
            "skipping integration_canvas_offset_bias_quad: renderer binary not found next to \
             the test binary; run `cargo build -p renderide` first"
        );
        return;
    };

    let case = canvas_offset_bias_quad();
    let outcome = run_integration_case(&case, &runner_cfg)
        .unwrap_or_else(|e| panic!("run_integration_case({}) failed: {e}", case.name));

    assert!(
        outcome.report.passed,
        "canvas_offset_bias_quad did not pass tolerance: {:?}",
        outcome.report
    );
    assert!(outcome.layout.actual_png.exists());
    assert!(outcome.layout.report_json.exists());
}
