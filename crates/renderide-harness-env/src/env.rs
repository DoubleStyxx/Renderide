//! Lavapipe probe, backend choice, Rayon default, and the stderr `env:` line.

use std::path::{Path, PathBuf};

/// Loader variable that wins when both ICD variables are set (loader 1.3.234+).
pub const VK_DRIVER_FILES: &str = "VK_DRIVER_FILES";
/// Older loader variable, used when [`VK_DRIVER_FILES`] is unset.
pub const VK_ICD_FILENAMES: &str = "VK_ICD_FILENAMES";
/// Rayon pool size. The harness sets this on the child when the parent left it unset.
pub const RAYON_NUM_THREADS: &str = "RAYON_NUM_THREADS";
/// wgpu's backend filter. The harness sets this on the child to the selected API.
pub const WGPU_BACKEND: &str = "WGPU_BACKEND";
/// Figment override for `[rendering] graphics_api`. Same token as [`WGPU_BACKEND`].
const GRAPHICS_API_ENV: &str = "RENDERIDE_RENDERING__GRAPHICS_API";

/// Lavapipe manifest names, in probe order, inside each search root.
pub const LAVAPIPE_FILENAMES: [&str; 3] = [
    "lvp_icd.x86_64.json",
    "lvp_icd.aarch64.json",
    "lvp_icd.json",
];

/// `--icd` value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IcdMode {
    /// Use an explicit loader variable, or probe lavapipe on Linux.
    Auto,
    /// Do not probe and do not assign an ICD. An explicit loader variable still wins.
    None,
    /// Manifest path used when no loader variable is set.
    Path(String),
}

/// `--backend` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendMode {
    /// Vulkan on Linux, DX12 on Windows, Metal on macOS.
    Auto,
    /// Vulkan.
    Vulkan,
    /// Direct3D 12 on the default adapter. Not WARP.
    Dx12,
    /// Metal.
    Metal,
}

/// Inputs that are not the process environment.
pub struct ResolveInput<'a> {
    /// Directories that may contain a lavapipe manifest. Tests inject temp dirs.
    pub search_roots: &'a [PathBuf],
    /// `std::env::consts::OS` (`linux`, `windows`, `macos`).
    pub os: &'a str,
    /// `--icd` mode.
    pub icd: &'a IcdMode,
    /// `--backend` mode.
    pub backend: BackendMode,
    /// Renderer binary the child will run.
    pub renderer: &'a str,
    /// CPU model, already read by the caller.
    pub cpu: &'a str,
    /// Adapter name, or `unavailable` before the renderer log exists.
    pub adapter: &'a str,
}

/// Environment the harness will give the renderer child.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedEnv {
    /// Operating system token.
    pub os: String,
    /// `vulkan`, `dx12`, or `metal`.
    pub backend: String,
    /// ICD manifest path, or `None` when the platform is not using Vulkan.
    pub icd_path: Option<String>,
    /// `env`, `auto`, `flag`, `none`, or `platform`.
    pub icd_source: String,
    /// Loader variable that supplied or will supply `icd_path`, or `None`.
    pub icd_var: Option<String>,
    /// Parenthesis text on the env line: `env`, `auto: lavapipe`, `flag`, `none`, or `platform`.
    pub icd_note: String,
    /// Rayon workers the child will use.
    pub rayon_threads: u32,
    /// `env` or `harness default`.
    pub rayon_note: String,
    /// Renderer binary path.
    pub renderer: String,
    /// CPU model.
    pub cpu: String,
    /// Adapter name.
    pub adapter: String,
    /// Variables to set on the child. Inherited variables are omitted.
    pub child_env: Vec<(String, String)>,
}

impl ResolvedEnv {
    /// The stderr line. `cpu` and `adapter` are quoted.
    pub fn line(&self) -> String {
        let icd = match &self.icd_path {
            Some(path) => format!("{path} ({})", self.icd_note),
            None => format!("none ({})", self.icd_note),
        };
        format!(
            "env: os={} backend={} icd={icd} rayon_threads={} ({}) renderer={} cpu=\"{}\" adapter=\"{}\"",
            self.os,
            self.backend,
            self.rayon_threads,
            self.rayon_note,
            self.renderer,
            self.cpu.replace('"', "'"),
            self.adapter.replace('"', "'"),
        )
    }
}

/// No lavapipe manifest was found and no loader variable was set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvError {
    message: String,
}

