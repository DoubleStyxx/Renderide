//! Resonite launch-argument quoting, output-mode detection, and renderer-start unwrapping.
//!
//! The launcher receives an argv from the OS. Resonite's host and its bootstrap protocol also
//! exchange those arguments as one command-line string. That string follows Windows
//! `CommandLineToArgvW` rules (the same rules .NET and Wine `start.exe` use): whitespace splits
//! tokens, double quotes group a token, and backslashes escape quotes. Unknown tokens are kept
//! and forwarded unchanged.

use std::path::{Path, PathBuf};
use std::process::ExitStatus;

/// Renderer executable name produced by the `renderide-renderer` bin target.
const RENDERER_FILE_NAME: &str = if cfg!(windows) {
    "renderide-renderer.exe"
} else {
    "renderide-renderer"
};

/// Compatibility name used by installs that expose the renderer as `Renderite.Renderer`.
const LEGACY_RENDERER_FILE_NAME: &str = if cfg!(windows) {
    "Renderite.Renderer.exe"
} else {
    "Renderite.Renderer"
};

/// Host-named renderer from a `STARTRENDERER` wrapper, plus the arguments to forward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RendererLaunchRequest {
    /// Executable token from `STARTRENDERER <exe>`, when that wrapper was present.
    pub executable: Option<String>,
    /// Arguments with the wrapper removed. Unknown tokens are preserved in order.
    pub args: Vec<String>,
}

/// Splits `command` with Windows `CommandLineToArgvW` rules.
///
/// Space and tab separate arguments. Double quotes group an argument and are not part of the
/// value. A backslash escapes a following quote; backslashes that do not precede a quote are
/// literal. `""` inside a quoted argument is one literal quote.
pub(crate) fn split_windows_command_line(command: &str) -> Vec<String> {
    let chars: Vec<char> = command.chars().collect();
    let mut index = 0;
    let mut args = Vec::new();
    while index < chars.len() {
        while index < chars.len() && is_command_whitespace(chars[index]) {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }
        let (arg, next) = parse_windows_argument(&chars, index);
        args.push(arg);
        index = next;
    }
    args
}

/// Quotes `args` so [`split_windows_command_line`] returns the same tokens.
pub(crate) fn quote_windows_command_line(args: &[impl AsRef<str>]) -> String {
    let mut command = String::new();
    for (index, arg) in args.iter().enumerate() {
        if index > 0 {
            command.push(' ');
        }
        command.push_str(&quote_windows_arg(arg.as_ref()));
    }
    command
}

/// Quotes one argument for a Windows command line.
pub(crate) fn quote_windows_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.chars().any(|c| c == ' ' || c == '\t' || c == '"') {
        return arg.to_string();
    }

    let mut escaped = String::with_capacity(arg.len().saturating_add(2));
    escaped.push('"');
    let mut slashes = 0usize;
    for ch in arg.chars() {
        match ch {
            '\\' => slashes += 1,
            '"' => {
                push_repeat(
                    &mut escaped,
                    '\\',
                    slashes.saturating_mul(2).saturating_add(1),
                );
                escaped.push('"');
                slashes = 0;
            }
            other => {
                push_repeat(&mut escaped, '\\', slashes);
                slashes = 0;
                escaped.push(other);
            }
        }
    }
    push_repeat(&mut escaped, '\\', slashes.saturating_mul(2));
    escaped.push('"');
    escaped
}

/// Expands a single packed command-line argument into argv tokens.
///
/// The OS usually splits argv itself. A parent that forwards one string (for example the
/// pre-fix `-Renderer` command line) leaves the quotes and spaces inside that string. Already
/// split argv is returned unchanged.
pub(crate) fn expand_launcher_args(args: &[String]) -> Vec<String> {
    if let [only] = args
        && is_packed_command_line(only)
    {
        return split_windows_command_line(only);
    }
    args.to_vec()
}

