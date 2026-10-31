//! `run`, `list`, and bless execution.
//!
//! `RENDERIDE_TEST_CLI_FIXTURE` is compiled only for debug builds with the
//! default-off `test-cli-fixture` feature. Release builds and the normal CI
//! binary (`cargo build` / `cargo test` without that feature) do not contain
//! the hook. `cli_contract` turns it on with
//! `cargo run -p renderide-test --features test-cli-fixture` in a separate
//! target directory, and sets the variable on that child. Values are
//! `mismatch`, `capture`, `unstable`, and `golden_missing`.
//! `std::env::set_var` is not used.

#[cfg(all(debug_assertions, feature = "test-cli-fixture"))]
use std::path::PathBuf;
use std::time::Duration;

use crate::error::HarnessError;
use crate::exit;
use crate::scene_dsl::cases::IntegrationCase;
use crate::scene_dsl::output::default_output_root;
use crate::scene_dsl::runner::{RunnerConfig, run_integration_case};
use crate::scene_dsl::suite::{
    SuiteCaseReport, SuiteConfig, SuiteReport, SuiteRunOutcome, run_suite, select_cases_in,
};

use super::SuiteOpts;
use super::print;

/// Result of one command, ready to print.
pub(super) struct Finished {
    /// Process exit code.
    pub exit_code: u8,
    /// `--json`.
    pub json: bool,
    /// Print the check-suite deprecation note.
    pub deprecation: bool,
    /// Stderr notes.
    pub notes: Vec<String>,
    /// Human suite lines.
    pub human: Vec<String>,
    /// Suite report, present for `run`.
    pub report: Option<SuiteReport>,
    /// Stderr `env:` line, printed with no prefix.
    pub env_line: Option<String>,
}

pub(super) fn list_command(json: bool, group: Option<&str>) -> Finished {
    let cases = select_cases_in(&[], group).unwrap_or_default();
    let names: Vec<String> = cases.iter().map(|case| case.name.clone()).collect();
    print::emit_list(json, &names);
    Finished {
        exit_code: exit::EXIT_OK,
        json: false,
        deprecation: false,
        notes: Vec::new(),
        human: Vec::new(),
        report: None,
        env_line: None,
    }
}

/// Flags for one `run` invocation. Bools stay in the struct so the function stays small.
pub(super) struct RunFlags<'a> {
    /// Positional scene names.
    pub scenes: &'a [String],
    /// `--case` names.
    pub case_flags: &'a [String],
    /// Shared harness options.
    pub opts: &'a SuiteOpts,
    /// `--json`.
    pub json: bool,
    /// `--bless`.
    pub bless: bool,
    /// `--force`.
    pub force: bool,
    /// `--repeat`.
    pub repeat: u32,
    /// `check-suite` deprecation note.
    pub deprecation: bool,
}

pub(super) fn run_command(flags: RunFlags<'_>) -> Finished {
    let RunFlags {
        scenes,
        case_flags,
        opts,
        json,
        bless,
        force,
        repeat,
        deprecation,
    } = flags;
    if repeat == 0 {
        return usage(json, deprecation, "repeat must be at least 1");
    }
    let mut names = scenes.to_vec();
    names.extend(case_flags.iter().cloned());
    let cases = match select_scenes(&names, opts.group.as_deref()) {
        Ok(cases) => cases,
        Err(err) => return usage(json, deprecation, &err.to_string()),
    };
    if let Some(finished) = fixture_finished(json, deprecation) {
        return finished;
    }
    let harness = match resolve_harness(opts) {
        Ok(env) => env,
        Err(StartError::Usage(message)) => return usage(json, deprecation, &message),
        Err(StartError::Environment(message)) => {
            return environment_failure(json, deprecation, &message);
        }
    };
    let config = match suite_config(opts, cases, harness) {
        Ok(config) => config,
        Err(err) => return usage(json, deprecation, &err.to_string()),
    };
    if bless {
        return bless_suite(&config, json, force, deprecation);
    }
    check_suite(&config, json, repeat, deprecation)
}