impl EnvError {
    /// Install hint. Starts with `no ICD` so the harness can classify it as exit 5.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for EnvError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for EnvError {}

/// ICD inherited from the process, without probing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InheritedIcd {
    /// Manifest path exactly as set, or `None` when the loader searches its defaults.
    pub path: Option<String>,
    /// `env` or `loader_default`.
    pub source: String,
    /// `VK_DRIVER_FILES` or `VK_ICD_FILENAMES`, or `None`.
    pub var: Option<String>,
}

/// Parses `--icd`. `auto` and `none` are modes. Anything else is a manifest path.
pub fn parse_icd_mode(raw: &str) -> Result<IcdMode, String> {
    match raw {
        "auto" => Ok(IcdMode::Auto),
        "none" => Ok(IcdMode::None),
        "" => Err("--icd must be auto, none, or a manifest path".to_string()),
        path => Ok(IcdMode::Path(path.to_string())),
    }
}

/// Parses `--backend`. `dx12-warp` is rejected.
pub fn parse_backend_mode(raw: &str) -> Result<BackendMode, String> {
    match raw {
        "auto" => Ok(BackendMode::Auto),
        "vulkan" => Ok(BackendMode::Vulkan),
        "dx12" => Ok(BackendMode::Dx12),
        "metal" => Ok(BackendMode::Metal),
        "dx12-warp" => {
            Err("--backend does not accept dx12-warp; use dx12 for the default adapter".to_string())
        }
        other => Err(format!(
            "--backend must be auto, vulkan, dx12, or metal (got {other})"
        )),
    }
}

/// Default lavapipe search roots on Linux.
pub fn default_icd_roots() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/share/vulkan/icd.d"),
        PathBuf::from("/etc/vulkan/icd.d"),
    ]
}

/// First lavapipe manifest under `roots`, in [`LAVAPIPE_FILENAMES`] order.
pub fn probe_lavapipe(roots: &[PathBuf], file_exists: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    for root in roots {
        for name in LAVAPIPE_FILENAMES {
            let path = root.join(name);
            if file_exists(&path) {
                return Some(path);
            }
        }
    }
    None
}

/// Reads `VK_DRIVER_FILES`, then `VK_ICD_FILENAMES`. Empty values are ignored.
pub fn inherited_icd(lookup: impl Fn(&str) -> Option<String>) -> InheritedIcd {
    for var in [VK_DRIVER_FILES, VK_ICD_FILENAMES] {
        if let Some(path) = lookup(var).filter(|value| !value.trim().is_empty()) {
            return InheritedIcd {
                path: Some(path),
                source: "env".to_string(),
                var: Some(var.to_string()),
            };
        }
    }
    InheritedIcd {
        path: None,
        source: "loader_default".to_string(),
        var: None,
    }
}

/// Resolves the child environment. `lookup` and `file_exists` are injected so tests
/// do not read the process environment or the real ICD directories.
pub fn resolve(
    input: &ResolveInput<'_>,
    lookup: impl Fn(&str) -> Option<String>,
    file_exists: impl Fn(&Path) -> bool,
) -> Result<ResolvedEnv, EnvError> {
    let backend = backend_name(input.backend, input.os);
    let (rayon_threads, rayon_note, pin_rayon) = rayon_choice(&lookup);
    let icd = select_icd(input, &lookup, &file_exists)?;
    let mut child_env = vec![
        (WGPU_BACKEND.to_string(), backend.to_string()),
        (GRAPHICS_API_ENV.to_string(), backend.to_string()),
    ];
    if pin_rayon {
        child_env.push((RAYON_NUM_THREADS.to_string(), "1".to_string()));
    }
    if let Some(path) = icd.assign {
        child_env.push((VK_DRIVER_FILES.to_string(), path.clone()));
        child_env.push((VK_ICD_FILENAMES.to_string(), path));
    }
    Ok(ResolvedEnv {
        os: input.os.to_string(),
        backend: backend.to_string(),
        icd_path: icd.path,
        icd_source: icd.source,
        icd_var: icd.var,
        icd_note: icd.note,
        rayon_threads,
        rayon_note,
        renderer: input.renderer.to_string(),
        cpu: input.cpu.to_string(),
        adapter: input.adapter.to_string(),
        child_env,
    })
}

struct ChosenIcd {
    path: Option<String>,
    source: String,
    var: Option<String>,
    note: String,
    assign: Option<String>,
}

