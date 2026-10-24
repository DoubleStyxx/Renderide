//! Command-line interface for the golden-image harness.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI tool: stdout/stderr is the user-facing interface"
)]

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};

use crate::error::HarnessError;
use crate::scene_dsl::output::default_output_root;
use crate::scene_dsl::runner::RunnerConfig;
use crate::scene_dsl::suite::{SuiteConfig, run_suite, select_cases, update_suite};

/// CLI entry point.
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    if let Err(err) = crate::logging::init_renderer_test_logging() {
        eprintln!("renderide-test: failed to initialize logging: {err}");
        return ExitCode::FAILURE;
    }

    let exit_code = match dispatch(cli) {
        Ok(()) => {
            logger::info!("renderide-test completed successfully");
            ExitCode::SUCCESS
        }
        Err(err) => {
            logger::error!("renderide-test failed: {err}");
            eprintln!("renderide-test: {err}");
            ExitCode::FAILURE
        }
    };
    logger::flush();
    exit_code
}

#[derive(Parser, Debug)]
#[command(
    name = "renderide-test",
    about = "Mock host harness for Renderide golden-image integration tests.",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the registered scene suite in parallel and fail if any case fails.
    CheckSuite {
        /// Suite harness options.
        #[command(flatten)]
        suite: SuiteOpts,
    },
    /// Run the registered scene suite and overwrite each selected golden with its capture.
    UpdateSuite {
        /// Suite harness options.
        #[command(flatten)]
        suite: SuiteOpts,
    },
}

#[derive(Parser, Debug, Clone)]
struct SuiteOpts {
    /// Path to the renderer binary to spawn (defaults to `target/{profile}/renderide-renderer`).
    #[arg(long)]
    renderer: Option<PathBuf>,
    /// Use the `dev-fast` profile renderer binary (`target/dev-fast/renderide-renderer`).
    #[arg(long, default_value_t = false, conflicts_with = "release")]
    dev_fast: bool,
    /// Use the release-mode renderer binary (`target/release/renderide-renderer`).
    #[arg(long, default_value_t = false, conflicts_with = "dev_fast")]
    release: bool,
    /// Case name to run. Repeat to run a subset; omitted means every registered case.
    #[arg(long = "case", value_name = "NAME")]
    case: Vec<String>,
    /// Maximum number of cases to run concurrently.
    ///
    /// Defaults to the selected case count, capped at the machine's available parallelism, so a
    /// full suite does not spawn more renderer processes than there are cores.
    #[arg(long)]
    jobs: Option<usize>,
    /// Test-output root. Defaults to the workspace target/renderide-test-out directory.
    #[arg(long)]
    output_root: Option<PathBuf>,
    /// How long to wait for handshake / asset acks / a fresh PNG per case.
    #[arg(long, default_value_t = 180)]
    timeout_seconds: u64,
    /// Renderer interval between consecutive offscreen renders (ms).
    #[arg(long, default_value_t = 500)]
    interval_ms: u64,
    /// Print renderer processes' stdout/stderr instead of swallowing them.
    #[arg(long, default_value_t = false)]
    verbose_renderer: bool,
}

fn dispatch(cli: Cli) -> Result<(), HarnessError> {
    match cli.command {
        Command::CheckSuite { suite } => run_suite_command(&suite),
        Command::UpdateSuite { suite } => run_update_suite_command(&suite),
    }
}

fn run_suite_command(opts: &SuiteOpts) -> Result<(), HarnessError> {
    let config = suite_config_from_opts(opts)?;
    logger::info!(
        "Suite: running {} case(s) with jobs={}, renderer_path={}, output_root={}",
        config.cases.len(),
        config.jobs,
        config.runner.renderer_path.display(),
        config.runner.output_root.display()
    );

    let outcome = run_suite(config)?;
    print_suite_outcome(&outcome, "PASS", "passed");
    if outcome.passed() {
        Ok(())
    } else {
        Err(HarnessError::SuiteFailed {
            failed: outcome.report.failed,
            total: outcome.report.total,
            report_path: outcome.report_path,
        })
    }
}

fn run_update_suite_command(opts: &SuiteOpts) -> Result<(), HarnessError> {
    let config = suite_config_from_opts(opts)?;
    logger::info!(
        "Suite update: running {} case(s) with jobs={}, renderer_path={}, output_root={}",
        config.cases.len(),
        config.jobs,
        config.runner.renderer_path.display(),
        config.runner.output_root.display()
    );

    let outcome = update_suite(config)?;
    print_suite_outcome(&outcome, "UPDATE", "updated");
    if outcome.passed() {
        Ok(())
    } else {
        Err(HarnessError::SuiteFailed {
            failed: outcome.report.failed,
            total: outcome.report.total,
            report_path: outcome.report_path,
        })
    }
}

/// Caps an omitted `--jobs` at one renderer process per available core.
fn default_suite_jobs(case_count: usize) -> usize {
    let cores = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    case_count.max(1).min(cores.max(1))
}

