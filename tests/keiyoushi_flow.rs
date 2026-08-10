//! Offline unit tests for each `keiyoushi::Keiyoushi` function, replaying the
//! responses captured by the live test (tests/live_keiyoushi.rs).
//!
//! Run `DEXVM_LIVE=1 cargo test --test live_keiyoushi -- --nocapture` once
//! from a network that can reach api.akuma.moe to generate fixtures/live/;
//! the tests below then run deterministically against that real data. Until
//! then (or when the capture was marked BLOCKED by a WAF) they skip.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;

use dexvm::keiyoushi::{FilterState, HttpData, HttpResp, Keiyoushi};

const APK: &str = "fixtures/tachiyomi-all.akuma-v1.4.10.apk";
const MANIFEST: &str = "fixtures/live/manifest.txt";
const BLOCKED: &str = "fixtures/live/BLOCKED";

/// APK under test; override with `DEXVM_APK` (e.g. to run the same flow
/// suite against the comicfury or mangapill fixture apks).
fn apk_path() -> String {
    std::env::var("DEXVM_APK").unwrap_or_else(|_| APK.to_string())
}

fn env_overridden() -> bool {
    std::env::var("DEXVM_APK").is_ok()
}

#[derive(Clone)]
struct Capture {
    code: i32,
    body: String,
    bytes: Option<Vec<u8>>,
}

fn captures() -> &'static Option<HashMap<String, Capture>> {
    static C: OnceLock<Option<HashMap<String, Capture>>> = OnceLock::new();
    C.get_or_init(|| {
        let Ok(text) = std::fs::read_to_string(MANIFEST) else {
            return None;
        };
        let mut map = HashMap::new();
        for line in text.lines() {
            let mut it = line.split('\t');
            let (Some(code), Some(file), Some(_method), Some(url)) =
                (it.next(), it.next(), it.next(), it.next())
            else {
                continue;
            };
            let Ok(raw) = std::fs::read(format!("fixtures/live/{file}")) else {
                continue;
            };
            let Ok(code) = code.parse() else { continue };
            map.insert(
                url.to_string(),
                Capture {
                    code,
                    body: String::from_utf8_lossy(&raw).into_owned(),
                    bytes: Some(raw),
                },
            );
        }
        Some(map)
    })
}

fn fixture_state() -> FixtureState {
    let map = captures();
    match map {
        None => FixtureState::Missing,
        Some(m) if m.is_empty() => FixtureState::Missing,
        Some(_) if std::path::Path::new(BLOCKED).exists() => FixtureState::Blocked,
        Some(_) => FixtureState::Ready,
    }
}

enum FixtureState {
    /// fixtures/live/ was never captured — nothing to replay.
    Missing,
    /// captured, but the live site served a WAF challenge — expect empty
    /// results, so only the code path (not the data) is asserted.
    Blocked,
    /// captured real content — assert real data.
    Ready,
}

fn skip_msg(state: &FixtureState) -> &'static str {
    match state {
        FixtureState::Missing => {
            "skipped: run `DEXVM_LIVE=1 cargo test --test live_keiyoushi -- --nocapture` \
             to capture fixtures/live/ first"
        }
        FixtureState::Blocked => {
            "skipped: the captured live run was WAF-blocked (fixtures/live/BLOCKED); \
             re-capture from an allowed network"
        }
        FixtureState::Ready => unreachable!(),
    }
}

/// Opens the extension with an HTTP handler replaying the captured responses;
/// unknown URLs get an empty page.
fn replay_ext() -> Option<(Keiyoushi, FixtureState)> {
    let state = fixture_state();
    match state {
        FixtureState::Ready | FixtureState::Blocked => {}
        FixtureState::Missing => return None,
    }
    let map = captures().as_ref()?.clone();
    let mut ext = Keiyoushi::open(&apk_path()).ok()?;
    ext.set_http_rc(Rc::new(move |req: &HttpData| match map.get(&req.url) {
        Some(c) => HttpResp {
            code: c.code,
            message: "OK".into(),
            headers: Vec::new(),
            body: c.body.clone(),
            body_bytes: c.bytes.clone(),
        },
        None => HttpResp::ok("<html></html>"),
    }));
    Some((ext, state))
}

