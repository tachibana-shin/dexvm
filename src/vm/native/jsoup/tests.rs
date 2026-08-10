//! Unit tests for the org.jsoup host shims — these are in `JSOUP_TABLE`,
//! exercised with a real dom_query document.

use super::*;
use crate::context::{Context, SandboxOptions};

fn with_vm<T>(f: impl FnOnce(&mut Vm) -> T) -> T {
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
    f(ctx.vm())
}

/// A bare document value (like `Jsoup.parse(html)`, minus the Response seam).
fn doc_of(vm: &mut Vm, html: &str, base: Option<&str>) -> JValue {
    let mut d = JsoupDocRef::new(Document::from(html));
    d.base = base.map(|x| x.to_string());
    alloc(vm, "Lorg/jsoup/nodes/Document;", Native::JsoupDoc(d)).unwrap()
}

fn s(vm: &mut Vm, x: &str) -> JValue {
    vm.alloc_string(x)
}

/// Decode an already-evaluated string result (no borrow overlap).
fn jstr_of(vm: &mut Vm, v: JValue) -> String {
    jstr(vm, v).unwrap()
}

/// Evaluate a native call, then decode its JValue string result. Kept as a
/// macro so the native runs to completion before `vm` is reborrowed for
/// decoding (a function would need `vm` twice in one expression).
macro_rules! s_of {
    ($vm:expr, $call:expr) => {{
        let r = $call.unwrap();
        jstr($vm, r).unwrap()
    }};
}

/// VM booleans are `JValue::Int(0|1)`.
fn bool_of(v: JValue) -> bool {
    match v {
        JValue::Int(i) => i != 0,
        other => panic!("expected int-bool, got {other:?}"),
    }
}

fn int_of(v: JValue) -> i32 {
    match v {
        JValue::Int(i) => i,
        other => panic!("expected int, got {other:?}"),
    }
}

/// Decode a List payload into its string elements. Macro: the native call
/// runs first, then `vm` is reborrowed for decoding.
macro_rules! list_of {
    ($vm:expr, $call:expr) => {{
        let r = $call.unwrap();
        let items: Vec<JValue> = match payload($vm, r) {
            Some(Native::List(items)) => items.clone(),
            _ => panic!("expected List payload"),
        };
        items
            .into_iter()
            .map(|v| jstr($vm, v).unwrap())
            .collect::<Vec<String>>()
    }};
}

#[test]
fn select_size_first_document_text() {
    with_vm(|vm| {
        let html = "<div id=\"a\"><p class=\"ch\">Chap 1</p></div>\
                    <div id=\"b\"><p class=\"ch\">Chap 2</p></div>";
        let doc = doc_of(vm, html, None);
        let sel_div = s(vm, "div");
        let divs = document_select(vm, &[doc, sel_div]).unwrap();
        assert_eq!(int_of(elements_size(vm, &[divs]).unwrap()), 2);
        assert!(!bool_of(elements_is_empty(vm, &[divs]).unwrap()));

        let sel_ch = s(vm, "p.ch");
        let chs = elements_select(vm, &[divs, sel_ch]).unwrap();
        assert_eq!(int_of(elements_size(vm, &[chs]).unwrap()), 2);

        let t = document_text(vm, &[doc]).unwrap();
        assert_eq!(jstr_of(vm, t), "Chap 1 Chap 2");

        // first() picks the first match; id_attr reflects the "id" attribute.
        let first = elements_first(vm, &[chs]).unwrap();
        let id = element_id_attr(vm, &[first]).unwrap();
        assert_eq!(jstr_of(vm, id), "");
        let parent = element_parent(vm, &[first]).unwrap();
        let sel_id = s(vm, "id");
        let pid = element_attr(vm, &[parent, sel_id]).unwrap();
        assert_eq!(jstr_of(vm, pid), "a");
    });
}