/// Whether these launcher arguments are Resonite starting this process as a renderer.
///
/// Returns the arguments to forward to `renderide-renderer`. `STARTRENDERER <exe> ARGS=...`
/// is unwrapped. A direct `-QueueName` / `-QueueCapacity` pair (the fixed `-Renderer` form)
/// is forwarded as-is, including any unknown tokens around it.
pub fn renderer_delegate_from_args(args: &[String]) -> Option<Vec<String>> {
    let launch = parse_renderer_launch_message_from_argv(expand_launcher_args(args));
    if launch.executable.is_some() || has_renderer_connection_args(&launch.args) {
        return Some(launch.args);
    }
    None
}

/// Spawns the sibling renderer and waits for it.
///
/// The Resonite bootstrapper tracks this process id. Waiting, instead of exiting after a
/// spawn, keeps that id alive until the renderer exits.
pub fn spawn_renderer_delegate(args: &[String]) -> std::io::Result<ExitStatus> {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let program = renderer_executable_in(&dir);
    std::process::Command::new(program).args(args).status()
}

/// Parses a host renderer-start message, unwrapping `STARTRENDERER` when present.
pub(crate) fn parse_renderer_launch_message(message: &str) -> RendererLaunchRequest {
    parse_renderer_launch_message_from_argv(split_windows_command_line(message))
}

/// Returns `true` when `args` already choose desktop, VR, or Resonite autodetect.
pub(crate) fn host_args_specify_output_mode(args: &[String]) -> bool {
    let mut skip_value = false;
    for token in args {
        if skip_value {
            skip_value = false;
            continue;
        }
        if let Some(flag) = parse_flag(token) {
            if flag_specifies_output_mode(&flag) {
                return true;
            }
            if flag.attached.is_none() && flag_takes_following_value(&flag.name) {
                skip_value = true;
            }
            continue;
        }
        if token.trim().eq_ignore_ascii_case("none") {
            return true;
        }
    }
    false
}

/// Resolves the renderer binary next to the launcher.
pub(crate) fn renderer_executable_in(dir: &Path) -> PathBuf {
    let primary = dir.join(RENDERER_FILE_NAME);
    if primary.is_file() {
        return primary;
    }
    let legacy = dir.join(LEGACY_RENDERER_FILE_NAME);
    if legacy.is_file() {
        return legacy;
    }
    primary
}

struct FlagToken {
    name: String,
    attached: Option<String>,
}

fn parse_windows_argument(chars: &[char], mut index: usize) -> (String, usize) {
    let mut out = String::new();
    let mut in_quotes = false;
    while index < chars.len() {
        let ch = chars[index];
        if !in_quotes && is_command_whitespace(ch) {
            break;
        }
        if ch == '\\' {
            index = consume_backslashes(chars, index, &mut out);
            continue;
        }
        if ch == '"' {
            index = consume_quote(chars, index, &mut out, &mut in_quotes);
            continue;
        }
        out.push(ch);
        index += 1;
    }
    (out, index)
}

fn consume_backslashes(chars: &[char], mut index: usize, out: &mut String) -> usize {
    let mut slashes = 0usize;
    while index < chars.len() && chars[index] == '\\' {
        slashes += 1;
        index += 1;
    }
    if index < chars.len() && chars[index] == '"' {
        push_repeat(out, '\\', slashes / 2);
        if slashes % 2 == 1 {
            out.push('"');
            index += 1;
        }
        return index;
    }
    push_repeat(out, '\\', slashes);
    index
}

fn consume_quote(chars: &[char], index: usize, out: &mut String, in_quotes: &mut bool) -> usize {
    if *in_quotes && chars.get(index + 1) == Some(&'"') {
        out.push('"');
        return index + 2;
    }
    *in_quotes = !*in_quotes;
    index + 1
}

fn push_repeat(out: &mut String, ch: char, count: usize) {
    for _ in 0..count {
        out.push(ch);
    }
}

fn is_command_whitespace(ch: char) -> bool {
    ch == ' ' || ch == '\t'
}

fn is_packed_command_line(token: &str) -> bool {
    if !token.chars().any(is_command_whitespace) {
        return false;
    }
    let lower = token.to_ascii_lowercase();
    lower.contains("startrenderer") || token.contains('-') || token.contains('/')
}

