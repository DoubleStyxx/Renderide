//! Resonite launch-argument quoting, output-mode detection, and renderer-start unwrapping.
//!
//! The launcher receives an argv from the OS. Resonite's host and its bootstrap protocol also
//! exchange those arguments as one command-line string. That string follows the argument rules
//! of Windows `CommandLineToArgvW` (the same rules .NET uses when it parses a command line, and
//! the rules Wine `start.exe` quotes for): whitespace splits tokens, quotes group a token, and
//! backslashes escape quotes only when they immediately precede one. Unknown tokens are kept
//! and forwarded unchanged.
//!
//! This parser does not apply the program-name rules of `CommandLineToArgvW`. These strings are
//! argument lists, not a full `CreateProcess` command line, so a leading quote starts an
//! argument instead of ending `argv[0]`.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

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

/// Splits `command` with Windows `CommandLineToArgvW` argument rules.
///
/// Space and tab separate arguments outside quotes. Backslashes are literal unless they
/// immediately precede a quote: an even run of `2n` backslashes plus a quote becomes `n`
/// backslashes and the quote is a delimiter; an odd run of `2n+1` becomes `n` backslashes and
/// one literal quote.
///
/// Consecutive quotes use `CommandLineToArgvW`'s rule of three, not the post-2005 C runtime
/// rule. The C runtime treats `""` inside a quoted argument as one literal quote and stays
/// quoted. `CommandLineToArgvW` counts a run of quotes, including the quote that opened the
/// run, and divides by three: `3n` quotes emit `n` quotes, `3n+1` emits `n` quotes and closes
/// quoting, and `3n+2` emits `n+1` quotes and closes quoting. From outside quotes, the first
/// quote only opens quoting and the rest of the run follows that rule. Two quotes in
/// `two""quotes` therefore produce no quote character and leave quoting off, so the following
/// space still starts the next argument.
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

/// Expands one packed renderer command line into argv tokens.
///
/// The OS usually splits argv itself. Resonite's pre-fix `-Renderer` handoff can instead pass
/// the whole renderer command as a single string. That string is split only when the split
/// result is a renderer launch: its first token is `STARTRENDERER`, or it contains both
/// `-QueueName` and `-QueueCapacity`. Any other single argument, including a path or record
/// URL that contains spaces, is returned unchanged. Already-split argv is returned unchanged.
pub(crate) fn expand_launcher_args(args: &[String]) -> Vec<String> {
    let [only] = args else {
        return args.to_vec();
    };
    if !only.chars().any(is_command_whitespace) {
        return args.to_vec();
    }
    let split = split_windows_command_line(only);
    if is_renderer_launch(&split) {
        split
    } else {
        args.to_vec()
    }
}

/// Whether these launcher arguments are Resonite starting this process as a renderer.
///
/// Returns the arguments to forward to `renderide-renderer`. `STARTRENDERER <exe> ARGS=...`
/// is unwrapped. A direct `-QueueName` / `-QueueCapacity` pair (the fixed `-Renderer` form)
/// is forwarded as-is, including any unknown tokens around it. Launcher-only flags
/// (`--log-level` / `-l`, `--resonite-dir` / `-ResoniteDir`, `--rollback-update`) are removed
/// first so they are not forwarded to the renderer.
pub fn renderer_delegate_from_args(args: &[String]) -> Option<Vec<String>> {
    let launch = parse_renderer_launch_message_from_argv(expand_launcher_args(args));
    if launch.executable.is_some() || has_renderer_connection_args(&launch.args) {
        return Some(strip_launcher_only_flags(&launch.args));
    }
    None
}

/// Spawns the sibling renderer, ties its lifetime to this process, and waits for it.
///
/// The Resonite bootstrapper tracks this process id and kills it when the session ends.
/// [`crate::child_lifetime::ChildLifetimeGroup`] applies the same parent-death behavior used
/// for the host (`PR_SET_PDEATHSIG` on Linux, a kill-on-close job on Windows, tracked PIDs on
/// macOS), so that kill also stops `renderide-renderer`. The returned status is the renderer's
/// own status.
///
/// Returns an error when the only candidate binary is this process. Launching that file would
/// start the launcher again.
pub fn spawn_renderer_delegate(args: &[String]) -> io::Result<ExitStatus> {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let program = renderer_executable_in(&dir)?;
    let lifetime = crate::child_lifetime::ChildLifetimeGroup::new()?;
    let mut cmd = Command::new(program);
    cmd.args(args);
    lifetime.prepare_command(&mut cmd);
    let mut child = cmd.spawn()?;
    if let Err(error) = lifetime.register_spawned(&child) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let status = child.wait()?;
    // macOS `Drop` signals every still-tracked pid and then sleeps through the grace periods.
    // The renderer has already been waited on, so leave the list empty and let `Drop` return
    // immediately. Linux parent-death is `PR_SET_PDEATHSIG` from `prepare_command`. Windows
    // kill-on-close stays armed until this function returns and drops the job handle; a kill
    // during `wait` still closes that handle with the process.
    lifetime.disarm();
    Ok(status)
}

