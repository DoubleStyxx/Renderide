//! Drives a single [`IntegrationCase`] through the harness end-to-end and emits a
//! [`super::output::CaseReport`].
//!
//! The runner owns:
//! 1. Building the harness configuration from case + caller defaults.
//! 2. Spawning the renderer through [`crate::host::HostHarness`].
//! 3. Loading the freshly produced PNG plus the case's golden.
//! 4. Evaluating [`super::tolerance::Tolerance`] and writing the per-case output layout
//!    (actual / golden copy / diff / report.json).
//!
//! Cases are dispatched by [`super::cases::CaseTemplate`] -- adding a new template means
//! adding an arm here and a builder in [`super::cases`].

use std::time::Duration;

use crate::error::HarnessError;
use crate::host::{HostHarness, HostHarnessConfig, SessionTemplate};
use crate::image_io::{load_rgba, save_rgba, write_diff_image};
use crate::scene::gltf_fixture::load_static_textured_mesh;
use crate::scene::perlin::generate_perlin_rgba;
use crate::scene::textures::{alpha_rings_rgba, checker_rgba, uv_ramp_rgba};

use super::cases::{CaseTemplate, IntegrationCase};
use super::output::{
    CaseOutputLayout, CaseReport, default_output_root, report_from_error, report_from_evaluation,
    write_report,
};
use super::tolerance::ToleranceEvaluation;

/// Default per-case overall timeout (handshake + asset upload + first stable PNG capture).
///
/// Sized for debug-build runs where PNG encoding inside the renderer is much slower than
/// release; release-built runs typically finish within a handful of seconds.
pub const DEFAULT_CASE_TIMEOUT: Duration = Duration::from_secs(180);

/// Default cadence at which the renderer rewrites the headless PNG. Larger than the
/// rewrite-time floor so the harness can observe a stable mtime between writes.
pub const DEFAULT_INTERVAL_MS: u64 = 500;

/// Inputs that vary between local development and CI but do not change per case.
#[derive(Clone, Debug)]
pub struct RunnerConfig {
    /// Path to the `renderide-renderer` binary to spawn.
    pub renderer_path: std::path::PathBuf,
    /// Per-case overall timeout. Defaults to [`DEFAULT_CASE_TIMEOUT`].
    pub timeout: Duration,
    /// Renderer PNG-rewrite cadence in milliseconds.
    pub interval_ms: u64,
    /// Forward the renderer's stdout/stderr when `true`.
    pub verbose_renderer: bool,
    /// Test-output root. Defaults to `target/renderide-test-out`.
    pub output_root: std::path::PathBuf,
}

impl RunnerConfig {
    /// Convenience constructor that fills in the defaults around a renderer binary path.
    pub fn with_defaults(renderer_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            renderer_path: renderer_path.into(),
            timeout: DEFAULT_CASE_TIMEOUT,
            interval_ms: DEFAULT_INTERVAL_MS,
            verbose_renderer: false,
            output_root: default_output_root(),
        }
    }

    /// Builds a config suitable for `#[test]` shims under `cargo test`. The renderer binary
    /// is discovered next to the test binary in `target/<profile>/renderide-renderer`. Returns `None`
    /// when invoked from a context that is not a cargo-built test binary, or when the
    /// renderer hasn't been built.
    pub fn for_cargo_test() -> Option<Self> {
        let renderer = renderer_next_to_current_exe()?;
        Some(Self::with_defaults(renderer))
    }
}

/// Locates `target/<profile>/renderide-renderer(.exe)` next to the running test binary.
pub fn renderer_next_to_current_exe() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let profile_dir = exe.parent()?;
    let renderer_name = if cfg!(windows) {
        "renderide-renderer.exe"
    } else {
        "renderide-renderer"
    };
    let candidate = profile_dir.join(renderer_name);
    if candidate.is_file() {
        return Some(candidate);
    }
    // Some test binaries land in `target/<profile>/deps/`; check one level up.
    let candidate = profile_dir.parent()?.join(renderer_name);
    candidate.is_file().then_some(candidate)
}

/// Outcome of [`run_integration_case`]. The report has been written to disk regardless of
/// whether the case passed; failed comparisons additionally produce `diff.png`.
#[derive(Debug)]
pub struct CaseRunOutcome {
    /// Per-case output paths.
    pub layout: CaseOutputLayout,
    /// Final report describing what was compared and the outcome.
    pub report: CaseReport,
}

