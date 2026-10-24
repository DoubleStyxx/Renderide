//! WARP-only expected failure for `pbs_lit_material_matrix`.
//!
//! The known mismatch is https://github.com/DoubleStyxx/Renderide/issues/212
//! (the body that could not be written onto issue 209). A lavapipe or real-GPU
//! run does not use this path. Any other case still fails the suite, and a
//! WARP run where `pbs_lit` passes is a failure too.

use std::fs;
use std::path::Path;

/// Case whose WARP mismatch is expected.
pub(crate) const PBS_LIT: &str = "pbs_lit_material_matrix";

/// What a check-suite should do with `pbs_lit` after the other cases are known.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Ruling {
    /// Leave the suite result alone.
    Unchanged,
    /// `pbs_lit` failed on WARP and every other case passed.
    ExpectedFailure,
    /// `pbs_lit` passed on WARP. The known mismatch went away, or the gate did not run.
    UnexpectedPass,
}

/// Decides the WARP exception from the adapter and the case results.
///
/// `cases` is `(name, passed)`.
pub(crate) fn ruling(is_warp: bool, cases: &[(&str, bool)]) -> Ruling {
    if !is_warp {
        return Ruling::Unchanged;
    }
    let Some(pbs_passed) = cases
        .iter()
        .find(|(name, _)| *name == PBS_LIT)
        .map(|(_, passed)| *passed)
    else {
        return Ruling::Unchanged;
    };
    let other_failed = cases
        .iter()
        .any(|(name, passed)| *name != PBS_LIT && !*passed);
    if other_failed {
        return Ruling::Unchanged;
    }
    if pbs_passed {
        Ruling::UnexpectedPass
    } else {
        Ruling::ExpectedFailure
    }
}

/// True when every selected adapter is WARP and a candidate line carries its id.
///
/// WARP is `Microsoft Basic Render Driver`, device type `Cpu`, vendor `0x1414`,
/// device `0x008c`, backend `Dx12`. A missing id is not WARP.
pub(crate) fn logs_are_warp(logs: &[&str]) -> bool {
    let mut saw_selected = false;
    let mut saw_identity = false;
    for log in logs {
        for line in log.lines() {
            if line.contains("wgpu adapter selected") {
                saw_selected = true;
                if !selected_line_is_warp(line) {
                    return false;
                }
            }
            if candidate_line_is_warp(line) {
                saw_identity = true;
            }
        }
    }
    saw_selected && saw_identity
}

fn selected_line_is_warp(line: &str) -> bool {
    line.contains("Microsoft Basic Render Driver")
        && line.contains("type=Cpu")
        && line.contains("backend=Dx12")
}

fn candidate_line_is_warp(line: &str) -> bool {
    line.contains("wgpu adapter candidate: Microsoft Basic Render Driver")
        && line.contains("type=Cpu")
        && line.contains("vendor=0x1414")
        && line.contains("device=0x008c")
}

/// Reads `*.log` files under `dir`. An unreadable directory is not WARP.
pub(crate) fn directory_is_warp(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
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
    logs_are_warp(&borrowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WARP_LOG: &str = "\
wgpu adapter candidate: Microsoft Basic Render Driver type=Cpu backend=Dx12 vendor=0x1414 device=0x008c active_backends=DX12
wgpu adapter selected (headless): Microsoft Basic Render Driver type=Cpu backend=Dx12 (preference=HighPerformance)
";

    const LAVAPIPE_LOG: &str = "\
wgpu adapter candidate: llvmpipe (LLVM 20.1.2, 256 bits) type=Cpu backend=Vulkan vendor=0x10005 device=0x0000 active_backends=VULKAN
wgpu adapter selected (headless): llvmpipe (LLVM 20.1.2, 256 bits) type=Cpu backend=Vulkan (preference=HighPerformance)
";

    #[test]
    fn warp_identity_requires_the_selected_adapter_and_the_device_id() {
        assert!(logs_are_warp(&[WARP_LOG]));
        assert!(!logs_are_warp(&[LAVAPIPE_LOG]));
        assert!(!logs_are_warp(&[
            "wgpu adapter selected (headless): Microsoft Basic Render Driver type=Cpu backend=Dx12"
        ]));
        assert!(!logs_are_warp(&[]));
    }

    #[test]
    fn a_real_gpu_selected_next_to_a_warp_candidate_is_not_warp() {
        let mixed = "\
wgpu adapter candidate: Microsoft Basic Render Driver type=Cpu backend=Dx12 vendor=0x1414 device=0x008c active_backends=DX12
wgpu adapter selected (headless): NVIDIA GeForce RTX 4090 type=DiscreteGpu backend=Dx12 (preference=HighPerformance)
";
        assert!(!logs_are_warp(&[mixed]));
    }

    #[test]
    fn only_pbs_lit_may_fail_on_warp() {
        assert_eq!(
            ruling(true, &[("unlit_sphere", true), (PBS_LIT, false)]),
            Ruling::ExpectedFailure
        );
        assert_eq!(
            ruling(true, &[("unlit_sphere", true), (PBS_LIT, true)]),
            Ruling::UnexpectedPass
        );
        assert_eq!(
            ruling(true, &[("unlit_sphere", false), (PBS_LIT, false)]),
            Ruling::Unchanged
        );
        assert_eq!(
            ruling(true, &[("unlit_sphere", false), (PBS_LIT, true)]),
            Ruling::Unchanged
        );
        assert_eq!(
            ruling(false, &[("unlit_sphere", true), (PBS_LIT, false)]),
            Ruling::Unchanged
        );
        assert_eq!(ruling(true, &[("unlit_sphere", true)]), Ruling::Unchanged);
    }
}
