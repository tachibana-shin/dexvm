//! Live Moetruyen test: drives the REAL extension code through the VM and
//! the REAL network — no mock HTTP. Exercises the standard tachiyomi
//! `HttpSource` contract the way Tachiyomi's own network layer does:
//!
//! - `getBaseUrl`/`getName`/`getLang`/`getSupportsLatest` (source metadata)
//! - `getFilterList` (no network)
//! - `getPopularManga` / `getLatestUpdates` / `getSearchManga`
//!   (suspend coroutine API — moetruyen v1.6.8 stubs the classic
//!   `*Request`/`*Parse` pairs with UnsupportedOperationException)
//! - `getMangaDetails`/`getChapterList`/`getPageList`: asserted to surface
//!   the extension's own UnsupportedOperationException (listing-only source)
//!
//! Gated on `DEXVM_LIVE=1` — without it the test prints a note and passes,
//! keeping CI offline. moetruyen.net sits behind geo-blocking and serves
//! flaky data in some regions, so the assertions adapt: when the live
//! server returns parseable content the test asserts real data; when it
//! serves a block/error page the pipeline is still exercised end to end and
//! captured into `fixtures/live_moetruyen/` for inspection.

use std::cell::RefCell;
use std::rc::Rc;

use dexvm::keiyoushi::{Chapter, FilterState, HttpData, HttpResp, Keiyoushi, Manga};

const APK: &str = "fixtures/tachiyomi-vi.moetruyen-v1.6.8.apk";
const LIVE_DIR: &str = "fixtures/live_moetruyen";
const QUERY: &str = "one piece";

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
    let resp = if req.method == "POST" {
        let mut rq = agent.post(&req.url);
        for (k, v) in &req.headers {
            rq = rq.header(k, v);
        }
        match rq.send(req.body.as_deref().unwrap_or("")) {
            Ok(r) => r,
            Err(e) => return err_resp(e),
        }
    } else {
        let mut rq = agent.get(&req.url);
        for (k, v) in &req.headers {
            rq = rq.header(k, v);
        }
        match rq.call() {
            Ok(r) => r,
            Err(e) => return err_resp(e),
        }
    };
    let code = resp.status().as_u16() as i32;
    let bytes = resp.into_body().read_to_vec().unwrap_or_default();
    HttpResp {
        code,
        message: "OK".into(),
        headers: Vec::new(),
        body: Some(bytes),
    }
}

fn err_resp(e: ureq::Error) -> HttpResp {
    HttpResp {
        code: 0,
        message: e.to_string(),
        headers: Vec::new(),
        body: None,
    }
}

/// Saves each distinct response as `fixtures/live_moetruyen/NNN-slug.html`
/// plus a tab-separated manifest (code, file, method, url).
fn save_fixtures(caps: &[(String, String, i32, String)], blocked: bool) {
    let _ = std::fs::create_dir_all(LIVE_DIR);
    let mut manifest = String::new();
    let mut seen = std::collections::HashSet::new();
    let mut n = 0usize;
    for (method, url, code, body) in caps {
        if !seen.insert((method.clone(), url.clone())) {
            continue;
        }
        let slug: String = url
            .replace("https://", "")
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let fname: String = format!("{n:03}-{slug}").chars().take(100).collect();
        let _ = std::fs::write(format!("{LIVE_DIR}/{fname}"), body);
        manifest.push_str(&format!("{code}\t{fname}\t{method}\t{url}\n"));
        n += 1;
    }
    let _ = std::fs::write(format!("{LIVE_DIR}/manifest.txt"), manifest);
    if blocked {
        let _ = std::fs::write(format!("{LIVE_DIR}/BLOCKED"), "");
    } else {
        let _ = std::fs::remove_file(format!("{LIVE_DIR}/BLOCKED"));
    }
    eprintln!("live: captured {n} responses into {LIVE_DIR}/ (blocked={blocked})");
}

fn init_logger() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();
    });
}

