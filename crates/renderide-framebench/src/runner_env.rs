//! Runner identity recorded in schema-2 reports: OS, CPU model, Vulkan ICD and Rayon threads.
//!
//! This module only *reports* what the renderer child will inherit. It never selects an ICD
//! and never mutates the process environment (`std::env::set_var` is unsafe in edition 2024).
//!
//! TODO(T3): swap [`detect`] for the shared environment helper from Bridge's T3
//! (`renderide-harness-env`: lavapipe probe, `--icd`, `RAYON_NUM_THREADS` child default, the
//! stderr `env:` line). Keep the [`RunnerInfo`] JSON shape; T3 may add `"auto"` as an
//! [`IcdInfo::source`] value when it starts selecting lavapipe itself.

use serde::{Deserialize, Serialize};

/// Vulkan loader variable that wins when both are set (loader 1.3.234+).
const VK_DRIVER_FILES: &str = "VK_DRIVER_FILES";
/// Older loader variable, still honoured when `VK_DRIVER_FILES` is unset.
const VK_ICD_FILENAMES: &str = "VK_ICD_FILENAMES";
/// Rayon's global pool size override, inherited by the renderer child.
const RAYON_NUM_THREADS: &str = "RAYON_NUM_THREADS";

/// Machine the report was measured on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnerInfo {
    /// `std::env::consts::OS`.
    pub os: String,
    /// CPU model string, or `unavailable`. Lavapipe numbers depend on it (EPYC 7763 vs 9V74).
    pub cpu_model: String,
    /// Vulkan ICD the renderer child inherits.
    pub icd: IcdInfo,
    /// Rayon worker count the renderer child will use.
    pub rayon_threads: u32,
    /// `env` when `RAYON_NUM_THREADS` set it, `default` when Rayon falls back to the CPU count.
    pub rayon_threads_source: String,
}

/// Vulkan ICD selection as seen from the environment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IcdInfo {
    /// ICD manifest path(s) exactly as set, or null when the loader searches its defaults.
    pub path: Option<String>,
    /// `env` when a loader variable set it, `loader_default` otherwise.
    pub source: String,
    /// Which variable supplied `path` (`VK_DRIVER_FILES` or `VK_ICD_FILENAMES`), or null.
    pub var: Option<String>,
}

/// Reads the runner identity from this process. See the module TODO about T3.
pub fn detect() -> RunnerInfo {
    let available = std::thread::available_parallelism()
        .ok()
        .and_then(|n| u32::try_from(n.get()).ok())
        .unwrap_or(1);
    from_lookup(
        |key| std::env::var(key).ok(),
        renderide_test::scene_dsl::host_info::cpu_model(),
        available,
    )
}

/// Pure core of [`detect`]: `lookup` reads an environment variable.
pub(crate) fn from_lookup(
    lookup: impl Fn(&str) -> Option<String>,
    cpu_model: String,
    available_parallelism: u32,
) -> RunnerInfo {
    let (rayon_threads, rayon_threads_source) = match lookup(RAYON_NUM_THREADS)
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|threads| *threads > 0)
    {
        Some(threads) => (threads, "env"),
        // Rayon treats unset, unparsable and 0 alike: one worker per available CPU.
        None => (available_parallelism.max(1), "default"),
    };
    RunnerInfo {
        os: std::env::consts::OS.to_string(),
        cpu_model,
        icd: icd_from_lookup(&lookup),
        rayon_threads,
        rayon_threads_source: rayon_threads_source.to_string(),
    }
}

fn icd_from_lookup(lookup: &impl Fn(&str) -> Option<String>) -> IcdInfo {
    for var in [VK_DRIVER_FILES, VK_ICD_FILENAMES] {
        if let Some(path) = lookup(var).filter(|value| !value.trim().is_empty()) {
            return IcdInfo {
                path: Some(path),
                source: "env".to_string(),
                var: Some(var.to_string()),
            };
        }
    }
    IcdInfo {
        path: None,
        source: "loader_default".to_string(),
        var: None,
    }
}

/// One-line summary for stderr, e.g. `env: os=linux cpu="..." icd=... (env:VK_DRIVER_FILES)`.
pub fn env_line(runner: &RunnerInfo) -> String {
    let icd = match (&runner.icd.path, &runner.icd.var) {
        (Some(path), Some(var)) => format!("{path} (env:{var})"),
        (Some(path), None) => format!("{path} ({})", runner.icd.source),
        (None, _) => format!("none ({})", runner.icd.source),
    };
    format!(
        "env: os={} cpu=\"{}\" icd={icd} rayon_threads={} ({})",
        runner.os, runner.cpu_model, runner.rayon_threads, runner.rayon_threads_source
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{env_line, from_lookup};

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect::<HashMap<_, _>>();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn vk_driver_files_wins_over_vk_icd_filenames() {
        let info = from_lookup(
            lookup(&[
                ("VK_DRIVER_FILES", "/a/lvp_icd.x86_64.json"),
                ("VK_ICD_FILENAMES", "/b/other.json"),
            ]),
            "cpu".to_string(),
            8,
        );
        assert_eq!(info.icd.path.as_deref(), Some("/a/lvp_icd.x86_64.json"));
        assert_eq!(info.icd.source, "env");
        assert_eq!(info.icd.var.as_deref(), Some("VK_DRIVER_FILES"));
    }

    #[test]
    fn vk_icd_filenames_is_used_alone() {
        let info = from_lookup(
            lookup(&[
                ("VK_ICD_FILENAMES", "/b/lvp.json"),
                ("VK_DRIVER_FILES", " "),
            ]),
            "cpu".to_string(),
            8,
        );
        assert_eq!(info.icd.path.as_deref(), Some("/b/lvp.json"));
        assert_eq!(info.icd.var.as_deref(), Some("VK_ICD_FILENAMES"));
    }

    #[test]
    fn no_icd_variable_is_loader_default() {
        let info = from_lookup(lookup(&[]), "cpu".to_string(), 8);
        assert_eq!(info.icd.path, None);
        assert_eq!(info.icd.source, "loader_default");
        assert_eq!(info.icd.var, None);
    }

    #[test]
    fn rayon_threads_from_env_or_default() {
        let env = from_lookup(lookup(&[("RAYON_NUM_THREADS", "2")]), String::new(), 8);
        assert_eq!(
            (env.rayon_threads, env.rayon_threads_source.as_str()),
            (2, "env")
        );
        for bad in ["0", "abc", ""] {
            let info = from_lookup(lookup(&[("RAYON_NUM_THREADS", bad)]), String::new(), 6);
            assert_eq!(
                (info.rayon_threads, info.rayon_threads_source.as_str()),
                (6, "default"),
                "RAYON_NUM_THREADS={bad:?}"
            );
        }
        let unset = from_lookup(lookup(&[]), String::new(), 0);
        assert_eq!(unset.rayon_threads, 1);
    }

    #[test]
    fn env_line_names_every_field() {
        let info = from_lookup(
            lookup(&[
                ("VK_ICD_FILENAMES", "/x/lvp.json"),
                ("RAYON_NUM_THREADS", "2"),
            ]),
            "AMD EPYC 7763 64-Core Processor".to_string(),
            8,
        );
        let line = env_line(&info);
        assert!(line.starts_with("env: os="), "{line}");
        assert!(
            line.contains("cpu=\"AMD EPYC 7763 64-Core Processor\""),
            "{line}"
        );
        assert!(
            line.contains("icd=/x/lvp.json (env:VK_ICD_FILENAMES)"),
            "{line}"
        );
        assert!(line.contains("rayon_threads=2 (env)"), "{line}");
    }
}