fn check_suite(config: &SuiteConfig, json: bool, repeat: u32, deprecation: bool) -> Finished {
    let mut notes = Vec::new();
    let mut first = match run_once(config) {
        Ok(outcome) => outcome,
        Err(err) => return usage(json, deprecation, &err.to_string()),
    };
    if repeat > 1 {
        for _ in 1..repeat {
            let again = match run_once(config) {
                Ok(outcome) => outcome,
                Err(err) => return usage(json, deprecation, &err.to_string()),
            };
            mark_flaky(&mut first.report, &again.report, &mut notes);
        }
    }
    let env_line = env_line_for(config, &mut first.report);
    finalize(first, json, deprecation, notes, "passed", true, env_line)
}

fn bless_suite(config: &SuiteConfig, json: bool, force: bool, deprecation: bool) -> Finished {
    let mut notes = Vec::new();
    let mut cases = Vec::new();
    for case in &config.cases {
        let (report, note) = bless_one(
            case,
            &config.runner,
            force,
            &crate::scene_dsl::bless_forbidden,
        );
        if let Some(note) = note {
            notes.push(note);
        }
        cases.push(report);
    }
    let mut outcome = SuiteRunOutcome {
        report: SuiteReport::from_cases(cases),
        report_path: config
            .runner
            .output_root
            .join(crate::scene_dsl::suite::SUITE_REPORT_FILENAME),
    };
    let env_line = env_line_for(config, &mut outcome.report);
    finalize(
        outcome,
        json,
        deprecation,
        notes,
        "blessed",
        false,
        env_line,
    )
}

fn bless_one(
    case: &IntegrationCase,
    runner: &RunnerConfig,
    force: bool,
    forbidden: &impl Fn(&str) -> Option<&'static str>,
) -> (SuiteCaseReport, Option<String>) {
    let layout =
        crate::scene_dsl::output::CaseOutputLayout::for_case(&runner.output_root, &case.name);
    if let Some(reason) = forbidden(&case.name) {
        let mut skipped = SuiteCaseReport::plain(
            case.name.clone(),
            true,
            layout.root,
            layout.report_json,
            None,
        );
        skipped.bless_skip = Some(reason.to_string());
        return (skipped, Some(format!("{}: {reason}", case.name)));
    }
    if case.golden_path.is_file() && !force {
        let refused = SuiteCaseReport::plain(
            case.name.clone(),
            false,
            layout.root,
            layout.report_json,
            Some("bless_refused: pass --force to overwrite an existing golden".to_string()),
        );
        return (
            refused,
            Some(format!(
                "{}: bless_refused: pass --force to overwrite {}",
                case.name,
                case.golden_path.display()
            )),
        );
    }
    let existed = case.golden_path.is_file();
    match run_integration_case(case, runner) {
        Err(err) => (
            SuiteCaseReport::from_harness_error(
                case.name.clone(),
                layout.root,
                layout.report_json,
                &err,
            ),
            None,
        ),
        Ok(outcome) => promote_bless(case, &outcome, existed),
    }
}

