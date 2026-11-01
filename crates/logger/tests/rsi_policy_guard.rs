//! Fails the workspace suite when the retired policy file is tracked again,
//! or when a tracked file outside `third_party/` and this guard still names it.
//!
//! `RSI_POLICY` contains that retired token as a suffix. A match preceded by
//! `R` is the living name and is not a violation.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const RETIRED_FILE: &str = "SI_POLICY.md";
const RETIRED_TOKEN: &[u8] = b"SI_POLICY";
const GUARD_REL: &str = "crates/logger/tests/rsi_policy_guard.rs";

/// Runs `git` in `root` and returns its output. A non-zero exit is an error.
fn git(root: &Path, args: &[&str]) -> Result<Output, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_PREFIX")
        .output()
        .map_err(|err| format!("git {args:?} failed to start: {err}"))?;
    if output.status.success() {
        return Ok(output);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!("git {args:?} exited {}: {stderr}", output.status))
}

/// Lists tracked paths from `git ls-files -z`.
fn tracked_paths(root: &Path) -> Result<Vec<String>, String> {
    let output = git(root, &["ls-files", "-z"])?;
    let mut paths = Vec::new();
    for part in output.stdout.split(|byte| *byte == 0) {
        if part.is_empty() {
            continue;
        }
        let rel = String::from_utf8(part.to_vec())
            .map_err(|_err| "git ls-files path is not utf-8".to_string())?;
        paths.push(rel);
    }
    Ok(paths)
}

/// True when `rel` is the vendored tree the guard does not edit.
fn is_third_party(rel: &str) -> bool {
    rel == "third_party" || rel.starts_with("third_party/") || rel.starts_with("third_party\\")
}

/// True when the path's last component is the retired policy filename.
fn is_retired_filename(rel: &str) -> bool {
    rel.rsplit(['/', '\\']).next() == Some(RETIRED_FILE)
}

/// True when `bytes` names the retired policy and the hit is not `RSI_POLICY`.
fn references_retired_token(bytes: &[u8]) -> bool {
    bytes
        .windows(RETIRED_TOKEN.len())
        .enumerate()
        .any(|(index, window)| window == RETIRED_TOKEN && (index == 0 || bytes[index - 1] != b'R'))
}

/// Violations for the git index at `root`. An empty list means the tree passes.
fn policy_violations(root: &Path) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for rel in tracked_paths(root)? {
        if is_retired_filename(&rel) {
            violations.push(format!("retired policy file is tracked: {rel}"));
        }
        if is_third_party(&rel) || rel == GUARD_REL {
            continue;
        }
        let bytes = fs::read(root.join(&rel)).map_err(|err| format!("cannot read {rel}: {err}"))?;
        if references_retired_token(&bytes) {
            violations.push(format!(
                "tracked file references the retired policy name: {rel}"
            ));
        }
    }
    violations.sort();
    Ok(violations)
}

/// Walks ancestors until the Renderide workspace root is found.
fn workspace_root() -> Result<PathBuf, String> {
    let start = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut current = start.as_path();
    loop {
        if current.join("Cargo.toml").is_file()
            && current.join("crates/logger/Cargo.toml").is_file()
            && current.join("crates/renderide/Cargo.toml").is_file()
        {
            return Ok(current.to_path_buf());
        }
        current = current
            .parent()
            .ok_or("CARGO_MANIFEST_DIR is inside the Renderide workspace")?;
    }
}

/// Creates a temp git repo and stages `files` as `(relative path, body)`.
fn init_repo(files: &[(&str, &str)]) -> Result<tempfile::TempDir, String> {
    let dir = tempfile::tempdir().map_err(|err| format!("tempdir: {err}"))?;
    git(dir.path(), &["init", "-b", "master"])?;
    for (rel, body) in files {
        let path = dir.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("parent dirs for {rel}: {err}"))?;
        }
        fs::write(&path, body).map_err(|err| format!("write {rel}: {err}"))?;
    }
    git(dir.path(), &["add", "--all"])?;
    Ok(dir)
}

/// The living filename contains the retired token and must not trip the scan.
#[test]
fn replacement_name_is_not_the_retired_token() {
    assert!(!references_retired_token(b"see RSI_POLICY.md"));
    assert!(!references_retired_token(b"RSI_POLICY"));
    assert!(references_retired_token(b"see SI_POLICY.md"));
    assert!(references_retired_token(b"SI_POLICY"));
    assert!(references_retired_token(b"\nSI_POLICY.md\n"));
    assert!(!references_retired_token(b"short"));
}

/// A clean index passes. Staging the retired file makes the same tree fail.
#[test]
fn restored_policy_file_fails() {
    let repo = init_repo(&[
        ("RSI_POLICY.md", "ratchet\n"),
        ("README.md", "see RSI_POLICY.md\n"),
    ])
    .expect("init fixture repo");
    let clean = policy_violations(repo.path()).expect("clean scan");
    assert!(
        clean.is_empty(),
        "fixture without the retired file must pass: {clean:?}"
    );

    fs::write(repo.path().join(RETIRED_FILE), "restored policy\n").expect("restore policy file");
    git(repo.path(), &["add", "--", RETIRED_FILE]).expect("stage restored policy file");

    let violations = policy_violations(repo.path()).expect("scan after restore");
    assert!(
        violations.iter().any(|item| item.contains(RETIRED_FILE)),
        "restoring {RETIRED_FILE} must fail the guard, got {violations:?}"
    );
}

/// A tracked reference outside the guard and `third_party/` fails the scan.
#[test]
fn tracked_reference_fails() {
    let repo = init_repo(&[
        ("RSI_POLICY.md", "ok\n"),
        ("docs/note.md", "see SI_POLICY.md\n"),
    ])
    .expect("init fixture repo");
    let violations = policy_violations(repo.path()).expect("scan");
    assert!(
        violations.iter().any(|item| item.contains("docs/note.md")),
        "a tracked reference must fail, got {violations:?}"
    );
}

/// `third_party/` and this guard may still contain the retired name.
#[test]
fn third_party_and_guard_are_ignored() {
    let repo = init_repo(&[
        ("RSI_POLICY.md", "RSI means recursive self-improvement.\n"),
        ("README.md", "See RSI_POLICY.md.\n"),
        ("third_party/old.txt", "mentions SI_POLICY.md\n"),
        (GUARD_REL, "const NAME: &str = \"SI_POLICY.md\";\n"),
    ])
    .expect("init fixture repo");
    let violations = policy_violations(repo.path()).expect("scan");
    assert!(
        violations.is_empty(),
        "third_party and the guard must not fail the scan: {violations:?}"
    );
}

/// The checkout this test runs in has no retired policy file or reference.
#[test]
fn workspace_has_no_retired_policy_references() {
    let root = workspace_root().expect("workspace root");
    let violations = policy_violations(&root).expect("scan workspace");
    assert!(
        violations.is_empty(),
        "retired policy name is still in the tree:\n{}",
        violations.join("\n")
    );
}
