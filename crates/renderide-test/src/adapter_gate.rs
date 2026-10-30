//! Labels a suite case from the adapter the renderer actually selected.
//!
//! Lavapipe stays the behavior gate. `pbs_lit_material_matrix` is an adapter
//! comparison on WARP and on Metal. A real GPU keeps that case on the behavior
//! gate. The decision takes the adapter as input, not the build target.

use std::fs;
use std::path::Path;

/// Case that is an adapter comparison on WARP and Metal.
pub(crate) const PBS_LIT: &str = "pbs_lit_material_matrix";

/// JSON value for a lavapipe behavior gate.
pub(crate) const BEHAVIOR: &str = "behavior";

/// JSON value for a WARP or Metal comparison.
pub(crate) const ADAPTER_COMPARISON: &str = "adapter_comparison";

/// Which gate a case was judged against.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Gate {
    /// The capture is the behavior gate.
    Behavior,
    /// The capture compares adapters. It is not the behavior gate.
    AdapterComparison,
}

impl Gate {
    /// Stable `report.json` value.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Behavior => BEHAVIOR,
            Self::AdapterComparison => ADAPTER_COMPARISON,
        }
    }
}

/// Adapter selected by the renderer, plus the vendor id when a candidate line has it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ObservedAdapter {
    /// Adapter name from the selected line.
    pub name: String,
    /// `type=` value, such as `Cpu`.
    pub device_type: String,
    /// `backend=` value, such as `Dx12` or `Metal`.
    pub backend: String,
    /// PCI vendor from the matching candidate line.
    pub vendor: Option<u32>,
    /// PCI device from the matching candidate line.
    pub device: Option<u32>,
}

impl ObservedAdapter {
    /// Text printed on the `ADAPTER` line and stored in `report.json`.
    pub(crate) fn label(&self) -> String {
        match (self.vendor, self.device) {
            (Some(vendor), Some(device)) => format!(
                "{} type={} backend={} vendor=0x{vendor:04x} device=0x{device:04x}",
                self.name, self.device_type, self.backend
            ),
            _ => format!(
                "{} type={} backend={}",
                self.name, self.device_type, self.backend
            ),
        }
    }

    fn is_warp(&self) -> bool {
        match (self.vendor, self.device) {
            (Some(vendor), Some(device)) => crate::scene_dsl::adapter_is_warp(vendor, device),
            _ => false,
        }
    }

    fn is_metal(&self) -> bool {
        self.backend == "Metal"
    }
}

/// `pbs_lit` on WARP or Metal is an adapter comparison. Everything else is behavior.
pub(crate) fn gate_for(case_name: &str, adapter: Option<&ObservedAdapter>) -> Gate {
    let Some(adapter) = adapter else {
        return Gate::Behavior;
    };
    if case_name == PBS_LIT && (adapter.is_warp() || adapter.is_metal()) {
        Gate::AdapterComparison
    } else {
        Gate::Behavior
    }
}

/// Reads the selected adapter from renderer logs.
///
/// Every `wgpu adapter selected` line must name the same adapter. A missing
/// line, or two different selected adapters, is `None`.
pub(crate) fn adapter_from_logs(logs: &[&str]) -> Option<ObservedAdapter> {
    let mut selected: Option<(String, String, String)> = None;
    let mut candidates = Vec::new();
    for log in logs {
        for line in log.lines() {
            if line.contains("wgpu adapter selected") {
                let parsed = parse_selected(line)?;
                if let Some(previous) = &selected
                    && previous != &parsed
                {
                    return None;
                }
                selected = Some(parsed);
            } else if let Some(candidate) = parse_candidate(line) {
                candidates.push(candidate);
            }
        }
    }
    let (name, device_type, backend) = selected?;
    let identity = candidates.iter().find(|candidate| {
        candidate.name == name
            && candidate.device_type == device_type
            && candidate.backend == backend
    });
    Some(ObservedAdapter {
        name,
        device_type,
        backend,
        vendor: identity.and_then(|candidate| candidate.vendor),
        device: identity.and_then(|candidate| candidate.device),
    })
}