fn parse_renderer_launch_message_from_argv(mut argv: Vec<String>) -> RendererLaunchRequest {
    let wrapped = argv
        .first()
        .is_some_and(|token| token.eq_ignore_ascii_case("STARTRENDERER"));
    if !wrapped {
        return RendererLaunchRequest {
            executable: None,
            args: argv,
        };
    }
    argv.remove(0);
    let executable = if argv.is_empty() {
        None
    } else {
        Some(argv.remove(0))
    };
    strip_args_prefix(&mut argv);
    RendererLaunchRequest {
        executable,
        args: argv,
    }
}

fn strip_args_prefix(args: &mut Vec<String>) {
    let Some(first) = args.first() else {
        return;
    };
    let Some(rest) = strip_prefix_ignore_ascii_case(first, "ARGS=") else {
        return;
    };
    if rest.is_empty() {
        args.remove(0);
        return;
    }
    args[0] = rest.to_string();
}

fn strip_prefix_ignore_ascii_case<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let mut rest = value.chars();
    for expected in prefix.chars() {
        let found = rest.next()?;
        if !found.eq_ignore_ascii_case(&expected) {
            return None;
        }
    }
    Some(rest.as_str())
}

fn has_renderer_connection_args(args: &[String]) -> bool {
    let mut saw_name = false;
    let mut saw_capacity = false;
    for arg in args {
        let Some(flag) = parse_flag(arg) else {
            continue;
        };
        if flag.name.ends_with("queuename") {
            saw_name = true;
        } else if flag.name.ends_with("queuecapacity") {
            saw_capacity = true;
        }
    }
    saw_name && saw_capacity
}

fn parse_flag(token: &str) -> Option<FlagToken> {
    let trimmed = token.trim();
    let body = trimmed
        .strip_prefix("--")
        .or_else(|| trimmed.strip_prefix('-'))
        .or_else(|| trimmed.strip_prefix('/'))?;
    if body.is_empty() || !body.starts_with(|ch: char| ch.is_ascii_alphanumeric()) {
        return None;
    }
    if let Some((name, value)) = body.split_once('=') {
        return flag_token(name, Some(value));
    }
    if let Some((name, value)) = body.split_once(':') {
        return flag_token(name, Some(value));
    }
    flag_token(body, None)
}

fn flag_token(name: &str, attached: Option<&str>) -> Option<FlagToken> {
    if name.is_empty() {
        return None;
    }
    Some(FlagToken {
        name: name.to_ascii_lowercase(),
        attached: attached.map(str::to_string),
    })
}

fn flag_specifies_output_mode(flag: &FlagToken) -> bool {
    flag.name == "screen"
        || flag.name == "device"
        || flag.name == "none"
        || legacy_output_flag(&flag.name)
}

fn legacy_output_flag(name: &str) -> bool {
    matches!(
        name,
        "steamvr"
            | "rifttouch"
            | "oculus"
            | "oculusquest"
            | "windowsmr"
            | "screen360"
            | "staticcamera"
            | "staticcamera360"
            | "headless"
    )
}

