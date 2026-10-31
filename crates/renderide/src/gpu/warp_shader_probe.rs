//! DXIL-versus-SPIR-V note for the production lit fragment (#216 experiment 8a).
//!
//! Off unless `RENDERIDE_LIT_SHADER_PROBE` is `1` or `true`, and only for
//! `negative_light_parity`, `negative_light_physically_based`, and
//! `pbs_lit_material_matrix`. The Windows binary still links static DXC. This
//! dump is the naga HLSL and SPIR-V of the WGSL those cases compile. When `dxc`
//! or `spirv-dis` is on `PATH`, it also writes DXIL and SPIR-V disassembly.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use parking_lot::Mutex;

const ENV: &str = "RENDERIDE_LIT_SHADER_PROBE";
const CASES: &[&str] = &[
    "negative_light_parity",
    "negative_light_physically_based",
    "pbs_lit_material_matrix",
];

static ENABLED: OnceLock<bool> = OnceLock::new();
static ARMED: AtomicBool = AtomicBool::new(false);
static DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static SEEN: Mutex<Vec<u64>> = Mutex::new(Vec::new());

/// Arms the probe for the production lit cases when the env is set.
pub(crate) fn arm(path: &Path) {
    if !enabled() {
        return;
    }
    let Some(name) = case_name(path) else {
        return;
    };
    if let Some(dir) = path.parent() {
        *DIR.lock() = Some(dir.to_path_buf());
    }
    ARMED.store(true, Ordering::Release);
    logger::info!("diag-8a armed case={name} path={}", path.display());
}

/// Dumps naga SPIR-V and HLSL for one production lit shader. Later identical sources are skipped.
pub(crate) fn note_wgsl(source: &str) {
    if !ARMED.load(Ordering::Acquire) || !source.contains("shade_metallic_clustered") {
        return;
    }
    if source.contains("shade_metallic_term_debug") || source.contains("DEBUG_TERM_") {
        return;
    }
    let hash = fnv1a64(source.as_bytes());
    {
        let mut seen = SEEN.lock();
        if seen.contains(&hash) {
            return;
        }
        seen.push(hash);
    }
    let (spv_hash, hlsl_hash, note) = translate(source);
    let dir = DIR.lock().clone();
    if let Some(dir) = dir.as_ref() {
        let _ = std::fs::write(dir.join("lit-production.wgsl"), source);
        if let Some(text) = note.hlsl.as_ref() {
            let _ = std::fs::write(dir.join("lit-production.hlsl"), text);
        }
        if let Some(words) = note.spv.as_ref() {
            let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
            let _ = std::fs::write(dir.join("lit-production.spv"), bytes);
            disassemble_spirv(dir);
        }
        if note.hlsl.is_some() {
            compile_dxil(dir);
        }
    }
    logger::info!(
        "diag-8a production wgsl_hash={hash:016x} spv_hash={} hlsl_hash={} fragment_entries={} translate={}",
        fmt_hash(spv_hash),
        fmt_hash(hlsl_hash),
        note.entries,
        note.status,
    );
}

struct Translate {
    spv: Option<Vec<u32>>,
    hlsl: Option<String>,
    entries: String,
    status: String,
}

fn translate(source: &str) -> (Option<u64>, Option<u64>, Translate) {
    let module = match naga::front::wgsl::parse_str(source) {
        Ok(module) => module,
        Err(err) => {
            return (
                None,
                None,
                Translate {
                    spv: None,
                    hlsl: None,
                    entries: String::new(),
                    status: format!("wgsl parse {err}"),
                },
            );
        }
    };
    let entries = module
        .entry_points
        .iter()
        .filter(|entry| entry.stage == naga::ShaderStage::Fragment)
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let info = match naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    {
        Ok(info) => info,
        Err(err) => {
            return (
                None,
                None,
                Translate {
                    spv: None,
                    hlsl: None,
                    entries,
                    status: format!("validate {err}"),
                },
            );
        }
    };
    let processed = naga::back::pipeline_constants::process_overrides(
        &module,
        &info,
        None,
        &naga::back::PipelineConstants::default(),
    );
    let (module, info) = match processed {
        Ok((module, info)) => (module, info),
        Err(err) => {
            return (
                None,
                None,
                Translate {
                    spv: None,
                    hlsl: None,
                    entries,
                    status: format!("overrides {err}"),
                },
            );
        }
    };
    let spv_result =
        naga::back::spv::write_vec(&module, &info, &naga::back::spv::Options::default(), None);
    let spv_err = spv_result.as_ref().err().map(|err| err.to_string());
    let spv = spv_result.ok();
    let spv_hash = spv.as_ref().map(|words| {
        let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        fnv1a64(&bytes)
    });
    let hlsl = write_hlsl(module.as_ref(), info.as_ref());
    let hlsl_hash = hlsl.as_ref().map(|text| fnv1a64(text.as_bytes()));
    let status = match (&spv, &hlsl) {
        (Some(_), Some(_)) => "ok".to_string(),
        (None, Some(_)) => format!("spirv {}", spv_err.unwrap_or_default()),
        (Some(_), None) => "hlsl failed".to_string(),
        (None, None) => format!("spirv {} hlsl failed", spv_err.unwrap_or_default()),
    };
    (
        spv_hash,
        hlsl_hash,
        Translate {
            spv,
            hlsl,
            entries,
            status,
        },
    )
}