/// Reads `*.log` under `dir`. An unreadable directory has no adapter.
pub(crate) fn adapter_from_directory(dir: &Path) -> Option<ObservedAdapter> {
    let entries = fs::read_dir(dir).ok()?;
    let mut logs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            logs.push(text);
        }
    }
    let borrowed: Vec<&str> = logs.iter().map(String::as_str).collect();
    adapter_from_logs(&borrowed)
}

fn parse_selected(line: &str) -> Option<(String, String, String)> {
    let rest = line.split_once("wgpu adapter selected")?.1;
    let rest = rest.split_once(':')?.1.trim();
    parse_name_type_backend(rest).map(|(name, device_type, backend)| {
        (
            name.to_string(),
            device_type.to_string(),
            backend.to_string(),
        )
    })
}

fn parse_candidate(line: &str) -> Option<ObservedAdapter> {
    let rest = line.split_once("wgpu adapter candidate:")?.1.trim();
    let (name, device_type, backend) = parse_name_type_backend(rest)?;
    Some(ObservedAdapter {
        name: name.to_string(),
        device_type: device_type.to_string(),
        backend: backend.to_string(),
        vendor: parse_hex_field(rest, "vendor="),
        device: parse_hex_field(rest, "device="),
    })
}

fn parse_name_type_backend(text: &str) -> Option<(&str, &str, &str)> {
    let type_at = text.find(" type=")?;
    let name = text[..type_at].trim();
    let after_type = &text[type_at + " type=".len()..];
    let backend_at = after_type.find(" backend=")?;
    let device_type = after_type[..backend_at].trim();
    let after_backend = &after_type[backend_at + " backend=".len()..];
    let backend = after_backend.split_whitespace().next()?.trim();
    if name.is_empty() || device_type.is_empty() || backend.is_empty() {
        return None;
    }
    Some((name, device_type, backend))
}

fn parse_hex_field(text: &str, key: &str) -> Option<u32> {
    let rest = text.split_once(key)?.1;
    let token = rest.split_whitespace().next()?;
    let hex = token.strip_prefix("0x")?;
    u32::from_str_radix(hex, 16).ok()
}

/// One case, as printed by check-suite.
pub(crate) struct PrintCase<'a> {
    /// Case identifier.
    pub name: &'a str,
    /// Whether the case passed.
    pub passed: bool,
    /// Per-case artifact directory.
    pub artifacts_dir: &'a Path,
    /// Failure text when the case failed.
    pub error: Option<&'a str>,
    /// Gate chosen from the selected adapter.
    pub gate: Gate,
    /// Selected adapter label. Empty when the adapter was not recorded.
    pub adapter: &'a str,
}

/// Lines check-suite prints for the cases, plus the summary line.
pub(crate) struct PrintedSuite {
    /// One line per case.
    pub case_lines: Vec<String>,
    /// `Suite result:` line. Failures are counted apart from either gate.
    pub summary: String,
}

