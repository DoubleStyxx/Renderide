//! Parsing Host messages into bootstrapper commands.

/// Command sent from the Host over `bootstrapper_in`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCommand {
    /// Extends the IPC watchdog deadline.
    Heartbeat,
    /// Clean shutdown request.
    Shutdown,
    /// Clipboard read request.
    GetText,
    /// Clipboard write (payload after `SETTEXT` prefix).
    SetText(String),
    /// Spawn the renderer. `STARTRENDERER <exe> ARGS=...` is unwrapped first.
    StartRenderer(crate::launch_args::RendererLaunchRequest),
}

impl HostCommand {
    /// Redacted command summary safe for logs.
    pub fn log_summary(&self) -> String {
        match self {
            Self::Heartbeat => String::from("HEARTBEAT"),
            Self::Shutdown => String::from("SHUTDOWN"),
            Self::GetText => String::from("GETTEXT"),
            Self::SetText(text) => format!("SETTEXT redacted_len={}", text.len()),
            Self::StartRenderer(launch) => {
                let first = launch.args.first().map_or("<empty>", String::as_str);
                format!("StartRenderer argc={} first={first}", launch.args.len())
            }
        }
    }
}

/// Parses a UTF-8 message from the Host into a [`HostCommand`].
///
/// Recognized prefixes: `HEARTBEAT`, `SHUTDOWN`, `GETTEXT`, `SETTEXT<payload>`.
/// Any other input is a renderer launch. Tokens follow Windows command-line quoting, and a
/// `STARTRENDERER <exe> ARGS=...` wrapper is removed before the arguments are forwarded.
/// This catch-all is how `BootstrapperManager` requests a renderer launch.
pub fn parse_host_command(s: &str) -> HostCommand {
    match s {
        "HEARTBEAT" => HostCommand::Heartbeat,
        "SHUTDOWN" => HostCommand::Shutdown,
        "GETTEXT" => HostCommand::GetText,
        _ if s.starts_with("SETTEXT") => HostCommand::SetText(
            s.strip_prefix("SETTEXT")
                .map(str::to_string)
                .unwrap_or_default(),
        ),
        _ => {
            let launch = crate::launch_args::parse_renderer_launch_message(s);
            let first = launch.args.first().map_or("<empty>", String::as_str);
            logger::debug!(
                "Bootstrap message did not match a known command; treating as renderer argv (first token: {first})"
            );
            HostCommand::StartRenderer(launch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HostCommand, parse_host_command};

    #[test]
    fn settext_log_summary_redacts_payload() {
        let cmd = parse_host_command("SETTEXTsuper-secret-token");

        assert_eq!(
            cmd,
            HostCommand::SetText(String::from("super-secret-token"))
        );
        let summary = cmd.log_summary();
        assert!(summary.contains("redacted_len=18"));
        assert!(!summary.contains("super-secret-token"));
    }
}