/// Parses a host renderer-start message, unwrapping `STARTRENDERER` when present.
pub(crate) fn parse_renderer_launch_message(message: &str) -> RendererLaunchRequest {
    parse_renderer_launch_message_from_argv(split_windows_command_line(message))
}

/// Returns `true` when `args` already choose desktop, VR, or Resonite autodetect.
///
/// A bare `none` means autodetect only when it is its own argument. It is not a mode when it
/// is the following value of a flag that is known to take one (`-LoadAssembly none`,
/// `-Join none`, `-Data none`). It is still a mode after a boolean switch such as
/// `-Invisible none`. Unknown flags are treated the same way as boolean switches: Resonite has
/// many valueless switches, and assuming every flag consumes the next token would hide a real
/// `none`.
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
///
/// Prefers `renderide-renderer` and falls back to `Renderite.Renderer`. A candidate whose
/// canonical path is this process is skipped. Launching it would start the launcher again
/// whenever the launcher itself is installed under that file name. When every existing
/// candidate is this process, or neither file exists, this returns an error instead of a path
/// that [`Command::new`] would execute.
pub(crate) fn renderer_executable_in(dir: &Path) -> io::Result<PathBuf> {
    select_renderer_executable(dir, std::env::current_exe().ok().as_deref())
}

/// Picks a renderer binary in `dir`, skipping `current_exe` when that path is one of the names.
fn select_renderer_executable(dir: &Path, current_exe: Option<&Path>) -> io::Result<PathBuf> {
    let mut skipped_self = false;
    for name in [RENDERER_FILE_NAME, LEGACY_RENDERER_FILE_NAME] {
        let candidate = dir.join(name);
        if !candidate.is_file() {
            continue;
        }
        if current_exe.is_some_and(|current| same_executable(&candidate, current)) {
            skipped_self = true;
            continue;
        }
        return Ok(candidate);
    }
    if skipped_self {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refusing to launch {RENDERER_FILE_NAME} or {LEGACY_RENDERER_FILE_NAME} in {}: that file is this process, and launching it would re-launch the launcher forever",
                dir.display()
            ),
        ));
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!(
            "renderide-renderer was not found in {} (looked for {RENDERER_FILE_NAME} and {LEGACY_RENDERER_FILE_NAME})",
            dir.display()
        ),
    ))
}

/// Returns whether `candidate` and `current_exe` are the same file after resolving symlinks.
fn same_executable(candidate: &Path, current_exe: &Path) -> bool {
    match (
        std::fs::canonicalize(candidate),
        std::fs::canonicalize(current_exe),
    ) {
        (Ok(candidate), Ok(current_exe)) => candidate == current_exe,
        _ => candidate == current_exe,
    }
}

struct FlagToken {
    name: String,
    attached: Option<String>,
}

/// Parses one argument using the `CommandLineToArgvW` copy loop (not the `argv[0]` rules).
///
/// `quote_count` is `0` outside quotes and `1` inside quotes. A run that leaves it at `2` is
/// normalized back to `0`, which is how two quotes close a quoted argument after emitting the
/// quote characters required by the rule of three.
fn parse_windows_argument(chars: &[char], mut index: usize) -> (String, usize) {
    let mut out = String::new();
    let mut quote_count = 0usize;
    let mut backslashes = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if quote_count == 0 && is_command_whitespace(ch) {
            break;
        }
        if ch == '\\' {
            out.push('\\');
            backslashes += 1;
            index += 1;
            continue;
        }
        if ch == '"' {
            if backslashes.is_multiple_of(2) {
                pop_backslashes(&mut out, backslashes / 2);
                quote_count += 1;
            } else {
                pop_backslashes(&mut out, backslashes / 2 + 1);
                out.push('"');
            }
            index += 1;
            backslashes = 0;
            while index < chars.len() && chars[index] == '"' {
                quote_count += 1;
                if quote_count == 3 {
                    out.push('"');
                    quote_count = 0;
                }
                index += 1;
            }
            if quote_count == 2 {
                quote_count = 0;
            }
            continue;
        }
        out.push(ch);
        backslashes = 0;
        index += 1;
    }
    (out, index)
}