#[test]
fn elements_first_last_get() {
    with_vm(|vm| {
        let html = "<ul><li>1</li><li>2</li><li>3</li></ul>";
        let doc = doc_of(vm, html, None);
        let sel_li = s(vm, "li");
        let lis = document_select(vm, &[doc, sel_li]).unwrap();

        let first = elements_first(vm, &[lis]).unwrap();
        assert_eq!(s_of!(vm, element_text(vm, &[first])), "1");

        let last = elements_last(vm, &[lis]).unwrap();
        assert_eq!(s_of!(vm, element_text(vm, &[last])), "3");

        let second = elements_get(vm, &[lis, JValue::Int(1)]).unwrap();
        assert_eq!(s_of!(vm, element_text(vm, &[second])), "2");
    });
}

#[test]
fn attr_abs_hasattr_tag() {
    with_vm(|vm| {
        let html = r#"<a href="/manga/5" class="x">Title</a>"#;
        let doc = doc_of(vm, html, Some("https://api.akuma.moe"));
        let sel_a = s(vm, "a");
        let a = element_select_first(vm, &[doc, sel_a]).unwrap();
        assert!(!a.is_null(), "selector must find the anchor");

        let sel_href = s(vm, "href");
        assert_eq!(s_of!(vm, element_attr(vm, &[a, sel_href])), "/manga/5");
        let sel_class = s(vm, "class");
        assert!(bool_of(element_has_attr(vm, &[a, sel_class]).unwrap()));
        let sel_missing = s(vm, "missing");
        assert!(!bool_of(element_has_attr(vm, &[a, sel_missing]).unwrap()));

        // abs: resolves against the document base URI.
        let sel_abshref = s(vm, "abs:href");
        assert_eq!(
            s_of!(vm, element_attr(vm, &[a, sel_abshref])),
            "https://api.akuma.moe/manga/5"
        );
        // absUrl(name) is the same resolution.
        assert_eq!(
            s_of!(vm, element_abs_url(vm, &[a, sel_href])),
            "https://api.akuma.moe/manga/5"
        );
        assert_eq!(s_of!(vm, element_tag_name(vm, &[a])), "a");
    });
}

#[test]
fn abs_attr_without_base_stays_raw() {
    with_vm(|vm| {
        let html = r#"<a href="/manga/5">Title</a>"#;
        let doc = doc_of(vm, html, None);
        let sel_a = s(vm, "a");
        let a = element_select_first(vm, &[doc, sel_a]).unwrap();
        let sel_abs = s(vm, "abs:href");
        assert_eq!(s_of!(vm, element_attr(vm, &[a, sel_abs])), "/manga/5");
    });
}

#[test]
fn text_drops_script_style() {
    with_vm(|vm| {
        let html = "<div>keep <script>drop()</script><style>.x {}</style> <p> tail </p></div>";
        let doc = doc_of(vm, html, None);
        let sel_div = s(vm, "div");
        let div = element_select_first(vm, &[doc, sel_div]).unwrap();
        assert_eq!(s_of!(vm, element_text(vm, &[div])), "keep tail");
        // own_text() is the raw immediate text: script/style nodes are kept
        // and whitespace is preserved verbatim (dom_query semantics).
        assert_eq!(s_of!(vm, element_own_text(vm, &[div])), "keep  ");
    });
}

#[test]
fn children_parent_html() {
    with_vm(|vm| {
        let html = "<ul><li>1</li><li>2</li></ul>";
        let doc = doc_of(vm, html, None);
        let sel_ul = s(vm, "ul");
        let ul = element_select_first(vm, &[doc, sel_ul]).unwrap();
        let ch = element_children(vm, &[ul]).unwrap();
        assert_eq!(int_of(elements_size(vm, &[ch]).unwrap()), 2);

        let li0 = elements_first(vm, &[ch]).unwrap();
        let parent = element_parent(vm, &[li0]).unwrap();
        assert_eq!(s_of!(vm, element_tag_name(vm, &[parent])), "ul");

        let inner = s_of!(vm, element_html(vm, &[ul]));
        assert!(inner.contains("<li>1</li>") && inner.contains("<li>2</li>"));
        let outer = s_of!(vm, element_outer_html(vm, &[ul]));
        assert!(outer.starts_with("<ul>") && outer.ends_with("</ul>"));
    });
}

