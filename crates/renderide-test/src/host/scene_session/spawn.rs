//! Renderer process lifecycle: arg construction, spawn, RAII guard.

use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::error::HarnessError;

use super::super::ipc_setup::DEFAULT_QUEUE_CAPACITY_BYTES;
use super::config::SceneSessionConfig;

/// RAII-guarded spawned renderer process. [`Drop`] kills the child if still running.
pub(super) struct SpawnedRenderer {
    /// Live child process; `None` after a clean shutdown via the shutdown helper.
    pub child: Option<Child>,
}

impl Drop for SpawnedRenderer {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            logger::warn!("SpawnedRenderer: dropping with live child; killing");
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Figment key for the renderer `[rendering] physically_based` setting.
///
/// `--ignore-config` skips the TOML file and still merges `RENDERIDE_*` variables, so the
/// harness can select the mode without a config file.
const PHYSICALLY_BASED_ENV: &str = "RENDERIDE_RENDERING__PHYSICALLY_BASED";

/// Must match `SHADOW_ATLAS_PASS_LOG_ENV` in the renderer headless driver.
///
/// The suite reads `{png}.atlas-passes` and the append-only pass log. A normal headless run
/// leaves those files unwritten.
const SHADOW_ATLAS_PASS_LOG_ENV: &str = "RENDERIDE_SHADOW_ATLAS_PASS_LOG";

/// Spawns the renderer binary with all flags wired up for headless operation.
pub(super) fn spawn_renderer(
    cfg: &SceneSessionConfig,
    queue_name: &str,
    backing_dir: &Path,
) -> Result<SpawnedRenderer, HarnessError> {
    let mut cmd = Command::new(&cfg.renderer_path);
    let args = renderer_spawn_args(cfg, queue_name);
    cmd.args(&args);
    cmd.env("RENDERIDE_INTERPROCESS_DIR", backing_dir);
    cmd.env(SHADOW_ATLAS_PASS_LOG_ENV, "1");
    let (physically_based_key, physically_based_value) = physically_based_env(cfg.physically_based);
    cmd.env(physically_based_key, physically_based_value);

    if cfg.verbose_renderer {
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }

    logger::info!(
        "Spawning renderer: {} {}",
        cfg.renderer_path.display(),
        args.join(" "),
    );

    let child = cmd.spawn().map_err(HarnessError::SpawnRenderer)?;
    Ok(SpawnedRenderer { child: Some(child) })
}

/// Environment pair that selects physically based light upload for one renderer process.
fn physically_based_env(enabled: bool) -> (&'static str, &'static str) {
    (PHYSICALLY_BASED_ENV, if enabled { "true" } else { "false" })
}

/// Builds the renderer process arguments for one harness session.
pub fn renderer_spawn_args(cfg: &SceneSessionConfig, queue_name: &str) -> Vec<String> {
    vec![
        "--headless".to_string(),
        "--headless-output".to_string(),
        cfg.output_path.display().to_string(),
        "--headless-resolution".to_string(),
        format!("{}x{}", cfg.width, cfg.height),
        "--headless-interval-ms".to_string(),
        cfg.interval_ms.to_string(),
        "-QueueName".to_string(),
        queue_name.to_string(),
        "-QueueCapacity".to_string(),
        DEFAULT_QUEUE_CAPACITY_BYTES.to_string(),
        "-LogLevel".to_string(),
        "debug".to_string(),
        "--ignore-config".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::super::super::ipc_setup::DEFAULT_QUEUE_CAPACITY_BYTES;
    use super::{SceneSessionConfig, renderer_spawn_args};

    fn minimal_config() -> SceneSessionConfig {
        SceneSessionConfig {
            renderer_path: PathBuf::from("target/debug/renderide-renderer"),
            output_path: PathBuf::from("target/headless.png"),
            width: 64,
            height: 32,
            interval_ms: 250,
            timeout: Duration::from_secs(5),
            verbose_renderer: false,
            physically_based: false,
        }
    }

    #[test]
    fn physically_based_env_names_the_rendering_flag() {
        assert_eq!(
            super::physically_based_env(false),
            ("RENDERIDE_RENDERING__PHYSICALLY_BASED", "false")
        );
        assert_eq!(
            super::physically_based_env(true),
            ("RENDERIDE_RENDERING__PHYSICALLY_BASED", "true")
        );
    }

    #[test]
    fn spawn_args_preserve_required_ipc_and_headless_values() {
        let args = renderer_spawn_args(&minimal_config(), "queue-a");
        let capacity = DEFAULT_QUEUE_CAPACITY_BYTES.to_string();
        assert_eq!(args[0], "--headless");
        assert!(
            args.windows(2)
                .any(|w| w == ["--headless-output", "target/headless.png"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["--headless-resolution", "64x32"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["--headless-interval-ms", "250"])
        );
        assert!(args.windows(2).any(|w| w == ["-QueueName", "queue-a"]));
        assert!(
            args.windows(2)
                .any(|w| w[0] == "-QueueCapacity" && w[1] == capacity)
        );
    }

    #[test]
    fn spawn_args_include_log_level_debug() {
        let args = renderer_spawn_args(&minimal_config(), "q");
        assert!(args.windows(2).any(|w| w == ["-LogLevel", "debug"]));
    }

    #[test]
    fn spawn_args_include_ignore_config() {
        let args = renderer_spawn_args(&minimal_config(), "q");
        assert!(args.iter().any(|a| a == "--ignore-config"));
    }

    #[test]
    fn spawn_args_reflect_varied_resolution_and_interval() {
        let mut cfg = minimal_config();
        cfg.width = 1920;
        cfg.height = 1080;
        cfg.interval_ms = 33;
        let args = renderer_spawn_args(&cfg, "q");
        assert!(
            args.windows(2)
                .any(|w| w == ["--headless-resolution", "1920x1080"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["--headless-interval-ms", "33"])
        );
    }

    #[test]
    fn spawn_args_pair_flag_with_value_in_order() {
        let cfg = minimal_config();
        let args = renderer_spawn_args(&cfg, "queue-x");
        let pairs: &[(&str, &str)] = &[
            ("--headless-output", "target/headless.png"),
            ("--headless-resolution", "64x32"),
            ("--headless-interval-ms", "250"),
            ("-QueueName", "queue-x"),
            ("-QueueCapacity", &DEFAULT_QUEUE_CAPACITY_BYTES.to_string()),
            ("-LogLevel", "debug"),
        ];
        for (flag, expected_value) in pairs {
            let pos = args
                .iter()
                .position(|a| a == flag)
                .unwrap_or_else(|| panic!("flag {flag} missing in {args:?}"));
            assert!(pos + 1 < args.len(), "flag {flag} has no value slot");
            assert_eq!(&args[pos + 1], expected_value, "flag {flag} value mismatch");
        }
    }
}
