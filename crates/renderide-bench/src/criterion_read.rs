//! Reads criterion 0.8 `estimates.json` trees under `target/criterion`.
//!
//! Layout (verified against criterion 0.8.2): `target/criterion/<group>/<id>/new/estimates.json`
//! with `median.point_estimate` in nanoseconds and
//! `median.confidence_interval.{lower_bound,upper_bound}`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::BenchError;
use crate::report::CpuSample;

#[derive(Debug, Deserialize)]
struct EstimatesFile {
    median: Estimate,
}

#[derive(Debug, Deserialize)]
struct Estimate {
    point_estimate: f64,
    confidence_interval: ConfidenceInterval,
}

#[derive(Debug, Deserialize)]
struct ConfidenceInterval {
    lower_bound: f64,
    upper_bound: f64,
}

/// Walks `criterion_dir` for `*/new/estimates.json` and returns one [`CpuSample`] per bench.
///
/// # Errors
///
/// Returns [`BenchError::Run`] when a file is unreadable or not valid estimates JSON.
pub fn collect_cpu_samples(criterion_dir: &Path) -> Result<Vec<CpuSample>, BenchError> {
    if !criterion_dir.is_dir() {
        return Err(BenchError::Run(format!(
            "criterion directory {} is missing; run the CPU suite first",
            criterion_dir.display()
        )));
    }
    let mut samples = Vec::new();
    for estimates in find_new_estimates(criterion_dir)? {
        let rel = estimates
            .strip_prefix(criterion_dir)
            .unwrap_or(estimates.as_path());
        // <group>/<id>/new/estimates.json
        let parts: Vec<_> = rel
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        if parts.len() < 4 {
            continue;
        }
        let group = parts[0].clone();
        let id = parts[1].clone();
        if parts[2] != "new" || parts[3] != "estimates.json" {
            continue;
        }
        let raw = fs::read(&estimates)
            .map_err(|err| BenchError::Run(format!("read {}: {err}", estimates.display())))?;
        let parsed: EstimatesFile = serde_json::from_slice(&raw)
            .map_err(|err| BenchError::Run(format!("parse {}: {err}", estimates.display())))?;
        if !parsed.median.point_estimate.is_finite() {
            return Err(BenchError::Run(format!(
                "{} has a non-finite median",
                estimates.display()
            )));
        }
        samples.push(CpuSample {
            group,
            id,
            median_ns: parsed.median.point_estimate,
            ci95_ns: [
                parsed.median.confidence_interval.lower_bound,
                parsed.median.confidence_interval.upper_bound,
            ],
            throughput: None,
        });
    }
    samples.sort_by(|a, b| (&a.group, &a.id).cmp(&(&b.group, &b.id)));
    Ok(samples)
}

fn find_new_estimates(root: &Path) -> Result<Vec<PathBuf>, BenchError> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir)
            .map_err(|err| BenchError::Run(format!("read_dir {}: {err}", dir.display())))?;
        for entry in entries {
            let entry = entry
                .map_err(|err| BenchError::Run(format!("read_dir {}: {err}", dir.display())))?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some("estimates.json") {
                let parent = path
                    .parent()
                    .and_then(Path::file_name)
                    .and_then(|n| n.to_str());
                if parent == Some("new") {
                    out.push(path);
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::collect_cpu_samples;
    use std::fs;

    #[test]
    fn reads_criterion_0_8_estimates_layout() {
        let dir = tempfile::tempdir().expect("tempdir");
        let est = dir
            .path()
            .join("culling")
            .join("homogeneous_clip_aabb")
            .join("new");
        fs::create_dir_all(&est).expect("mkdir");
        fs::write(
            est.join("estimates.json"),
            r#"{"mean":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":2.0},"point_estimate":1.5,"standard_error":0.1},"median":{"confidence_interval":{"confidence_level":0.95,"lower_bound":80000.0,"upper_bound":82000.0},"point_estimate":81234.0,"standard_error":100.0},"median_abs_dev":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":2.0},"point_estimate":1.5,"standard_error":0.1},"slope":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":2.0},"point_estimate":1.5,"standard_error":0.1},"std_dev":{"confidence_interval":{"confidence_level":0.95,"lower_bound":1.0,"upper_bound":2.0},"point_estimate":1.5,"standard_error":0.1}}"#,
        )
        .expect("write");
        // change/ and report/ noise must be ignored
        fs::create_dir_all(dir.path().join("culling/homogeneous_clip_aabb/change")).expect("mkdir");
        fs::write(
            dir.path()
                .join("culling/homogeneous_clip_aabb/change/estimates.json"),
            "{}",
        )
        .expect("write");
        let samples = collect_cpu_samples(dir.path()).expect("collect");
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].group, "culling");
        assert_eq!(samples[0].id, "homogeneous_clip_aabb");
        assert!((samples[0].median_ns - 81234.0).abs() < f64::EPSILON);
        assert!((samples[0].ci95_ns[0] - 80000.0).abs() < f64::EPSILON);
        assert!((samples[0].ci95_ns[1] - 82000.0).abs() < f64::EPSILON);
    }
}
