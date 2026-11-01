//! Port of `scripts/ci/bench_pr_report.py`.
use clap::Parser;
use std::fs;
use std::path::{Path, PathBuf};

use crate::bench_pr_assemble;

const FRAME_SUMMARY_MISSING: &str = "_frame gate artifact missing_";
const CPU_SUMMARY_MISSING: &str =
    "_cpu_hot artifact missing (job may have been skipped or failed before upload)_";
const GATE_JSON_MISSING: &str = "gate.json missing; cannot assemble bench-compare.json";

#[derive(Debug, Parser)]
pub struct Args {
    #[arg(long = "gate-art")]
    pub gate_art: PathBuf,
    #[arg(long = "cpu-art")]
    pub cpu_art: PathBuf,
    #[arg(long = "summary-out")]
    pub summary_out: PathBuf,
    #[arg(long = "compare-out")]
    pub compare_out: PathBuf,
    #[arg(long = "frame-job-result")]
    pub frame_job_result: String,
    #[arg(long = "gate-exit", default_value = "")]
    pub gate_exit: String,
    #[arg(long = "wall-time-sec", default_value = "")]
    pub wall_time_sec: String,
    #[arg(long = "cpu-job-result")]
    pub cpu_job_result: String,
}

pub fn build_combined_summary(
    frame_job_result: &str,
    gate_exit: &str,
    wall_time_sec: &str,
    cpu_job_result: &str,
    frame_summary: Option<&str>,
    cpu_summary: Option<&str>,
    gate_json_present: bool,
) -> String {
    let mut lines = vec![
        "# bench-pr (combined)".to_string(),
        String::new(),
        format!(
            "- frame job: `{frame_job_result}` (gate_exit={gate_exit}, wall_time_sec={wall_time_sec})"
        ),
        format!("- cpu_hot job: `{cpu_job_result}` (report-only; never gates)"),
        "- decision rule: `scripts/ci/frame_bench_gate.py` only".to_string(),
        String::new(),
    ];
    if let Some(fs) = frame_summary {
        lines.push(fs.trim_end_matches('\n').to_string());
    } else {
        lines.push(FRAME_SUMMARY_MISSING.into());
    }
    lines.push(String::new());
    if let Some(cs) = cpu_summary {
        lines.push(cs.trim_end_matches('\n').to_string());
    } else {
        lines.push(CPU_SUMMARY_MISSING.into());
    }
    lines.push(String::new());
    if !gate_json_present {
        lines.push(GATE_JSON_MISSING.into());
        lines.push(String::new());
    }
    lines.join("\n")
}

fn read_optional_text(path: &Path) -> Option<String> {
    if path.is_file() {
        fs::read_to_string(path).ok()
    } else {
        None
    }
}

pub fn run(args: Args) -> i32 {
    let frame_summary = read_optional_text(&args.gate_art.join("frame-summary.md"));
    let cpu_summary = read_optional_text(&args.cpu_art.join("cpu-summary.md"));
    let gate_json = args.gate_art.join("gate.json");
    let cpu_compare = args.cpu_art.join("cpu-compare.json");
    let gate_present = gate_json.is_file();

    let summary = build_combined_summary(
        &args.frame_job_result,
        &args.gate_exit,
        &args.wall_time_sec,
        &args.cpu_job_result,
        frame_summary.as_deref(),
        cpu_summary.as_deref(),
        gate_present,
    );
    if let Some(parent) = args.summary_out.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(e) = fs::write(&args.summary_out, &summary) {
        eprintln!("bench_pr_report: {e}");
        return 2;
    }
    if gate_present {
        let assemble = bench_pr_assemble::Args {
            gate: gate_json,
            cpu_compare: cpu_compare.is_file().then_some(cpu_compare),
            out: args.compare_out,
        };
        let rc = bench_pr_assemble::run(assemble);
        if rc != 0 {
            eprintln!("bench_pr_report: assemble exited {rc}");
        }
    }
    0
}
