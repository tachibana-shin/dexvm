use dexvm::keiyoushi::{HttpResp, Keiyoushi, Manga};

const APK: &str = "fixtures/tachiyomi-all.akuma-v1.4.10.apk";

const POPULAR_HTML: &str = r#"<html><body>
<div class="post-loop">
<ul>
<li>
  <a href="/manga/one-piece-2"><div class="cover"><img src="/img/one.jpg"></div></a>
  <div class="overlay-title">One Piece</div>
</li>
<li>
  <a href="/manga/boruto-ng"><div class="cover"><img src="/img/boruto.jpg"></div></a>
  <div class="overlay-title">Boruto Next Generations</div>
</li>
</ul>
<nav class="page-nav"><a rel="prev">1</a></nav>
</div>
</body></html>"#;

#[test]
fn smoke_all_apis() {
    let mut ext = Keiyoushi::open(APK).unwrap();
    ext.set_http(move |_req| HttpResp::ok(POPULAR_HTML));
    let srcs = ext.sources().unwrap();
    assert!(!srcs.is_empty());
    let src = &srcs[0];

    assert_eq!(ext.source_name(src).unwrap(), "Akuma");
    let _ = ext.source_lang(src).unwrap();
    // The fixture dex declares getSupportsLatest=false and latestUpdatesRequest
    // is a throwing stub, so latest() must fail with Uncaught.
    assert!(!ext.supports_latest(src).unwrap());
    assert!(ext.latest(src, 1).is_err(), "latest should be unsupported");

    let popular = ext.popular(src, 1).unwrap();
    assert_eq!(popular.mangas.len(), 2);
    let manga = Manga {
        title: "One Piece".into(),
        thumbnail_url: "https://cdn.example/img/one.jpg".into(),
        url: "/manga/one-piece-2".into(),
        ..Default::default()
    };

    let latest = ext.latest(src, 1);
    assert!(latest.is_err(), "latest should be unsupported");

    let details = ext.manga_details(src, &manga);
    assert!(details.is_ok(), "manga_details failed: {details:?}");

    let chapters = ext.chapters(src, &manga);
    assert!(chapters.is_ok(), "chapters failed: {chapters:?}");
    if let Ok(chs) = &chapters {
        if let Some(c) = chs.first() {
            let pages = ext.pages(src, c);
            assert!(pages.is_ok(), "pages failed: {pages:?}");
        }
    }

    let found = ext.search(src, 1, "One Piece", &[]);
    assert!(found.is_ok(), "search failed: {found:?}");

    let fl = ext.filters(src).unwrap();
    assert!(!fl.is_empty());
}