fn write_hlsl(module: &naga::Module, info: &naga::valid::ModuleInfo) -> Option<String> {
    let mut text = String::new();
    let options = naga::back::hlsl::Options::default();
    let pipeline = naga::back::hlsl::PipelineOptions::default();
    let mut writer = naga::back::hlsl::Writer::new(&mut text, &options, &pipeline);
    writer.write(module, info, None).ok()?;
    Some(text)
}

/// Compiles the dumped HLSL with a `dxc` on `PATH`, using wgpu 30's DX12 flags.
///
/// wgpu calls DXC with `-T ps_<model>`, `-HV 2018`, `-Ges`, and `-no-warnings`,
/// and no `-O` flag. DXC's default is `-O3`. The profile here is `ps_6_7`, which
/// is static DXC's cap. A missing `dxc` is logged and skipped.
fn compile_dxil(dir: &Path) {
    let hlsl = dir.join("lit-production.hlsl");
    let listing = dir.join("lit-production.dxil.ll");
    let object = dir.join("lit-production.dxil");
    let output = std::process::Command::new("dxc").arg("--version").output();
    let version = output.ok().map(|output| {
        let text = String::from_utf8_lossy(&output.stdout);
        let err = String::from_utf8_lossy(&output.stderr);
        format!("{text}{err}")
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("unparsed")
            .to_string()
    });
    logger::info!(
        "diag-8a dxc version={}",
        version.unwrap_or_else(|| "absent".to_string())
    );
    let compiled = std::process::Command::new("dxc")
        .args([
            "-T",
            "ps_6_7",
            "-E",
            "fs_forward_base",
            "-HV",
            "2018",
            "-Ges",
            "-no-warnings",
            "-Fc",
        ])
        .arg(&listing)
        .arg("-Fo")
        .arg(&object)
        .arg(&hlsl)
        .output();
    match compiled {
        Ok(output) => {
            let status = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let first = stderr
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("");
            logger::info!(
                "diag-8a dxc status={status} dxil_bytes={} listing_bytes={} stderr={first}",
                std::fs::metadata(&object)
                    .map(|meta| meta.len())
                    .unwrap_or(0),
                std::fs::metadata(&listing)
                    .map(|meta| meta.len())
                    .unwrap_or(0),
            );
        }
        Err(err) => logger::info!("diag-8a dxc spawn failed: {err}"),
    }
}

fn disassemble_spirv(dir: &Path) {
    let spv = dir.join("lit-production.spv");
    let listing = dir.join("lit-production.spv.dis");
    let compiled = std::process::Command::new("spirv-dis")
        .arg(&spv)
        .arg("-o")
        .arg(&listing)
        .output();
    match compiled {
        Ok(output) => {
            let status = output.status.code().unwrap_or(-1);
            logger::info!(
                "diag-8a spirv-dis status={status} bytes={}",
                std::fs::metadata(&listing)
                    .map(|meta| meta.len())
                    .unwrap_or(0),
            );
        }
        Err(err) => logger::info!("diag-8a spirv-dis spawn failed: {err}"),
    }
}

fn enabled() -> bool {
    *ENABLED.get_or_init(|| {
        std::env::var(ENV).is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
    })
}

fn case_name(path: &Path) -> Option<&'static str> {
    let text = path.to_string_lossy();
    CASES.iter().copied().find(|name| text.contains(name))
}

fn fmt_hash(hash: Option<u64>) -> String {
    match hash {
        Some(hash) => format!("{hash:016x}"),
        None => "none".to_string(),
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::case_name;
    use std::path::Path;

    #[test]
    fn only_the_production_lit_cases_arm() {
        assert_eq!(
            case_name(Path::new("out/negative_light_parity/actual.png")),
            Some("negative_light_parity")
        );
        assert_eq!(
            case_name(Path::new("out/negative_light_physically_based/actual.png")),
            Some("negative_light_physically_based")
        );
        assert_eq!(
            case_name(Path::new("out/pbs_lit_material_matrix/actual.png")),
            Some("pbs_lit_material_matrix")
        );
        assert_eq!(
            case_name(Path::new("out/warp216_negparity_composed/actual.png")),
            None
        );
    }
}