fn select_icd(
    input: &ResolveInput<'_>,
    lookup: &impl Fn(&str) -> Option<String>,
    file_exists: &impl Fn(&Path) -> bool,
) -> Result<ChosenIcd, EnvError> {
    let inherited = inherited_icd(lookup);
    if inherited.path.is_some() {
        return Ok(ChosenIcd {
            path: inherited.path,
            source: "env".to_string(),
            var: inherited.var,
            note: "env".to_string(),
            assign: None,
        });
    }
    match input.icd {
        IcdMode::None => Ok(ChosenIcd {
            path: None,
            source: "none".to_string(),
            var: None,
            note: "none".to_string(),
            assign: None,
        }),
        IcdMode::Path(path) => Ok(ChosenIcd {
            path: Some(path.clone()),
            source: "flag".to_string(),
            var: Some(VK_DRIVER_FILES.to_string()),
            note: "flag".to_string(),
            assign: Some(path.clone()),
        }),
        IcdMode::Auto if input.os == "linux" => {
            match probe_lavapipe(input.search_roots, file_exists) {
                Some(path) => {
                    let text = path.to_string_lossy().into_owned();
                    Ok(ChosenIcd {
                        path: Some(text.clone()),
                        source: "auto".to_string(),
                        var: Some(VK_DRIVER_FILES.to_string()),
                        note: "auto: lavapipe".to_string(),
                        assign: Some(text),
                    })
                }
                None => Err(EnvError {
                    message: not_found_message(input.search_roots),
                }),
            }
        }
        IcdMode::Auto => Ok(ChosenIcd {
            path: None,
            source: "platform".to_string(),
            var: None,
            note: "platform".to_string(),
            assign: None,
        }),
    }
}

fn not_found_message(roots: &[PathBuf]) -> String {
    let listed = if roots.is_empty() {
        "no search roots".to_string()
    } else {
        roots
            .iter()
            .map(|root| root.display().to_string())
            .collect::<Vec<_>>()
            .join(" and ")
    };
    format!(
        "no ICD found; install lavapipe (mesa-vulkan-drivers) or set VK_DRIVER_FILES to an ICD JSON manifest. Searched {listed} for lvp_icd.x86_64.json, lvp_icd.aarch64.json, and lvp_icd.json"
    )
}

fn backend_name(mode: BackendMode, os: &str) -> &'static str {
    match mode {
        BackendMode::Vulkan => "vulkan",
        BackendMode::Dx12 => "dx12",
        BackendMode::Metal => "metal",
        BackendMode::Auto => match os {
            "windows" => "dx12",
            "macos" => "metal",
            _ => "vulkan",
        },
    }
}

