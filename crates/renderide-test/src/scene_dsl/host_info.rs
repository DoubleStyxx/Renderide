//! Runner CPU and adapter identity recorded with each headless case.
//!
//! The gray `pbs_lit` frame follows the runner CPU. The suite log and `report.json` keep the
//! model next to the wgpu adapter line so a later comparison does not mix machines.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// CPU model and the headless wgpu adapter line for one case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostInfo {
    /// `model name` from `/proc/cpuinfo`, or the platform brand string.
    pub cpu_model: String,
    /// Text after `wgpu adapter selected (headless):`, or `unavailable`.
    pub adapter: String,
}

/// Logs the runner CPU once, before the suite fans out.
pub fn log_suite_host() {
    let cpu = cpu_model();
    logger::info!("Headless suite host: cpu={cpu}");
}

/// Reads the runner CPU and the adapter line for the renderer that wrote `output_png`.
pub fn capture(output_png: &Path) -> HostInfo {
    let cpu_model = cpu_model();
    let adapter = adapter_for_output(output_png);
    logger::info!(
        "Headless suite adapter: output={} cpu={cpu_model} adapter={adapter}",
        output_png.display()
    );
    HostInfo { cpu_model, adapter }
}

fn cpu_model() -> String {
    static CPU: OnceLock<String> = OnceLock::new();
    CPU.get_or_init(read_cpu_model).clone()
}

fn read_cpu_model() -> String {
    if let Ok(text) = std::fs::read_to_string("/proc/cpuinfo")
        && let Some(model) = cpu_model_from_proc_cpuinfo(&text)
    {
        return model;
    }
    command_cpu_model().unwrap_or_else(|| "unavailable".to_string())
}

/// Returns the first `model name` value in `/proc/cpuinfo` text.
pub fn cpu_model_from_proc_cpuinfo(text: &str) -> Option<String> {
    for line in text.lines() {
        let Some(rest) = line.strip_prefix("model name") else {
            continue;
        };
        let Some((_, value)) = rest.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn command_cpu_model() -> Option<String> {
    command_stdout("sysctl", &["-n", "machdep.cpu.brand_string"])
}

#[cfg(target_os = "windows")]
fn command_cpu_model() -> Option<String> {
    command_stdout(
        "powershell",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name)",
        ],
    )
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn command_cpu_model() -> Option<String> {
    None
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().find(|line| !line.trim().is_empty())?;
    let line = line.trim();
    (!line.is_empty()).then(|| line.to_string())
}

fn adapter_for_output(output_png: &Path) -> String {
    let needle = format!("Headless mode: output={}", output_png.display());
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for dir in renderer_log_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if !text.contains(&needle) {
                continue;
            }
            let Some(adapter) = adapter_from_renderer_log(&text) else {
                continue;
            };
            let modified = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if best.as_ref().is_none_or(|(time, _)| modified >= *time) {
                best = Some((modified, adapter));
            }
        }
    }
    best.map(|(_, adapter)| adapter)
        .unwrap_or_else(|| "unavailable".to_string())
}

fn renderer_log_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("logs/renderer")];
    let resolved = logger::log_dir_for(logger::LogComponent::Renderer);
    if !dirs.iter().any(|dir| dir == &resolved) {
        dirs.push(resolved);
    }
    dirs
}

/// Returns the text after the last headless adapter line in a renderer log.
pub fn adapter_from_renderer_log(text: &str) -> Option<String> {
    const MARKER: &str = "wgpu adapter selected (headless): ";
    text.lines()
        .rev()
        .find_map(|line| {
            line.find(MARKER)
                .map(|index| line[index + MARKER.len()..].trim().to_string())
        })
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_cpuinfo_model_name_is_trimmed() {
        let text = "processor\t: 0\nmodel name\t: AMD EPYC 7763 64-Core Processor\nflags\t: fpu\n";
        assert_eq!(
            cpu_model_from_proc_cpuinfo(text).as_deref(),
            Some("AMD EPYC 7763 64-Core Processor")
        );
    }

    #[test]
    fn adapter_line_keeps_name_type_and_backend() {
        let text = "\
Headless mode: output=target/renderide-test-out/pbs_lit_material_matrix/actual.png size=320x320
wgpu adapter selected (headless): llvmpipe (LLVM 20.1.2, 256 bits) type=Cpu backend=Vulkan (preference=HighPerformance)
";
        assert_eq!(
            adapter_from_renderer_log(text).as_deref(),
            Some(
                "llvmpipe (LLVM 20.1.2, 256 bits) type=Cpu backend=Vulkan (preference=HighPerformance)"
            )
        );
    }

    #[test]
    fn capture_reads_a_cpu_model() {
        let host = capture(Path::new("target/missing-case/actual.png"));
        assert!(!host.cpu_model.is_empty());
        assert_eq!(host.adapter, "unavailable");
    }
}
