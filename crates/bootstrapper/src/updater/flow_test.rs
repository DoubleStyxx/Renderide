//! Update-flow tests against [`super::mock_release`].
//!
//! The success case spawns a copy of this test binary as the installed launcher and lets
//! `self_replace` swap that copy. The parent process is not the file being replaced.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use self_update::update::ReleaseAsset;
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

use super::bundle::EntryKind;
use super::github::asset_name_for;
use super::mock_release::{MockServer, ServedAsset, ServedRelease};
use super::{
    RELEASE_CHANNEL, ReleaseBuildMetadata, StartupUpdateOutcome, UpdateCandidate, UpdateError,
    UpdatePromptChoice,
};

const CURRENT_TAG: &str = "nightly-2026-05-26-1111111";
const UPDATE_TAG: &str = "nightly-2026-05-27-2222222";
const CURRENT_COMMIT: &str = "1111111111111111111111111111111111111111";
const UPDATE_COMMIT: &str = "2222222222222222222222222222222222222222";
const NEW_LAUNCHER: &[u8] = b"updated-launcher-bytes";
const NEW_RENDERER: &[u8] = b"updated-renderer-bytes";
const OLD_LAUNCHER: &[u8] = b"old-launcher-bytes";
const OLD_RENDERER: &[u8] = b"old-renderer-bytes";
const REPLACE_TEST: &str =
    "updater::flow_test::running_exe_is_replaced_when_a_newer_release_is_installed";

fn platform() -> &'static str {
    super::current_platform().expect("updater tests run on a release platform")
}

fn metadata() -> ReleaseBuildMetadata {
    ReleaseBuildMetadata {
        channel: RELEASE_CHANNEL.to_owned(),
        tag: CURRENT_TAG.to_owned(),
        commit: CURRENT_COMMIT.to_owned(),
        platform: platform().to_owned(),
    }
}

fn release_body(commit: &str) -> String {
    format!("Commit: {commit}\n\n## Changelog\n\n- fixture release\n")
}

fn launcher_relative(platform: &str) -> &'static str {
    super::bundle::required_bundle_entries(platform)
        .into_iter()
        .find(|entry| entry.launcher)
        .map(|entry| entry.relative)
        .expect("launcher entry")
}

fn renderer_relative(platform: &str) -> &'static str {
    super::bundle::required_bundle_entries(platform)
        .into_iter()
        .find(|entry| !entry.launcher && entry.relative.contains("renderer"))
        .map(|entry| entry.relative)
        .expect("renderer entry")
}

/// Files the manifest must hash: launcher, renderer, loader (when required), and one file in each
/// required directory.
fn payload_files(platform: &str, launcher: &[u8], renderer: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    for entry in super::bundle::required_bundle_entries(platform) {
        match entry.kind {
            EntryKind::File if entry.launcher => {
                files.push((entry.relative.to_owned(), launcher.to_vec()));
            }
            EntryKind::File if entry.relative.contains("renderer") => {
                files.push((entry.relative.to_owned(), renderer.to_vec()));
            }
            EntryKind::File => {
                files.push((entry.relative.to_owned(), b"loader".to_vec()));
            }
            EntryKind::Directory => {
                let name = match entry.relative {
                    "xr" => "xr/actions.json",
                    "shaders" => "shaders/shader_manifest.toml",
                    other => panic!("unexpected required directory {other}"),
                };
                files.push((name.to_owned(), b"{}".to_vec()));
            }
        }
    }
    files
}

fn manifest_text(platform: &str, files: &[(String, Vec<u8>)]) -> String {
    let mut sha256 = serde_json::Map::new();
    for (name, bytes) in files {
        sha256.insert(name.clone(), serde_json::Value::String(sha256_hex(bytes)));
    }
    let required: Vec<&str> = super::bundle::required_bundle_entries(platform)
        .iter()
        .map(|entry| entry.relative)
        .collect();
    serde_json::json!({
        "schema": 1,
        "channel": RELEASE_CHANNEL,
        "tag": UPDATE_TAG,
        "commit": UPDATE_COMMIT,
        "platform": platform,
        "required_files": required,
        "sha256": sha256,
    })
    .to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn zip_archive(files: &[(String, Vec<u8>)], manifest: &str, signature: &str) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .unix_permissions(0o100_644);
    let executable = stored.unix_permissions(0o100_755);
    write_zip_file(
        &mut writer,
        "renderide-release.json",
        manifest.as_bytes(),
        stored,
    );
    write_zip_file(
        &mut writer,
        "renderide-release.json.sig",
        signature.as_bytes(),
        stored,
    );
    for (name, bytes) in files {
        let options = if name.contains("renderide") {
            executable
        } else {
            stored
        };
        write_zip_file(&mut writer, name, bytes, options);
    }
    writer.finish().expect("finish zip").into_inner()
}