fn suite_config_from_opts(opts: &SuiteOpts) -> Result<SuiteConfig, HarnessError> {
    let cases = select_cases(&opts.case)?;
    let case_count = cases.len();
    let renderer_path = match &opts.renderer {
        Some(p) => p.clone(),
        None => resolve_renderer_path(BuildProfile::from_flags(opts.release, opts.dev_fast)),
    };
    let mut runner = RunnerConfig::with_defaults(renderer_path);
    runner.timeout = Duration::from_secs(opts.timeout_seconds);
    runner.interval_ms = opts.interval_ms;
    runner.verbose_renderer = opts.verbose_renderer;
    runner.output_root = opts.output_root.clone().unwrap_or_else(default_output_root);

    let jobs = opts
        .jobs
        .unwrap_or_else(|| default_suite_jobs(case_count))
        .max(1);
    Ok(SuiteConfig {
        cases,
        runner,
        jobs,
    })
}

fn print_suite_outcome(
    outcome: &crate::scene_dsl::suite::SuiteRunOutcome,
    success_label: &str,
    result_label: &str,
) {
    if let Some(warning) = outcome.report.quarantine_warning.as_deref() {
        println!("{}", crate::scene_dsl::warn_line(warning));
    }
    for case in &outcome.report.cases {
        if crate::scene_dsl::warp_pbs_is_expected(case) {
            println!(
                "EXPECTED FAIL {} ({}) {}",
                case.name,
                case.artifacts_dir.display(),
                crate::scene_dsl::WARP_EXPECTED_FAILURE_LABEL
            );
        } else if crate::scene_dsl::case_is_quarantined_216(case) {
            println!(
                "{}",
                crate::scene_dsl::quarantined_line(&case.name, &case.artifacts_dir)
            );
        } else if case.passed {
            println!(
                "{} {} ({})",
                success_label,
                case.name,
                case.artifacts_dir.display()
            );
        } else {
            println!(
                "FAIL {} ({}) {}",
                case.name,
                case.artifacts_dir.display(),
                case.error.as_deref().unwrap_or("case failed")
            );
        }
    }
    println!("Suite report: {}", outcome.report_path.display());
    println!("{}", format_suite_result(&outcome.report, result_label));
    println!("{}", crate::scene_dsl::suite::SUITE_READINESS_SUMMARY);
}

fn format_suite_result(
    report: &crate::scene_dsl::suite::SuiteReport,
    result_label: &str,
) -> String {
    let passed = report.passed;
    let total = report.total;
    let expected = report.expected_failures;
    let quarantined = report.quarantined;
    let expected_label = crate::scene_dsl::WARP_EXPECTED_FAILURE_LABEL;
    match (expected, quarantined) {
        (0, 0) => format!("Suite result: {passed}/{total} {result_label}"),
        (_, 0) => {
            format!("Suite result: {passed}/{total} {result_label}, {expected} {expected_label}")
        }
        (0, _) => {
            format!("Suite result: {passed}/{total} {result_label}, {quarantined} quarantined")
        }
        (_, _) => format!(
            "Suite result: {passed}/{total} {result_label}, {expected} {expected_label}, {quarantined} quarantined"
        ),
    }
}

/// Cargo build profile selecting which `target/<profile>/renderide-renderer` binary to spawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BuildProfile {
    /// `target/debug/renderide-renderer` -- default `cargo build` profile.
    Debug,
    /// `target/release/renderide-renderer` -- `cargo build --release`.
    Release,
    /// `target/dev-fast/renderide-renderer` -- the project's `dev-fast` workspace profile.
    DevFast,
}

impl BuildProfile {
    /// Resolves the profile from the mutually-exclusive `--release` / `--dev-fast` CLI flags.
    const fn from_flags(release: bool, dev_fast: bool) -> Self {
        if dev_fast {
            Self::DevFast
        } else if release {
            Self::Release
        } else {
            Self::Debug
        }
    }

    /// Subdirectory name under `target/` for this profile.
    const fn target_dir(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
            Self::DevFast => "dev-fast",
        }
    }
}

fn default_renderer_path(profile: BuildProfile) -> PathBuf {
    let exe = if cfg!(windows) {
        "renderide-renderer.exe"
    } else {
        "renderide-renderer"
    };
    PathBuf::from("target").join(profile.target_dir()).join(exe)
}

/// Picks a renderer next to this binary when no `--release` / `--dev-fast` flags are set, so
/// e.g. `target/dev-fast/renderide-test` uses `target/dev-fast/renderide-renderer` by default.
fn resolve_renderer_path(profile: BuildProfile) -> PathBuf {
    if profile != BuildProfile::Debug {
        return default_renderer_path(profile);
    }
    if let Some(p) = renderide_next_to_this_test_binary() {
        return p;
    }
    default_renderer_path(BuildProfile::Debug)
}

fn renderide_next_to_this_test_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let name = exe.file_name()?.to_str()?;
    if name != "renderide-test" && name != "renderide-test.exe" {
        return None;
    }
    let profile_dir = exe.parent()?;
    let under_target = profile_dir.parent()?;
    if under_target.file_name() != Some(std::ffi::OsStr::new("target")) {
        return None;
    }
    let candidate = profile_dir.join(if cfg!(windows) {
        "renderide-renderer.exe"
    } else {
        "renderide-renderer"
    });
    candidate.is_file().then_some(candidate)
}

