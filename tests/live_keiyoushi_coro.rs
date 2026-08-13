//! Live tests for BOTH keiyoushi extension generations, driven through the
//! modern suspend/coroutine entry points (the dexvm host-default natives on
//! the HttpSource shim plus the APK's real coroutine state machines):
//!
//! - pre-1.6 (classic generation, e.g. `tachiyomi-all.akuma-v1.4.10.apk`):
//!   the APK has no suspend methods, so `popular_coro`/`search_coro`/
//!   `pages_coro` resolve through the host-default shim natives bridging to
//!   the APK's real `*Request`/`*Parse` pairs, and details/chapters fall back
//!   from `getMangaUpdate` (missing) to the classic request/parse flow — the
//!   exact `call_fallback` semantics rakuyomi uses.
//! - 1.6+ (suspend generation, e.g. `tachiyomi-vi.moetruyen-v1.6.8.apk`):
//!   the APK implements `getPopularManga`, `getSearchManga`, `getPageList`
//!   and `getMangaUpdate` as real suspend functions; the classic
//!   `mangaDetailsRequest`/`chapterListRequest` methods are throwing stubs.
//!
//! Gated on `DEXVM_LIVE=1` — without it the tests print a note and pass,
//! keeping CI offline. When the target site serves a WAF/geo block the
//! pipeline is still exercised end to end but data assertions are relaxed;
//! the era signature (resolution outcome of the modern vs classic entries)
//! is verified regardless, since it needs no network.

use std::cell::RefCell;
use std::rc::Rc;

use dexvm::keiyoushi::{HttpData, HttpResp, Keiyoushi, Manga, Source};

const APK_PRE16: &str = "fixtures/tachiyomi-all.akuma-v1.4.10.apk";
const APK_16PLUS: &str = "fixtures/tachiyomi-vi.moetruyen-v1.6.8.apk";
const QUERY: &str = "one piece";

/// APK under test; override with DEXVM_APK to point at another keiyoushi
/// extension (any generation).
fn apk_path(default: &str) -> String {
    std::env::var("DEXVM_APK").unwrap_or_else(|_| default.to_string())
}

fn init_logger() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();
    });
}

fn real_http(req: &HttpData) -> HttpResp {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    let agent = AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .user_agent(
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/126.0 Safari/537.36",
            )
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .http_status_as_error(false)
            .build()
            .into()
    });
    if req.method == "POST" {
        let mut rq = agent.post(&req.url);
        for (k, v) in &req.headers {
            rq = rq.header(k, v);
        }
        match rq.send(req.body.as_deref().unwrap_or("")) {
            Ok(r) => to_resp(r),
            Err(e) => HttpResp {
                code: 0,
                message: e.to_string(),
                headers: Vec::new(),
                body: None,
            },
        }
    } else {
        let mut rq = agent.get(&req.url);
        for (k, v) in &req.headers {
            rq = rq.header(k, v);
        }
        match rq.call() {
            Ok(r) => to_resp(r),
            Err(e) => HttpResp {
                code: 0,
                message: e.to_string(),
                headers: Vec::new(),
                body: None,
            },
        }
    }
}

fn to_resp(r: ureq::http::Response<ureq::Body>) -> HttpResp {
    let code = r.status().as_u16() as i32;
    let bytes = r.into_body().read_to_vec().unwrap_or_default();
    HttpResp {
        code,
        message: "OK".into(),
        headers: Vec::new(),
        body: Some(bytes),
    }
}

/// Wraps a call result: on success return the value; on error, when the
/// site provably served real data (`live`), fail the test, otherwise treat
/// it as a WAF/geo block and keep the pipeline exercised end to end.
fn or_blocked<T>(
    r: Result<T, dexvm::vm::error::JvmError>,
    ext: &mut Keiyoushi,
    what: &str,
    live: bool,
) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            if live {
                panic!("{what} failed: {}", ext.describe_error(&e));
            }
            eprintln!(
                "warn: {what} failed (blocked site?): {}",
                ext.describe_error(&e)
            );
            None
        }
    }
}

/// Era signature, verifiable without any network: the resolution outcome of
/// the modern combined entry vs the classic request/parse flow.
fn check_era(ext: &mut Keiyoushi, src: &Source, m0: &Manga, expect_16plus: bool) {
    if expect_16plus {
        // `getMangaUpdate` must resolve (run, or surface a runtime error);
        // it must NOT be a resolution error, and the classic request/parse
        // pair must be the throwing-stub signature of the 1.6 generation.
        match ext.manga_update_details(src, m0) {
            Ok(_) => {}
            Err(e) => assert!(
                !ext.describe_error(&e).contains("resolution error"),
                "1.6-era apk must provide getMangaUpdate: {}",
                ext.describe_error(&e)
            ),
        }
        assert!(
            ext.manga_details(src, m0).is_err(),
            "1.6-era apk must stub the classic mangaDetailsRequest"
        );
        assert!(
            ext.chapters(src, m0).is_err(),
            "1.6-era apk must stub the classic chapterListRequest"
        );
    } else {
        // No suspend methods at all: the combined entry must fail to resolve,
        // while the classic request/parse flow is the APK's real one.
        match ext.manga_update_details(src, m0) {
            Err(e) => assert!(
                ext.describe_error(&e).contains("getMangaUpdate"),
                "pre-1.6 apk must lack getMangaUpdate: {}",
                ext.describe_error(&e)
            ),
            Ok(_) => panic!("pre-1.6 apk unexpectedly provides getMangaUpdate"),
        }
        match ext.manga_update_chapters(src, m0) {
            Err(e) => assert!(
                ext.describe_error(&e).contains("getMangaUpdate"),
                "pre-1.6 apk must lack getMangaUpdate: {}",
                ext.describe_error(&e)
            ),
            Ok(_) => panic!("pre-1.6 apk unexpectedly provides getMangaUpdate"),
        }
    }
}