#[test]
fn select_includes_self_when_matching() {
    with_vm(|vm| {
        let html = "<div><p>a</p><span>b</span></div>";
        let doc = doc_of(vm, html, None);
        let sel_div = s(vm, "div");
        let div = element_select_first(vm, &[doc, sel_div]).unwrap();
        // Selecting "div" from the div includes the element itself.
        let divs = element_select(vm, &[div, sel_div]).unwrap();
        assert_eq!(int_of(elements_size(vm, &[divs]).unwrap()), 1);
        let sel_span = s(vm, "span");
        let spans = element_select(vm, &[div, sel_span]).unwrap();
        assert_eq!(int_of(elements_size(vm, &[spans]).unwrap()), 1);
        // No match: first() -> null.
        let sel_footer = s(vm, "footer");
        let none = element_select_first(vm, &[doc, sel_footer]).unwrap();
        assert!(none.is_null());
    });
}

#[test]
fn elements_each_text_and_attr() {
    with_vm(|vm| {
        let html = r#"<a href="/a">A</a><a href="/b">B</a>"#;
        let doc = doc_of(vm, html, Some("https://api.example.com"));
        let sel_a = s(vm, "a");
        let links = document_select(vm, &[doc, sel_a]).unwrap();

        assert_eq!(list_of!(vm, elements_each_text(vm, &[links])), ["A", "B"]);
        let sel_href = s(vm, "href");
        assert_eq!(
            list_of!(vm, elements_each_attr(vm, &[links, sel_href])),
            ["/a", "/b"]
        );
        let sel_abs = s(vm, "abs:href");
        assert_eq!(
            list_of!(vm, elements_each_attr(vm, &[links, sel_abs])),
            ["https://api.example.com/a", "https://api.example.com/b"]
        );
        // elements.attr() returns the first present value.
        assert_eq!(s_of!(vm, elements_attr(vm, &[links, sel_href])), "/a");
    });
}

#[test]
fn contains_normalization() {
    assert_eq!(
        normalize_contains(":contains(Manga) a"),
        r#":contains("Manga") a"#
    );
    assert_eq!(
        normalize_contains(":contains(\"Manga\") a"),
        r#":contains("Manga") a"#
    );
    assert_eq!(normalize_contains("div > p"), "div > p");
    assert_eq!(
        normalize_contains("a:contains(x) b"),
        r#"a:contains("x") b"#
    );
}

#[test]
#[cfg(feature = "okhttp")]
fn parse_from_response_seam() {
    with_vm(|vm| {
        // jsoup_parse consumes a Native::Response; the originating request
        // URL becomes the document base URI for abs: resolution.
        let req = alloc(
            vm,
            "Lokhttp3/Request;",
            Native::Request {
                url: "https://api.akuma.moe/manga/5".into(),
                method: "GET".into(),
                headers: Vec::new(),
                body: None,
            },
        )
        .unwrap();
        let resp = alloc(
            vm,
            "Lokhttp3/Response;",
            Native::Response {
                code: 200,
                message: "OK".into(),
                headers: Vec::new(),
                body: Some(r#"<div class="x"><a href="/img/1">pix</a></div>"#.as_bytes().to_vec()),
                request: req,
            },
        )
        .unwrap();
        let doc = jsoup_parse(vm, &[resp]).unwrap();
        let sel_x = s(vm, "div.x");
        let div = element_select_first(vm, &[doc, sel_x]).unwrap();
        let sel_a = s(vm, "a");
        let a = element_select_first(vm, &[div, sel_a]).unwrap();
        let sel_href = s(vm, "href");
        assert_eq!(
            s_of!(vm, element_abs_url(vm, &[a, sel_href])),
            "https://api.akuma.moe/img/1"
        );
    });
}

#[test]
fn empty_selection() {
    with_vm(|vm| {
        let doc = doc_of(vm, "<html><body></body></html>", None);
        let sel_p = s(vm, "p");
        let els = document_select(vm, &[doc, sel_p]).unwrap();
        assert!(bool_of(elements_is_empty(vm, &[els]).unwrap()));
        assert_eq!(int_of(elements_size(vm, &[els]).unwrap()), 0);
        assert!(elements_first(vm, &[els]).unwrap().is_null());
        assert!(elements_last(vm, &[els]).unwrap().is_null());
    });
}