impl CaseRunOutcome {
    /// Whether the integration case passed end-to-end.
    pub fn passed(&self) -> bool {
        self.report.passed
    }

    /// Computed tolerance evaluation, if any.
    pub fn evaluation(&self) -> Option<&ToleranceEvaluation> {
        self.report.evaluation.as_ref()
    }
}

/// Runs `case` through the harness, comparing the captured PNG against `case.golden_path`.
pub fn run_integration_case(
    case: &IntegrationCase,
    runner: &RunnerConfig,
) -> Result<CaseRunOutcome, HarnessError> {
    let layout = CaseOutputLayout::for_case(&runner.output_root, &case.name);
    layout.prepare().map_err(|e| {
        HarnessError::QueueOptions(format!("prepare {}: {e}", layout.root.display()))
    })?;

    let prepared_template = prepare_template(&case.template, &layout)?;
    let physically_based = template_requests_physically_based(&prepared_template.session_template);

    let harness_cfg = HostHarnessConfig {
        renderer_path: runner.renderer_path.clone(),
        forced_output_path: Some(layout.actual_png.clone()),
        width: case.resolution.0,
        height: case.resolution.1,
        interval_ms: runner.interval_ms,
        timeout: runner.timeout,
        verbose_renderer: runner.verbose_renderer,
        physically_based,
        template: prepared_template.session_template,
    };

    match drive_template(harness_cfg) {
        Ok(()) => {
            let mut outcome = evaluate_and_finalize(case, layout)?;
            if let Err(err) = positive_only_reference_failure(case, runner, &outcome.layout) {
                outcome.report.passed = false;
                let detail = err.to_string();
                outcome.report.error = Some(match outcome.report.error.take() {
                    Some(existing) => format!("{existing}; {detail}"),
                    None => detail,
                });
                let _ = write_report(&outcome.layout, &mut outcome.report);
                return Err(err);
            }
            Ok(outcome)
        }
        Err(err) => {
            let mut report = report_from_error(case, format!("harness error: {err}"));
            let _ = write_report(&layout, &mut report);
            Err(err)
        }
    }
}

fn template_requests_physically_based(template: &SessionTemplate) -> bool {
    matches!(
        template,
        SessionTemplate::NegativeLightPair {
            physically_based: true,
            ..
        }
    )
}

/// Renders the physically based scene again with the negative light omitted and compares it
/// to the case capture. Parity cases and every other template skip this check.
///
/// A capture failure from that spawn is returned unchanged. A pixel mismatch is
/// [`HarnessError::ImageCompare`] and has no capture-failure kind.
fn positive_only_reference_failure(
    case: &IntegrationCase,
    runner: &RunnerConfig,
    layout: &CaseOutputLayout,
) -> Result<(), HarnessError> {
    let CaseTemplate::NegativeLightPair {
        physically_based: true,
        include_negative: true,
        ..
    } = case.template
    else {
        return Ok(());
    };
    if !layout.actual_png.is_file() {
        return Err(HarnessError::ImageCompare(format!(
            "physically based capture missing at {}",
            layout.actual_png.display()
        )));
    }

    let reference_path = layout.root.join("positive_only.png");
    let harness_cfg = HostHarnessConfig {
        renderer_path: runner.renderer_path.clone(),
        forced_output_path: Some(reference_path.clone()),
        width: case.resolution.0,
        height: case.resolution.1,
        interval_ms: runner.interval_ms,
        timeout: runner.timeout,
        verbose_renderer: runner.verbose_renderer,
        physically_based: false,
        template: SessionTemplate::NegativeLightPair {
            physically_based: false,
            include_negative: false,
            negative_color: false,
        },
    };
    let mut harness = HostHarness::start(harness_cfg)?;
    harness.run().map_err(reference_spawn_failure)?;

    let actual =
        load_rgba(&layout.actual_png).map_err(|err| HarnessError::ImageCompare(err.to_string()))?;
    let reference =
        load_rgba(&reference_path).map_err(|err| HarnessError::ImageCompare(err.to_string()))?;
    let eval = case
        .tolerance
        .evaluate(&actual, &reference)
        .map_err(HarnessError::ImageCompare)?;
    if eval.passed {
        return Ok(());
    }
    let diff_path = layout.root.join("positive_only_diff.png");
    if let Err(err) = write_diff_image(&actual, &reference, &diff_path) {
        logger::warn!(
            "runner: failed to write positive-only diff PNG at {}: {err}",
            diff_path.display()
        );
    }
    Err(reference_mismatch_error(
        eval.ssim,
        eval.failing_pixel_fraction,
    ))
}

