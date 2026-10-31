//! Command-line interface for the golden-image harness.
//!
//! `run` is the supported command. `check-suite` is a hidden alias of `run` and
//! `update-suite` is a hidden alias of `run --bless`. CI keeps calling `check-suite`.

mod print;
mod run;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::exit;

use print::Presentation;

/// CLI entry point.
pub fn run() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => err.exit(),
    };
    if let Err(err) = crate::logging::init_renderer_test_logging() {
        print::startup_error(&format!("failed to initialize logging: {err}"));
        return ExitCode::from(exit::EXIT_ENVIRONMENT);
    }
    let finished = dispatch(cli);
    print::emit(&Presentation {
        json: finished.json,
        deprecation: finished.deprecation,
        notes: &finished.notes,
        human: &finished.human,
        report: finished.report.as_ref(),
    });
    if finished.exit_code == exit::EXIT_OK {
        logger::info!("renderide-test completed successfully");
    } else {
        logger::error!("renderide-test failed with exit {}", finished.exit_code);
    }
    logger::flush();
    ExitCode::from(finished.exit_code)
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
    /// Run scenes. No names means every scene for this OS. Names may be globs.
    Run {
        /// Scene names or globs. An unknown name exits 2.
        #[arg(value_name = "SCENE")]
        scenes: Vec<String>,
        /// Print one JSON suite report on stdout. Human text goes to stderr.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Capture and write goldens. Refuses #212 and #216 cases.
        #[arg(long, default_value_t = false)]
        bless: bool,
        /// Allow bless to overwrite an existing golden.
        #[arg(long, default_value_t = false)]
        force: bool,
        /// Repeat the run. A disagreement is flaky and exits 6.
        #[arg(long, default_value_t = 1)]
        repeat: u32,
        /// Suite harness options.
        #[command(flatten)]
        suite: SuiteOpts,
    },
    /// List scene names for this OS.
    List {
        /// Print `{"scenes":[...]}` on stdout.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Opt-in case group.
        #[arg(long = "group", value_name = "NAME")]
        group: Option<String>,
    },
    /// Deprecated alias of `run`. CI still calls this.
    #[command(hide = true)]
    CheckSuite {
        /// Suite harness options.
        #[command(flatten)]
        suite: SuiteOpts,
    },
    /// Deprecated alias of `run --bless`.
    #[command(hide = true)]
    UpdateSuite {
        /// Suite harness options.
        #[command(flatten)]
        suite: SuiteOpts,
    },
}

#[derive(Parser, Debug, Clone)]
pub(super) struct SuiteOpts {
    /// Path to the renderer binary to spawn (defaults to `target/{profile}/renderide-renderer`).
    #[arg(long)]
    renderer: Option<PathBuf>,
    /// Use the `dev-fast` profile renderer binary (`target/dev-fast/renderide-renderer`).
    #[arg(long, default_value_t = false, conflicts_with = "release")]
    dev_fast: bool,
    /// Use the release-mode renderer binary (`target/release/renderide-renderer`).
    #[arg(long, default_value_t = false, conflicts_with = "dev_fast")]
    release: bool,
    /// Case name to run. Repeat to run a subset; omitted means every case in the
    /// selected group, or every registered case when `--group` is omitted.
    ///
    /// Names from the opt-in `warp_216_debug` group resolve here without `--group`.
    #[arg(long = "case", value_name = "NAME")]
    case: Vec<String>,
    /// Opt-in case group. `warp_216_debug` is the #216 lighting-term set and is not
    /// part of the default registry.
    #[arg(long = "group", value_name = "NAME")]
    group: Option<String>,
    /// Maximum number of cases to run concurrently.
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
    /// Set when `--json` is active so the renderer child's stdout goes to stderr.
    #[arg(skip)]
    json_stdout: bool,
}

fn dispatch(cli: Cli) -> run::Finished {
    match cli.command {
        Command::List { json, group } => run::list_command(json, group.as_deref()),
        Command::Run {
            scenes,
            json,
            bless,
            force,
            repeat,
            mut suite,
        } => {
            suite.json_stdout = json;
            run::run_command(run::RunFlags {
                scenes: &scenes,
                case_flags: &suite.case,
                opts: &suite,
                json,
                bless,
                force,
                repeat,
                deprecation: false,
            })
        }
        Command::CheckSuite { mut suite } => {
            suite.json_stdout = false;
            run::run_command(run::RunFlags {
                scenes: &[],
                case_flags: &suite.case,
                opts: &suite,
                json: false,
                bless: false,
                force: false,
                repeat: 1,
                deprecation: true,
            })
        }
        Command::UpdateSuite { mut suite } => {
            suite.json_stdout = false;
            run::run_command(run::RunFlags {
                scenes: &[],
                case_flags: &suite.case,
                opts: &suite,
                json: false,
                bless: true,
                force: false,
                repeat: 1,
                deprecation: false,
            })
        }
    }
}

/// Caps an omitted `--jobs` at one renderer process per available core.
pub(super) fn default_suite_jobs(case_count: usize) -> usize {
    let cores = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    case_count.max(1).min(cores.max(1))
}

/// Cargo build profile selecting which `target/<profile>/renderide-renderer` binary to spawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BuildProfile {
    /// `target/debug/renderide-renderer` -- default `cargo build` profile.
    Debug,
    /// `target/release/renderide-renderer` -- `cargo build --release`.
    Release,
    /// `target/dev-fast/renderide-renderer` -- the project's `dev-fast` workspace profile.
    DevFast,
}

impl BuildProfile {
    /// Resolves the profile from the mutually-exclusive `--release` / `--dev-fast` CLI flags.
    pub(super) const fn from_flags(release: bool, dev_fast: bool) -> Self {
        if dev_fast {
            Self::DevFast
        } else if release {
            Self::Release
        } else {
            Self::Debug
        }
    }

    const fn target_dir(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
            Self::DevFast => "dev-fast",
        }
    }
}

pub(super) fn default_renderer_path(profile: BuildProfile) -> PathBuf {
    let exe = if cfg!(windows) {
        "renderide-renderer.exe"
    } else {
        "renderide-renderer"
    };
    PathBuf::from("target").join(profile.target_dir()).join(exe)
}

pub(super) fn resolve_renderer_path(profile: BuildProfile) -> PathBuf {
    if profile != BuildProfile::Debug {
        return default_renderer_path(profile);
    }
    renderide_next_to_this_test_binary()
        .unwrap_or_else(|| default_renderer_path(BuildProfile::Debug))
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
    fn cli_exposes_run_list_and_hidden_aliases() {
        let names: Vec<_> = Cli::command()
            .get_subcommands()
            .map(|cmd| cmd.get_name().to_string())
            .collect();
        assert_eq!(names, ["run", "list", "check-suite", "update-suite"]);
        let command = Cli::command();
        let check = command.find_subcommand("check-suite").expect("alias");
        assert!(check.is_hide_set());
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
        assert_eq!(BuildProfile::from_flags(true, true), BuildProfile::DevFast);
    }

    fn expected_exe() -> &'static str {
        if cfg!(windows) {
            "renderide-renderer.exe"
        } else {
            "renderide-renderer"
        }
    }
}