#[test]
fn live_tachiyomi_source_api() {
    init_logger();
    if std::env::var("DEXVM_LIVE").is_err() {
        eprintln!("note: set DEXVM_LIVE=1 to run the live network test (hits moetruyen.net)");
        return;
    }

    let mut ext = Keiyoushi::open(APK).expect("open moetruyen apk");
    let captures = Rc::new(RefCell::new(Vec::<(String, String, i32, String)>::new()));
    {
        let captures = captures.clone();
        ext.set_http_rc(Rc::new(move |req| {
            let resp = real_http(req);
            captures.borrow_mut().push((
                req.method.clone(),
                req.url.clone(),
                resp.code,
                String::from_utf8_lossy(resp.body.as_deref().unwrap_or(b"")).into_owned(),
            ));
            resp
        }));
    }

    // 1. source metadata (getBaseUrl/getName/getLang/getSupportsLatest)
    let srcs = ext.sources().expect("sources");
    assert!(!srcs.is_empty(), "moetruyen apk must yield at least one source");
    let src = &srcs[0];
    let name = ext.source_name(src).expect("source_name");
    assert_eq!(name, "MoeTruyen");
    let lang = ext.source_lang(src).expect("source_lang");
    assert_eq!(lang, "vi");
    let supports_latest = ext.supports_latest(src).expect("supports_latest");
    assert!(supports_latest, "moetruyen supports getLatestUpdates");

    // 2. filters (pure dex, no network)
    let fl = ext
        .filters(src)
        .unwrap_or_else(|e| panic!("getFilterList failed: {}", ext.describe_error(&e)));
    assert!(!fl.is_empty(), "getFilterList must yield filters");
    let states: Vec<FilterState> = fl
        .iter()
        .map(|f| FilterState {
            name: f.name.clone(),
            state: f.state,
        })
        .collect();

    // 3. real network: popular + search + latest via the suspend API
    let popular = ext
        .popular_coro(src, 1)
        .unwrap_or_else(|e| panic!("getPopularManga failed: {}", ext.describe_error(&e)));
    let found = ext
        .search_coro(src, 1, QUERY, &states)
        .unwrap_or_else(|e| panic!("getSearchManga failed: {}", ext.describe_error(&e)));
    let latest = ext
        .latest_coro(src, 1)
        .unwrap_or_else(|e| panic!("getLatestUpdates failed: {}", ext.describe_error(&e)));

    let live = !(popular.mangas.is_empty() && found.mangas.is_empty());
    if !live {
        eprintln!(
            "warn: no manga parsed from the live site (WAF/geo/outage?) — pipeline \
             still exercised end to end; see fixtures/live_moetruyen/*.html"
        );
    }

    let m0 = popular
        .mangas
        .first()
        .or_else(|| found.mangas.first())
        .or_else(|| latest.mangas.first())
        .cloned()
        .unwrap_or_else(|| Manga {
            title: "synthetic".into(),
            url: "/truyen/synthetic".into(),
            ..Default::default()
        });
    assert!(
        !m0.url.is_empty() && m0.url.starts_with('/'),
        "bad manga url: {:?}",
        m0.url
    );

    // 4. details + chapters + pages: this extension is listing-only — every
    // classic pair is stubbed, so the standard calls must surface the stub's
    // UnsupportedOperationException instead of hanging the pipeline.
    for (label, res) in [
        ("getMangaDetails", ext.manga_details(src, &m0).map(|_| ())),
        ("getChapterList", ext.chapters(src, &m0).map(|_| ())),
    ] {
        let e = res.expect_err(&format!("{label} must fail on the stub"));
        let msg = ext.describe_error(&e);
        assert!(
            msg.contains("UnsupportedOperationException"),
            "{label}: unexpected error: {msg}"
        );
    }
    let pages = ext.pages_coro(src, &Chapter {
        url: m0.url.clone(),
        name: m0.title.clone(),
        ..Default::default()
    });
    let e = pages.expect_err("getPageList must fail on the stub");
    let msg = ext.describe_error(&e);
    assert!(
        msg.contains("UnsupportedOperationException"),
        "getPageList: unexpected error: {msg}"
    );

    let caps = captures.borrow();
    assert!(!caps.is_empty(), "pipeline must issue real HTTP requests");
    let net_ok = caps.iter().filter(|(_, _, c, _)| *c == 200).count();
    let mut seen_hosts = std::collections::BTreeSet::new();
    for (_, url, _, _) in caps.iter() {
        if let Some(host) = url.split("://").nth(1).and_then(|h| h.split('/').next()) {
            seen_hosts.insert(host.to_string());
        }
    }
    eprintln!(
        "live: {} requests issued, {} with HTTP 200, hosts: {}",
        caps.len(),
        net_ok,
        seen_hosts.into_iter().collect::<Vec<_>>().join(", ")
    );
    save_fixtures(&caps, !live);
}