/// Capture failure from the positive-only reference spawn.
///
/// The variant and its fields are returned as the suite sees a primary capture
/// failure. This is not wrapped in [`HarnessError::ImageCompare`].
fn reference_spawn_failure(err: HarnessError) -> HarnessError {
    match err {
        HarnessError::FlatOrUnstableFrame {
            path,
            wait,
            last_pending,
            ready_chunks,
            pending_chunks,
            missing_chunks,
            frame_count,
            frame_pending,
            gpu_done,
            dropped_draws,
        } => HarnessError::FlatOrUnstableFrame {
            path,
            wait,
            last_pending,
            ready_chunks,
            pending_chunks,
            missing_chunks,
            frame_count,
            frame_pending,
            gpu_done,
            dropped_draws,
        },
        other => other,
    }
}

/// Pixel mismatch between the physically based capture and the omitted-light render.
fn reference_mismatch_error(ssim: Option<f64>, failing_fraction: Option<f64>) -> HarnessError {
    HarnessError::ImageCompare(format!(
        "physically based capture does not match the same scene without the negative light \
         (ssim={ssim:?}, failing_fraction={failing_fraction:?})"
    ))
}

fn drive_template(cfg: HostHarnessConfig) -> Result<(), HarnessError> {
    let mut h = HostHarness::start(cfg)?;
    h.run().map(|_| ())
}

/// Session template plus any side artifacts prepared from a case template.
struct PreparedTemplate {
    /// Host-session template that drives the renderer.
    session_template: SessionTemplate,
}

/// Prepares CPU-generated template data once for both renderer input and side artifacts.
fn prepare_template(
    template: &CaseTemplate,
    layout: &CaseOutputLayout,
) -> Result<PreparedTemplate, HarnessError> {
    match template {
        CaseTemplate::SphereNull => Ok(PreparedTemplate {
            session_template: SessionTemplate::Sphere,
        }),
        CaseTemplate::TorusUnlitPerlin { perlin } => {
            let img = generate_perlin_rgba(perlin);
            let texture_size = (img.width(), img.height());
            let path = layout.root.join("perlin_texture.png");
            save_rgba(&img, &path)?;
            logger::info!(
                "runner: wrote Perlin noise side artifact ({}x{}, seed=0x{:X}) to {}",
                perlin.width,
                perlin.height,
                perlin.seed,
                path.display()
            );
            let texture_rgba = img.into_raw();
            Ok(PreparedTemplate {
                session_template: SessionTemplate::Torus {
                    texture_rgba,
                    texture_size,
                },
            })
        }
        CaseTemplate::MultiPrimitiveUnlitGrid => {
            let checker = checker_rgba(256, 256, 8, [245, 245, 245, 255], [24, 34, 54, 255]);
            let uv_ramp = uv_ramp_rgba(256, 256);
            save_rgba(&checker, &layout.root.join("checker_texture.png"))?;
            save_rgba(&uv_ramp, &layout.root.join("uv_ramp_texture.png"))?;
            Ok(PreparedTemplate {
                session_template: SessionTemplate::MultiPrimitiveUnlitGrid {
                    checker_rgba: checker.into_raw(),
                    uv_ramp_rgba: uv_ramp.into_raw(),
                    texture_size: (256, 256),
                },
            })
        }
        CaseTemplate::PbsLitMaterialMatrix => Ok(PreparedTemplate {
            session_template: SessionTemplate::PbsLitMaterialMatrix,
        }),
        CaseTemplate::AlphaCutoutMaskedQuads => {
            let mask = alpha_rings_rgba(256, 256);
            save_rgba(&mask, &layout.root.join("alpha_mask_texture.png"))?;
            Ok(PreparedTemplate {
                session_template: SessionTemplate::AlphaCutoutMaskedQuads {
                    mask_rgba: mask.into_raw(),
                    texture_size: (256, 256),
                },
            })
        }
        CaseTemplate::CustomShaderPropertyUpdate => Ok(PreparedTemplate {
            session_template: SessionTemplate::CustomShaderPropertyUpdate,
        }),
        CaseTemplate::ExoticShaderPair => Ok(PreparedTemplate {
            session_template: SessionTemplate::ExoticShaderPair,
        }),
        CaseTemplate::CoplanarCanvasUiQuads => Ok(PreparedTemplate {
            session_template: SessionTemplate::CoplanarCanvasUiQuads,
        }),
        CaseTemplate::TiltedCanvasUiQuads => Ok(PreparedTemplate {
            session_template: SessionTemplate::TiltedCanvasUiQuads,
        }),
        CaseTemplate::CanvasOffsetBiasQuad => Ok(PreparedTemplate {
            session_template: SessionTemplate::CanvasOffsetBiasQuad,
        }),
        CaseTemplate::TiltedCanvasDistanceBias => Ok(PreparedTemplate {
            session_template: SessionTemplate::TiltedCanvasDistanceBias,
        }),
        CaseTemplate::ShadowedCasterMove => Ok(PreparedTemplate {
            session_template: SessionTemplate::ShadowedCasterMove,
        }),
        CaseTemplate::DoubleBufferNothingOnSkybox => Ok(PreparedTemplate {
            session_template: SessionTemplate::DoubleBufferNothingOnSkybox,
        }),
        CaseTemplate::NegativeLightPair {
            physically_based,
            include_negative,
            negative_color,
        } => Ok(PreparedTemplate {
            session_template: SessionTemplate::NegativeLightPair {
                physically_based: *physically_based,
                include_negative: *include_negative,
                negative_color: *negative_color,
            },
        }),
        CaseTemplate::GltfTexturedStaticMesh { path } => gltf_prepared_template(path, layout),
    }
}