fn promote_bless(
    case: &IntegrationCase,
    outcome: &crate::scene_dsl::runner::CaseRunOutcome,
    existed: bool,
) -> (SuiteCaseReport, Option<String>) {
    let mut report = SuiteCaseReport::plain(
        case.name.clone(),
        false,
        outcome.layout.root.clone(),
        outcome.layout.report_json.clone(),
        None,
    );
    let image = match crate::image_io::load_rgba(&outcome.layout.actual_png) {
        Ok(image) => image,
        Err(err) => {
            report.error = Some(err.to_string());
            return (report, None);
        }
    };
    let eval = case.tolerance.evaluate(&image, &image);
    let coverage_failed = eval.as_ref().is_ok_and(|eval| {
        eval.criteria.iter().any(|criterion| {
            matches!(
                criterion,
                crate::scene_dsl::tolerance::CriterionResult::ImageCoverage { passed: false, .. }
            )
        })
    });
    if crate::golden::is_flat_image(&image) || coverage_failed || eval.is_err() {
        report.error = Some("golden_flat: capture is flat or failed coverage".to_string());
        return (report, Some(format!("{}: golden_flat", case.name)));
    }
    if let Err(err) = std::fs::copy(&outcome.layout.actual_png, &case.golden_path) {
        report.error = Some(format!(
            "golden image not found at {}: {err}",
            case.golden_path.display()
        ));
        return (report, None);
    }
    let golden = match crate::image_io::load_rgba(&case.golden_path) {
        Ok(golden) => golden,
        Err(err) => {
            report.error = Some(format!("unstable: {err}"));
            return (report, None);
        }
    };
    match case.tolerance.evaluate(&image, &golden) {
        Ok(eval) if eval.passed => {
            report.passed = true;
            report.blessed = Some(if existed { "overwritten" } else { "new" }.to_string());
            report.image_compared = true;
            let stats = crate::scene_dsl::region_stats::compare(&golden, &golden);
            let note = one_colour(&stats).then(|| {
                format!(
                    "{}: every region of the new golden is one colour",
                    case.name
                )
            });
            report.region_stats = Some(stats);
            (report, note)
        }
        Ok(_) | Err(_) => {
            report.error =
                Some("unstable: blessed scene failed against its new golden".to_string());
            report.blessed = Some(if existed { "overwritten" } else { "new" }.to_string());
            (report, Some(format!("{}: unstable", case.name)))
        }
    }
}

fn one_colour(stats: &[crate::scene_dsl::region_stats::RegionStatRow]) -> bool {
    !stats.is_empty()
        && stats.iter().all(|row| {
            row.variance
                .iter()
                .take(3)
                .all(|value| value.abs().to_bits() == 0)
        })
}

fn run_once(config: &SuiteConfig) -> Result<SuiteRunOutcome, HarnessError> {
    logger::info!(
        "Suite: running {} case(s) with jobs={}",
        config.cases.len(),
        config.jobs
    );
    let mut outcome = run_suite(config.clone())?;
    stamp_adapter_gate(&mut outcome);
    Ok(outcome)
}

fn mark_flaky(first: &mut SuiteReport, again: &SuiteReport, notes: &mut Vec<String>) {
    for case in &mut first.cases {
        let other = again.cases.iter().find(|item| item.name == case.name);
        if other.is_some_and(|item| item.passed != case.passed) {
            case.passed = false;
            case.error = Some("flaky: results differed across --repeat".to_string());
            notes.push(format!("{}: flaky", case.name));
        }
    }
    first.recompute();
}

fn finalize(
    mut outcome: SuiteRunOutcome,
    json: bool,
    deprecation: bool,
    notes: Vec<String>,
    result_label: &str,
    print_adapter: bool,
    env_line: Option<String>,
) -> Finished {
    exit::stamp(&mut outcome.report);
    if let Some(root) = outcome.report_path.parent()
        && !root.as_os_str().is_empty()
    {
        let _ = crate::scene_dsl::suite::write_suite_report(root, &outcome.report);
    }
    let human = print::suite_lines(&outcome, result_label, print_adapter);
    Finished {
        exit_code: outcome.report.exit_code,
        json,
        deprecation,
        notes,
        human,
        report: Some(outcome.report),
        env_line,
    }
}

fn usage(json: bool, deprecation: bool, message: &str) -> Finished {
    let mut report = SuiteReport::from_cases(Vec::new());
    report.error = Some(message.to_string());
    report.exit_code = exit::EXIT_USAGE;
    Finished {
        exit_code: exit::EXIT_USAGE,
        json,
        deprecation,
        notes: vec![message.to_string()],
        human: Vec::new(),
        report: Some(report),
        env_line: None,
    }
}

fn environment_failure(json: bool, deprecation: bool, message: &str) -> Finished {
    let mut report = SuiteReport::from_cases(Vec::new());
    report.error = Some(message.to_string());
    report.exit_code = exit::EXIT_ENVIRONMENT;
    Finished {
        exit_code: exit::EXIT_ENVIRONMENT,
        json,
        deprecation,
        notes: vec![message.to_string()],
        human: Vec::new(),
        report: Some(report),
        env_line: None,
    }
}

