//! Loopback GitHub-style release server for updater tests.
//!
//! Speaks the listing shape `github_api` parses: a JSON array of releases, asset `url`s on this
//! server, and `Link: rel="next"` when another page remains.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// One downloadable asset served at `/assets/{name}`.
pub(super) struct ServedAsset {
    /// File name, matched exactly by the updater.
    pub(super) name: String,
    /// Response body.
    pub(super) bytes: Vec<u8>,
}

/// One release object in a listing page.
pub(super) struct ServedRelease {
    /// Raw `tag_name`, before the leading-`v` trim.
    pub(super) tag: String,
    /// Release body. Selection reads `Commit: ` and `## Changelog` from this.
    pub(super) body: String,
    /// Assets whose `url` points back at this server.
    pub(super) assets: Vec<ServedAsset>,
}

/// Local stand-in for `https://api.github.com`.
pub(super) struct MockServer {
    /// `http://127.0.0.1:<port>`, with no trailing slash.
    pub(super) base: String,
    listing_requests: Arc<AtomicUsize>,
    download_requests: Arc<AtomicUsize>,
    targets: Arc<Mutex<Vec<String>>>,
    raw_requests: Arc<Mutex<Vec<String>>>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MockServer {
    /// One page of releases, one dummy asset per tag.
    pub(super) fn from_tags(tags: &[&str]) -> Self {
        let releases = tags.iter().copied().map(release_with_named_asset).collect();
        Self::serving(vec![releases])
    }

    /// One tag per inner vec, each inner vec a listing page, in order.
    pub(super) fn paginated(pages: Vec<Vec<&str>>) -> Self {
        let pages = pages
            .into_iter()
            .map(|tags| tags.into_iter().map(release_with_named_asset).collect())
            .collect();
        Self::serving(pages)
    }

    /// Listing requests receive `status` and an empty body.
    pub(super) fn listing_status(status: u16) -> Self {
        Self::start(Vec::new(), ListingMode::Pages { status }, false, false)
    }

    /// Accepts the connection and closes it before any HTTP response.
    pub(super) fn closes_immediately() -> Self {
        Self::start(Vec::new(), ListingMode::Pages { status: 200 }, false, true)
    }

    /// Every listing page sends `Link: rel="next"` back at the same request URL.
    pub(super) fn repeating_next_link() -> Self {
        Self::start(Vec::new(), ListingMode::RepeatNext, false, false)
    }

    /// Every listing page sends `Link: rel="next"` at a page URL that has not been used yet.
    pub(super) fn endless_distinct_pages() -> Self {
        Self::start(Vec::new(), ListingMode::EndlessDistinct, false, false)
    }

    /// Serves `pages` in order. Page 1 is the first entry. Asset paths are `/assets/{name}`.
    pub(super) fn serving(pages: Vec<Vec<ServedRelease>>) -> Self {
        Self::start(pages, ListingMode::Pages { status: 200 }, false, false)
    }

    /// Like [`Self::serving`], but each asset `url` is `/redirect/{name}` and that path
    /// answers `302` to `/assets/{name}` on this same server.
    pub(super) fn serving_with_asset_redirect(pages: Vec<Vec<ServedRelease>>) -> Self {
        Self::start(pages, ListingMode::Pages { status: 200 }, true, false)
    }

    /// Absolute URL for an asset this server hosts.
    pub(super) fn asset_url(&self, name: &str) -> String {
        format!("{}/assets/{name}", self.base)
    }

    /// How many release-listing requests were served.
    pub(super) fn listing_requests(&self) -> usize {
        self.listing_requests.load(Ordering::SeqCst)
    }

    /// How many `/assets/` requests were served.
    pub(super) fn download_requests(&self) -> usize {
        self.download_requests.load(Ordering::SeqCst)
    }

    /// Request targets, in arrival order, including the query string.
    pub(super) fn request_targets(&self) -> Vec<String> {
        self.targets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Values of `header_name` on requests whose target contains `path_fragment`.
    ///
    /// Header names are matched without regard to case. The value is trimmed.
    pub(super) fn header_values(&self, path_fragment: &str, header_name: &str) -> Vec<String> {
        self.raw_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter(|raw| request_target(raw).is_some_and(|target| target.contains(path_fragment)))
            .filter_map(|raw| header_value(raw, header_name))
            .collect()
    }

    fn start(
        pages: Vec<Vec<ServedRelease>>,
        mode: ListingMode,
        redirect_assets: bool,
        close_immediately: bool,
    ) -> Self {
        let mut assets = HashMap::new();
        for page in &pages {
            for release in page {
                for asset in &release.assets {
                    assets.insert(asset.name.clone(), asset.bytes.clone());
                }
            }
        }
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock release server");
        let address = listener.local_addr().expect("mock server address");
        let base = format!("http://{address}");
        let page_bodies = pages
            .iter()
            .map(|page| page_json(&base, page, redirect_assets))
            .collect::<Vec<_>>();
        let listing_requests = Arc::new(AtomicUsize::new(0));
        let download_requests = Arc::new(AtomicUsize::new(0));
        let targets = Arc::new(Mutex::new(Vec::new()));
        let raw_requests = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread = {
            let state = ServeState {
                base: base.clone(),
                close_immediately,
                mode,
                pages: page_bodies,
                assets,
                listing_requests: Arc::clone(&listing_requests),
                download_requests: Arc::clone(&download_requests),
                targets: Arc::clone(&targets),
                raw_requests: Arc::clone(&raw_requests),
                shutdown: Arc::clone(&shutdown),
            };
            thread::spawn(move || serve(listener, &state))
        };
        Self {
            base,
            listing_requests,
            download_requests,
            targets,
            raw_requests,
            shutdown,
            thread: Some(thread),
        }
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Ok(address) = self.base.trim_start_matches("http://").parse() {
            let _ = TcpStream::connect_timeout(&address, Duration::from_secs(1));
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn release_with_named_asset(tag: &str) -> ServedRelease {
    let name = format!("renderide-linux-x86_64-{tag}.zip");
    ServedRelease {
        tag: tag.to_owned(),
        body: format!(
            "Commit: 2222222222222222222222222222222222222222\n\n## Changelog\n\n- {tag}\n"
        ),
        assets: vec![ServedAsset {
            name,
            bytes: b"unused".to_vec(),
        }],
    }
}

fn page_json(base: &str, releases: &[ServedRelease], redirect_assets: bool) -> Vec<u8> {
    let releases = releases
        .iter()
        .map(|release| {
            let assets = release
                .assets
                .iter()
                .map(|asset| {
                    let path = if redirect_assets {
                        format!("/redirect/{}", asset.name)
                    } else {
                        format!("/assets/{}", asset.name)
                    };
                    serde_json::json!({
                        "name": asset.name,
                        "url": format!("{base}{path}"),
                    })
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "tag_name": release.tag,
                "created_at": "2026-05-27T00:00:00Z",
                "name": release.tag,
                "body": release.body,
                "assets": assets,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&releases).expect("encode release page")
}

enum ListingMode {
    Pages { status: u16 },
    RepeatNext,
    EndlessDistinct,
}

struct ServeState {
    base: String,
    mode: ListingMode,
    close_immediately: bool,
    pages: Vec<Vec<u8>>,
    assets: HashMap<String, Vec<u8>>,
    listing_requests: Arc<AtomicUsize>,
    download_requests: Arc<AtomicUsize>,
    targets: Arc<Mutex<Vec<String>>>,
    raw_requests: Arc<Mutex<Vec<String>>>,
    shutdown: Arc<AtomicBool>,
}

fn serve(listener: TcpListener, state: &ServeState) {
    while !state.shutdown.load(Ordering::SeqCst) {
        let Ok((mut stream, _)) = listener.accept() else {
            break;
        };
        if state.shutdown.load(Ordering::SeqCst) {
            break;
        }
        if state.close_immediately {
            drop(stream);
            continue;
        }
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let Some(request) = read_headers(&mut stream) else {
            continue;
        };
        let Some(target) = request_target(&request) else {
            continue;
        };
        state
            .targets
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(target.clone());
        state
            .raw_requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request.clone());
        let (path, query) = target.split_once('?').unwrap_or((target.as_str(), ""));
        if path == "/repos/DoubleStyx/Renderide/releases" {
            state.listing_requests.fetch_add(1, Ordering::SeqCst);
            match &state.mode {
                ListingMode::Pages { status } if *status != 200 => {
                    let _ = write_response(&mut stream, *status, None, None, b"");
                }
                ListingMode::Pages { .. } => {
                    let page_index = query_page(query).saturating_sub(1);
                    let Some(body) = state.pages.get(page_index) else {
                        let _ = write_response(&mut stream, 404, None, None, b"no such page");
                        continue;
                    };
                    let next = page_index
                        .checked_add(1)
                        .filter(|next| state.pages.get(*next).is_some())
                        .map(|next| {
                            format!(
                                "<{}/repos/DoubleStyx/Renderide/releases?page={}>; rel=\"next\"",
                                state.base,
                                next + 1
                            )
                        });
                    let _ = write_response(&mut stream, 200, next.as_deref(), None, body);
                }
                ListingMode::RepeatNext => {
                    let link = format!("<{base}{target}>; rel=\"next\"", base = state.base);
                    let _ = write_response(
                        &mut stream,
                        200,
                        Some(&link),
                        None,
                        &minimal_release("nightly-repeat"),
                    );
                }
                ListingMode::EndlessDistinct => {
                    let page_number = state.listing_requests.load(Ordering::SeqCst);
                    let link = format!(
                        "<{base}/repos/DoubleStyx/Renderide/releases?page={next}>; rel=\"next\"",
                        base = state.base,
                        next = page_number + 1
                    );
                    let _ = write_response(
                        &mut stream,
                        200,
                        Some(&link),
                        None,
                        &minimal_release(&format!("nightly-page-{page_number}")),
                    );
                }
            }
            continue;
        }
        if let Some(name) = path.strip_prefix("/redirect/") {
            let location = format!("{}/assets/{name}", state.base);
            let _ = write_response(&mut stream, 302, None, Some(&location), b"");
            continue;
        }
        if let Some(name) = path.strip_prefix("/assets/") {
            state.download_requests.fetch_add(1, Ordering::SeqCst);
            match state.assets.get(name) {
                Some(bytes) => {
                    let _ = write_response(&mut stream, 200, None, None, bytes);
                }
                None => {
                    let _ = write_response(&mut stream, 404, None, None, b"missing asset");
                }
            }
        } else {
            let _ = write_response(&mut stream, 404, None, None, b"not found");
        }
    }
}

fn query_page(query: &str) -> usize {
    query
        .split('&')
        .find_map(|part| part.strip_prefix("page=")?.parse().ok())
        .unwrap_or(1)
}

fn header_value(request: &str, name: &str) -> Option<String> {
    request.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_owned())
    })
}

fn request_target(request: &str) -> Option<String> {
    let line = request.lines().next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    if method != "GET" {
        return None;
    }
    parts.next().map(str::to_owned)
}

fn read_headers(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut tmp = [0_u8; 1024];
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&tmp[..n]);
                if buf.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
                if buf.len() > 65_536 {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
    String::from_utf8(buf).ok()
}

fn minimal_release(tag: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!([{
        "tag_name": tag,
        "created_at": "2026-05-27T00:00:00Z",
        "assets": [],
    }]))
    .expect("encode release")
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    link: Option<&str>,
    location: Option<&str>,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        302 => "Found",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Error",
    };
    let mut head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(link) = link {
        head.push_str("Link: ");
        head.push_str(link);
        head.push_str("\r\n");
    }
    if let Some(location) = location {
        head.push_str("Location: ");
        head.push_str(location);
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}
