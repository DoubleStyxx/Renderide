//! `run`, `list`, and bless execution.
//!
//! `RENDERIDE_TEST_CLI_FIXTURE` is a debug-only hook for the CLI contract tests.
//! Release builds ignore it. The integration test sets it on the child process
//! (`mismatch`, `capture`, `unstable`, or `golden_missing`) and does not spawn a renderer.
//! `std::env::set_var` is not used.

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
    if let Some(kind) = test_fixture() {
        let outcome = SuiteRunOutcome {
            report: fixture_report(&kind),
            report_path: PathBuf::from("suite-report.json"),
        };
        return finalize(outcome, json, deprecation, Vec::new(), "passed", false);
    }
    let config = match suite_config(opts, cases) {
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
    finalize(first, json, deprecation, notes, "passed", true)
}

fn bless_suite(config: &SuiteConfig, json: bool, force: bool, deprecation: bool) -> Finished {
    let mut notes = Vec::new();
    let mut cases = Vec::new();
    for case in &config.cases {
        let (report, note) = bless_one(case, &config.runner, force);
        if let Some(note) = note {
            notes.push(note);
        }
        cases.push(report);
    }
    let outcome = SuiteRunOutcome {
        report: SuiteReport::from_cases(cases),
        report_path: config
            .runner
            .output_root
            .join(crate::scene_dsl::suite::SUITE_REPORT_FILENAME),
    };
    finalize(outcome, json, deprecation, notes, "blessed", false)
}

fn bless_one(
    case: &IntegrationCase,
    runner: &RunnerConfig,
    force: bool,
) -> (SuiteCaseReport, Option<String>) {
    let layout =
        crate::scene_dsl::output::CaseOutputLayout::for_case(&runner.output_root, &case.name);
    if let Some(reason) = crate::scene_dsl::bless_forbidden(&case.name) {
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
    }
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

/// Debug-only contract-test hook. Release builds return `None`.
fn test_fixture() -> Option<String> {
    #[cfg(debug_assertions)]
    {
        std::env::var("RENDERIDE_TEST_CLI_FIXTURE")
            .ok()
            .filter(|value| !value.is_empty())
    }
    #[cfg(not(debug_assertions))]
    {
        None
    }
}

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
    fn bless_force_refuses_every_quarantine_and_expected_failure_case() {
        let runner = RunnerConfig::with_defaults("/nonexistent/renderide");
        let names = crate::scene_dsl::bless_refused_scenes();
        assert!(
            names.len() >= 4,
            "the #216 list and the #212 case must both be present"
        );
        for name in names {
            let case = crate::scene_dsl::cases::lookup(name)
                .unwrap_or_else(|| panic!("{name} is quarantined but not registered"));
            let (report, note) = bless_one(&case, &runner, true);
            let skip = report.bless_skip.as_deref().unwrap_or("");
            assert!(report.passed, "{name}");
            assert!(report.blessed.is_none(), "{name} was blessed: {report:?}");
            assert!(skip.contains("bless refused"), "{name}: {skip}");
            assert!(skip.contains("#216"), "{name}: {skip}");
            if *name == "pbs_lit_material_matrix" {
                assert!(skip.contains("#212"), "{name}: {skip}");
            }
            assert!(note.unwrap_or_default().contains("bless refused"), "{name}");
        }
    }
}