/// Check-suite lines. A passing adapter comparison prints `ADAPTER` and the adapter name.
pub(crate) fn format_check_suite(cases: &[PrintCase<'_>]) -> PrintedSuite {
    let mut case_lines = Vec::with_capacity(cases.len());
    let mut behavior = 0usize;
    let mut adapter_comparison = 0usize;
    let mut failed = 0usize;
    for case in cases {
        if !case.passed {
            failed += 1;
            case_lines.push(format!(
                "FAIL {} ({}) {}",
                case.name,
                case.artifacts_dir.display(),
                case.error.unwrap_or("case failed")
            ));
            continue;
        }
        if case.gate == Gate::AdapterComparison {
            adapter_comparison += 1;
            case_lines.push(format!(
                "ADAPTER {} ({}) {}",
                case.name,
                case.artifacts_dir.display(),
                case.adapter
            ));
        } else {
            behavior += 1;
            case_lines.push(format!(
                "PASS {} ({})",
                case.name,
                case.artifacts_dir.display()
            ));
        }
    }
    PrintedSuite {
        case_lines,
        summary: summary_line(behavior, adapter_comparison, failed),
    }
}

fn summary_line(behavior: usize, adapter_comparison: usize, failed: usize) -> String {
    let split = format!("{behavior} behavior + {adapter_comparison} adapter comparison");
    if failed == 0 {
        format!("Suite result: {split}")
    } else {
        format!("Suite result: {split}, {failed} failed")
    }
}

/// Update-suite lines. The adapter label is not printed.
pub(crate) fn format_update_suite(cases: &[PrintCase<'_>]) -> PrintedSuite {
    let mut case_lines = Vec::with_capacity(cases.len());
    let mut passed = 0usize;
    for case in cases {
        if case.passed {
            passed += 1;
            case_lines.push(format!(
                "UPDATE {} ({})",
                case.name,
                case.artifacts_dir.display()
            ));
        } else {
            case_lines.push(format!(
                "FAIL {} ({}) {}",
                case.name,
                case.artifacts_dir.display(),
                case.error.unwrap_or("case failed")
            ));
        }
    }
    PrintedSuite {
        case_lines,
        summary: format!("Suite result: {passed}/{} updated", cases.len()),
    }
}

/// Writes `gate` and `adapter` onto an existing per-case `report.json`.
pub(crate) fn stamp_case_report(path: &Path, gate: Gate, adapter: &str) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    object.insert(
        "gate".to_string(),
        serde_json::Value::String(gate.as_str().to_string()),
    );
    object.insert(
        "adapter".to_string(),
        serde_json::Value::String(adapter.to_string()),
    );
    let Ok(pretty) = serde_json::to_string_pretty(&value) else {
        return false;
    };
    fs::write(path, pretty).is_ok()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    struct RecordedCase {
        name: String,
        passed: bool,
        artifacts_dir: PathBuf,
        error: Option<String>,
    }

    struct StampedCase {
        name: String,
        gate: String,
        adapter: String,
    }

    fn label_cases(
        adapter: Option<&ObservedAdapter>,
        cases: &[RecordedCase],
    ) -> (PrintedSuite, Vec<StampedCase>) {
        let adapter_label = adapter.map(ObservedAdapter::label).unwrap_or_default();
        let stamped = cases
            .iter()
            .map(|case| StampedCase {
                name: case.name.clone(),
                gate: gate_for(&case.name, adapter).as_str().to_string(),
                adapter: adapter_label.clone(),
            })
            .collect::<Vec<_>>();
        let print_cases = cases
            .iter()
            .zip(&stamped)
            .map(|(case, stamp)| PrintCase {
                name: &case.name,
                passed: case.passed,
                artifacts_dir: &case.artifacts_dir,
                error: case.error.as_deref(),
                gate: gate_for(&case.name, adapter),
                adapter: &stamp.adapter,
            })
            .collect::<Vec<_>>();
        (format_check_suite(&print_cases), stamped)
    }

    const WARP_LOG: &str = "\
wgpu adapter candidate: Microsoft Basic Render Driver type=Cpu backend=Dx12 vendor=0x1414 device=0x008c active_backends=DX12
wgpu adapter selected (headless): Microsoft Basic Render Driver type=Cpu backend=Dx12 (preference=HighPerformance)
";

    const LAVAPIPE_LOG: &str = "\
wgpu adapter candidate: llvmpipe (LLVM 20.1.2, 256 bits) type=Cpu backend=Vulkan vendor=0x10005 device=0x0000 active_backends=VULKAN
wgpu adapter selected (headless): llvmpipe (LLVM 20.1.2, 256 bits) type=Cpu backend=Vulkan (preference=HighPerformance)
";

    const METAL_LOG: &str = "\
wgpu adapter candidate: Apple Paravirtual device type=IntegratedGpu backend=Metal vendor=0x106b device=0x0000 active_backends=METAL
wgpu adapter selected (headless): Apple Paravirtual device type=IntegratedGpu backend=Metal (preference=HighPerformance)
";

    fn recorded(name: &str, passed: bool) -> RecordedCase {
        RecordedCase {
            name: name.to_string(),
            passed,
            artifacts_dir: PathBuf::from("out").join(name),
            error: (!passed).then(|| "case failed tolerance".to_string()),
        }
    }

    fn windows_suite(pbs_passed: bool) -> Vec<RecordedCase> {
        let mut cases = Vec::new();
        for index in 0..11 {
            cases.push(recorded(&format!("case_{index}"), true));
        }
        cases.push(recorded(PBS_LIT, pbs_passed));
        cases
    }

    #[test]
    fn formatting_uses_the_adapter_argument_on_any_host() {
        let warp = adapter_from_logs(&[WARP_LOG]).expect("warp");
        let (printed, stamped) = label_cases(Some(&warp), &windows_suite(true));
        let adapter_line = printed
            .case_lines
            .iter()
            .find(|line| line.starts_with("ADAPTER "))
            .expect("adapter line");
        assert!(adapter_line.contains("Microsoft Basic Render Driver"));
        assert!(adapter_line.contains("type=Cpu"));
        assert!(adapter_line.contains("backend=Dx12"));
        assert!(adapter_line.contains("vendor=0x1414"));
        assert!(adapter_line.contains("device=0x008c"));
        assert!(!adapter_line.contains("Windows WARP is not"));
        assert_eq!(
            printed.summary,
            "Suite result: 11 behavior + 1 adapter comparison"
        );
        let pbs = stamped
            .iter()
            .find(|case| case.name == PBS_LIT)
            .expect("pbs");
        assert_eq!(pbs.gate, ADAPTER_COMPARISON);
        assert_eq!(pbs.adapter, warp.label());
        assert!(
            stamped
                .iter()
                .filter(|case| case.name != PBS_LIT)
                .all(|case| case.gate == BEHAVIOR)
        );

        let lavapipe = adapter_from_logs(&[LAVAPIPE_LOG]).expect("lavapipe");
        let (lavapipe_printed, lavapipe_stamped) =
            label_cases(Some(&lavapipe), &windows_suite(true));
        assert!(
            lavapipe_printed
                .case_lines
                .iter()
                .all(|line| line.starts_with("PASS "))
        );
        assert_eq!(
            lavapipe_printed.summary,
            "Suite result: 12 behavior + 0 adapter comparison"
        );
        assert!(lavapipe_stamped.iter().all(|case| case.gate == BEHAVIOR));
        assert!(
            lavapipe_stamped
                .iter()
                .all(|case| case.adapter.contains("llvmpipe"))
        );
    }

    #[test]
    fn metal_pbs_lit_is_an_adapter_comparison_and_a_real_gpu_is_not() {
        let metal = adapter_from_logs(&[METAL_LOG]).expect("metal");
        let (printed, stamped) = label_cases(Some(&metal), &[recorded(PBS_LIT, true)]);
        assert!(printed.case_lines[0].contains("Apple Paravirtual device"));
        assert!(printed.case_lines[0].contains("backend=Metal"));
        assert_eq!(stamped[0].gate, ADAPTER_COMPARISON);
        assert_eq!(
            printed.summary,
            "Suite result: 0 behavior + 1 adapter comparison"
        );

        let mixed = "\
wgpu adapter candidate: Microsoft Basic Render Driver type=Cpu backend=Dx12 vendor=0x1414 device=0x008c active_backends=DX12
wgpu adapter selected (headless): NVIDIA GeForce RTX 4090 type=DiscreteGpu backend=Dx12 (preference=HighPerformance)
";
        let gpu = adapter_from_logs(&[mixed]).expect("gpu");
        assert!(!gpu.is_warp());
        let (gpu_printed, gpu_stamped) = label_cases(Some(&gpu), &[recorded(PBS_LIT, true)]);
        assert!(gpu_printed.case_lines[0].starts_with("PASS "));
        assert!(!gpu_printed.case_lines[0].contains("NVIDIA GeForce RTX 4090"));
        assert_eq!(gpu_stamped[0].gate, BEHAVIOR);
        assert!(gpu_stamped[0].adapter.contains("NVIDIA GeForce RTX 4090"));
    }

    #[test]
    fn a_failed_case_stays_a_failure() {
        let warp = adapter_from_logs(&[WARP_LOG]).expect("warp");
        let (printed, stamped) = label_cases(Some(&warp), &windows_suite(false));
        assert!(
            printed
                .case_lines
                .iter()
                .any(|line| line.starts_with("FAIL pbs_lit"))
        );
        assert!(
            !printed
                .case_lines
                .iter()
                .any(|line| line.starts_with("ADAPTER "))
        );
        assert_eq!(
            printed.summary,
            "Suite result: 11 behavior + 0 adapter comparison, 1 failed"
        );
        assert_eq!(
            stamped
                .iter()
                .find(|case| case.name == PBS_LIT)
                .expect("pbs")
                .gate,
            ADAPTER_COMPARISON
        );
    }

    #[test]
    fn update_suite_does_not_print_the_adapter_label() {
        let warp = adapter_from_logs(&[WARP_LOG]).expect("warp");
        let cases = [recorded(PBS_LIT, true)];
        let print_cases = [PrintCase {
            name: PBS_LIT,
            passed: true,
            artifacts_dir: &cases[0].artifacts_dir,
            error: None,
            gate: gate_for(PBS_LIT, Some(&warp)),
            adapter: "Microsoft Basic Render Driver type=Cpu backend=Dx12",
        }];
        let printed = format_update_suite(&print_cases);
        let artifacts = PathBuf::from("out").join(PBS_LIT);
        assert_eq!(
            printed.case_lines,
            vec![format!("UPDATE {PBS_LIT} ({})", artifacts.display())]
        );
        assert!(!printed.case_lines[0].contains("ADAPTER"));
        assert_eq!(printed.summary, "Suite result: 1/1 updated");
    }

    #[test]
    fn report_json_records_gate_and_adapter() {
        let warp = adapter_from_logs(&[WARP_LOG]).expect("warp");
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("report.json");
        fs::write(&path, r#"{"name":"pbs_lit_material_matrix","passed":true}"#).expect("write");
        assert!(stamp_case_report(
            &path,
            gate_for(PBS_LIT, Some(&warp)),
            &warp.label()
        ));
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
        assert_eq!(value["gate"], ADAPTER_COMPARISON);
        assert_eq!(value["adapter"], warp.label());
        assert_eq!(value["name"], PBS_LIT);
    }

    #[test]
    fn a_missing_identity_or_disagreement_is_not_warp() {
        assert!(adapter_from_logs(&[]).is_none());
        assert!(adapter_from_logs(&["wgpu adapter selected (headless): Microsoft Basic Render Driver type=Cpu backend=Dx12"]).is_some());
        let unnamed = adapter_from_logs(&[
            "wgpu adapter selected (headless): Microsoft Basic Render Driver type=Cpu backend=Dx12",
        ])
        .expect("selected");
        assert!(!unnamed.is_warp());
        let disagree = "\
wgpu adapter selected (headless): Microsoft Basic Render Driver type=Cpu backend=Dx12 (preference=HighPerformance)
wgpu adapter selected (headless): NVIDIA GeForce RTX 4090 type=DiscreteGpu backend=Dx12 (preference=HighPerformance)
";
        assert!(adapter_from_logs(&[disagree]).is_none());
    }
}
