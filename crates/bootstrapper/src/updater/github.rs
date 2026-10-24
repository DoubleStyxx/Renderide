//! GitHub release selection for update candidates.
//!
//! Listing does not go through `self_update::backends::github::ReleaseList`. That 1.x API parses
//! each tag as semver and skips the rest, and [`self_update::ReleaseBuilder`] rejects a non-semver
//! version. Nightly tags (`nightly-*`) are not semver. [`ListedRelease`] keeps the 0.44 shape:
//! GitHub list order, a leading `v` stripped from the tag, and the asset `url`.

use self_update::update::ReleaseAsset;

use super::github_api::list_releases;
use super::release_metadata::is_full_sha;
use super::{
    NIGHTLY_PREFIX, REPO_NAME, REPO_OWNER, ReleaseBuildMetadata, UpdateCandidate, UpdateError,
};

/// One GitHub release in the shape `self_update` 0.44 exposed as `Release`.
///
/// `tag` is `tag_name` with leading `v` characters removed, matching 0.44's
/// `trim_start_matches('v')`. A `nightly-*` tag has no such prefix, so it is unchanged.
#[derive(Clone, Debug)]
pub(super) struct ListedRelease {
    /// Tag used for comparison and for the asset file name.
    pub(super) tag: String,
    /// Release body, when GitHub sent one.
    pub(super) body: Option<String>,
    /// Assets in API order. Download URLs are the asset `url` field.
    pub(super) assets: Vec<ReleaseAsset>,
}

/// Fetches GitHub releases and selects the newest eligible update candidate.
pub(super) fn fetch_latest_candidate(
    metadata: &ReleaseBuildMetadata,
) -> Result<Option<UpdateCandidate>, UpdateError> {
    fetch_latest_candidate_from(super::github_api::GITHUB_API_BASE, metadata)
}

/// Same selection as [`fetch_latest_candidate`], against an API base URL.
///
/// Tests point this at a local mock. Production uses `https://api.github.com`.
pub(super) fn fetch_latest_candidate_from(
    api_base: &str,
    metadata: &ReleaseBuildMetadata,
) -> Result<Option<UpdateCandidate>, UpdateError> {
    let releases = list_releases(api_base, REPO_OWNER, REPO_NAME)?;
    Ok(select_update_candidate(&releases, metadata))
}

/// Selects the newest eligible release asset when it is newer than the running build.
///
/// "Newest" is GitHub's list order (newest first), not a semver comparison. The first `nightly-*`
/// release that has this platform's asset and a commit line wins. The same tag or the same commit
/// is not an update.
pub(super) fn select_update_candidate(
    releases: &[ListedRelease],
    metadata: &ReleaseBuildMetadata,
) -> Option<UpdateCandidate> {
    let candidate = releases
        .iter()
        .filter(|release| release.tag.starts_with(NIGHTLY_PREFIX))
        .find_map(|release| candidate_from_release(release, metadata))?;
    if candidate.tag == metadata.tag || candidate.commit == metadata.commit {
        None
    } else {
        Some(candidate)
    }
}

/// Converts a GitHub release into an update candidate when it has this platform's asset.
fn candidate_from_release(
    release: &ListedRelease,
    metadata: &ReleaseBuildMetadata,
) -> Option<UpdateCandidate> {
    let body = release.body.as_deref();
    let commit = release_commit(body)?;
    let changelog = release_changelog(body).unwrap_or_else(no_changelog_available);
    let asset_name = asset_name_for(&metadata.platform, &release.tag);
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name() == asset_name)
        .cloned()?;
    Some(UpdateCandidate {
        tag: release.tag.clone(),
        commit,
        changelog,
        asset,
    })
}

/// Builds the exact GitHub release asset name for a platform and tag.
pub(super) fn asset_name_for(platform: &str, tag: &str) -> String {
    format!("renderide-{platform}-{tag}.zip")
}

/// Parses the commit SHA recorded in a GitHub release body.
pub(super) fn release_commit(body: Option<&str>) -> Option<String> {
    body.and_then(|body| {
        body.lines().find_map(|line| {
            let commit = line.strip_prefix("Commit: ")?;
            is_full_sha(commit).then(|| commit.to_owned())
        })
    })
}