#[test]
fn flow_sources_and_metadata() {
    let Some((mut ext, _state)) = replay_ext() else {
        eprintln!("{}", skip_msg(&FixtureState::Missing));
        return;
    };
    let srcs = ext.sources().unwrap();
    assert!(!srcs.is_empty());
    let src = &srcs[0];
    let name = ext.source_name(src).unwrap();
    assert!(!name.is_empty());
    if !env_overridden() {
        // default fixture apk: the name must come from the dex source class,
        // not from a host-side constant
        assert_eq!(name, "Akuma");
    }
    let lang = ext.source_lang(src).unwrap();
    assert!(!lang.is_empty());
    let _ = ext.supports_latest(src).unwrap();
}

#[test]
fn flow_multiapk_metadata() {
    // Every fixture apk must give per-source metadata straight from its own
    // dex — no host-side assumptions about any single source. This is the
    // regression guard for the old hardcoded "Akuma"/"all"/baseUrl stubs.
    let mut apks: Vec<String> = std::fs::read_dir("fixtures")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let p = e.unwrap().path();
            let f = p.file_name()?.to_string_lossy().into_owned();
            (f.starts_with("tachiyomi-")
                && f.ends_with(".apk")
                && p.to_string_lossy() != apk_path())
            .then(|| p.to_string_lossy().into_owned())
        })
        .collect();
    apks.sort();
    assert!(
        !apks.is_empty(),
        "expected fixture apks beyond the default one"
    );
    for apk in apks {
        let mut ext = Keiyoushi::open(&apk).unwrap_or_else(|e| panic!("open {apk}: {e}"));
        let srcs = ext
            .sources()
            .unwrap_or_else(|e| panic!("sources {apk}: {e}"));
        assert!(!srcs.is_empty(), "{apk}: no sources");
        for src in &srcs {
            let name = ext
                .source_name(src)
                .unwrap_or_else(|e| panic!("{apk}: {e}"));
            assert!(
                !name.is_empty() && name.len() < 64,
                "{apk}: bad name {name:?}"
            );
            let lang = ext.source_lang(src).unwrap();
            assert!(
                !lang.is_empty() && lang.len() <= 6,
                "{apk}: bad lang {lang:?}"
            );
            let _ = ext.supports_latest(src).unwrap();
        }
        let fl = ext
            .filters(&srcs[0])
            .unwrap_or_else(|e| panic!("filters {apk}: {e}"));
        assert!(!fl.is_empty(), "{apk}: no filters");
    }
}

#[test]
fn flow_filters() {
    let Some((mut ext, _state)) = replay_ext() else {
        eprintln!("{}", skip_msg(&FixtureState::Missing));
        return;
    };
    let src = &ext.sources().unwrap()[0];
    let fl = ext.filters(src).unwrap();
    assert!(!fl.is_empty());
    let kinds: Vec<_> = fl.iter().map(|f| f.kind).collect();
    if !env_overridden() {
        // akuma-specific filter shapes
        assert!(kinds.contains(&dexvm::keiyoushi::FilterKind::Group));
        assert!(kinds.contains(&dexvm::keiyoushi::FilterKind::TriState));
    }
    // every Select filter has non-empty options
    for f in fl
        .iter()
        .filter(|f| f.kind == dexvm::keiyoushi::FilterKind::Select)
    {
        assert!(
            !f.options.is_empty(),
            "select filter {:?} has no options",
            f.name
        );
    }
}

#[test]
fn flow_popular() {
    let Some((mut ext, state)) = replay_ext() else {
        eprintln!("{}", skip_msg(&FixtureState::Missing));
        return;
    };
    let src = &ext.sources().unwrap()[0];
    let pages = ext.popular(src, 1).unwrap();
    match state {
        FixtureState::Ready => {
            assert!(
                !pages.mangas.is_empty(),
                "popular: expected mangas from captured html"
            );
            assert!(
                !pages.has_next,
                "page 1 should have no [rel=next] for this source"
            );
        }
        FixtureState::Blocked => assert!(pages.mangas.is_empty()),
        FixtureState::Missing => unreachable!(),
    }
    for m in &pages.mangas {
        assert!(!m.title.is_empty());
        assert!(m.url.starts_with('/'));
    }
}

