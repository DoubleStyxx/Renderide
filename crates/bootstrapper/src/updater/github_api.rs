//! GitHub releases listing that keeps non-semver nightly tags.
//!
//! `self_update` 1.x drops a tag `ReleaseBuilder` cannot parse as semver. This walks the same
//! endpoint 0.44's `ReleaseList::fetch` used, with the same asset `url` field and the same leading
//! `v` trim, and it does not drop `nightly-*` tags.
//!
//! Pagination follows `Link: rel="next"` as the header wrote it. 0.44 appended `?per_page=100` to
//! that URL even when it already had a query string, which fails the whole check once a second
//! page exists. 1.x does not. The first request still asks for `per_page=100`.

use self_update::reqwest::blocking::Client;
use self_update::reqwest::header::{HeaderMap, LINK, USER_AGENT};
use serde_json::Value;

use super::UpdateError;
use super::github::ListedRelease;
use self_update::update::ReleaseAsset;

/// GitHub API origin used for production update checks.
pub(super) const GITHUB_API_BASE: &str = "https://api.github.com";

/// User-Agent 0.44 sent on release-listing requests.
const LISTING_USER_AGENT: &str = "rust/self-update";

/// Stops a `Link` cycle from walking forever. The repository's release list fits in one page.
const MAX_LISTING_PAGES: usize = 100;

/// Fetches every release page for `owner`/`repo` under `api_base`.
pub(super) fn list_releases(
    api_base: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<ListedRelease>, UpdateError> {
    let client = Client::builder()
        .build()
        .map_err(|error| UpdateError::SelfUpdate(format!("build GitHub API client: {error}")))?;
    let mut url = format!(
        "{}/repos/{owner}/{repo}/releases?per_page=100",
        api_base.trim_end_matches('/')
    );
    let mut releases = Vec::new();
    for _page in 0..MAX_LISTING_PAGES {
        let response = client
            .get(&url)
            .header(USER_AGENT, LISTING_USER_AGENT)
            .send()
            .map_err(|error| UpdateError::SelfUpdate(format!("api request failed: {error}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(UpdateError::SelfUpdate(format!(
                "api request failed with status: {status} - for: {url}"
            )));
        }
        let headers = response.headers().clone();
        let body: Value = response.json().map_err(|error| {
            UpdateError::SelfUpdate(format!("release listing was not JSON: {error}"))
        })?;
        let page = body.as_array().ok_or_else(|| {
            UpdateError::SelfUpdate("release listing was not a JSON array".to_owned())
        })?;
        for release in page {
            releases.push(parse_release(release)?);
        }
        match next_link(&headers) {
            Some(next) => url = next,
            None => return Ok(releases),
        }
    }
    Err(UpdateError::SelfUpdate(format!(
        "stopped after {MAX_LISTING_PAGES} release pages"
    )))
}

fn parse_release(release: &Value) -> Result<ListedRelease, UpdateError> {
    let tag = json_str(release, "tag_name")?;
    let _created_at = json_str(release, "created_at")?;
    let assets = release
        .get("assets")
        .and_then(Value::as_array)
        .ok_or_else(|| UpdateError::SelfUpdate("Release missing `assets`".to_owned()))?;
    let assets = assets
        .iter()
        .map(parse_asset)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ListedRelease {
        tag: tag.trim_start_matches('v').to_owned(),
        body: release
            .get("body")
            .and_then(Value::as_str)
            .map(str::to_owned),
        assets,
    })
}

fn parse_asset(asset: &Value) -> Result<ReleaseAsset, UpdateError> {
    let download_url = json_str(asset, "url")?;
    let name = json_str(asset, "name")?;
    Ok(ReleaseAsset::new(name, download_url))
}

fn json_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, UpdateError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| UpdateError::SelfUpdate(format!("Release missing `{key}`")))
}

/// First `rel="next"` URI in a GitHub `Link` header, if any.
fn next_link(headers: &HeaderMap) -> Option<String> {
    headers.get_all(LINK).iter().find_map(|value| {
        let text = value.to_str().ok()?;
        find_rel_next_link(text).map(str::to_owned)
    })
}

/// Same link-header scan 0.44 used (`<url>; rel="next"`).
fn find_rel_next_link(link_str: &str) -> Option<&str> {
    for link in link_str.split(',') {
        let mut uri = None;
        let mut is_rel_next = false;
        for part in link.split(';') {
            let part = part.trim();
            if part.starts_with('<') && part.ends_with('>') {
                uri = Some(part.trim_start_matches('<').trim_end_matches('>'));
            } else if part.starts_with("rel=") {
                let part = part
                    .trim_start_matches("rel=")
                    .trim_end_matches('"')
                    .trim_start_matches('"');
                if part == "next" {
                    is_rel_next = true;
                }
            }
            if is_rel_next && uri.is_some() {
                return uri;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updater::mock_release::MockServer;

    #[test]
    fn link_header_parser_returns_the_first_next_uri() {
        let header = r#" <https://api.github.com/resource?page=2>; rel="next" "#;
        assert_eq!(
            find_rel_next_link(header),
            Some("https://api.github.com/resource?page=2")
        );
        assert_eq!(
            find_rel_next_link(r#"<https://place.com>; rel="next", <https://wow.com>; rel="next""#),
            Some("https://place.com")
        );
        assert_eq!(
            find_rel_next_link(r#"<https://good.com>; rel="preconnect""#),
            None
        );
    }

    #[test]
    fn listing_keeps_nightly_tags_trims_v_and_uses_asset_url() {
        let server = MockServer::from_tags(&["nightly-2026-05-27-2222222", "v1.2.3", "latest"]);
        let releases = list_releases(&server.base, "DoubleStyx", "Renderide").expect("list");

        assert_eq!(
            releases
                .iter()
                .map(|release| release.tag.as_str())
                .collect::<Vec<_>>(),
            vec!["nightly-2026-05-27-2222222", "1.2.3", "latest"]
        );
        assert_eq!(
            releases[0].assets[0].download_url(),
            server.asset_url("renderide-linux-x86_64-nightly-2026-05-27-2222222.zip")
        );
        assert_eq!(server.listing_requests(), 1);
    }

    #[test]
    fn listing_follows_next_link_without_appending_another_query() {
        let server = MockServer::paginated(vec![
            vec!["nightly-2026-05-27-2222222"],
            vec!["nightly-2026-05-26-1111111"],
        ]);
        let releases = list_releases(&server.base, "DoubleStyx", "Renderide").expect("list");

        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].tag, "nightly-2026-05-27-2222222");
        assert_eq!(releases[1].tag, "nightly-2026-05-26-1111111");
        assert_eq!(server.listing_requests(), 2);
        let targets = server.request_targets();
        assert!(
            targets[0].contains("per_page=100"),
            "first page keeps per_page=100: {targets:?}"
        );
        assert!(
            targets[1].ends_with("/releases?page=2"),
            "next page follows the Link URL unchanged: {targets:?}"
        );
    }

    #[test]
    fn listing_http_failure_is_an_error() {
        let server = MockServer::listing_status(500);
        let error = list_releases(&server.base, "DoubleStyx", "Renderide").expect_err("status");
        let UpdateError::SelfUpdate(message) = error else {
            panic!("expected self-update error");
        };
        assert!(message.contains("500"), "{message}");
    }

    #[test]
    fn listing_connection_refused_is_an_error() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("addr");
        drop(listener);
        let error = list_releases(&format!("http://{address}"), "DoubleStyx", "Renderide")
            .expect_err("refused");
        assert!(matches!(error, UpdateError::SelfUpdate(_)));
    }
}