#[cfg(test)]
mod cli_tests {
    use std::path::PathBuf;

    use clap::{CommandFactory, Parser, error::ErrorKind};

    use super::{
        BuildProfile, Cli, default_renderer_path, default_suite_jobs, resolve_renderer_path,
    };

    #[test]
    fn default_suite_jobs_stays_within_available_parallelism() {
        let cores = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1);
        assert_eq!(default_suite_jobs(0), 1);
        assert_eq!(default_suite_jobs(1), 1);
        assert_eq!(default_suite_jobs(10_000), cores);
        assert!(default_suite_jobs(cores.saturating_add(1)) <= cores);
    }

    #[test]
    fn cli_exposes_only_suite_subcommands() {
        let names: Vec<_> = Cli::command()
            .get_subcommands()
            .map(|cmd| cmd.get_name().to_string())
            .collect();

        assert_eq!(names, ["check-suite", "update-suite"]);
    }

    #[test]
    fn help_subcommand_is_disabled_but_help_flag_still_works() {
        let help_subcommand = Cli::try_parse_from(["renderide-test", "help"])
            .expect_err("help subcommand should be disabled");
        assert_ne!(help_subcommand.kind(), ErrorKind::DisplayHelp);

        let help_flag = Cli::try_parse_from(["renderide-test", "--help"])
            .expect_err("--help should print clap help");
        assert_eq!(help_flag.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn default_renderer_path_profiles_and_exe_name() {
        assert_eq!(
            default_renderer_path(BuildProfile::DevFast),
            PathBuf::from("target")
                .join("dev-fast")
                .join(expected_exe())
        );
        assert_eq!(
            default_renderer_path(BuildProfile::Release),
            PathBuf::from("target").join("release").join(expected_exe())
        );
        assert_eq!(
            default_renderer_path(BuildProfile::Debug),
            PathBuf::from("target").join("debug").join(expected_exe())
        );
    }

    #[test]
    fn resolve_renderer_path_matches_explicit_profiles() {
        assert_eq!(
            resolve_renderer_path(BuildProfile::Release),
            default_renderer_path(BuildProfile::Release)
        );
        assert_eq!(
            resolve_renderer_path(BuildProfile::DevFast),
            default_renderer_path(BuildProfile::DevFast)
        );
    }

    #[test]
    fn build_profile_from_flags_maps_correctly() {
        assert_eq!(BuildProfile::from_flags(false, false), BuildProfile::Debug);
        assert_eq!(BuildProfile::from_flags(true, false), BuildProfile::Release);
        assert_eq!(BuildProfile::from_flags(false, true), BuildProfile::DevFast);
        // dev_fast wins when both flags are set; clap rejects that combination at the CLI layer.
        assert_eq!(BuildProfile::from_flags(true, true), BuildProfile::DevFast);
    }

    #[test]
    fn suite_result_line_counts_quarantine_apart_from_expected_failures() {
        use crate::scene_dsl::WARP_EXPECTED_FAILURE_LABEL;
        use crate::scene_dsl::suite::{SuiteCaseReport, SuiteReport};

        fn case(
            name: &str,
            passed: bool,
            result: Option<&str>,
            status: Option<&str>,
            reason: Option<&str>,
        ) -> SuiteCaseReport {
            SuiteCaseReport {
                name: name.to_string(),
                passed,
                artifacts_dir: PathBuf::from(name),
                report_json: PathBuf::from(name),
                error: None,
                result: result.map(str::to_string),
                status: status.map(str::to_string),
                reason: reason.map(str::to_string),
            }
        }

        let pass = case("a", true, None, None, None);
        let expected = case("b", false, Some(WARP_EXPECTED_FAILURE_LABEL), None, None);
        let quarantined = case(
            "c",
            false,
            None,
            Some("quarantined"),
            Some("quarantined:#216"),
        );
        let passes = SuiteReport::from_cases(vec![pass.clone()]);
        assert_eq!(
            super::format_suite_result(&passes, "passed"),
            "Suite result: 1/1 passed"
        );
        let with_expected = SuiteReport::from_cases(vec![pass.clone(), expected.clone()]);
        assert_eq!(
            super::format_suite_result(&with_expected, "passed"),
            "Suite result: 1/2 passed, 1 expected failure (WARP, #212)"
        );
        let with_quarantine = SuiteReport::from_cases(vec![pass.clone(), quarantined.clone()]);
        assert_eq!(
            super::format_suite_result(&with_quarantine, "passed"),
            "Suite result: 1/2 passed, 1 quarantined"
        );
        let both = SuiteReport::from_cases(vec![pass, expected, quarantined]);
        assert_eq!(
            super::format_suite_result(&both, "passed"),
            "Suite result: 1/3 passed, 1 expected failure (WARP, #212), 1 quarantined"
        );
    }

    fn expected_exe() -> &'static str {
        if cfg!(windows) {
            "renderide-renderer.exe"
        } else {
            "renderide-renderer"
        }
    }
}