#[test]
fn flow_search_with_filter_states() {
    let Some((mut ext, state)) = replay_ext() else {
        eprintln!("{}", skip_msg(&FixtureState::Missing));
        return;
    };
    let src = &ext.sources().unwrap()[0];
    let fl = ext.filters(src).unwrap();
    // default states for every filter — exercises FilterList building + search
    let states: Vec<FilterState> = fl
        .iter()
        .map(|f| FilterState {
            name: f.name.clone(),
            state: f.state,
        })
        .collect();
    let pages = ext.search(src, 1, "one piece", &states).unwrap();
    match state {
        FixtureState::Ready => assert!(
            !pages.mangas.is_empty(),
            "search: expected mangas from captured html"
        ),
        FixtureState::Blocked => assert!(pages.mangas.is_empty()),
        FixtureState::Missing => unreachable!(),
    }
    for m in &pages.mangas {
        assert!(!m.title.is_empty());
        assert!(m.url.starts_with('/'));
    }
}

#[test]
fn flow_manga_details() {
    let Some((mut ext, state)) = replay_ext() else {
        eprintln!("{}", skip_msg(&FixtureState::Missing));
        return;
    };
    let src = &ext.sources().unwrap()[0];
    let manga = match ext.popular(src, 1).unwrap().mangas.into_iter().next() {
        Some(m) => m,
        None => return, // blocked capture: nothing to resolve
    };
    let details = ext.manga_details(src, &manga).unwrap();
    match state {
        FixtureState::Ready => {
            assert_eq!(
                details.url, manga.url,
                "details must echo the requested url"
            );
            assert!(
                !details.description.is_empty(),
                "expected a description from captured html"
            );
        }
        FixtureState::Blocked => {}
        FixtureState::Missing => unreachable!(),
    }
}

#[test]
fn flow_chapters_and_pages() {
    let Some((mut ext, state)) = replay_ext() else {
        eprintln!("{}", skip_msg(&FixtureState::Missing));
        return;
    };
    let src = &ext.sources().unwrap()[0];
    let manga = match ext.popular(src, 1).unwrap().mangas.into_iter().next() {
        Some(m) => m,
        None => return, // blocked capture: nothing to resolve
    };
    let chapters = ext.chapters(src, &manga).unwrap();
    match state {
        FixtureState::Ready => {
            assert!(!chapters.is_empty(), "expected chapters from captured html");
            let first = &chapters[0];
            assert!(!first.name.is_empty());
            assert!(first.url.starts_with('/'));
            let pages = ext.pages(src, first).unwrap();
            assert!(!pages.is_empty(), "expected pages from captured html");
            for p in &pages {
                assert!(!p.url.is_empty());
                assert!(!p.name.is_empty());
            }
        }
        FixtureState::Blocked => {}
        FixtureState::Missing => unreachable!(),
    }
    for c in &chapters {
        assert!(!c.url.is_empty());
    }
}

#[test]
fn flow_latest_unsupported() {
    // akuma-specific: the fixture dex reports supportsLatest=false and throws
    // latestUpdatesRequest. Other apks differ, so only run against the
    // default fixture apk.
    if env_overridden() {
        return;
    }
    let Some((mut ext, _state)) = replay_ext() else {
        eprintln!("{}", skip_msg(&FixtureState::Missing));
        return;
    };
    let src = &ext.sources().unwrap()[0];
    // fixture dex: getSupportsLatest=false + throwing latestUpdatesRequest
    assert!(!ext.supports_latest(src).unwrap());
    assert!(ext.latest(src, 1).is_err());
}

// ---------------------------------------------------------------------------
// pure unit tests that need no fixtures
// ---------------------------------------------------------------------------

#[test]
fn http_resp_header_is_case_insensitive() {
    let mut resp = HttpResp::ok("x");
    resp.headers = vec![("Content-Type".to_string(), "text/html".to_string())];
    assert_eq!(resp.header("content-type"), Some("text/html"));
    assert_eq!(resp.header("CONTENT-TYPE"), Some("text/html"));
    assert_eq!(resp.header("Content-Encoding"), None);
    // last occurrence wins
    resp.headers
        .push(("content-type".to_string(), "text/plain".to_string()));
    assert_eq!(resp.header("Content-Type"), Some("text/plain"));
}