fn gltf_prepared_template(
    path: &std::path::Path,
    layout: &CaseOutputLayout,
) -> Result<PreparedTemplate, HarnessError> {
    let imported = load_static_textured_mesh(path)?;
    let texture_size = imported.texture_size;
    let texture_rgba = imported.texture_rgba;
    let texture_preview =
        image::RgbaImage::from_raw(texture_size.0, texture_size.1, texture_rgba.clone())
            .ok_or_else(|| {
                HarnessError::QueueOptions(format!(
                    "GLB texture {}x{} did not match RGBA byte length",
                    texture_size.0, texture_size.1
                ))
            })?;
    save_rgba(
        &texture_preview,
        &layout.root.join("gltf_base_color_texture.png"),
    )?;
    Ok(PreparedTemplate {
        session_template: SessionTemplate::GltfTexturedStaticMesh {
            mesh: imported.mesh,
            texture_rgba,
            texture_size,
        },
    })
}

fn evaluate_and_finalize(
    case: &IntegrationCase,
    layout: CaseOutputLayout,
) -> Result<CaseRunOutcome, HarnessError> {
    if !layout.actual_png.exists() {
        let mut report = report_from_error(case, "renderer produced no PNG at actual.png");
        let _ = write_report(&layout, &mut report);
        return Ok(CaseRunOutcome { layout, report });
    }
    if !case.golden_path.exists() {
        let msg = format!(
            "golden image missing at {}; run `renderide-test update-suite --case {}` to create it",
            case.golden_path.display(),
            case.name
        );
        let mut report = report_from_error(case, msg);
        let _ = write_report(&layout, &mut report);
        return Ok(CaseRunOutcome { layout, report });
    }

    if let Err(e) = std::fs::copy(&case.golden_path, &layout.golden_png_copy) {
        logger::warn!("runner: failed to copy golden into output layout: {e} (continuing)");
    }

    let actual = load_rgba(&layout.actual_png)?;
    let golden = load_rgba(&case.golden_path)?;
    if let Err(e) = crate::golden::reject_flat_image(&actual, &layout.actual_png) {
        let mut report = report_from_error(case, e.to_string());
        let _ = write_report(&layout, &mut report);
        return Ok(CaseRunOutcome { layout, report });
    }
    if let Err(e) = crate::golden::reject_flat_image(&golden, &case.golden_path) {
        let mut report = report_from_error(case, e.to_string());
        let _ = write_report(&layout, &mut report);
        return Ok(CaseRunOutcome { layout, report });
    }

    match case.tolerance.evaluate(&actual, &golden) {
        Ok(eval) => {
            if !eval.passed
                && let Err(e) = write_diff_image(&actual, &golden, &layout.diff_png)
            {
                logger::warn!(
                    "runner: failed to write diff PNG at {}: {e}",
                    layout.diff_png.display()
                );
            }
            let mut report = report_from_evaluation(case, eval);
            if let Err(message) = plain_capture_failure(case, &layout) {
                report.passed = false;
                report.error = Some(match report.error.take() {
                    Some(existing) => format!("{existing}; {message}"),
                    None => message,
                });
            }
            write_report(&layout, &mut report)
                .map_err(|e| HarnessError::QueueOptions(format!("write report: {e}")))?;
            Ok(CaseRunOutcome { layout, report })
        }
        Err(msg) => {
            let mut report = report_from_error(case, format!("tolerance evaluate: {msg}"));
            let _ = write_report(&layout, &mut report);
            Ok(CaseRunOutcome { layout, report })
        }
    }
}