fn run_flow(apk_default: &str, expect_16plus: bool) {
    init_logger();
    if std::env::var("DEXVM_LIVE").is_err() {
        eprintln!(
            "note: set DEXVM_LIVE=1 to run the live network test (hits the \
             {} fixture site)",
            apk_default
        );
        return;
    }

    let apk = apk_path(apk_default);
    let mut ext = Keiyoushi::open(&apk).unwrap();
    let captured = Rc::new(RefCell::new(Vec::<String>::new()));
    {
        let captured = captured.clone();
        ext.set_http_rc(Rc::new(move |req| {
            captured.borrow_mut().push(req.url.clone());
            real_http(req)
        }));
    }

    // sources + source metadata
    let srcs = ext.sources().unwrap();
    assert!(!srcs.is_empty(), "no sources in apk");
    let src = &srcs[0];
    let name = ext.source_name(src).unwrap();
    assert!(!name.is_empty(), "source has no name");
    let _lang = ext.source_lang(src).unwrap();
    let supports_latest = ext.supports_latest(src).unwrap();

    // popular + search through the suspend entries
    let popular = or_blocked(ext.popular_coro(src, 1), &mut ext, "popular_coro", false);
    let found = or_blocked(
        ext.search_coro(src, 1, QUERY, &[]),
        &mut ext,
        "search_coro",
        false,
    );

    let live = popular.as_ref().map_or(false, |p| !p.mangas.is_empty())
        || found.as_ref().map_or(false, |f| !f.mangas.is_empty());
    if !live {
        eprintln!(
            "warn: no manga parsed from the live site (WAF/geo/outage?) — pipeline \
             still exercised end to end"
        );
    }

    let m0 = popular
        .as_ref()
        .and_then(|p| p.mangas.first())
        .or_else(|| found.as_ref().and_then(|f| f.mangas.first()))
        .cloned()
        .unwrap_or_else(|| Manga {
            title: "synthetic".into(),
            url: "/manga/1".into(),
            ..Default::default()
        });
    assert!(
        !m0.url.is_empty() && m0.url.starts_with('/'),
        "bad manga url: {:?}",
        m0.url
    );

    // era signature (no network needed)
    check_era(&mut ext, src, &m0, expect_16plus);

    // details: combined entry first, classic fallback on the pre-1.6
    // resolution failure (rakuyomi call_fallback semantics)
    let details = match ext.manga_update_details(src, &m0) {
        Ok(d) => Some(d),
        Err(_e) if !expect_16plus => or_blocked(
            ext.manga_details(src, &m0),
            &mut ext,
            "classic manga_details",
            live,
        ),
        Err(e) => or_blocked(Err(e), &mut ext, "manga_update_details", live),
    };
    if let Some(d) = &details {
        if live {
            // Note: the parsed url is NOT asserted against m0.url — some
            // sources (e.g. noxenscans details parse, AHottie getMangaUpdate)
            // legitimately return a manga without an url, and the host keeps
            // the requested one (mihon UpdateMangaFromRemote semantics).
            assert!(!d.title.is_empty(), "expected a title on live data");
        }
    }

    // chapters: same fallback pattern
    let chapters = match ext.manga_update_chapters(src, &m0) {
        Ok(c) => Some(c),
        Err(_e) if !expect_16plus => {
            or_blocked(ext.chapters(src, &m0), &mut ext, "classic chapters", live)
        }
        Err(e) => or_blocked(Err(e), &mut ext, "manga_update_chapters", live),
    };
    if let Some(chs) = &chapters {
        if live {
            assert!(!chs.is_empty(), "expected chapters on live data");
        }
        for c in chs {
            assert!(!c.url.is_empty() && c.url.starts_with('/'));
        }
    }

    // pages through the suspend entry (classic fallback for pre-1.6)
    let pages = match chapters.as_ref().and_then(|l| l.first()) {
        Some(c) => match ext.pages_coro(src, c) {
            Ok(p) => Some(p),
            Err(_e) if !expect_16plus => {
                or_blocked(ext.pages(src, c), &mut ext, "classic pages", live)
            }
            Err(e) => or_blocked(Err(e), &mut ext, "pages_coro", live),
        },
        None => None,
    };
    if let Some(ps) = &pages {
        if live {
            assert!(!ps.is_empty(), "expected pages on live data");
        }
        for p in ps {
            assert!(
                !p.url.is_empty() || !p.image_url.is_empty(),
                "every page must carry a url or an image_url (got neither)"
            );
        }
    }

    // latest through the suspend entry when the source supports it
    if supports_latest {
        let _lp = or_blocked(ext.latest_coro(src, 1), &mut ext, "latest_coro", live);
    }

    let caps = captured.borrow();
    assert!(!caps.is_empty(), "pipeline must issue real HTTP requests");
    eprintln!(
        "live: {} requests issued against {} (expect_16plus={})",
        caps.len(),
        apk,
        expect_16plus
    );
}

#[test]
fn live_pre16_flow() {
    run_flow(APK_PRE16, false);
}

#[test]
fn live_16plus_flow() {
    run_flow(APK_16PLUS, true);
}
