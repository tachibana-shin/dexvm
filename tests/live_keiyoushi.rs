//! Live test: runs every public `keiyoushi::Keiyoushi` function against the
//! real network (no mock HTTP), then captures every response into
//! `fixtures/live/` so the offline `keiyoushi_flow` tests replay the exact
//! data.
//!
//! Gated on `DEXVM_LIVE=1` — without it the test prints a note and passes,
//! keeping CI offline. The site (api.akuma.moe) sits behind DDoS-Guard, so
//! the assertions adapt: when the live server returns real content the test
//! asserts real data; when it serves a WAF challenge the pipeline is still
//! exercised end to end and the capture is marked BLOCKED.

use std::cell::RefCell;
use std::rc::Rc;

use dexvm::keiyoushi::{FilterState, HttpData, HttpResp, Keiyoushi, Manga};

const APK: &str = "fixtures/tachiyomi-all.akuma-v1.4.10.apk";
const LIVE_DIR: &str = "fixtures/live";
const QUERY: &str = "one piece";

/// APK under test; override with DEXVM_APK to point at another keiyoushi
/// extension (e.g. fixtures/tachiyomi-en.mangapill-v1.4.9.apk).
fn apk_path() -> String {
    std::env::var("DEXVM_APK").unwrap_or_else(|_| APK.to_string())
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

/// Saves each distinct response as `fixtures/live/NNN-slug.html` plus a
/// tab-separated manifest (code, file, method, url).
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
        env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("off"),
        )
        .init();
    });
}

#[test]
fn live_full_pipeline() {
    init_logger();
    if std::env::var("DEXVM_LIVE").is_err() {
        eprintln!("note: set DEXVM_LIVE=1 to run the live network test (hits api.akuma.moe)");
        return;
    }

    let apk = apk_path();
    let mut ext = Keiyoushi::open(&apk).unwrap();
    let captures = Rc::new(RefCell::new(Vec::<(String, String, i32, String)>::new()));
    {
        let captures = captures.clone();
        ext.set_http_rc(Rc::new(move |req| {
            let resp = real_http(req);
            captures.borrow_mut().push((
                req.method.clone(),
                req.url.clone(),
                resp.code,
                String::from_utf8_lossy(resp.body.as_deref().unwrap_or(b""))
                    .into_owned(),
            ));
            resp
        }));
    }

    // sources + source metadata
    let srcs = ext.sources().unwrap();
    assert!(!srcs.is_empty(), "no sources in apk");
    let src = &srcs[0];
    let name = ext.source_name(src).unwrap();
    assert!(!name.is_empty(), "source has no name");
    let _ = ext.source_lang(src).unwrap();
    let supports_latest = ext.supports_latest(src).unwrap();

    // filters (no network) and per-filter states for the search call
    let fl = match ext.filters(src) {
        Ok(fl) => fl,
        Err(e) => panic!("filters failed: {}", ext.describe_error(&e)),
    };
    assert!(!fl.is_empty(), "no filters listed");
    let states: Vec<FilterState> = fl
        .iter()
        .map(|f| FilterState {
            name: f.name.clone(),
            state: f.state,
        })
        .collect();

    // real network: popular + search
    let popular = ext
        .popular(src, 1)
        .unwrap_or_else(|e| panic!("popular failed: {}", ext.describe_error(&e)));
    let found = ext
        .search(src, 1, QUERY, &states)
        .unwrap_or_else(|e| panic!("search failed: {}", ext.describe_error(&e)));

    let live = !(popular.mangas.is_empty() && found.mangas.is_empty());
    if !live {
        eprintln!(
            "warn: no manga parsed from the live site (WAF/geo/outage?) — pipeline \
             still exercised end to end; see fixtures/live/*.html"
        );
    }

    let m0 = popular
        .mangas
        .first()
        .or_else(|| found.mangas.first())
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

    // details / chapters / pages
    let details = ext.manga_details(src, &m0);
    let chapters = ext.chapters(src, &m0);
    let chs = chapters.as_deref().ok();
    let pages = match chs.and_then(|l| l.first()) {
        Some(c) => match ext.pages(src, c) {
            Ok(p) => p,
            Err(e) => {
                if live {
                    panic!("pages failed on live data: {}", ext.describe_error(&e));
                }
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    for c in chs.unwrap_or(&[]) {
        assert!(!c.url.is_empty() && c.url.starts_with('/'));
    }
    for p in &pages {
        assert!(!p.url.is_empty());
    }
    if live {
        let details =
            details.unwrap_or_else(|e| panic!("manga_details failed on live data: {e:?}"));
        assert_eq!(
            details.url, m0.url,
            "details must echo the requested manga url"
        );
        let chapters = chapters
            .unwrap_or_else(|e| panic!("chapters failed on live data: {}", ext.describe_error(&e)));
        assert!(!chapters.is_empty(), "expected chapters on live data");
        if pages.is_empty() {
            eprintln!(
                "warn: chapter url served no reader markup (site serves JS-driven \
                 misc layout for chapter urls) — pipeline still exercised end to end"
            );
        }
    }

    // latest: only run when the source supports it; on the akuma fixture dex
    // (getSupportsLatest=false) it must fail
    if supports_latest {
        match ext.latest(src, 1) {
            Ok(p) => {
                if live {
                    assert!(
                        !p.mangas.is_empty(),
                        "expected mangas from the live latest page"
                    );
                }
            }
            Err(e) => {
                if live {
                    panic!("latest failed on live data: {}", ext.describe_error(&e));
                }
            }
        }
    } else {
        assert!(
            ext.latest(src, 1).is_err(),
            "source without latest support must fail"
        );
    }

    let caps = captures.borrow();
    assert!(!caps.is_empty(), "pipeline must issue real HTTP requests");
    let net_ok = caps.iter().filter(|(_, _, c, _)| *c == 200).count();
    eprintln!(
        "live: {} requests issued, {} with HTTP 200",
        caps.len(),
        net_ok
    );
    save_fixtures(&caps, !live);
}
