//! Integration: [`bootstrapper::cli`] parsing as a downstream crate would call it (separate test binary).

#![forbid(unsafe_code)]
#![deny(clippy::multiple_unsafe_ops_per_block)]

use logger::LogLevel;

/// Strips bootstrapper-only flags and preserves Host forwarding semantics.
#[test]
fn parse_host_args_tokens_forwards_host_argv_and_log_level() {
    let args = vec!["--log-level".into(), "info".into(), "-Invisible".into()];
    let (host, level) = bootstrapper::cli::parse_host_args_tokens(&args);
    assert_eq!(host, vec!["-Invisible".to_string()]);
    assert_eq!(level, Some(LogLevel::Info));
}

/// Empty argv yields empty host list and no log level override.
#[test]
fn parse_host_args_tokens_empty_slice() {
    let (host, level) = bootstrapper::cli::parse_host_args_tokens(&[]);
    assert!(host.is_empty());
    assert!(level.is_none());
}

/// Unknown Resonite arguments, including values with spaces, are forwarded intact.
#[test]
fn parse_bootstrap_args_tokens_forwards_unknown_resonite_args() {
    let args = vec![
        "--log-level".into(),
        "warn".into(),
        "-Screen".into(),
        "-LoadAssembly".into(),
        "Libraries/My Mod.dll".into(),
        "-CustomFlag".into(),
        "still here".into(),
        "-Renderer".into(),
        r"C:\Program Files\AltRenderer.exe".into(),
    ];
    let parsed = bootstrapper::cli::parse_bootstrap_args_tokens(&args);
    assert_eq!(parsed.log_level, Some(LogLevel::Warn));
    assert!(!parsed.rollback_update);
    assert_eq!(
        parsed.host_args,
        vec![
            "-Screen".to_string(),
            "-LoadAssembly".to_string(),
            "Libraries/My Mod.dll".to_string(),
            "-CustomFlag".to_string(),
            "still here".to_string(),
            "-Renderer".to_string(),
            r"C:\Program Files\AltRenderer.exe".to_string(),
        ]
    );
}

/// Update rollback is a launcher-only flag and must not leak to Host argv.
#[test]
fn parse_bootstrap_args_tokens_consumes_rollback_update() {
    let args = vec!["--rollback-update".into(), "-Invisible".into()];
    let parsed = bootstrapper::cli::parse_bootstrap_args_tokens(&args);
    assert_eq!(parsed.host_args, vec!["-Invisible".to_string()]);
    assert!(parsed.rollback_update);
}