fn pop_backslashes(out: &mut String, count: usize) {
    for _ in 0..count {
        let removed = out.pop();
        debug_assert_eq!(removed, Some('\\'));
    }
}

fn push_repeat(out: &mut String, ch: char, count: usize) {
    for _ in 0..count {
        out.push(ch);
    }
}

fn is_command_whitespace(ch: char) -> bool {
    ch == ' ' || ch == '\t'
}

fn is_renderer_launch(argv: &[String]) -> bool {
    argv.first()
        .is_some_and(|token| token.eq_ignore_ascii_case("STARTRENDERER"))
        || has_renderer_connection_args(argv)
}

fn strip_launcher_only_flags(args: &[String]) -> Vec<String> {
    crate::cli::parse_bootstrap_args_tokens(args).host_args
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
        // Exact names only. A suffix match would treat `-NotQueueName` as the renderer handoff.
        if flag.name == "queuename" {
            saw_name = true;
        } else if flag.name == "queuecapacity" {
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
    fn quote_split_roundtrip_backslashes_whitespace_and_unc() {
        let args = owned(&[
            r#"a\\"b"#,
            r#"\""#,
            r"C:\dir\\",
            "   ",
            "\t",
            " \t ",
            r"\\server\share\file",
            r"\\server\share\My Documents",
            r"a\b",
            r"a\\b",
            r"a\\\b",
            r"a\\\\b",
        ]);
        let command = quote_windows_command_line(&args);
        assert_eq!(split_windows_command_line(&command), args);
    }

    #[test]
    fn split_matches_command_line_to_argvw_backslash_and_quote_runs() {
        // Even backslashes before a quote are halved and the quote is a delimiter.
        assert_eq!(
            split_windows_command_line(r#"a\\"b c""#),
            owned(&[r"a\b c"])
        );
        // Four backslashes are an even run as well.
        assert_eq!(
            split_windows_command_line(r#"a\\\\"b c""#),
            owned(&[r"a\\b c"])
        );
        // Odd runs emit a literal quote. `\"` is one backslash plus a quote.
        assert_eq!(split_windows_command_line(r#"a\"b"#), owned(&[r#"a"b"#]));
        assert_eq!(split_windows_command_line(r#"a\\\"b"#), owned(&[r#"a\"b"#]));
        // Argument tails from Wine's CommandLineToArgvW tests (the `exe` token omitted).
        assert_eq!(
            split_windows_command_line(r#"\"one \\"two" \\\"three \\\\"four" end"#),
            owned(&[r#""one"#, r"\two", r#"\"three"#, r"\\four", "end"])
        );
        // Rule of three, not the C runtime's "stay quoted after `""`".
        assert_eq!(
            split_windows_command_line(r#""a"" b""#),
            owned(&[r#"a""#, "b"])
        );
        assert_eq!(
            split_windows_command_line(r#"two""quotes next"#),
            owned(&["twoquotes", "next"])
        );
        assert_eq!(
            split_windows_command_line(r#"three"""quotes next"#),
            owned(&[r#"three"quotes"#, "next"])
        );
        assert_eq!(split_windows_command_line("\"   \""), owned(&["   "]));
        assert_eq!(split_windows_command_line("\"\t\""), owned(&["\t"]));
        assert_eq!(
            split_windows_command_line(r#""\\server\share\My Documents""#),
            owned(&[r"\\server\share\My Documents"])
        );
        // A trailing backslash run that does not precede a quote stays literal.
        assert_eq!(
            split_windows_command_line(r"C:\dir\\"),
            owned(&[r"C:\dir\\"])
        );
    }

    #[test]
    fn spaced_path_and_record_url_pass_through_unchanged() {
        for token in ["C:/Users/John Doe/x", "resrec:///G-Group Name/R-World"] {
            let args = owned(&[token]);
            assert_eq!(expand_launcher_args(&args), args);
            let parsed = crate::cli::parse_bootstrap_args_tokens(&args);
            assert_eq!(parsed.host_args, args);
        }
    }

    #[test]
    fn non_renderer_packed_line_is_not_split() {
        let packed = quote_windows_command_line(&owned(&[
            "--log-level",
            "debug",
            "-Screen",
            "-LoadAssembly",
            "Libraries/My Mod.dll",
        ]));
        assert_eq!(
            expand_launcher_args(std::slice::from_ref(&packed)),
            vec![packed]
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
        // Boolean and unknown switches do not consume the next token, so `none` is still
        // autodetect. Known value-taking flags do consume it.
        assert!(host_args_specify_output_mode(&owned(&[
            "-Invisible",
            "none"
        ])));
        assert!(host_args_specify_output_mode(&owned(&[
            "-CustomFlag",
            "none"
        ])));
        assert!(!host_args_specify_output_mode(&owned(&["-Join", "none"])));
        // `-Device` is itself an output flag, so the value `none` is not what selects the mode.
        assert!(host_args_specify_output_mode(&owned(&["-Device", "none"])));
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
    fn delegate_matches_exact_queue_flag_names() {
        assert!(
            renderer_delegate_from_args(&owned(&["-NotQueueName", "q", "-QueueCapacity", "1"]))
                .is_none()
        );
        assert!(
            renderer_delegate_from_args(&owned(&["-QueueNameSuffix", "q", "-QueueCapacity", "1"]))
                .is_none()
        );
        assert!(
            renderer_delegate_from_args(&owned(&["-QueueName", "q", "-MyQueueCapacity", "1"]))
                .is_none()
        );
        assert_eq!(
            renderer_delegate_from_args(&owned(&["--QueueName", "q", "/QueueCapacity", "8"])),
            Some(owned(&["--QueueName", "q", "/QueueCapacity", "8"]))
        );
    }

    #[test]
    fn delegate_strips_launcher_only_flags() {
        let args = owned(&[
            "--log-level",
            "debug",
            "-l",
            "trace",
            "--resonite-dir",
            "/games/Resonite",
            "-ResoniteDir",
            "/other",
            "--rollback-update",
            "-QueueName",
            "q",
            "-QueueCapacity",
            "10",
            "-Custom",
            "kept",
        ]);
        assert_eq!(
            renderer_delegate_from_args(&args),
            Some(owned(&[
                "-QueueName",
                "q",
                "-QueueCapacity",
                "10",
                "-Custom",
                "kept"
            ]))
        );
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
        let dir = temp_launch_dir("prefer");
        let primary = dir.join(RENDERER_FILE_NAME);
        let legacy = dir.join(LEGACY_RENDERER_FILE_NAME);
        std::fs::write(&legacy, b"legacy").unwrap();
        assert_eq!(renderer_executable_in(&dir).unwrap(), legacy);
        std::fs::write(&primary, b"primary").unwrap();
        assert_eq!(renderer_executable_in(&dir).unwrap(), primary);
        std::fs::remove_file(&primary).unwrap();
        std::fs::remove_file(&legacy).unwrap();
        let missing = renderer_executable_in(&dir).unwrap_err();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn renderer_executable_skips_candidate_that_is_this_process() {
        let dir = temp_launch_dir("self");
        let primary = dir.join(RENDERER_FILE_NAME);
        let legacy = dir.join(LEGACY_RENDERER_FILE_NAME);
        std::fs::write(&legacy, b"self").unwrap();

        let err = select_renderer_executable(&dir, Some(legacy.as_path())).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("re-launch the launcher"), "{err}");

        std::fs::write(&primary, b"primary").unwrap();
        assert_eq!(
            select_renderer_executable(&dir, Some(legacy.as_path())).unwrap(),
            primary
        );
        assert_eq!(
            select_renderer_executable(&dir, Some(primary.as_path())).unwrap(),
            legacy
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn renderer_executable_in_rejects_symlink_to_current_exe() {
        let dir = temp_launch_dir("symlink");
        let legacy = dir.join(LEGACY_RENDERER_FILE_NAME);
        let current = std::env::current_exe().unwrap();
        std::os::unix::fs::symlink(&current, &legacy).unwrap();
        let err = renderer_executable_in(&dir).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("re-launch the launcher"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn temp_launch_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "renderide_launch_args_{label}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