fn env_line_for(config: &SuiteConfig, report: &mut SuiteReport) -> Option<String> {
    let env = config.runner.harness_env.as_ref()?;
    report.runner = Some(crate::scene_dsl::suite::SuiteRunner {
        icd: crate::scene_dsl::suite::SuiteIcd {
            path: env.icd_path.clone(),
            source: env.icd_source.clone(),
            var: env.icd_var.clone(),
        },
    });
    let mut shown = env.clone();
    shown.adapter = adapter_name(report);
    Some(shown.line())
}

fn adapter_name(report: &SuiteReport) -> String {
    report
        .cases
        .iter()
        .find_map(|case| {
            let label = case.adapter.trim();
            if label.is_empty() {
                return None;
            }
            let name = label.split(" type=").next().unwrap_or(label).trim();
            (!name.is_empty()).then(|| name.to_string())
        })
        .unwrap_or_else(|| "unavailable".to_string())
}

enum StartError {
    Usage(String),
    Environment(String),
}

fn resolve_harness(opts: &SuiteOpts) -> Result<renderide_harness_env::ResolvedEnv, StartError> {
    let icd = renderide_harness_env::parse_icd_mode(&opts.icd).map_err(StartError::Usage)?;
    let backend =
        renderide_harness_env::parse_backend_mode(&opts.backend).map_err(StartError::Usage)?;
    let renderer = match &opts.renderer {
        Some(path) => path.display().to_string(),
        None => super::resolve_renderer_path(super::BuildProfile::from_flags(
            opts.release,
            opts.dev_fast,
        ))
        .display()
        .to_string(),
    };
    let cpu = crate::scene_dsl::host_info::cpu_model();
    let roots = renderide_harness_env::default_icd_roots();
    let input = renderide_harness_env::ResolveInput {
        search_roots: &roots,
        os: std::env::consts::OS,
        icd: &icd,
        backend,
        renderer: &renderer,
        cpu: &cpu,
        adapter: "unavailable",
    };
    renderide_harness_env::resolve(&input, |key| std::env::var(key).ok(), |path| path.is_file())
        .map_err(|err| StartError::Environment(err.to_string()))
}

fn stamp_adapter_gate(outcome: &mut SuiteRunOutcome) {
    let adapter = crate::adapter_gate::adapter_from_directory(&logger::log_dir_for(
        logger::LogComponent::Renderer,
    ));
    let label = adapter
        .as_ref()
        .map(crate::adapter_gate::ObservedAdapter::label)
        .unwrap_or_default();
    for case in &mut outcome.report.cases {
        let gate = crate::adapter_gate::gate_for(&case.name, adapter.as_ref());
        case.gate = gate.as_str().to_string();
        case.adapter.clone_from(&label);
        if !crate::adapter_gate::stamp_case_report(&case.report_json, gate, &label) {
            logger::warn!(
                "Suite: could not record the adapter gate on {}",
                case.report_json.display()
            );
        }
    }
    if let Some(root) = outcome.report_path.parent()
        && let Err(err) = crate::scene_dsl::suite::write_suite_report(root, &outcome.report)
    {
        logger::warn!("Suite: could not record the adapter gate on the suite report: {err}");
    }
}

fn suite_config(
    opts: &SuiteOpts,
    cases: Vec<IntegrationCase>,
    harness: renderide_harness_env::ResolvedEnv,
) -> Result<SuiteConfig, HarnessError> {
    let case_count = cases.len();
    let renderer_path = match &opts.renderer {
        Some(path) => path.clone(),
        None => super::resolve_renderer_path(super::BuildProfile::from_flags(
            opts.release,
            opts.dev_fast,
        )),
    };
    let mut runner = RunnerConfig::with_defaults(renderer_path);
    runner.timeout = Duration::from_secs(opts.timeout_seconds);
    runner.interval_ms = opts.interval_ms;
    runner.verbose_renderer = opts.verbose_renderer;
    runner.renderer_stdout_to_stderr = opts.verbose_renderer && opts.json_stdout;
    runner.output_root = opts.output_root.clone().unwrap_or_else(default_output_root);
    runner.scene_color_readback = opts.scene_color_readback;
    runner.harness_env = Some(harness);
    let jobs = opts
        .jobs
        .unwrap_or_else(|| super::default_suite_jobs(case_count))
        .max(1);
    Ok(SuiteConfig {
        cases,
        runner,
        jobs,
    })
}

