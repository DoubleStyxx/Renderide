//! Minimal drone against a renderer started with `--features control-api`.
//!
//! ```text
//! renderide-renderer --control-port 9876
//! cargo run -p renderide-control-client --example drone -- 127.0.0.1:9876
//! ```
//!
//! `renderide-test` can pass the same flag through to a headless renderer when that
//! optional hook is needed. This example is the small standalone client.

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use renderide_control_client::{ControlClient, MouseMoveParams};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(addr) = args.next() else {
        write_line("usage: drone <127.0.0.1:port>");
        return ExitCode::from(2);
    };
    match run(&addr) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            write_line(&err.to_string());
            ExitCode::from(1)
        }
    }
}

fn run(addr: &str) -> Result<(), renderide_control_client::ClientError> {
    let mut client = ControlClient::connect(addr)?;
    let meta = client.meta()?;
    write_line(&format!(
        "adapter={} backend={} {}x{} sha={}",
        meta.adapter, meta.backend, meta.resolution[0], meta.resolution[1], meta.git_sha
    ));
    client.mouse_move(&MouseMoveParams {
        x: Some(0.0),
        y: Some(0.0),
        dx: 1.0,
        dy: 0.0,
    })?;
    let stats = client.stats_query(0)?;
    write_line(&format!("stats={}", stats.len()));
    Ok(())
}

fn write_line(line: &str) {
    let mut stderr = io::stderr().lock();
    let _ = writeln!(stderr, "{line}");
}
