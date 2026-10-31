//! Material `@group(1)` bind identity for hypothesis 6.
//!
//! Off unless `RENDERIDE_MATERIAL_BIND_PROBE` is `1` or `true`. The canary
//! workflow sets it only for the four quarantined lit cases. A normal run
//! does not log.

use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;

const ENV: &str = "RENDERIDE_MATERIAL_BIND_PROBE";

const CASES: &[&str] = &[
    "pbs_lit_material_matrix",
    "negative_light_parity",
    "negative_light_physically_based",
    "negative_color_physically_based",
];

static ENABLED: OnceLock<bool> = OnceLock::new();
static ARMED: AtomicBool = AtomicBool::new(false);
static CASE: Mutex<String> = Mutex::new(String::new());
static STATE: Mutex<ProbeState> = Mutex::new(ProbeState::new());

struct ProbeState {
    uniforms: Vec<UniformNote>,
    binds: Vec<BindNote>,
    logged_uniform: Vec<u64>,
    logged_bind: Vec<u64>,
    summary: String,
}

struct UniformNote {
    offset: u32,
    hash: u64,
}

struct BindNote {
    offset: Option<u32>,
    key: String,
    bg: usize,
}

impl ProbeState {
    const fn new() -> Self {
        Self {
            uniforms: Vec::new(),
            binds: Vec::new(),
            logged_uniform: Vec::new(),
            logged_bind: Vec::new(),
            summary: String::new(),
        }
    }
}

/// `true` after [`arm`] matched one of the four quarantined cases.
pub(crate) fn armed() -> bool {
    ARMED.load(Ordering::Acquire)
}

/// Arms the probe when `path` is one of the four lit captures and the env is set.
pub(crate) fn arm(path: &Path) {
    if !enabled() {
        return;
    }
    let Some(name) = case_name(path) else {
        return;
    };
    *CASE.lock() = name.to_string();
    ARMED.store(true, Ordering::Release);
    logger::info!("diag-h6 armed case={name} path={}", path.display());
}

/// Logs one material-uniform upload the first time its bytes and offset are seen.
pub(crate) fn note_uniform(stem: &str, offset: u32, bytes: &[u8]) {
    if !armed() {
        return;
    }
    let hash = fnv1a64(bytes);
    let fresh = {
        let mut state = STATE.lock();
        let id = fnv1a64(format!("{stem}|{offset}|{hash:016x}").as_bytes());
        if state.logged_uniform.contains(&id) {
            false
        } else {
            state.logged_uniform.push(id);
            state.uniforms.push(UniformNote { offset, hash });
            true
        }
    };
    if fresh {
        let case = CASE.lock().clone();
        logger::info!(
            "diag-h6 uniform case={case} stem={stem} offset={offset} bytes={} hash={hash:016x}",
            bytes.len()
        );
    }
}

/// Logs one group-1 bind the first time its key, offset, and bind-group pointer are seen.
pub(crate) fn note_bind(batch: usize, offset: Option<u32>, key: &str, bg: usize) {
    if !armed() {
        return;
    }
    let offset_text = offset
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".to_string());
    let fresh = {
        let mut state = STATE.lock();
        let id = fnv1a64(format!("{batch}|{offset_text}|{key}|{bg}").as_bytes());
        if state.logged_bind.contains(&id) {
            false
        } else {
            state.logged_bind.push(id);
            state.binds.push(BindNote {
                offset,
                key: key.to_string(),
                bg,
            });
            true
        }
    };
    if fresh {
        let case = CASE.lock().clone();
        logger::info!(
            "diag-h6 bind case={case} batch={batch} offset={offset_text} bg={bg:#x} key={key}"
        );
    }
}

/// Logs whether the binds seen so far share one offset with different uniform hashes.
pub(crate) fn log_summary() {
    if !armed() {
        return;
    }
    let case = CASE.lock().clone();
    let line = {
        let state = STATE.lock();
        format_summary(&case, &state)
    };
    let changed = {
        let mut state = STATE.lock();
        if state.summary == line {
            false
        } else {
            state.summary.clone_from(&line);
            true
        }
    };
    if changed {
        logger::info!("{line}");
    }
}

fn format_summary(case: &str, state: &ProbeState) -> String {
    let mut offsets = Vec::new();
    let mut keys = Vec::new();
    let mut bgs = Vec::new();
    for bind in &state.binds {
        if let Some(offset) = bind.offset
            && !offsets.contains(&offset)
        {
            offsets.push(offset);
        }
        if !keys.contains(&bind.key) {
            keys.push(bind.key.clone());
        }
        if !bgs.contains(&bind.bg) {
            bgs.push(bind.bg);
        }
    }
    let shared = shared_offset_different_hash(
        &state
            .uniforms
            .iter()
            .map(|note| (note.offset, note.hash))
            .collect::<Vec<_>>(),
    );
    format!(
        "diag-h6 summary case={case} uniforms={} binds={} distinct_offset={} distinct_key={} distinct_bg={} shared_offset_different_hash={}",
        state.uniforms.len(),
        state.binds.len(),
        offsets.len(),
        keys.len(),
        bgs.len(),
        u8::from(shared),
    )
}

fn shared_offset_different_hash(rows: &[(u32, u64)]) -> bool {
    for (index, (offset, hash)) in rows.iter().enumerate() {
        if rows
            .iter()
            .skip(index + 1)
            .any(|(other_offset, other_hash)| other_offset == offset && other_hash != hash)
        {
            return true;
        }
    }
    false
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
    use super::{case_name, fnv1a64, shared_offset_different_hash};
    use std::path::Path;

    #[test]
    fn case_name_matches_the_four_quarantined_cases() {
        assert_eq!(
            case_name(Path::new("out/pbs_lit_material_matrix/actual.png")),
            Some("pbs_lit_material_matrix")
        );
        assert_eq!(
            case_name(Path::new("out/negative_light_parity/actual.png")),
            Some("negative_light_parity")
        );
        assert_eq!(case_name(Path::new("out/unlit_sphere/actual.png")), None);
        assert_eq!(
            case_name(Path::new("out/warp216_directional_diffuse_g/actual.png")),
            None
        );
    }

    #[test]
    fn same_offset_with_two_hashes_is_an_overwrite() {
        assert!(shared_offset_different_hash(&[(0, 1), (0, 2)]));
        assert!(!shared_offset_different_hash(&[(0, 1), (256, 2)]));
        assert_eq!(fnv1a64(b"abc"), fnv1a64(b"abc"));
        assert_ne!(fnv1a64(b"abc"), fnv1a64(b"abd"));
    }
}