fn select_scenes(
    names: &[String],
    group: Option<&str>,
) -> Result<Vec<IntegrationCase>, HarnessError> {
    if names.iter().all(|name| !is_glob(name)) {
        return select_cases_in(names, group);
    }
    let pool = scene_pool(group)?;
    let mut selected = Vec::new();
    for name in names {
        let matched = if is_glob(name) {
            pool.iter()
                .filter(|case| glob_match(name, &case.name))
                .count()
        } else {
            usize::from(pool.iter().any(|case| case.name == *name))
        };
        if matched == 0 {
            return Err(HarnessError::UnknownCase(name.clone()));
        }
        for case in &pool {
            let hit = if is_glob(name) {
                glob_match(name, &case.name)
            } else {
                case.name == *name
            };
            if hit
                && selected
                    .iter()
                    .all(|kept: &IntegrationCase| kept.name != case.name)
            {
                selected.push(case.clone());
            }
        }
    }
    Ok(selected)
}

fn scene_pool(group: Option<&str>) -> Result<Vec<IntegrationCase>, HarnessError> {
    match group {
        None => {
            let mut cases = crate::scene_dsl::cases::registry();
            for case in crate::scene_dsl::warp_216_debug::cases() {
                if cases.iter().all(|kept| kept.name != case.name) {
                    cases.push(case);
                }
            }
            Ok(cases)
        }
        Some(name) if name == crate::scene_dsl::warp_216_debug::GROUP => {
            Ok(crate::scene_dsl::warp_216_debug::cases())
        }
        Some(name) => Err(HarnessError::UnknownGroup(name.to_string())),
    }
}

fn is_glob(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?')
}

fn glob_match(pattern: &str, name: &str) -> bool {
    glob_from(pattern.as_bytes(), name.as_bytes())
}