/// Compares the pre-update `plain.png` when this case records two frames.
///
/// Returns `Ok(())` when the case has no companion capture or the companion matches.
/// Returns `Err(message)` when the companion is missing or outside tolerance.
fn plain_capture_failure(case: &IntegrationCase, layout: &CaseOutputLayout) -> Result<(), String> {
    let Some(golden_path) = case.plain_capture_golden() else {
        return Ok(());
    };
    let actual_path = layout.actual_png.with_file_name("plain.png");
    if !actual_path.is_file() {
        return Err(format!(
            "plain capture missing at {}",
            actual_path.display()
        ));
    }
    if !golden_path.is_file() {
        return Err(format!(
            "plain golden missing at {}; run `renderide-test update-suite --case {}`",
            golden_path.display(),
            case.name
        ));
    }
    let actual = load_rgba(&actual_path).map_err(|err| err.to_string())?;
    let golden = load_rgba(&golden_path).map_err(|err| err.to_string())?;
    crate::golden::reject_flat_image(&actual, &actual_path).map_err(|err| err.to_string())?;
    crate::golden::reject_flat_image(&golden, &golden_path).map_err(|err| err.to_string())?;
    let eval = case.tolerance.evaluate(&actual, &golden)?;
    if eval.passed {
        return Ok(());
    }
    let diff_path = layout.root.join("plain_diff.png");
    if let Err(err) = write_diff_image(&actual, &golden, &diff_path) {
        logger::warn!(
            "runner: failed to write plain diff PNG at {}: {err}",
            diff_path.display()
        );
    }
    Err(format!(
        "plain capture failed tolerance (ssim={:?}, failing_fraction={:?})",
        eval.ssim, eval.failing_pixel_fraction
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn non_flat_image() -> image::RgbaImage {
        let mut img = image::RgbaImage::new(8, 8);
        for y in 0..8 {
            for x in 0..8 {
                img.put_pixel(x, y, image::Rgba([(x * 16) as u8, (y * 16) as u8, 31, 255]));
            }
        }
        img
    }

    #[test]
    fn evaluate_and_finalize_reports_flat_actual_without_tolerance_eval() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output_root = temp.path().join("out");
        let layout = CaseOutputLayout::for_case(&output_root, "unlit_sphere");
        layout.prepare().expect("prepare layout");

        let mut case = crate::scene_dsl::cases::unlit_sphere();
        case.golden_path = temp.path().join("unlit_sphere.png");
        non_flat_image()
            .save(&case.golden_path)
            .expect("write golden");

        let mut flat = image::RgbaImage::new(8, 8);
        for pixel in flat.pixels_mut() {
            *pixel = image::Rgba([39, 63, 97, 255]);
        }
        flat.save(&layout.actual_png).expect("write actual");

        let outcome = evaluate_and_finalize(&case, layout).expect("finalize");

        assert!(!outcome.report.passed);
        assert!(outcome.report.evaluation.is_none());
        assert!(
            outcome
                .report
                .error
                .as_deref()
                .is_some_and(|msg| msg.contains("flat single color"))
        );
    }

    fn reference_flat_frame(frame_pending: Option<u64>, gpu_done: bool) -> HarnessError {
        reference_spawn_failure(HarnessError::FlatOrUnstableFrame {
            path: std::path::PathBuf::from("positive_only.png"),
            wait: Duration::from_secs(300),
            last_pending: frame_pending.unwrap_or(0),
            ready_chunks: 780,
            pending_chunks: 455,
            missing_chunks: 0,
            frame_count: 1235,
            frame_pending,
            gpu_done,
            dropped_draws: Some(0),
        })
    }

    fn quarantine_outcome(err: &HarnessError, warp: bool) -> crate::scene_dsl::suite::SuiteReport {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().join("negative_light_physically_based");
        std::fs::create_dir_all(&root).expect("artifacts");
        let report_json = root.join("report.json");
        let integration = crate::scene_dsl::cases::negative_light_physically_based();
        let case_report = report_from_error(&integration, err.to_string());
        std::fs::write(
            &report_json,
            serde_json::to_string_pretty(&case_report).expect("report json"),
        )
        .expect("write report");
        let case = crate::scene_dsl::suite::SuiteCaseReport::from_harness_error(
            &integration.name,
            root,
            report_json,
            err,
        );
        let mut report = crate::scene_dsl::suite::SuiteReport::from_cases(vec![case]);
        crate::scene_dsl::quarantine_216::apply_quarantine(
            &mut report,
            std::time::SystemTime::UNIX_EPOCH,
            move |_| warp,
        );
        report
    }

    #[test]
    fn reference_spawn_flat_frame_keeps_its_kind_and_matches_only_the_216_signature() {
        let err = reference_flat_frame(Some(0), true);
        match &err {
            HarnessError::FlatOrUnstableFrame {
                path,
                frame_pending,
                gpu_done,
                dropped_draws,
                ..
            } => {
                assert_eq!(path, std::path::Path::new("positive_only.png"));
                assert_eq!(*frame_pending, Some(0));
                assert!(*gpu_done);
                assert_eq!(*dropped_draws, Some(0));
            }
            other => panic!("expected FlatOrUnstableFrame, got {other:?}"),
        }
        let text = err.to_string();
        assert!(
            !text.starts_with("image-compare"),
            "reference capture must keep its kind, got {text}"
        );

        let quarantined = quarantine_outcome(&err, true);
        let case = &quarantined.cases[0];
        assert_eq!(
            case.failure_kind,
            Some(crate::scene_dsl::suite::SuiteFailureKind::FlatOrUnstableFrame)
        );
        assert_eq!(case.frame_pending, Some(0));
        assert_eq!(case.gpu_done, Some(true));
        assert_eq!(case.dropped_draws, Some(0));
        assert_eq!(quarantined.quarantined, 1);
        assert_eq!(quarantined.failed, 0);
        assert!(crate::scene_dsl::quarantine_216::case_is_quarantined(case));

        let other_adapter = quarantine_outcome(&err, false);
        assert_eq!(other_adapter.quarantined, 0);
        assert_eq!(other_adapter.failed, 1);
        assert!(other_adapter.cases[0].status.is_none());

        let pending = quarantine_outcome(&reference_flat_frame(Some(4), true), true);
        assert_eq!(pending.cases[0].frame_pending, Some(4));
        assert_eq!(
            pending.cases[0].failure_kind,
            Some(crate::scene_dsl::suite::SuiteFailureKind::FlatOrUnstableFrame)
        );
        assert_eq!(pending.quarantined, 0);
        assert_eq!(pending.failed, 1);

        let not_done = quarantine_outcome(&reference_flat_frame(Some(0), false), true);
        assert_eq!(not_done.cases[0].gpu_done, Some(false));
        assert_eq!(not_done.quarantined, 0);
        assert_eq!(not_done.failed, 1);
    }

    #[test]
    fn reference_spawn_mismatch_is_an_image_compare_and_is_not_quarantined() {
        let err = reference_mismatch_error(Some(0.834), Some(0.15));
        assert!(matches!(err, HarnessError::ImageCompare(_)));
        assert!(err.to_string().starts_with("image-compare:"));

        let report = quarantine_outcome(&err, true);
        assert!(report.cases[0].failure_kind.is_none());
        assert!(report.cases[0].frame_pending.is_none());
        assert!(report.cases[0].gpu_done.is_none());
        assert_eq!(report.quarantined, 0);
        assert_eq!(report.failed, 1);
        assert!(report.cases[0].status.is_none());
        assert!(report.cases[0].reason.is_none());

        let wrapped = HarnessError::ImageCompare(reference_flat_frame(Some(0), true).to_string());
        let wrapped_report = quarantine_outcome(&wrapped, true);
        assert!(wrapped_report.cases[0].failure_kind.is_none());
        assert_eq!(wrapped_report.quarantined, 0);
        assert_eq!(wrapped_report.failed, 1);
    }
}
