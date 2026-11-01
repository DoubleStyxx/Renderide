//! Structural guard for driver-thread `Queue::submit` ownership.
//!
//! The driver loop is the only production submit on the renderer's shared queue.
//! Readback and repair passes enqueue through that loop and must observe a flush
//! timeout instead of submitting on the caller thread anyway.
//!
//! These checks fail on master `a557b624` (caller-thread submits, and `flush`
//! discards the timeout) and pass once that work is routed through the driver.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Production files allowed to call `wgpu::Queue::submit` on the shared renderer queue.
const DRIVER_THREAD_SUBMIT_FILES: &[&str] = &["src/gpu/driver_thread/worker.rs"];

/// Tests that construct a private `wgpu::Device` and never touch the renderer queue
/// or `GpuQueueAccessGate`.
const PRIVATE_DEVICE_TEST_FILES: &[&str] = &["src/gpu_pools/pools/render_texture_ping_pong_gpu.rs"];

/// Caller-thread submits that must move onto the driver thread.
const DRIVER_ORDERED_SUBMIT_FILES: &[&str] = &[
    "src/app/headless/readback.rs",
    "src/app/headless/scene_color_dump.rs",
    "src/gpu/frame_bench_queries.rs",
    "src/runtime/offscreen_tasks/camera.rs",
    "src/runtime/offscreen_tasks/camera/alpha_coverage.rs",
    "src/runtime/offscreen_tasks/camera/camera360.rs",
    "src/runtime/offscreen_tasks/reflection_probe/readback.rs",
];

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources() -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_rust_sources(&manifest_dir().join("src"), &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_rust_sources(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_rust_sources(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn file_label(path: &Path) -> String {
    path.strip_prefix(manifest_dir())
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

fn submit_allowed(label: &str) -> bool {
    DRIVER_THREAD_SUBMIT_FILES.contains(&label) || PRIVATE_DEVICE_TEST_FILES.contains(&label)
}

fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*")
}

fn line_submits_queue(line: &str) -> bool {
    let trimmed = line.trim();
    if is_comment_line(trimmed) {
        return false;
    }
    trimmed.contains(".queue().submit(") || trimmed.contains(".queue.submit(")
}

/// `Queue::submit` outside the driver thread, or a private test device, is a regression.
#[test]
fn renderer_queue_submit_stays_on_the_driver_thread() -> io::Result<()> {
    let mut offenders = Vec::new();
    for path in rust_sources()? {
        let label = file_label(&path);
        let src = fs::read_to_string(&path)?;
        for (idx, line) in src.lines().enumerate() {
            if line_submits_queue(line) && !submit_allowed(&label) {
                offenders.push(format!("{label}:{}", idx + 1));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "Queue::submit on the shared renderer queue must stay in the driver thread:\n  - {}",
        offenders.join("\n  - ")
    );
    let worker = fs::read_to_string(manifest_dir().join(DRIVER_THREAD_SUBMIT_FILES[0]))?;
    assert!(
        worker.lines().any(line_submits_queue),
        "driver thread worker no longer submits; the allowlist would be vacuous"
    );
    Ok(())
}

/// `DriverThread::flush` must report the timeout, and no caller may ignore it.
#[test]
fn flush_timeout_is_returned_to_callers() -> io::Result<()> {
    let driver = fs::read_to_string(manifest_dir().join("src/gpu/driver_thread.rs"))?;
    assert!(
        !driver.contains("let _ = self.flush_within"),
        "DriverThread::flush discards flush_within's timeout bool"
    );
    assert!(
        driver.contains("pub fn flush(&self) -> bool"),
        "DriverThread::flush must return whether the driver signaled"
    );
    let submission = fs::read_to_string(manifest_dir().join("src/gpu/context/submission.rs"))?;
    assert!(
        submission.contains("pub fn flush_driver(&self) -> bool"),
        "GpuContext::flush_driver must return the driver timeout to its caller"
    );

    let mut ignored = Vec::new();
    for path in rust_sources()? {
        let label = file_label(&path);
        let src = fs::read_to_string(&path)?;
        for (idx, line) in src.lines().enumerate() {
            let trimmed = line.trim();
            if is_comment_line(trimmed) || !trimmed.contains("flush_driver()") {
                continue;
            }
            let bare_statement = trimmed.contains("flush_driver();");
            let discarded = trimmed.contains("let _ =") && trimmed.contains("flush_driver()");
            if bare_statement || discarded {
                ignored.push(format!("{label}:{}", idx + 1));
            }
        }
    }
    assert!(
        ignored.is_empty(),
        "flush_driver() timeout must be handled; a bare call ignores it:\n  - {}",
        ignored.join("\n  - ")
    );
    Ok(())
}

/// Alpha repair, Camera360 projection, and readback copies submit on the driver thread.
#[test]
fn ordered_gpu_work_uses_the_driver_submit_path() -> io::Result<()> {
    let mut missing = Vec::new();
    for label in DRIVER_ORDERED_SUBMIT_FILES {
        let src = fs::read_to_string(manifest_dir().join(label))?;
        if !src.contains("submit_background_and_wait(") {
            missing.push((*label).to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "these passes must enqueue through submit_background_and_wait so they stay behind the capture:\n  - {}",
        missing.join("\n  - ")
    );
    Ok(())
}