fn glob_from(pattern: &[u8], name: &[u8]) -> bool {
    let mut p = 0;
    let mut n = 0;
    let mut star = None;
    let mut star_name = 0;
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            star_name = n;
            p += 1;
        } else if let Some(star_at) = star {
            p = star_at + 1;
            star_name += 1;
            n = star_name;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

fn fixture_finished(json: bool, deprecation: bool) -> Option<Finished> {
    #[cfg(all(debug_assertions, feature = "test-cli-fixture"))]
    {
        let kind = std::env::var("RENDERIDE_TEST_CLI_FIXTURE")
            .ok()
            .filter(|value| !value.is_empty())?;
        let outcome = SuiteRunOutcome {
            report: fixture_report(&kind),
            report_path: PathBuf::from("suite-report.json"),
        };
        Some(finalize(
            outcome,
            json,
            deprecation,
            Vec::new(),
            "passed",
            false,
            None,
        ))
    }
    #[cfg(not(all(debug_assertions, feature = "test-cli-fixture")))]
    {
        let _ = (json, deprecation);
        None
    }
}

#[cfg(all(debug_assertions, feature = "test-cli-fixture"))]
fn fixture_report(kind: &str) -> SuiteReport {
    let error = match kind {
        "capture" => "headless PNG stayed pipeline-pending for 60s",
        "unstable" => "unstable: blessed scene failed against its new golden",
        "golden_missing" => "golden image not found at missing.png",
        _ => "case failed tolerance; channel-zero: G is near 0",
    };
    let mut case = SuiteCaseReport::plain(
        "fixture",
        false,
        PathBuf::from("out/fixture"),
        PathBuf::from("out/fixture/report.json"),
        Some(error.to_string()),
    );
    case.image_compared = kind != "capture";
    if kind == "capture" {
        case.failure_kind = Some(crate::scene_dsl::suite::SuiteFailureKind::PipelineNotReady);
    }
    SuiteReport::from_cases(vec![case])
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn glob_matches_scene_names() {
        assert!(glob_match("unlit*", "unlit_sphere"));
        assert!(glob_match("*sphere*", "unlit_sphere"));
        assert!(!glob_match("unlit*", "pbs_lit_material_matrix"));
        assert!(glob_match(
            "pbs_lit_material_matri?",
            "pbs_lit_material_matrix"
        ));
    }

    #[test]
    fn unknown_exact_name_is_rejected() {
        let err = select_scenes(&["missing_case".to_string()], None).expect_err("unknown");
        assert!(matches!(err, HarnessError::UnknownCase(name) if name == "missing_case"));
    }

    #[test]
    fn bless_force_refuses_an_injected_quarantine_fixture() {
        let mut case = crate::scene_dsl::cases::unlit_sphere();
        case.name = "synthetic_quarantine_fixture".to_string();
        let runner = RunnerConfig::with_defaults("/nonexistent/renderide");
        let (report, note) = bless_one(&case, &runner, true, &|name| {
            (name == "synthetic_quarantine_fixture").then_some("quarantined fixture; bless refused")
        });
        assert!(report.passed);
        assert!(report.blessed.is_none());
        assert_eq!(
            report.bless_skip.as_deref(),
            Some("quarantined fixture; bless refused")
        );
        assert!(note.unwrap_or_default().contains("bless refused"));
        assert!(crate::scene_dsl::bless_forbidden(&case.name).is_none());
    }

    #[test]
    fn bless_force_refuses_every_name_on_the_quarantine_list() {
        let runner = RunnerConfig::with_defaults("/nonexistent/renderide");
        for name in crate::scene_dsl::bless_refused_scenes() {
            let case = crate::scene_dsl::cases::lookup(name)
                .unwrap_or_else(|| panic!("{name} is listed but not registered"));
            let (report, note) =
                bless_one(&case, &runner, true, &crate::scene_dsl::bless_forbidden);
            assert!(report.blessed.is_none(), "{name}");
            assert!(
                report
                    .bless_skip
                    .as_deref()
                    .unwrap_or("")
                    .contains("bless refused"),
                "{name}"
            );
            assert!(note.unwrap_or_default().contains("bless refused"), "{name}");
        }
    }

    #[test]
    fn missing_lavapipe_exits_5() {
        let mode = renderide_harness_env::IcdMode::Auto;
        let roots = [PathBuf::from("/tmp/renderide-no-icd")];
        let input = renderide_harness_env::ResolveInput {
            search_roots: &roots,
            os: "linux",
            icd: &mode,
            backend: renderide_harness_env::BackendMode::Auto,
            renderer: "target/debug/renderide-renderer",
            cpu: "cpu",
            adapter: "unavailable",
        };
        let err = renderide_harness_env::resolve(&input, |_| None, |_| false).expect_err("missing");
        let finished = environment_failure(true, false, &err.to_string());
        assert_eq!(finished.exit_code, exit::EXIT_ENVIRONMENT);
        let report = finished.report.expect("report");
        assert_eq!(report.exit_code, exit::EXIT_ENVIRONMENT);
        let error = report.error.clone().unwrap_or_default();
        assert!(error.contains("no ICD"), "{error}");
        assert!(error.contains("lavapipe"), "{error}");
        assert!(finished.notes.iter().any(|note| note.contains("no ICD")));
        let document = serde_json::to_string(&report).expect("json");
        serde_json::from_str::<serde_json::Value>(&document).expect("one document");
    }

    #[test]
    fn fixture_hook_is_compiled_out_without_the_feature() {
        #[cfg(not(feature = "test-cli-fixture"))]
        assert!(fixture_finished(true, false).is_none());
    }
}
