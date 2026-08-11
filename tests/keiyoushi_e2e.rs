//! End-to-end: run the bundled keiyoushi fixture extension through the typed
//! bridge (`keiyoushi::Keiyoushi`).

use dexvm::keiyoushi::{FilterKind, HttpResp, Keiyoushi};

const APK: &str = "fixtures/tachiyomi-all.akuma-v1.4.10.apk";

const POPULAR_HTML: &str = r#"<html><body>
<div class="post-loop">
<ul>
<li>
  <a href="/manga/one-piece-2"><div class="cover"><img src="/img/one.jpg"></div></a>
  <div class="overlay-title">One Piece"""</div>
</li>
<li>
  <a href="/manga/boruto-ng"><div class="cover"><img src="/img/boruto.jpg"></div></a>
  <div class="overlay-title">Boruto Next Generations</div>
</li>
</ul>
<nav class="page-nav"><a rel="prev">1</a></nav>
</div>
</body></html>"#;



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
fn create_sources() {
    init_logger();
    let mut ext = Keiyoushi::open(APK).unwrap();
    let srcs = ext.sources().unwrap();
    assert!(!srcs.is_empty(), "expected at least one source");
    let name = ext.source_name(&srcs[0]).unwrap();
    let lang = ext.source_lang(&srcs[0]).unwrap();
    assert_eq!(name, "Akuma");
    assert_eq!(lang, "all");
}

#[test]
fn popular_parses_html() {
    init_logger();
    let mut ext = Keiyoushi::open(APK).unwrap();
    ext.set_http(move |_req| HttpResp::ok(POPULAR_HTML));
    let srcs = ext.sources().unwrap();
    let pages = ext.popular(&srcs[0], 1).unwrap();
    assert_eq!(pages.mangas.len(), 2, "two <li> in fixture html");
    let m = &pages.mangas[0];
    assert_eq!(m.title, "One Piece");
    assert_eq!(m.url, "/manga/one-piece-2");
    assert!(m.thumbnail_url.ends_with("/img/one.jpg"));
    assert!(!pages.has_next, "no [rel=next] link in fixture html");
}

#[test]
fn filters_listed() {
    init_logger();
    let mut ext = Keiyoushi::open(APK).unwrap();
    ext.set_http(move |_| HttpResp::ok("<html></html>"));
    let srcs = ext.sources().unwrap();
    let fl = ext.filters(&srcs[0]).unwrap();
    assert!(!fl.is_empty());
    // TriState / Select / Text / Group are all present (list starts with Headers)
    let kinds: Vec<_> = fl.iter().map(|f| f.kind).collect();
    assert!(kinds.contains(&FilterKind::Group));
    assert!(kinds.contains(&FilterKind::TriState));
    assert!(kinds.contains(&FilterKind::Select));
    assert!(kinds.contains(&FilterKind::Text));
    // the two headers come first
    assert_eq!(fl[0].kind, FilterKind::Plain);
    assert_eq!(fl[0].name, "Separate tags with commas (,)");
}