fn write_zip_file<W: Write + std::io::Seek>(
    writer: &mut zip::ZipWriter<W>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) {
    writer.start_file(name, options).expect("start zip file");
    writer.write_all(bytes).expect("write zip file");
}

fn signed_archive(platform: &str, launcher: &[u8], renderer: &[u8]) -> Vec<u8> {
    let files = payload_files(platform, launcher, renderer);
    let manifest = manifest_text(platform, &files);
    let signature = super::bundle::signed_manifest_for_test(&manifest);
    zip_archive(&files, &manifest, &signature)
}

fn served_update(platform: &str, archive: Vec<u8>) -> ServedRelease {
    ServedRelease {
        tag: UPDATE_TAG.to_owned(),
        body: release_body(UPDATE_COMMIT),
        assets: vec![ServedAsset {
            name: asset_name_for(platform, UPDATE_TAG),
            bytes: archive,
        }],
    }
}

fn write_install_tree(dir: &Path, platform: &str, launcher: &[u8], renderer: &[u8]) {
    for (name, bytes) in payload_files(platform, launcher, renderer) {
        let path = dir.join(&name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create install parent");
        }
        fs::write(&path, bytes).expect("write install file");
    }
}

fn read_file(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    fs::set_permissions(path, permissions).expect("chmod");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

#[test]
fn already_up_to_date_does_not_download_or_prompt() {
    let platform = platform();
    let server = MockServer::serving(vec![vec![ServedRelease {
        tag: CURRENT_TAG.to_owned(),
        body: release_body(CURRENT_COMMIT),
        assets: vec![ServedAsset {
            name: asset_name_for(platform, CURRENT_TAG),
            bytes: b"not-downloaded".to_vec(),
        }],
    }]]);
    let fetched = super::github::fetch_latest_candidate_from(&server.base, &metadata())
        .expect("listing succeeds");

    assert!(fetched.is_none());
    assert_eq!(server.download_requests(), 0);

    let mut prompted = false;
    let outcome = super::startup::run_with_fetched_candidate(
        &metadata(),
        Ok(None),
        |_| {
            prompted = true;
            UpdatePromptChoice::Update
        },
        |_| {},
    );
    assert!(!prompted);
    assert_eq!(outcome, StartupUpdateOutcome::Continue);
}

#[test]
fn network_failure_does_not_prompt_or_replace_the_installed_binary() {
    let platform = platform();
    let install = tempfile::tempdir().expect("install dir");
    write_install_tree(install.path(), platform, OLD_LAUNCHER, OLD_RENDERER);
    let launcher = install.path().join(launcher_relative(platform));
    let renderer = install.path().join(renderer_relative(platform));

    let outcome = super::startup::run_with_fetched_candidate(
        &metadata(),
        Err(UpdateError::SelfUpdate("connection refused".to_owned())),
        |_| panic!("a failed update check must not prompt"),
        |_| {},
    );
    assert_eq!(outcome, StartupUpdateOutcome::Continue);

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("addr");
    drop(listener);
    let candidate = UpdateCandidate {
        tag: UPDATE_TAG.to_owned(),
        commit: UPDATE_COMMIT.to_owned(),
        changelog: String::new(),
        asset: ReleaseAsset::new(
            asset_name_for(platform, UPDATE_TAG),
            format!("http://{address}/asset.zip"),
        ),
    };
    let error = super::install::install_candidate_in(install.path(), &metadata(), &candidate)
        .expect_err("download must fail");
    assert!(
        matches!(error, UpdateError::SelfUpdate(_)),
        "unexpected error: {error}"
    );
    assert_eq!(read_file(&launcher), OLD_LAUNCHER);
    assert_eq!(read_file(&renderer), OLD_RENDERER);
}

#[test]
fn corrupt_or_mismatched_archive_leaves_the_old_binary_in_place() {
    let platform = platform();
    let install = tempfile::tempdir().expect("install dir");
    write_install_tree(install.path(), platform, OLD_LAUNCHER, OLD_RENDERER);
    let launcher = install.path().join(launcher_relative(platform));
    let renderer = install.path().join(renderer_relative(platform));

    let garbage = MockServer::serving(vec![vec![served_update(
        platform,
        b"this is not a zip archive".to_vec(),
    )]]);
    let candidate = super::github::fetch_latest_candidate_from(&garbage.base, &metadata())
        .expect("list")
        .expect("newer nightly");
    let error = super::install::install_candidate_in(install.path(), &metadata(), &candidate)
        .expect_err("corrupt archive");
    assert!(
        matches!(error, UpdateError::InvalidArchive(_)),
        "unexpected error: {error}"
    );
    assert_eq!(read_file(&launcher), OLD_LAUNCHER);
    assert_eq!(read_file(&renderer), OLD_RENDERER);

    let files = payload_files(platform, NEW_LAUNCHER, NEW_RENDERER);
    let manifest = manifest_text(platform, &files);
    let signature = super::bundle::signed_manifest_for_test(&manifest);
    let mut tampered = files;
    let renderer_entry = tampered
        .iter_mut()
        .find(|(name, _)| name.contains("renderer"))
        .expect("renderer");
    renderer_entry.1 = b"tampered-renderer".to_vec();
    let mismatched = MockServer::serving(vec![vec![served_update(
        platform,
        zip_archive(&tampered, &manifest, &signature),
    )]]);
    let candidate = super::github::fetch_latest_candidate_from(&mismatched.base, &metadata())
        .expect("list")
        .expect("newer nightly");
    let error = super::install::install_candidate_in(install.path(), &metadata(), &candidate)
        .expect_err("checksum mismatch");
    assert!(
        matches!(error, UpdateError::InvalidBundle(_)),
        "unexpected error: {error}"
    );
    assert_eq!(read_file(&launcher), OLD_LAUNCHER);
    assert_eq!(read_file(&renderer), OLD_RENDERER);
}

#[test]
fn skip_once_does_not_install() {
    let platform = platform();
    let candidate = UpdateCandidate {
        tag: UPDATE_TAG.to_owned(),
        commit: UPDATE_COMMIT.to_owned(),
        changelog: "notes".to_owned(),
        asset: ReleaseAsset::new(
            asset_name_for(platform, UPDATE_TAG),
            "http://127.0.0.1/unused",
        ),
    };
    let mut prompted = false;
    let outcome = super::startup::run_with_fetched_candidate(
        &metadata(),
        Ok(Some(candidate)),
        |_| {
            prompted = true;
            UpdatePromptChoice::SkipOnce
        },
        |_| panic!("skip once must not notify an install result"),
    );
    assert!(prompted);
    assert_eq!(outcome, StartupUpdateOutcome::Continue);
}

#[test]
fn running_exe_is_replaced_when_a_newer_release_is_installed() {
    if std::env::var_os("RENDERIDE_UPDATE_CHILD").is_some() {
        run_replacement_child();
        return;
    }

    let platform = platform();
    let archive = signed_archive(platform, NEW_LAUNCHER, NEW_RENDERER);
    let server = MockServer::serving(vec![vec![served_update(platform, archive)]]);
    let install = tempfile::tempdir().expect("install dir");
    write_install_tree(install.path(), platform, OLD_LAUNCHER, OLD_RENDERER);
    let launcher = install.path().join(launcher_relative(platform));
    let renderer = install.path().join(renderer_relative(platform));
    let current_exe = std::env::current_exe().expect("current test executable");
    fs::copy(&current_exe, &launcher).expect("copy test executable into the install");
    make_executable(&launcher);

    let output = Command::new(&launcher)
        .arg(REPLACE_TEST)
        .arg("--exact")
        .env("RENDERIDE_UPDATE_CHILD", "1")
        .env("RENDERIDE_UPDATE_API_BASE", &server.base)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn installed launcher");
    assert!(
        output.status.success(),
        "replacement child exited {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(read_file(&launcher), NEW_LAUNCHER);
    assert_eq!(read_file(&renderer), NEW_RENDERER);
    assert_ne!(
        read_file(&launcher),
        fs::read(&current_exe).expect("parent exe")
    );
    assert!(
        backup_contains(
            &install.path().join(".renderide-update").join("backups"),
            OLD_RENDERER
        ),
        "rollback backup should keep the previous renderer"
    );
    assert_eq!(server.download_requests(), 1);
}

fn run_replacement_child() {
    let base = std::env::var("RENDERIDE_UPDATE_API_BASE").expect("mock API base");
    let metadata = metadata();
    let outcome = super::startup::run_with_fetched_candidate(
        &metadata,
        super::github::fetch_latest_candidate_from(&base, &metadata),
        |_| UpdatePromptChoice::Update,
        |_| {},
    );
    assert_eq!(
        outcome,
        StartupUpdateOutcome::Exit,
        "a successful install exits so the new launcher can be started"
    );
}

fn backup_contains(backups: &Path, needle: &[u8]) -> bool {
    let Ok(entries) = fs::read_dir(backups) else {
        return false;
    };
    for entry in entries.flatten() {
        if file_tree_contains(&entry.path(), needle) {
            return true;
        }
    }
    false
}

fn file_tree_contains(path: &Path, needle: &[u8]) -> bool {
    if path.is_file() {
        return fs::read(path).is_ok_and(|bytes| bytes == needle);
    }
    let Ok(entries) = fs::read_dir(path) else {
        return false;
    };
    entries
        .flatten()
        .any(|entry| file_tree_contains(&entry.path(), needle))
}