fn flag_takes_following_value(name: &str) -> bool {
    matches!(
        name,
        "device"
            | "renderer"
            | "loadassembly"
            | "data"
            | "datapath"
            | "cachepath"
            | "watchdog"
            | "cloudprofile"
            | "engineconfig"
            | "backgroundworkers"
            | "priorityworkers"
            | "join"
            | "world"
            | "sessionurl"
            | "session"
            | "shmprefix"
            | "log"
            | "screen-width"
            | "screen-height"
            | "screen-fullscreen"
            | "log-level"
            | "l"
            | "resonite-dir"
            | "resonitedir"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_string()).collect()
    }

    #[test]
    fn quote_split_roundtrip_preserves_unknown_args() {
        let args = owned(&[
            "-Screen",
            "-LoadAssembly",
            "Libraries/My Mod.dll",
            "-TotallyUnknown",
            "value with spaces",
            "",
            "quote\"inside",
            r"C:\Path\",
            "tab\there",
            "-Renderer",
            r"C:\Program Files\Alt Renderer.exe",
            "none",
        ]);
        let command = quote_windows_command_line(&args);
        assert_eq!(split_windows_command_line(&command), args);
    }

    #[test]
    fn split_keeps_quoted_spaces_and_literal_quotes() {
        assert_eq!(
            split_windows_command_line(r#"-Data "C:\My Worlds" -Verbose"#),
            owned(&[r"-Data", r"C:\My Worlds", "-Verbose"])
        );
        assert_eq!(split_windows_command_line(r#""a""b""#), owned(&[r#"a"b"#]));
        assert_eq!(split_windows_command_line(r#"a\"b"#), owned(&[r#"a"b"#]));
        assert_eq!(split_windows_command_line("a  \tb"), owned(&["a", "b"]));
        assert_eq!(split_windows_command_line(r#""""#), vec![String::new()]);
    }

    #[test]
    fn split_empty_and_whitespace_have_no_args() {
        assert!(split_windows_command_line("").is_empty());
        assert!(split_windows_command_line("   \t  ").is_empty());
    }

    #[test]
    fn packed_launcher_line_forwards_unknown_args_and_strips_launcher_flags() {
        let packed = quote_windows_command_line(&owned(&[
            "--log-level",
            "debug",
            "-Screen",
            "-LoadAssembly",
            "Libraries/My Mod.dll",
            "-Custom",
            "still here",
            "--resonite-dir",
            "/games/Resonite",
        ]));
        let parsed = crate::cli::parse_bootstrap_args_tokens(&expand_launcher_args(&[packed]));
        assert_eq!(parsed.log_level, Some(logger::LogLevel::Debug));
        assert_eq!(parsed.resonite_dir, Some(PathBuf::from("/games/Resonite")));
        assert_eq!(
            parsed.host_args,
            owned(&[
                "-Screen",
                "-LoadAssembly",
                "Libraries/My Mod.dll",
                "-Custom",
                "still here"
            ])
        );
        let forwarded = crate::orchestration::assemble_host_args(parsed.host_args, "Px");
        assert_eq!(
            forwarded,
            owned(&[
                "-Screen",
                "-LoadAssembly",
                "Libraries/My Mod.dll",
                "-Custom",
                "still here",
                "-shmprefix",
                "Px",
            ])
        );
    }

    #[test]
    fn already_split_argv_is_not_reparsed() {
        let args = owned(&["-Data", "a b"]);
        assert_eq!(expand_launcher_args(&args), args);
    }

    #[test]
    fn output_mode_recognizes_resonite_device_spellings() {
        assert!(host_args_specify_output_mode(&owned(&["-Screen"])));
        assert!(host_args_specify_output_mode(&owned(&["--screen"])));
        assert!(host_args_specify_output_mode(&owned(&["/SCREEN"])));
        assert!(host_args_specify_output_mode(&owned(&[
            "-Device", "SteamVR"
        ])));
        assert!(host_args_specify_output_mode(&owned(&["-Device=Oculus"])));
        assert!(host_args_specify_output_mode(&owned(&[
            "-Device:Screen360"
        ])));
        assert!(host_args_specify_output_mode(&owned(&["-SteamVR"])));
        assert!(host_args_specify_output_mode(&owned(&["-RiftTouch"])));
        assert!(host_args_specify_output_mode(&owned(&["-WindowsMR"])));
        assert!(host_args_specify_output_mode(&owned(&["none"])));
        assert!(host_args_specify_output_mode(&owned(&["-none"])));
        assert!(host_args_specify_output_mode(&owned(&["-Headless"])));
    }

    #[test]
    fn output_mode_ignores_values_and_unknown_flags() {
        assert!(!host_args_specify_output_mode(&owned(&["-Data", "screen"])));
        assert!(!host_args_specify_output_mode(&owned(&[
            "-LoadAssembly",
            "none"
        ])));
        assert!(!host_args_specify_output_mode(&owned(&[
            "-CloudProfile",
            "none"
        ])));
        assert!(!host_args_specify_output_mode(&owned(&[
            "-Invisible",
            "-LoadAssembly",
            "Libraries/ResoniteModLoader.dll",
        ])));
        assert!(!host_args_specify_output_mode(&owned(&[
            "-screen-fullscreen",
            "0"
        ])));
        assert!(host_args_specify_output_mode(&owned(&[
            "-Renderer",
            r"C:\R.exe",
            "-Screen"
        ])));
    }

    #[test]
    fn start_renderer_wrapper_unwraps_args_prefix() {
        let message = concat!(
            "STARTRENDERER SeercatCustomRendererLaunchDebug.sh ARGS=-QueueName ",
            "tL7ez5VFwVD9JlA6_ySZ67FbTMOv6aZAJS4q_R1rP4b_bnIwf3TUaOhneJiM= ",
            "-QueueCapacity 8388608"
        );
        let launch = parse_renderer_launch_message(message);
        assert_eq!(
            launch.executable.as_deref(),
            Some("SeercatCustomRendererLaunchDebug.sh")
        );
        assert_eq!(
            launch.args,
            owned(&[
                "-QueueName",
                "tL7ez5VFwVD9JlA6_ySZ67FbTMOv6aZAJS4q_R1rP4b_bnIwf3TUaOhneJiM=",
                "-QueueCapacity",
                "8388608",
            ])
        );
    }

    #[test]
    fn start_renderer_wrapper_keeps_quoted_executable() {
        let message = r#"STARTRENDERER "C:\Program Files\Renderide\renderide-renderer.exe" ARGS= -QueueName q -QueueCapacity 4096 -Extra "value with spaces""#;
        let launch = parse_renderer_launch_message(message);
        assert_eq!(
            launch.executable.as_deref(),
            Some(r"C:\Program Files\Renderide\renderide-renderer.exe")
        );
        assert_eq!(
            launch.args,
            owned(&[
                "-QueueName",
                "q",
                "-QueueCapacity",
                "4096",
                "-Extra",
                "value with spaces"
            ])
        );
    }

    #[test]
    fn plain_renderer_message_is_passthrough() {
        let launch = parse_renderer_launch_message("-QueueName q -QueueCapacity 4096 -Unknown x");
        assert!(launch.executable.is_none());
        assert_eq!(
            launch.args,
            owned(&["-QueueName", "q", "-QueueCapacity", "4096", "-Unknown", "x"])
        );
    }

    #[test]
    fn delegate_detects_wrapper_and_direct_queue_args() {
        let wrapped = owned(&[
            "STARTRENDERER",
            "renderide",
            "ARGS=-QueueName",
            "queue",
            "-QueueCapacity",
            "4096",
            "-Custom",
            "kept",
        ]);
        assert_eq!(
            renderer_delegate_from_args(&wrapped),
            Some(owned(&[
                "-QueueName",
                "queue",
                "-QueueCapacity",
                "4096",
                "-Custom",
                "kept"
            ]))
        );

        let direct = owned(&["-QueueCapacity", "1024", "-QueueName", "n", "-AlsoUnknown"]);
        assert_eq!(renderer_delegate_from_args(&direct), Some(direct.clone()));
        assert!(renderer_delegate_from_args(&owned(&["-Screen", "-Invisible"])).is_none());
    }

    #[test]
    fn delegate_expands_one_packed_renderer_command() {
        let packed = "STARTRENDERER \"C:\\Program Files\\renderide.exe\" ARGS=-QueueName session -QueueCapacity 2048";
        let delegated = renderer_delegate_from_args(&owned(&[packed])).expect("delegate");
        assert_eq!(
            delegated,
            owned(&["-QueueName", "session", "-QueueCapacity", "2048"])
        );
    }

    #[test]
    fn renderer_executable_prefers_primary_then_legacy_name() {
        let dir =
            std::env::temp_dir().join(format!("renderide_launch_args_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let primary = dir.join(RENDERER_FILE_NAME);
        let legacy = dir.join(LEGACY_RENDERER_FILE_NAME);
        std::fs::write(&legacy, b"legacy").unwrap();
        assert_eq!(renderer_executable_in(&dir), legacy);
        std::fs::write(&primary, b"primary").unwrap();
        assert_eq!(renderer_executable_in(&dir), primary);
        std::fs::remove_file(&primary).unwrap();
        std::fs::remove_file(&legacy).unwrap();
        assert_eq!(renderer_executable_in(&dir), primary);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