fn rayon_choice(lookup: &impl Fn(&str) -> Option<String>) -> (u32, String, bool) {
    if let Some(threads) = lookup(RAYON_NUM_THREADS)
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|threads| *threads > 0)
    {
        return (threads, "env".to_string(), false);
    }
    (1, "harness default".to_string(), true)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;

    use super::{
        BackendMode, IcdMode, LAVAPIPE_FILENAMES, RAYON_NUM_THREADS, ResolveInput, VK_DRIVER_FILES,
        VK_ICD_FILENAMES, WGPU_BACKEND, inherited_icd, parse_backend_mode, probe_lavapipe, resolve,
    };

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<HashMap<_, _>>();
        move |key| map.get(key).cloned()
    }

    fn input<'a>(
        roots: &'a [std::path::PathBuf],
        os: &'a str,
        icd: &'a IcdMode,
        backend: BackendMode,
    ) -> ResolveInput<'a> {
        ResolveInput {
            search_roots: roots,
            os,
            icd,
            backend,
            renderer: "target/debug/renderide-renderer",
            cpu: "AMD EPYC 7763",
            adapter: "unavailable",
        }
    }

    fn absent(_: &Path) -> bool {
        false
    }

    #[test]
    fn driver_files_win_over_icd_filenames_and_over_a_probe() {
        let roots = super::default_icd_roots();
        let mode = IcdMode::Auto;
        let env = resolve(
            &input(&roots, "linux", &mode, BackendMode::Auto),
            lookup(&[
                (VK_DRIVER_FILES, "/a/lvp_icd.x86_64.json"),
                (VK_ICD_FILENAMES, "/b/other.json"),
            ]),
            |_| panic!("explicit env must not probe"),
        )
        .expect("env");
        assert_eq!(env.icd_path.as_deref(), Some("/a/lvp_icd.x86_64.json"));
        assert_eq!(env.icd_source, "env");
        assert_eq!(env.icd_var.as_deref(), Some(VK_DRIVER_FILES));
        assert!(env.line().contains("icd=/a/lvp_icd.x86_64.json (env)"));
        assert!(!env.child_env.iter().any(|(key, _)| key == VK_DRIVER_FILES));
    }

    #[test]
    fn blank_driver_files_falls_through_to_icd_filenames() {
        let inherited = inherited_icd(lookup(&[
            (VK_DRIVER_FILES, " "),
            (VK_ICD_FILENAMES, "/b/lvp.json"),
        ]));
        assert_eq!(inherited.path.as_deref(), Some("/b/lvp.json"));
        assert_eq!(inherited.var.as_deref(), Some(VK_ICD_FILENAMES));
    }

    #[test]
    fn probe_order_is_share_then_etc_and_arch_before_generic() {
        let root = tempfile::tempdir().expect("tempdir");
        let share = root.path().join("share");
        let etc = root.path().join("etc");
        std::fs::create_dir_all(&share).expect("share");
        std::fs::create_dir_all(&etc).expect("etc");
        std::fs::write(etc.join("lvp_icd.x86_64.json"), b"{}").expect("etc arch");
        std::fs::write(share.join("lvp_icd.json"), b"{}").expect("share generic");
        std::fs::write(share.join("lvp_icd.aarch64.json"), b"{}").expect("share arm");
        let roots = [share.clone(), etc];
        let found = probe_lavapipe(&roots, |path| path.is_file()).expect("manifest");
        assert_eq!(found, share.join("lvp_icd.aarch64.json"));

        std::fs::write(share.join("lvp_icd.x86_64.json"), b"{}").expect("share x64");
        let found = probe_lavapipe(&roots, |path| path.is_file()).expect("manifest");
        assert_eq!(found, share.join("lvp_icd.x86_64.json"));
        assert_eq!(
            LAVAPIPE_FILENAMES,
            [
                "lvp_icd.x86_64.json",
                "lvp_icd.aarch64.json",
                "lvp_icd.json"
            ]
        );
    }

    #[test]
    fn linux_auto_without_a_manifest_is_an_install_hint() {
        let mode = IcdMode::Auto;
        let roots = [std::path::PathBuf::from("/tmp/empty-icd")];
        let err = resolve(
            &input(&roots, "linux", &mode, BackendMode::Auto),
            lookup(&[]),
            absent,
        )
        .expect_err("missing");
        let message = err.to_string();
        assert!(message.starts_with("no ICD"), "{message}");
        assert!(message.contains("lavapipe"), "{message}");
        assert!(message.contains("mesa-vulkan-drivers"), "{message}");
        assert!(message.contains("/tmp/empty-icd"), "{message}");
    }

    #[test]
    fn icd_none_does_not_probe_or_fail() {
        let mode = IcdMode::None;
        let env = resolve(
            &input(&[], "linux", &mode, BackendMode::Auto),
            lookup(&[]),
            |_| panic!("none must not probe"),
        )
        .expect("none");
        assert_eq!(env.icd_source, "none");
        assert!(env.line().contains("icd=none (none)"));
    }

    #[test]
    fn explicit_env_wins_over_icd_none_and_over_a_path_flag() {
        let none = IcdMode::None;
        let path = IcdMode::Path("/flag/lvp.json".to_string());
        for mode in [&none, &path] {
            let env = resolve(
                &input(&[], "linux", mode, BackendMode::Auto),
                lookup(&[(VK_ICD_FILENAMES, "/env/lvp.json")]),
                |_| panic!("env wins"),
            )
            .expect("env");
            assert_eq!(env.icd_path.as_deref(), Some("/env/lvp.json"));
            assert_eq!(env.icd_note, "env");
        }
    }

    #[test]
    fn path_flag_is_assigned_when_no_variable_is_set() {
        let mode = IcdMode::Path("/flag/lvp.json".to_string());
        let env = resolve(
            &input(&[], "linux", &mode, BackendMode::Vulkan),
            lookup(&[]),
            |_| panic!("flag must not probe"),
        )
        .expect("flag");
        assert_eq!(env.icd_source, "flag");
        assert_eq!(env.icd_note, "flag");
        assert!(env.line().contains("icd=/flag/lvp.json (flag)"));
        assert!(
            env.child_env
                .iter()
                .any(|(key, value)| { key == VK_DRIVER_FILES && value == "/flag/lvp.json" })
        );
        assert!(
            env.child_env
                .iter()
                .any(|(key, value)| { key == VK_ICD_FILENAMES && value == "/flag/lvp.json" })
        );
    }

    #[test]
    fn auto_linux_probe_sets_both_loader_variables_and_prints_lavapipe() {
        let root = tempfile::tempdir().expect("tempdir");
        let manifest = root.path().join("lvp_icd.x86_64.json");
        std::fs::write(&manifest, b"{}").expect("manifest");
        let roots = [root.path().to_path_buf()];
        let mode = IcdMode::Auto;
        let mut request = input(&roots, "linux", &mode, BackendMode::Auto);
        request.adapter = "llvmpipe (LLVM 20.1.2, 256 bits)";
        let env = resolve(&request, lookup(&[]), |path| path.is_file()).expect("probe");
        let path = manifest.display().to_string();
        assert_eq!(env.icd_source, "auto");
        assert_eq!(env.icd_note, "auto: lavapipe");
        assert_eq!(
            env.line(),
            format!(
                "env: os=linux backend=vulkan icd={path} (auto: lavapipe) rayon_threads=1 (harness default) renderer=target/debug/renderide-renderer cpu=\"AMD EPYC 7763\" adapter=\"llvmpipe (LLVM 20.1.2, 256 bits)\""
            )
        );
        assert!(
            env.child_env
                .iter()
                .any(|(key, value)| { key == VK_DRIVER_FILES && value == &path })
        );
    }

    #[test]
    fn windows_and_macos_print_their_backends_and_do_not_probe() {
        let mode = IcdMode::Auto;
        let windows = resolve(
            &input(&[], "windows", &mode, BackendMode::Auto),
            lookup(&[]),
            |_| panic!("windows must not probe"),
        )
        .expect("windows");
        assert_eq!(windows.backend, "dx12");
        assert!(windows.line().contains("backend=dx12"));
        assert!(windows.line().contains("icd=none (platform)"));
        let macos = resolve(
            &input(&[], "macos", &mode, BackendMode::Auto),
            lookup(&[]),
            |_| panic!("macos must not probe"),
        )
        .expect("macos");
        assert_eq!(macos.backend, "metal");
        assert!(
            !windows
                .child_env
                .iter()
                .chain(macos.child_env.iter())
                .any(|(key, value)| key.contains("warp") || value.contains("warp"))
        );
    }

    #[test]
    fn explicit_backend_is_what_gets_printed() {
        let mode = IcdMode::None;
        let env = resolve(
            &input(&[], "linux", &mode, BackendMode::Dx12),
            lookup(&[]),
            absent,
        )
        .expect("dx12");
        assert_eq!(env.backend, "dx12");
        assert!(
            env.child_env
                .iter()
                .any(|(key, value)| { key == WGPU_BACKEND && value == "dx12" })
        );
        assert!(
            parse_backend_mode("dx12-warp")
                .expect_err("warp")
                .contains("dx12-warp")
        );
    }

    #[test]
    fn rayon_default_is_one_unless_the_parent_set_it() {
        let mode = IcdMode::None;
        let pinned = resolve(
            &input(&[], "linux", &mode, BackendMode::Auto),
            lookup(&[("RAYON_NUM_THREADS", "0")]),
            absent,
        )
        .expect("pin");
        assert_eq!(pinned.rayon_threads, 1);
        assert_eq!(pinned.rayon_note, "harness default");
        assert!(
            pinned
                .child_env
                .iter()
                .any(|(key, value)| { key == RAYON_NUM_THREADS && value == "1" })
        );
        let inherited = resolve(
            &input(&[], "linux", &mode, BackendMode::Auto),
            lookup(&[(RAYON_NUM_THREADS, "3")]),
            absent,
        )
        .expect("env");
        assert_eq!(
            (inherited.rayon_threads, inherited.rayon_note.as_str()),
            (3, "env")
        );
        assert!(
            !inherited
                .child_env
                .iter()
                .any(|(key, _)| key == RAYON_NUM_THREADS)
        );
        assert!(inherited.line().contains("rayon_threads=3 (env)"));
    }
}