/// Parses the Markdown changelog section recorded in a GitHub release body.
fn release_changelog(body: Option<&str>) -> Option<String> {
    let body = body?;
    let mut in_changelog = false;
    let mut lines = Vec::new();
    for line in body.lines() {
        if line.trim() == "## Changelog" {
            in_changelog = true;
            continue;
        }
        if !in_changelog {
            continue;
        }
        if line.trim_start().starts_with("## ") {
            break;
        }
        lines.push(line);
    }
    let changelog = lines.join("\n").trim().to_owned();
    (!changelog.is_empty()).then_some(changelog)
}

/// Returns the fallback changelog text used for releases created before changelog generation.
fn no_changelog_available() -> String {
    "No changelog is available for this release.".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::RELEASE_CHANNEL;

    const RELEASE_PLATFORMS: [&str; 6] = [
        "linux-x86_64",
        "linux-aarch64",
        "windows-x86_64",
        "windows-aarch64",
        "macos-x86_64",
        "macos-aarch64",
    ];

    fn metadata_for(platform: &str) -> ReleaseBuildMetadata {
        ReleaseBuildMetadata {
            channel: RELEASE_CHANNEL.to_owned(),
            tag: "nightly-2026-05-26-1111111".to_owned(),
            commit: "1111111111111111111111111111111111111111".to_owned(),
            platform: platform.to_owned(),
        }
    }

    fn metadata() -> ReleaseBuildMetadata {
        metadata_for("linux-x86_64")
    }

    fn release(tag: &str, commit: &str, asset_name: &str) -> ListedRelease {
        ListedRelease {
            tag: tag.to_owned(),
            body: Some(format!(
                "Commit: {commit}\n\n## Changelog\n\n- [{short}](https://github.com/DoubleStyx/Renderide/commit/{commit}) Add updater changelog by Renderer Developer\n",
                short = &commit[..8]
            )),
            assets: vec![ReleaseAsset::new(
                asset_name,
                "https://api.github.com/repos/DoubleStyx/Renderide/releases/assets/1",
            )],
        }
    }

    #[test]
    fn release_commit_parses_first_line_shape() {
        let commit = "2222222222222222222222222222222222222222";
        assert_eq!(
            release_commit(Some(&format!("Commit: {commit}\n\nbody"))),
            Some(commit.to_owned())
        );
        assert_eq!(release_commit(Some("Commit: bad")), None);
        assert_eq!(release_commit(None), None);
    }

    #[test]
    fn release_changelog_parses_markdown_section() {
        let body = "\
Commit: 2222222222222222222222222222222222222222

Nightly build.

## Changelog

Changes since previous release.

- [22222222](https://example.invalid/commit/22222222) Add feature by Developer

## Assets

Bundle details.
";

        assert_eq!(
            release_changelog(Some(body)),
            Some(
                "Changes since previous release.\n\n- [22222222](https://example.invalid/commit/22222222) Add feature by Developer"
                    .to_owned()
            )
        );
        assert_eq!(release_changelog(Some("Commit: abc\n")), None);
        assert_eq!(release_changelog(Some("## Changelog\n\n")), None);
        assert_eq!(release_changelog(None), None);
    }

    #[test]
    fn release_changelog_preserves_empty_range_explanation() {
        let body = "\
Commit: 2222222222222222222222222222222222222222

## Changelog

No previous release commit was available, so no commit changelog was generated for this release.
";

        assert_eq!(
            release_changelog(Some(body)),
            Some(
                "No previous release commit was available, so no commit changelog was generated for this release."
                    .to_owned()
            )
        );
    }

    #[test]
    fn candidate_selection_uses_newest_nightly_tag_and_exact_platform_asset() {
        let metadata = metadata();
        let commit = "2222222222222222222222222222222222222222";
        let releases = vec![
            release(
                "nightly-2026-05-27-2222222",
                commit,
                "renderide-linux-x86_64-nightly-2026-05-27-2222222.zip",
            ),
            release("v1.0.0", commit, "renderide-linux-x86_64-v1.0.0.zip"),
        ];

        let candidate = select_update_candidate(&releases, &metadata);

        match candidate {
            Some(candidate) => {
                assert_eq!(candidate.tag, "nightly-2026-05-27-2222222");
                assert_eq!(candidate.commit, commit);
                assert!(candidate.changelog.contains("Add updater changelog"));
            }
            None => panic!("expected update candidate"),
        }
    }

    #[test]
    fn candidate_selection_keeps_github_list_order_not_semver() {
        let metadata = metadata();
        let first_commit = "2222222222222222222222222222222222222222";
        let later_commit = "3333333333333333333333333333333333333333";
        let releases = vec![
            release(
                "nightly-2026-05-27-2222222",
                first_commit,
                &asset_name_for(&metadata.platform, "nightly-2026-05-27-2222222"),
            ),
            release(
                "nightly-2026-05-28-3333333",
                later_commit,
                &asset_name_for(&metadata.platform, "nightly-2026-05-28-3333333"),
            ),
        ];

        let candidate = select_update_candidate(&releases, &metadata).expect("candidate");

        assert_eq!(candidate.tag, "nightly-2026-05-27-2222222");
        assert_eq!(candidate.commit, first_commit);
    }

    #[test]
    fn candidate_selection_uses_exact_asset_name_on_every_platform() {
        let commit = "2222222222222222222222222222222222222222";
        let tag = "nightly-2026-05-27-2222222";
        let assets: Vec<ReleaseAsset> = RELEASE_PLATFORMS
            .iter()
            .map(|platform| {
                ReleaseAsset::new(
                    asset_name_for(platform, tag),
                    format!("https://example.invalid/{platform}"),
                )
            })
            .chain(std::iter::once(ReleaseAsset::new(
                "renderide-notes.txt",
                "https://example.invalid/notes",
            )))
            .collect();
        let release = ListedRelease {
            tag: tag.to_owned(),
            body: Some(format!("Commit: {commit}\n")),
            assets,
        };

        for platform in RELEASE_PLATFORMS {
            let metadata = metadata_for(platform);
            let candidate = select_update_candidate(std::slice::from_ref(&release), &metadata)
                .unwrap_or_else(|| panic!("expected asset for {platform}"));
            assert_eq!(candidate.asset.name(), asset_name_for(platform, tag));
            assert_eq!(
                candidate.asset.download_url(),
                format!("https://example.invalid/{platform}")
            );
        }
    }

    #[test]
    fn candidate_selection_skips_a_nightly_that_lacks_this_platforms_asset() {
        let metadata = metadata();
        let commit = "2222222222222222222222222222222222222222";
        let releases = vec![release(
            "nightly-2026-05-27-2222222",
            commit,
            "renderide-windows-x86_64-nightly-2026-05-27-2222222.zip",
        )];

        assert!(select_update_candidate(&releases, &metadata).is_none());
    }

    #[test]
    fn candidate_selection_uses_fallback_when_changelog_is_missing() {
        let metadata = metadata();
        let commit = "2222222222222222222222222222222222222222";
        let tag = "nightly-2026-05-27-2222222";
        let releases = vec![ListedRelease {
            tag: tag.to_owned(),
            body: Some(format!("Commit: {commit}\n\nNightly build.")),
            assets: vec![ReleaseAsset::new(
                asset_name_for(&metadata.platform, tag),
                "https://api.github.com/repos/DoubleStyx/Renderide/releases/assets/1",
            )],
        }];

        let candidate = select_update_candidate(&releases, &metadata).expect("candidate");

        assert_eq!(candidate.changelog, no_changelog_available());
    }

    #[test]
    fn candidate_selection_does_not_offer_previous_release_when_current_is_latest() {
        let metadata = metadata();
        let releases = vec![
            release(
                &metadata.tag,
                &metadata.commit,
                &asset_name_for(&metadata.platform, &metadata.tag),
            ),
            release(
                "nightly-2026-05-25-0000000",
                "0000000000000000000000000000000000000000",
                &asset_name_for(&metadata.platform, "nightly-2026-05-25-0000000"),
            ),
        ];

        assert!(select_update_candidate(&releases, &metadata).is_none());
    }

    #[test]
    fn candidate_selection_ignores_duplicate_tag_for_current_commit() {
        let metadata = metadata();
        let duplicate_tag = "nightly-2026-05-27-1111111";
        let releases = vec![
            release(
                duplicate_tag,
                &metadata.commit,
                &asset_name_for(&metadata.platform, duplicate_tag),
            ),
            release(
                "nightly-2026-05-25-0000000",
                "0000000000000000000000000000000000000000",
                &asset_name_for(&metadata.platform, "nightly-2026-05-25-0000000"),
            ),
        ];

        assert!(select_update_candidate(&releases, &metadata).is_none());
    }
}
