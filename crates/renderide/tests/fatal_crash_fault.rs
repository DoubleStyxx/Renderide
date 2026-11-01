//! Live fatal-fault check. The handler must write raw instruction pointers and
//! return; symbol names are appended by a second run of this same binary.
//!
//! `probe_capture_child` and `probe_symbolicate_child` return immediately unless
//! this process was re-executed with `RENDERIDE_FATAL_CRASH_CHILD` set.

#![cfg(any(unix, windows))]

use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const CHILD_ENV: &str = "RENDERIDE_FATAL_CRASH_CHILD";
const LOG_ENV: &str = "RENDERIDE_FATAL_CRASH_LOG";

#[test]
fn probe_capture_child() {
    if std::env::var(CHILD_ENV).ok().as_deref() != Some("capture") {
        return;
    }
    let path = std::env::var(LOG_ENV).expect("log path");
    renderide::fatal_crash_test_support::install_handler(Path::new(&path))
        .expect("install handler");
    renderide::fatal_crash_test_support::trip_fatal_fault();
}

#[test]
fn probe_symbolicate_child() {
    if std::env::var(CHILD_ENV).ok().as_deref() != Some("symbolicate") {
        return;
    }
    let path = std::env::var(LOG_ENV).expect("log path");
    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    {
        renderide::fatal_crash_test_support::symbolicate_log(Path::new(&path))
            .expect("symbolicate");
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
    {
        let _ = path;
    }
}

#[test]
fn fatal_fault_records_raw_ips_and_symbolicates_on_next_start() {
    let dir = tempfile::tempdir().expect("tempdir");
    let log = dir.path().join("crash.log");
    let log_str = log.to_str().expect("utf8 temp path");

    let (status, output) = run_child("probe_capture_child", log_str, Duration::from_secs(20));
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        !status.success(),
        "capture probe should die from the fault, status {status:?}, output:\n{output}\nlog:\n{text}"
    );
    assert!(
        text.contains("FATAL:"),
        "handler did not write a fatal line.\nstatus {status:?}\noutput:\n{output}\nlog:\n{text}"
    );
    assert!(
        !text.contains("SYMBOLS:\n"),
        "handler wrote a symbol block before returning:\n{text}"
    );

    #[cfg(any(target_os = "linux", target_os = "android", windows))]
    {
        assert!(text.contains("STACK ("), "no raw stack in log:\n{text}");
        assert!(
            text.contains("MODULE base=0x"),
            "no module span in log:\n{text}"
        );
        assert!(
            text.contains("SYMBOLS: deferred"),
            "no deferred notice in log:\n{text}"
        );

        let (status, output) =
            run_child("probe_symbolicate_child", log_str, Duration::from_secs(60));
        let symbolicated = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(
            status.success(),
            "symbolicate probe failed: {status:?}\n{output}\n{symbolicated}"
        );
        assert!(
            symbolicated.contains("\nSYMBOLS:\n") || symbolicated.starts_with("SYMBOLS:\n"),
            "next start did not append symbols:\n{symbolicated}"
        );
        assert!(
            symbolicated.contains("trip_fatal_fault")
                || symbolicated.contains("trip_unix")
                || symbolicated.contains("trip_windows"),
            "saved instruction pointers did not resolve to the probe:\n{symbolicated}"
        );
    }
}

#[expect(
    clippy::expect_used,
    reason = "a probe that cannot start is a test failure"
)]
#[expect(clippy::panic, reason = "a hung or unwaitable probe is a test failure")]
fn run_child(test_name: &str, log_path: &str, timeout: Duration) -> (ExitStatus, String) {
    let exe = std::env::current_exe().expect("current_exe");
    let mut child = Command::new(exe)
        .args(["--exact", test_name, "--test-threads=1"])
        .env(CHILD_ENV, child_mode(test_name))
        .env(LOG_ENV, log_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn probe");

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                let secs = timeout.as_secs();
                panic!("probe {test_name} still running after {secs}s");
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => panic!("wait failed: {e}"),
        }
    };
    let mut output = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_string(&mut output);
    }
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut output);
    }
    (status, output)
}

fn child_mode(test_name: &str) -> &'static str {
    if test_name.contains("symbolicate") {
        "symbolicate"
    } else {
        "capture"
    }
}
