//! Host shims for the mihon (keiyoushi) extension API surface and the
//! jsoup/okhttp network stack used by extensions.
//!
//! HTTP requests are not executed by the VM: the bridge registers an HTTP
//! callback on [`Vm::http`] and triggers it through the
//! `RequestsKt.__host_execute` native. HTML parsing mirrors the jsoup
//! compatibility layer in rakuyomi's `html_element.rs` (dom_query with
//! `:contains()` normalization, self-inclusive `select`, text that skips
//! script/style content).

use super::*;
use dom_query::{Document, Matcher, NodeId, NodeRef, Selection};
use std::rc::Rc;

use crate::vm::object::JsoupDocRef;

pub(crate) const SMANGA: &str = "Leu/kanade/tachiyomi/source/model/SManga;";
pub(crate) const SCHAPTER: &str = "Leu/kanade/tachiyomi/source/model/SChapter;";
pub(crate) const PAGE: &str = "Leu/kanade/tachiyomi/source/model/Page;";
pub(crate) const FILTER_LIST: &str = "Leu/kanade/tachiyomi/source/model/FilterList;";
pub(crate) const FILTER: &str = "Leu/kanade/tachiyomi/source/model/Filter;";
pub(crate) const HEADERS: &str = "Lokhttp3/Headers;";
pub(crate) const RESPONSE: &str = "Lokhttp3/Response;";
pub(crate) const REQUEST: &str = "Lokhttp3/Request;";
pub(crate) const HTTP_URL: &str = "Lokhttp3/HttpUrl;";

/// A request built by the extension, handed to the host HTTP callback.
#[derive(Debug, Clone)]
pub struct HttpData {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

/// The host HTTP callback output; becomes an `okhttp3.Response` object.
#[derive(Debug, Clone)]
pub struct HttpResp {
    pub code: i32,
    pub message: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl HttpResp {
    pub fn ok(body: impl Into<String>) -> Self {
        HttpResp {
            code: 200,
            message: "OK".into(),
            headers: Vec::new(),
            body: body.into(),
        }
    }
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

// ---------------------------------------------------------------------------
// lazies for shim class statics
// ---------------------------------------------------------------------------

pub(crate) fn lazy_smanga_companion(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Leu/kanade/tachiyomi/source/model/SManga$Companion;")
}

pub(crate) fn lazy_schapter_companion(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Leu/kanade/tachiyomi/source/model/SChapter$Companion;")
}

pub(crate) fn lazy_update_strategy_once(vm: &mut Vm) -> JValue {
    alloc(
        vm,
        "Leu/kanade/tachiyomi/source/model/UpdateStrategy;",
        Native::Enum {
            name: "ONLY_FETCH_ONCE".into(),
            ordinal: 0,
        },
    )
    .unwrap_or(JValue::Null)
}

// ---------------------------------------------------------------------------
// HTTP bridge
// ---------------------------------------------------------------------------

fn form_body_to_string(vm: &mut Vm, body: &Option<JValue>) -> Option<String> {
    let Some(JValue::Obj(id)) = body.as_ref() else {
        return None;
    };
    let o = vm.arena.get(*id)?;
    match o.native.as_ref()? {
        Native::FormBody(fields) => Some(
            fields
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("&"),
        ),
        _ => None,
    }
}

fn request_parts(vm: &mut Vm, v: JValue) -> Result<(String, String, Vec<(String, String)>, Option<JValue>), NatErr> {
    let Some(Native::Request { url, method, headers, body }) = payload(vm, v) else {
        return Err(npe(vm));
    };
    Ok((url.clone(), method.clone(), headers.clone(), body.clone()))
}

/// Bridge entry: executes the request through the registered HTTP callback
/// and builds an `okhttp3.Response` object for the extension to parse.
pub(crate) fn keiyoushi_execute(vm: &mut Vm, args: &[JValue]) -> R {
    let (url, method, headers, body) = request_parts(vm, args[0])?;
    let body_str = form_body_to_string(vm, &body);
    let Some(http) = vm.http.clone() else {
        return Err(uoe(vm, "no HTTP client registered for this SourceEngine"));
    };
    let resp = http(&HttpData {
        url,
        method,
        headers,
        body: body_str,
    });
    alloc(
        vm,
        RESPONSE,
        Native::Response {
            code: resp.code,
            message: resp.message,
            headers: resp.headers,
            body: resp.body,
            request: args[0],
        },
    )
}

/// Host callback that returns the stored `lazy_http`? (unused, kept for docs)
#[allow(dead_code)]
pub(crate) fn _http_client(vm: &mut Vm) -> Option<Rc<dyn Fn(&HttpData) -> HttpResp>> {
    vm.http.clone()
}

// ---------------------------------------------------------------------------
// okhttp3: Request / Request$Builder / Headers / Cookie / Response
// ---------------------------------------------------------------------------

pub(crate) fn request_builder_init(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lokhttp3/Request$Builder;",
        Native::RequestBuilder {
            url: String::new(),
            method: "GET".into(),
            headers: Vec::new(),
            body: None,
        },
    )
}

pub(crate) fn request_builder_url(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match jstr(vm, args[1]) {
        Ok(s) => s,
        Err(_) => return Err(npe(vm)),
    };
    let Some(Native::RequestBuilder { url, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *url = s;
    Ok(args[0])
}

pub(crate) fn request_builder_method(vm: &mut Vm, args: &[JValue]) -> R {
    let m = match jstr(vm, args[1]) {
        Ok(s) => s,
        Err(_) => return Err(npe(vm)),
    };
    let Some(Native::RequestBuilder { method, body, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *method = m.to_uppercase();
    *body = if m.eq_ignore_ascii_case("GET") {
        None
    } else {
        Some(args[2])
    };
    Ok(args[0])
}

fn builder_set_header(vm: &mut Vm, args: &[JValue], replace: bool) -> R {
    let (n, v) = match (jstr(vm, args[1]), jstr(vm, args[2])) {
        (Ok(n), Ok(v)) => (n, v),
        _ => return Err(npe(vm)),
    };
    let Some(Native::RequestBuilder { headers, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if replace {
        headers.retain(|(k, _)| !k.eq_ignore_ascii_case(&n));
    }
    headers.push((n, v));
    Ok(args[0])
}

pub(crate) fn request_builder_header(vm: &mut Vm, args: &[JValue]) -> R {
    builder_set_header(vm, args, true)
}

pub(crate) fn request_builder_add_header(vm: &mut Vm, args: &[JValue]) -> R {
    builder_set_header(vm, args, false)
}

pub(crate) fn request_builder_tag(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn request_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let (url, method, headers, body) = match payload(vm, args[0]) {
        Some(Native::RequestBuilder { url, method, headers, body }) => {
            (url.clone(), method.clone(), headers.clone(), body.clone())
        }
        _ => return Err(npe(vm)),
    };
    alloc(
        vm,
        REQUEST,
        Native::Request {
            url,
            method,
            headers,
            body,
        },
    )
}

pub(crate) fn request_url(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Request { url, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(vm, HTTP_URL, Native::HttpUrl(url.clone()))
}

pub(crate) fn request_method(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Request { method, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let m = method.clone();
    Ok(vm.alloc_string(&m))
}

pub(crate) fn request_header(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = jstr(vm, args[1]).ok() else {
        return Err(npe(vm));
    };
    let Some(Native::Request { headers, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let v = headers
        .iter()
        .rev()
        .find(|(k, _)| k.eq_ignore_ascii_case(&n))
        .map(|(_, v)| v.clone())
        .map(|v| vm.alloc_string(&v));
    Ok(v.unwrap_or(JValue::Null))
}

pub(crate) fn request_new_builder(vm: &mut Vm, args: &[JValue]) -> R {
    let (url, method, headers, body) = match payload(vm, args[0]) {
        Some(Native::Request { url, method, headers, body }) => {
            (url.clone(), method.clone(), headers.clone(), body.clone())
        }
        _ => return Err(npe(vm)),
    };
    alloc(
        vm,
        "Lokhttp3/Request$Builder;",
        Native::RequestBuilder {
            url,
            method,
            headers,
            body,
        },
    )
}

pub(crate) fn request_tag(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn headers_builder_init(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lokhttp3/Headers$Builder;", Native::Headers(Vec::new()))
}

pub(crate) fn headers_builder_add(vm: &mut Vm, args: &[JValue]) -> R {
    let (n, s) = match (jstr(vm, args[1]), jstr(vm, args[2])) {
        (Ok(n), Ok(v)) => (n, v),
        _ => return Err(npe(vm)),
    };
    let Some(Native::Headers(headers)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    headers.push((n, s));
    Ok(args[0])
}

pub(crate) fn headers_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Headers(headers)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(vm, HEADERS, Native::Headers(headers.clone()))
}

pub(crate) fn headers_size(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Headers(headers)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(headers.len() as i32))
}

pub(crate) fn headers_get(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = jstr(vm, args[1]).ok() else {
        return Ok(JValue::Null);
    };
    let Some(Native::Headers(headers)) = payload(vm, args[0]) else {
        return Ok(JValue::Null);
    };
    let v = headers
        .iter()
        .rev()
        .find(|(k, _)| k.eq_ignore_ascii_case(&n))
        .map(|(_, v)| v.clone())
        .map(|v| vm.alloc_string(&v));
    Ok(v.unwrap_or(JValue::Null))
}

pub(crate) fn headers_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Headers(headers)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let s = headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(vm.alloc_string(&s))
}

pub(crate) fn cookie_companion_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let raw = match jstr(vm, args[2]) {
        Ok(s) => s,
        Err(_) => return Ok(JValue::Null),
    };
    let (name, value) = match raw.split_once('=') {
        Some((n, v)) => (n.trim().to_string(), v.trim().to_string()),
        None => (String::new(), raw),
    };
    alloc(vm, "Lokhttp3/Cookie;", Native::Cookie { name, value })
}

pub(crate) fn response_code(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Response { code, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(*code))
}

pub(crate) fn response_message(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Response { message, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let m = message.clone();
    Ok(vm.alloc_string(&m))
}

pub(crate) fn response_is_successful(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Response { code, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from((200..300).contains(code))))
}

pub(crate) fn response_headers(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Response { headers, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(vm, HEADERS, Native::Headers(headers.clone()))
}

pub(crate) fn response_header(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = jstr(vm, args[1]).ok() else {
        return Ok(JValue::Null);
    };
    let Some(Native::Response { headers, .. }) = payload(vm, args[0]) else {
        return Ok(JValue::Null);
    };
    let v = headers
        .iter()
        .rev()
        .find(|(k, _)| k.eq_ignore_ascii_case(&n))
        .map(|(_, v)| v.clone())
        .map(|v| vm.alloc_string(&v));
    Ok(v.unwrap_or(JValue::Null))
}

pub(crate) fn response_header_default(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = jstr(vm, args[1]).ok() else {
        return Ok(args[2]);
    };
    let Some(Native::Response { headers, .. }) = payload(vm, args[0]) else {
        return Ok(args[2]);
    };
    let v = headers
        .iter()
        .rev()
        .find(|(k, _)| k.eq_ignore_ascii_case(&n))
        .map(|(_, v)| v.clone())
        .map(|v| vm.alloc_string(&v));
    Ok(v.unwrap_or(args[2]))
}

pub(crate) fn response_body(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Response { body, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(vm, "Lokhttp3/ResponseBody;", Native::Str(body.clone()))
}

pub(crate) fn response_request(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Response { request, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(*request)
}

pub(crate) fn response_close(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn response_body_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match jstr(vm, args[0]) {
        Ok(s) => s,
        Err(_) => return Err(npe(vm)),
    };
    Ok(vm.alloc_string(&s))
}

pub(crate) fn http_url_host(vm: &mut Vm, args: &[JValue]) -> R {
    let url = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Err(npe(vm)),
    };
    let host = url.split("://").nth(1).and_then(|r| r.split(['/', '?']).next()).unwrap_or("");
    let h = host.to_string();
    Ok(vm.alloc_string(&h))
}

pub(crate) fn http_url_scheme(vm: &mut Vm, args: &[JValue]) -> R {
    let url = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Err(npe(vm)),
    };
    let scheme = url.split("://").next().unwrap_or("");
    let s = scheme.to_string();
    Ok(vm.alloc_string(&s))
}

pub(crate) fn http_url_query_parameter(vm: &mut Vm, args: &[JValue]) -> R {
    let url = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Ok(JValue::Null),
    };
    let Some(name) = jstr(vm, args[1]).ok() else {
        return Ok(JValue::Null);
    };
    let query = url.split('?').nth(1).unwrap_or("");
    let mut out = None;
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == &name {
                out = Some(v.to_string());
                break;
            }
        }
    }
    Ok(out.map(|s| vm.alloc_string(&s)).unwrap_or(JValue::Null))
}

pub(crate) fn http_url_path_segments(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::HttpUrl(url)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let path = url.split("://").nth(1).and_then(|s| s.split(['?', '#']).next()).unwrap_or("");
    let path_owned = path.to_string();
    let segments = path_owned
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| vm.alloc_string(s))
        .collect::<Vec<_>>();
    collections::list_alloc(vm, segments)
}

pub(crate) fn http_url_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(vm.alloc_string(&s))
}

// ---------------------------------------------------------------------------
// eu.kanade.tachiyomi.source.online.HttpSource defaults
// ---------------------------------------------------------------------------

pub(crate) fn http_source_get_base_url(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(vm.alloc_string("https://api.akuma.moe"))
}

pub(crate) fn http_source_get_headers_default(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, HEADERS, Native::Headers(Vec::new()))
}

pub(crate) fn http_source_headers_builder(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lokhttp3/Headers$Builder;", Native::Headers(Vec::new()))
}

pub(crate) fn http_source_get_lang(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(vm.alloc_string("all"))
}

pub(crate) fn http_source_get_name(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(vm.alloc_string("Akuma"))
}

pub(crate) fn http_source_get_id(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Long(0))
}

pub(crate) fn http_source_get_supports_latest(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(1))
}

fn set_url_relative(p: &mut Native, url: &str, base: &str) {
    let target = match p {
        Native::SManga { url: u, .. } | Native::SChapter { url: u, .. } => u,
        _ => return,
    };
    *target = if url.starts_with(base) {
        url[base.len()..].to_string()
    } else {
        url.to_string()
    };
}

pub(crate) fn http_source_set_url_no_domain_manga(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = jstr(vm, args[2]).ok() else {
        return Ok(JValue::Null);
    };
    let Some(n) = payload_mut(vm, args[1]) else {
        return Err(npe(vm));
    };
    set_url_relative(n, &url, "https://api.akuma.moe");
    Ok(JValue::Null)
}

pub(crate) fn http_source_set_url_no_domain_chapter(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = jstr(vm, args[2]).ok() else {
        return Ok(JValue::Null);
    };
    let Some(n) = payload_mut(vm, args[1]) else {
        return Err(npe(vm));
    };
    set_url_relative(n, &url, "https://api.akuma.moe");
    Ok(JValue::Null)
}

/// Host default for `mangaDetailsRequest` when the extension does not
/// override it: `GET baseUrl + manga.url`.
pub(crate) fn http_source_manga_details_request(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = jstr(vm, args[1]).ok() else {
        return Err(npe(vm));
    };
    let full = if url.starts_with("http") {
        url
    } else {
        format!("https://api.akuma.moe{url}")
    };
    alloc(
        vm,
        REQUEST,
        Native::Request {
            url: full,
            method: "GET".into(),
            headers: Vec::new(),
            body: None,
        },
    )
}

pub(crate) fn http_source_chapter_list_request(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = jstr(vm, args[1]).ok() else {
        return Err(npe(vm));
    };
    let full = if url.starts_with("http") {
        url
    } else {
        format!("https://api.akuma.moe{url}")
    };
    alloc(
        vm,
        REQUEST,
        Native::Request {
            url: full,
            method: "GET".into(),
            headers: Vec::new(),
            body: None,
        },
    )
}

pub(crate) fn http_source_page_list_request(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = jstr(vm, args[1]).ok() else {
        return Err(npe(vm));
    };
    let full = if url.starts_with("http") {
        url
    } else {
        format!("https://api.akuma.moe{url}")
    };
    alloc(
        vm,
        REQUEST,
        Native::Request {
            url: full,
            method: "GET".into(),
            headers: Vec::new(),
            body: None,
        },
    )
}

// ---------------------------------------------------------------------------
// eu.kanade.tachiyomi.source.model.*
// ---------------------------------------------------------------------------

pub(crate) fn smanga_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn smanga_companion_create(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, SMANGA, empty_smanga())
}

pub(crate) fn empty_smanga() -> Native {
    Native::SManga {
        title: String::new(),
        author: None,
        artist: None,
        description: None,
        genre: None,
        status: 0,
        thumbnail_url: String::new(),
        url: String::new(),
        update_strategy: JValue::Null,
    }
}

macro_rules! sm_get_field {
    ($fname:ident, $field:ident) => {
        pub(crate) fn $fname(vm: &mut Vm, args: &[JValue]) -> R {
            match payload(vm, args[0]) {
                Some(Native::SManga { $field, .. }) => {
                    let v = $field.clone();
                    Ok(vm.alloc_string(&v))
                }
                _ => Err(npe(vm)),
            }
        }
    };
}

macro_rules! sm_set_field {
    ($fname:ident, $field:ident) => {
        pub(crate) fn $fname(vm: &mut Vm, args: &[JValue]) -> R {
            let s = match jstr(vm, args[1]) {
                Ok(s) => s,
                Err(_) => String::new(),
            };
            match payload_mut(vm, args[0]) {
                Some(Native::SManga { $field, .. }) => {
                    *$field = s;
                }
                _ => return Err(npe(vm)),
            }
            Ok(JValue::Null)
        }
    };
}

sm_get_field!(smanga_get_title, title);
sm_set_field!(smanga_set_title, title);
macro_rules! sm_get_field_opt {
    ($fname:ident, $field:ident) => {
        pub(crate) fn $fname(vm: &mut Vm, args: &[JValue]) -> R {
            match payload(vm, args[0]) {
                Some(Native::SManga { $field, .. }) => {
                    let v = $field.clone().unwrap_or_default();
                    Ok(vm.alloc_string(&v))
                }
                _ => Err(npe(vm)),
            }
        }
    };
}

macro_rules! sm_set_field_opt {
    ($fname:ident, $field:ident) => {
        pub(crate) fn $fname(vm: &mut Vm, args: &[JValue]) -> R {
            let s = match jstr(vm, args[1]) {
                Ok(s) => s,
                Err(_) => String::new(),
            };
            match payload_mut(vm, args[0]) {
                Some(Native::SManga { $field, .. }) => {
                    *$field = Some(s);
                }
                _ => return Err(npe(vm)),
            }
            Ok(JValue::Null)
        }
    };
}

sm_get_field_opt!(smanga_get_author, author);
sm_set_field_opt!(smanga_set_author, author);
sm_get_field_opt!(smanga_get_artist, artist);
sm_set_field_opt!(smanga_set_artist, artist);
sm_get_field_opt!(smanga_get_description, description);
sm_set_field_opt!(smanga_set_description, description);
sm_get_field_opt!(smanga_get_genre, genre);
sm_set_field_opt!(smanga_set_genre, genre);
sm_get_field!(smanga_get_thumbnail_url, thumbnail_url);
sm_set_field!(smanga_set_thumbnail_url, thumbnail_url);
sm_get_field!(smanga_get_url, url);
sm_set_field!(smanga_set_url, url);

pub(crate) fn smanga_get_status(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SManga { status, .. }) => Ok(JValue::Int(*status)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn smanga_set_status(vm: &mut Vm, args: &[JValue]) -> R {
    let state = int_of(vm, args[1]);
    match payload_mut(vm, args[0]) {
        Some(Native::SManga { status, .. }) => *status = state,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn smanga_get_update_strategy(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SManga { update_strategy, .. }) => Ok(*update_strategy),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn smanga_set_update_strategy(vm: &mut Vm, args: &[JValue]) -> R {
    match payload_mut(vm, args[0]) {
        Some(Native::SManga { update_strategy, .. }) => *update_strategy = args[1],
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

// ---- SChapter ----

pub(crate) fn empty_schapter() -> Native {
    Native::SChapter {
        name: String::new(),
        url: String::new(),
        date_upload: 0,
        scanlator: String::new(),
    }
}

pub(crate) fn schapter_init(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, SCHAPTER, empty_schapter())
}

pub(crate) fn schapter_companion_create(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, SCHAPTER, empty_schapter())
}

macro_rules! sc_get_field {
    ($fname:ident, $field:ident) => {
        pub(crate) fn $fname(vm: &mut Vm, args: &[JValue]) -> R {
            match payload(vm, args[0]) {
                Some(Native::SChapter { $field, .. }) => {
                    let v = $field.clone();
                    Ok(vm.alloc_string(&v))
                }
                _ => Err(npe(vm)),
            }
        }
    };
}

macro_rules! sc_set_field {
    ($fname:ident, $field:ident) => {
        pub(crate) fn $fname(vm: &mut Vm, args: &[JValue]) -> R {
            let s = match jstr(vm, args[1]) {
                Ok(s) => s,
                Err(_) => String::new(),
            };
            match payload_mut(vm, args[0]) {
                Some(Native::SChapter { $field, .. }) => *$field = s,
                _ => return Err(npe(vm)),
            }
            Ok(JValue::Null)
        }
    };
}

sc_get_field!(schapter_get_name, name);
sc_set_field!(schapter_set_name, name);
sc_get_field!(schapter_get_url, url);
sc_set_field!(schapter_set_url, url);
sc_get_field!(schapter_get_scanlator, scanlator);
sc_set_field!(schapter_set_scanlator, scanlator);

pub(crate) fn schapter_get_date_upload(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SChapter { date_upload, .. }) => Ok(JValue::Long(*date_upload)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn schapter_set_date_upload(vm: &mut Vm, args: &[JValue]) -> R {
    let v = long_of(vm, args[1]);
    match payload_mut(vm, args[0]) {
        Some(Native::SChapter { date_upload, .. }) => *date_upload = v,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

// ---- Page ----

pub(crate) fn page_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[2]).unwrap_or_default();
    let url = jstr(vm, args[3]).unwrap_or_default();
    let image_url = jstr(vm, args[4]).unwrap_or_default();
    alloc(
        vm,
        PAGE,
        Native::SPPage {
            index: int_of(vm, args[1]),
            name,
            url,
            image_url,
        },
    )
}

pub(crate) fn page_get_url(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SPPage { url, .. }) => {
            let v = url.clone();
            Ok(vm.alloc_string(&v))
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn page_get_name(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SPPage { name, .. }) => {
            let v = name.clone();
            Ok(vm.alloc_string(&v))
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn page_get_image_url(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SPPage { image_url, .. }) => {
            let v = image_url.clone();
            Ok(vm.alloc_string(&v))
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn page_get_index(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SPPage { index, .. }) => Ok(JValue::Int(*index)),
        _ => Err(npe(vm)),
    }
}

// ---- MangasPage / FilterList ----

pub(crate) fn mangas_page_init(vm: &mut Vm, args: &[JValue]) -> R {
    let mangas = match payload(vm, args[1]) {
        Some(Native::List(items)) => items.clone(),
        _ => Vec::new(),
    };
    let has_next = bool_of(vm, args[2]);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::SMangasPage { mangas: dst, has_next: dst_next } => {
            *dst = mangas;
            *dst_next = has_next;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn mangas_page_get_mangas(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::SMangasPage { mangas, .. }) => mangas.clone(),
        _ => return Err(npe(vm)),
    };
    collections::list_alloc(vm, items)
}

pub(crate) fn mangas_page_has_next(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SMangasPage { has_next, .. }) => Ok(JValue::Int(i32::from(*has_next))),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn filter_list_init(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[1]) {
        Some(Native::Array(data)) => {
            let mut v = Vec::new();
            for i in 0..data.len() {
                v.push(data.get(i));
            }
            v
        }
        _ => Vec::new(),
    };
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::SFilterList(dst) => *dst = items,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn filter_list_get_filters(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::SFilterList(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    collections::list_alloc(vm, items)
}

// ---- Filter hierarchy ----

pub(crate) fn filter_new(name: String, is_checked: bool, state: i32) -> Native {
    Native::SFilter {
        name,
        state,
        is_checked,
        children: Vec::new(),
        options: Vec::new(),
        text_value: String::new(),
    }
}

pub(crate) fn filter_init_name(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1]).unwrap_or_default();
    set_filter_payload(vm, args[0], filter_new(name, false, 0))
}

pub(crate) fn filter_init_checked(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1]).unwrap_or_default();
    let checked = bool_of(vm, args[2]);
    set_filter_payload(vm, args[0], filter_new(name, checked, 0))
}

pub(crate) fn filter_header_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1]).unwrap_or_default();
    set_filter_payload(vm, args[0], filter_new(name, false, 0))
}

pub(crate) fn filter_separator_init(vm: &mut Vm, args: &[JValue]) -> R {
    set_filter_payload(vm, args[0], filter_new(String::new(), false, 0))
}

pub(crate) fn filter_text_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1]).unwrap_or_default();
    let default = jstr(vm, args[2]).unwrap_or_default();
    let f = Native::SFilter {
        name,
        state: 0,
        is_checked: false,
        children: Vec::new(),
        options: Vec::new(),
        text_value: default,
    };
    set_filter_payload(vm, args[0], f)
}

pub(crate) fn filter_tristate_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1]).unwrap_or_default();
    let state = int_of(vm, args[2]);
    set_filter_payload(vm, args[0], filter_new(name, false, state))
}

pub(crate) fn filter_select_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1]).unwrap_or_default();
    let options = match payload(vm, args[2]) {
        Some(Native::Array(data)) => {
            let mut v = Vec::new();
            for i in 0..data.len() {
                v.push(data.get(i));
            }
            v
        }
        _ => Vec::new(),
    };
    let state = int_of(vm, args[3]);
    let f = Native::SFilter {
        name,
        state,
        is_checked: false,
        children: Vec::new(),
        options,
        text_value: String::new(),
    };
    set_filter_payload(vm, args[0], f)
}

pub(crate) fn filter_group_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1]).unwrap_or_default();
    let children = match payload(vm, args[2]) {
        Some(Native::List(items)) => items.clone(),
        _ => Vec::new(),
    };
    let f = Native::SFilter {
        name,
        state: 0,
        is_checked: false,
        children,
        options: Vec::new(),
        text_value: String::new(),
    };
    set_filter_payload(vm, args[0], f)
}

fn set_filter_payload(vm: &mut Vm, v: JValue, f: Native) -> R {
    let Some(n) = payload_mut(vm, v) else {
        return Err(npe(vm));
    };
    match (n, f) {
        (Native::SFilter { name, state, is_checked, children, options, text_value },
         Native::SFilter { name: n2, state: st2, is_checked: ic2, children: ch2, options: op2, text_value: tv2 }) => {
            *name = n2;
            *state = st2;
            *is_checked = ic2;
            *children = ch2;
            *options = op2;
            *text_value = tv2;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn filter_get_name(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SFilter { name, .. }) => {
            let v = name.clone();
            Ok(vm.alloc_string(&v))
        }
        _ => Err(npe(vm)),
    }
}
pub(crate) fn filter_get_state(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SFilter { state, .. }) => Ok(JValue::Int(*state)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn filter_set_state(vm: &mut Vm, args: &[JValue]) -> R {
    let state = int_of(vm, args[1]);
    match payload_mut(vm, args[0]) {
        Some(Native::SFilter { state: st, .. }) => *st = state,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn filter_select_state_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let (state, options) = match payload(vm, args[0]) {
        Some(Native::SFilter { state, options, .. }) => (*state, options.clone()),
        _ => return Ok(JValue::Null),
    };
    Ok(options.get(state as usize).copied().unwrap_or(JValue::Null))
}

pub(crate) fn filter_text_state_obj(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SFilter { text_value, .. }) => {
            let v = text_value.clone();
            Ok(vm.alloc_string(&v))
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn filter_group_state_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let children = match payload(vm, args[0]) {
        Some(Native::SFilter { children, .. }) => children.clone(),
        _ => return Ok(JValue::Null),
    };
    for c in &children {
        match payload(vm, *c) {
            Some(Native::SFilter { state, is_checked, .. }) => {
                if *state != 0 || *is_checked {
                    return Ok(*c);
                }
            }
            _ => return Ok(JValue::Null),
        }
    }
    Ok(JValue::Null)
}

pub(crate) fn filter_tristate_is_excluded(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SFilter { state, .. }) => Ok(JValue::Int(i32::from(state == &-1))),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn filter_tristate_is_included(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SFilter { state, .. }) => Ok(JValue::Int(i32::from(state == &1))),
        _ => Err(npe(vm)),
    }
}

// ---------------------------------------------------------------------------
// org.jsoup on top of dom_query (mirrors rakuyomi's html_element.rs)
// ---------------------------------------------------------------------------

fn select_selector(selector: &str) -> Option<Matcher> {
    Matcher::new(&normalize_contains(selector)).ok()
}

/// jsoup `:contains(...)` is not W3C; quote the argument inside `:contains`.
fn normalize_contains(selector: &str) -> String {
    let mut out = String::with_capacity(selector.len());
    let chars: Vec<char> = selector.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i..].starts_with(&[':', 'c', 'o', 'n', 't', 'a', 'i', 'n', 's', '(']) {
            out.push_str(":contains(");
            i += 10;
            let mut inner = String::new();
            let mut depth = 1;
            while i < chars.len() && depth > 0 {
                let c = chars[i];
                if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                inner.push(c);
                i += 1;
            }
            let inner_trim = inner.trim();
            if inner_trim.starts_with('"') && inner_trim.ends_with('"') {
                out.push_str(inner_trim);
            } else {
                out.push('"');
                out.push_str(inner_trim);
                out.push('"');
            }
            out.push(')');
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn node_ref_of<'a>(doc: &'a Document, id: NodeId) -> NodeRef<'a> {
    NodeRef::new(id, &doc.tree)
}

/// jsoup element().select: includes the node itself when it matches
/// (mirroring rakuyomi's compatibility layer).
fn select_matches(_doc: &Document, root: NodeRef, selector: &str) -> Vec<NodeId> {
    let Some(matcher) = select_selector(selector) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if matcher.match_element(&root) {
        out.push(root.id);
    }
    out.extend(
        Selection::from(root)
            .select_matcher(&matcher)
            .nodes()
            .iter()
            .map(|n| n.id),
    );
    out
}

/// jsoup Element.text(): descendant text with script/style content dropped,
/// each text node trimmed and joined by a single space.
fn soup_text(node: NodeRef) -> String {
    fn collect(node: NodeRef, out: &mut Vec<String>) {
        if node.is_text() {
            out.push(node.text().to_string());
            return;
        }
        if let Some(tag) = node.node_name() {
            if tag.eq_ignore_ascii_case("script") || tag.eq_ignore_ascii_case("style") {
                return;
            }
        }
        for child in node.children() {
            collect(child, out);
        }
    }
    let mut parts = Vec::new();
    collect(node, &mut parts);
    parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn jsoup_elements(vm: &mut Vm, doc: JsoupDocRef, ids: Vec<NodeId>) -> R {
    alloc(
        vm,
        "Lorg/jsoup/select/Elements;",
        Native::JsoupElements { doc, ids },
    )
}

pub(crate) fn jsoup_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Response { body, request, .. }) = payload(vm, args[0]) else {
        return Err(uoe(vm, "asJsoup: response body missing"));
    };
    let doc = Document::from(body.as_str());
    let base = match payload(vm, *request) {
        Some(Native::Request { url, .. }) => Some(url.clone()),
        _ => None,
    };
    let mut refd = JsoupDocRef::new(doc);
    refd.base = base;
    alloc(vm, "Lorg/jsoup/nodes/Document;", Native::JsoupDoc(refd))
}



fn jsoup_first_selector_arg(vm: &mut Vm, args: &[JValue]) -> Result<String, NatErr> {
    jstr(vm, args[1]).map_err(|_| npe(vm))
}

// kotlin.text.StringsKt synthetic default-arg shims (mask bit 2 = ignoreCase default false)
fn stringskt_contains_default(vm: &mut Vm, args: &[JValue]) -> R {
    let haystack = charseq_of(vm, args[0])?;
    let needle = charseq_of(vm, args[1])?;
    let ignore = args[2].as_int() != 0;
    let ignore_case = if args[3].as_int() & 2 != 0 { false } else { ignore };
    let found = if ignore_case {
        haystack.to_lowercase().contains(&needle.to_lowercase())
    } else {
        haystack.contains(&needle)
    };
    Ok(JValue::Int(found as i32))
}

fn stringskt_replace_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    let from = charseq_of(vm, args[1])?;
    let to = charseq_of(vm, args[2])?;
    let ignore = args[3].as_int() != 0;
    let ignore_case = if args[4].as_int() & 4 != 0 { false } else { ignore };
    let r = if ignore_case {
        regex_replace_case_insensitive(&s, &from, &to)
    } else {
        s.replace(&from, &to)
    };
    alloc(vm, "Ljava/lang/String;", Native::Str(r))
}

fn stringskt_trim(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    alloc(vm, "Ljava/lang/String;", Native::Str(s.trim().to_string()))
}

fn regex_replace_case_insensitive(s: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(idx) = rest.to_lowercase().find(&from.to_lowercase()) {
        out.push_str(&rest[..idx]);
        out.push_str(to);
        rest = &rest[idx + from.len()..];
    }
    out.push_str(rest);
    out
}

fn doc_of(vm: &mut Vm, v: JValue) -> Result<JsoupDocRef, NatErr> {
    let Some(n) = payload(vm, v) else {
        return Err(npe(vm));
    };
    let Some(doc) = doc_of_payload(n) else {
        return Err(npe(vm));
    };
    Ok(doc)
}

pub(crate) fn doc_of_payload(n: &Native) -> Option<JsoupDocRef> {
    match n {
        Native::JsoupDoc(doc) => Some(doc.clone()),
        Native::JsoupElement { doc, .. } | Native::JsoupElements { doc, .. } => Some(doc.clone()),
        _ => None,
    }
}

pub(crate) fn element_id_of(n: &Native) -> Option<NodeId> {
    match n {
        Native::JsoupElement { id, .. } => Some(*id),
        _ => None,
    }
}

pub(crate) fn document_select(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, selector) = (doc_of(vm, args[0])?, jsoup_first_selector_arg(vm, args)?);
    let ids = {
        let d = &*doc.doc;
        let root = d.root();
        select_matches(d, root, &selector)
    };
    jsoup_elements(vm, doc, ids)
}

pub(crate) fn document_text(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc,) = (doc_of(vm, args[0])?,);
    let root = doc.doc.root();
    let text = soup_text(root);
    Ok(vm.alloc_string(&text))
}

pub(crate) fn element_select(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, selector) = (
        doc_of(vm, args[0])?,
        jsoup_first_selector_arg(vm, args)?,
    );
    let node_payload = payload(vm, args[0]);
    let Some(node_id) = node_payload.and_then(element_id_of) else {
        return Err(npe(vm));
    };
    let ids = {
        let d = &*doc.doc;
        let node = NodeRef::new(node_id, &d.tree);
        select_matches(d, node, &selector)
    };
    jsoup_elements(vm, doc, ids)
}

pub(crate) fn element_select_first(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let node_payload = payload(vm, args[0]);
    let Some(node) = node_payload.and_then(element_id_of) else {
        return Err(npe(vm));
    };
    let selector = jsoup_first_selector_arg(vm, args)?;
    let out_id = {
        let d = &*doc.doc;
        let node_ref = NodeRef::new(node, &d.tree);
        let Some(matcher) = select_selector(&selector) else {
            return Ok(JValue::Null);
        };
        if matcher.match_element(&node_ref) {
            Some(node)
        } else {
            Selection::from(node_ref)
                .select_single_matcher(&matcher)
                .nodes()
                .first()
                .map(|n| n.id)
        }
    };
    match out_id {
        Some(id) => alloc(vm, "Lorg/jsoup/nodes/Element;", Native::JsoupElement { doc, id }),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn element_text(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(vm.alloc_string(&soup_text(node)))
}

pub(crate) fn element_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, name) = (doc_of(vm, args[0])?, jsoup_first_selector_arg(vm, args)?);
    let payload0 = payload(vm, args[0]);
    let id = payload0
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    let (abs, name) = match name.strip_prefix("abs:") {
        Some(n) => (true, n.to_string()),
        None => (false, name),
    };
    let mut v = node.attr(&name).unwrap_or_default().to_string();
    if abs {
        v = jsoup_abs_attr(&doc, &v);
    }
    Ok(vm.alloc_string(&v))
}

/// jsoup `abs:` attribute semantics: resolve a relative URL against the
/// document base URI (the response URL).
fn jsoup_abs_attr(doc: &JsoupDocRef, raw: &str) -> String {
    let Some(base) = doc.base.as_ref() else {
        return raw.to_string();
    };
    if raw.starts_with("http://") || raw.starts_with("https://") || raw.is_empty() {
        return raw.to_string();
    }
    if raw.starts_with("//") {
        let scheme = base.split("://").next().unwrap_or("http");
        return format!("{scheme}:{raw}");
    }
    if raw.starts_with('/') {
        let end = base.find("://").map(|i| i + 3).unwrap_or(0);
        let host_end = base[end..].find('/').map(|i| end + i).unwrap_or(base.len());
        return format!("{}{raw}", &base[..host_end]);
    }
    match base.rfind('/') {
        Some(i) if i > base.find("://").unwrap_or(usize::MAX).saturating_add(2) => {
            format!("{}{raw}", &base[..=i])
        }
        _ => format!("{base}/{raw}"),
    }
}

pub(crate) fn element_has_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, name) = (doc_of(vm, args[0])?, jsoup_first_selector_arg(vm, args)?);
    let payload0 = payload(vm, args[0]);
    let id = payload0
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(JValue::Int(i32::from(node.has_attr(&name))))
}

pub(crate) fn element_id_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(vm.alloc_string(&node.attr("id").unwrap_or_default()))
}

pub(crate) fn element_tag_name(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    let tag = node.node_name().unwrap_or_default().to_lowercase();
    Ok(vm.alloc_string(&tag))
}

pub(crate) fn element_own_text(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(vm.alloc_string(&node.immediate_text().to_string()))
}

pub(crate) fn element_html(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(vm.alloc_string(&node.inner_html().to_string()))
}

pub(crate) fn element_outer_html(vm: &mut Vm, args: &[JValue]) -> R {
    let doc = doc_of(vm, args[0])?;
    let payload0 = payload(vm, args[0]);
    let id = payload0
        .and_then(element_id_of)
        .ok_or_else(|| npe(vm))?;
    let d = &*doc.doc;
    let node = node_ref_of(d, id);
    Ok(vm.alloc_string(&node.html().to_string()))
}

pub(crate) fn elements_first(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    match ids.first() {
        Some(id) => alloc(vm, "Lorg/jsoup/nodes/Element;", Native::JsoupElement { doc, id: *id }),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn elements_get(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let idx = int_of(vm, args[1]);
    match ids.get(idx as usize) {
        Some(id) => alloc(vm, "Lorg/jsoup/nodes/Element;", Native::JsoupElement { doc, id: *id }),
        None => Err(aioobe(vm, idx, ids.len() as i32)),
    }
}

pub(crate) fn elements_size(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::JsoupElements { ids, .. }) => Ok(JValue::Int(ids.len() as i32)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn elements_is_empty(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::JsoupElements { ids, .. }) => Ok(JValue::Int(i32::from(ids.is_empty()))),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn elements_text(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let d = &*doc.doc;
    let mut parts = Vec::new();
    for id in &ids {
        parts.push(soup_text(node_ref_of(d, *id)));
    }
    let s = parts
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(vm.alloc_string(&s))
}

pub(crate) fn elements_each_text(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let d = &*doc.doc;
    let items = ids
        .iter()
        .map(|id| vm.alloc_string(&soup_text(node_ref_of(d, *id))))
        .collect();
    collections::list_alloc(vm, items)
}

pub(crate) fn elements_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let name = jsoup_first_selector_arg(vm, args)?;
    let (abs, name) = match name.strip_prefix("abs:") {
        Some(n) => (true, n.to_string()),
        None => (false, name),
    };
    let d = &*doc.doc;
    let mut value = String::new();
    for id in &ids {
        if let Some(v) = node_ref_of(d, *id).attr(&name) {
            value = v.to_string();
            break;
        }
    }
    if abs {
        value = jsoup_abs_attr(&doc, &value);
    }
    Ok(vm.alloc_string(&value))
}

pub(crate) fn elements_each_attr(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let name = jsoup_first_selector_arg(vm, args)?;
    let (abs, name) = match name.strip_prefix("abs:") {
        Some(n) => (true, n.to_string()),
        None => (false, name),
    };
    let d = &*doc.doc;
    let items = ids
        .iter()
        .map(|id| {
            let mut v = node_ref_of(d, *id)
                .attr(&name)
                .unwrap_or_default()
                .to_string();
            if abs {
                v = jsoup_abs_attr(&doc, &v);
            }
            vm.alloc_string(&v)
        })
        .collect();
    collections::list_alloc(vm, items)
}

pub(crate) fn elements_select(vm: &mut Vm, args: &[JValue]) -> R {
    let (doc, ids) = match payload(vm, args[0]) {
        Some(Native::JsoupElements { doc, ids }) => (doc.clone(), ids.clone()),
        _ => return Err(npe(vm)),
    };
    let selector = jsoup_first_selector_arg(vm, args)?;
    let d = &*doc.doc;
    let mut out = Vec::new();
    for id in &ids {
        out.extend(select_matches(d, node_ref_of(d, *id), &selector));
    }
    jsoup_elements(vm, doc, out)
}

// ---------------------------------------------------------------------------
// kotlin.time.Duration value-class methods (host stdlib)
// ---------------------------------------------------------------------------

pub(crate) fn keiyoushi_duration_minus(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0]) - long_of(vm, args[1])))
}

pub(crate) fn keiyoushi_duration_compare(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(long_of(vm, args[0]).cmp(&long_of(vm, args[1])) as i32))
}

pub(crate) fn keiyoushi_duration_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(
        long_of(vm, args[0]) == long_of(vm, args[1]),
    )))
}

// ---------------------------------------------------------------------------
// android framework
// ---------------------------------------------------------------------------

pub(crate) fn context_get_shared_prefs(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/content/SharedPreferences;", Native::Opaque)
}

pub(crate) fn shared_prefs_get_boolean(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[2])
}

pub(crate) fn prefs_obj(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn prefs_ctx(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/content/Context;", Native::Opaque)
}

pub(crate) fn requests_kt_get_default(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = jstr(vm, args[0]).ok() else {
        return Err(npe(vm));
    };
    let headers = match payload(vm, args[1]) {
        Some(Native::Headers(h)) => h.clone(),
        _ => Vec::new(),
    };
    alloc(
        vm,
        REQUEST,
        Native::Request {
            url,
            method: "GET".into(),
            headers,
            body: None,
        },
    )
}

pub(crate) fn prefs_set(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

// ---------------------------------------------------------------------------
// KEIYOUSHI_TABLE
// ---------------------------------------------------------------------------

pub const KEIYOUSHI_TABLE: &[NativeEntry] = &[
    // ---- model classes ----
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "<init>", "()V", true, smanga_init),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "getTitle", "()Ljava/lang/String;", true, smanga_get_title),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "setTitle", "(Ljava/lang/String;)V", true, smanga_set_title),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "getAuthor", "()Ljava/lang/String;", true, smanga_get_author),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "setAuthor", "(Ljava/lang/String;)V", true, smanga_set_author),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "getArtist", "()Ljava/lang/String;", true, smanga_get_artist),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "setArtist", "(Ljava/lang/String;)V", true, smanga_set_artist),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "getDescription", "()Ljava/lang/String;", true, smanga_get_description),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "setDescription", "(Ljava/lang/String;)V", true, smanga_set_description),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "getGenre", "()Ljava/lang/String;", true, smanga_get_genre),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "setGenre", "(Ljava/lang/String;)V", true, smanga_set_genre),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "getThumbnail_url", "()Ljava/lang/String;", true, smanga_get_thumbnail_url),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "setThumbnail_url", "(Ljava/lang/String;)V", true, smanga_set_thumbnail_url),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "getUrl", "()Ljava/lang/String;", true, smanga_get_url),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "setUrl", "(Ljava/lang/String;)V", true, smanga_set_url),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "getStatus", "()I", true, smanga_get_status),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "setStatus", "(I)V", true, smanga_set_status),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "getUpdate_strategy", "()Leu/kanade/tachiyomi/source/model/UpdateStrategy;", true, smanga_get_update_strategy),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "setUpdate_strategy", "(Leu/kanade/tachiyomi/source/model/UpdateStrategy;)V", true, smanga_set_update_strategy),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "getUrlWithoutDomain", "()Ljava/lang/String;", true, smanga_get_url),
    ne!("Leu/kanade/tachiyomi/source/model/SManga$Companion;", "create", "()Leu/kanade/tachiyomi/source/model/SManga;", true, smanga_companion_create),
    ne!("Leu/kanade/tachiyomi/source/model/SManga$Companion;", "create", "()Leu/kanade/tachiyomi/source/model/SChapter;", true, smanga_companion_create),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "<init>", "()V", true, schapter_init),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "getName", "()Ljava/lang/String;", true, schapter_get_name),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "setName", "(Ljava/lang/String;)V", true, schapter_set_name),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "getUrl", "()Ljava/lang/String;", true, schapter_get_url),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "setUrl", "(Ljava/lang/String;)V", true, schapter_set_url),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "getDate_upload", "()J", true, schapter_get_date_upload),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "setDate_upload", "(J)V", true, schapter_set_date_upload),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "getScanlator", "()Ljava/lang/String;", true, schapter_get_scanlator),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "setScanlator", "(Ljava/lang/String;)V", true, schapter_set_scanlator),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter$Companion;", "create", "()Leu/kanade/tachiyomi/source/model/SChapter;", true, schapter_companion_create),
    ne!("Leu/kanade/tachiyomi/source/model/Page;", "<init>", "(ILjava/lang/String;Ljava/lang/String;Landroid/net/Uri;ILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, page_init),
    ne!("Leu/kanade/tachiyomi/source/model/Page;", "getUrl", "()Ljava/lang/String;", true, page_get_url),
    ne!("Leu/kanade/tachiyomi/source/model/Page;", "getName", "()Ljava/lang/String;", true, page_get_name),
    ne!("Leu/kanade/tachiyomi/source/model/Page;", "getImageUrl", "()Ljava/lang/String;", true, page_get_image_url),
    ne!("Leu/kanade/tachiyomi/source/model/Page;", "getIndex", "()I", true, page_get_index),
    ne!("Leu/kanade/tachiyomi/source/model/MangasPage;", "<init>", "(Ljava/util/List;Z)V", true, mangas_page_init),
    ne!("Leu/kanade/tachiyomi/source/model/MangasPage;", "getMangas", "()Ljava/util/List;", true, mangas_page_get_mangas),
    ne!("Leu/kanade/tachiyomi/source/model/MangasPage;", "hasNextPage", "()Z", true, mangas_page_has_next),
    ne!("Leu/kanade/tachiyomi/source/model/FilterList;", "<init>", "([Leu/kanade/tachiyomi/source/model/Filter;)V", true, filter_list_init),    ne!("Leu/kanade/tachiyomi/source/model/FilterList;", "getFilters", "()Ljava/util/List;", true, filter_list_get_filters),
    ne!("Leu/kanade/tachiyomi/source/model/Filter;", "<init>", "(Ljava/lang/String;)V", true, filter_init_name),
    ne!("Leu/kanade/tachiyomi/source/model/Filter;", "<init>", "(Ljava/lang/String;Z)V", true, filter_init_checked),
    ne!("Leu/kanade/tachiyomi/source/model/Filter;", "getName", "()Ljava/lang/String;", true, filter_get_name),
    ne!("Leu/kanade/tachiyomi/source/model/Filter;", "getState", "()I", true, filter_get_state),
    ne!("Leu/kanade/tachiyomi/source/model/Filter;", "setState", "(I)V", true, filter_set_state),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Header;", "<init>", "(Ljava/lang/String;)V", true, filter_header_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Header;", "<init>", "(Ljava/lang/String;IILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, filter_header_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Separator;", "<init>", "()V", true, filter_separator_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Separator;", "<init>", "(Ljava/lang/String;ILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, filter_separator_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Text;", "<init>", "(Ljava/lang/String;)V", true, filter_text_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Text;", "<init>", "(Ljava/lang/String;Ljava/lang/String;)V", true, filter_text_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Text;", "<init>", "(Ljava/lang/String;Ljava/lang/String;ILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, filter_text_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Text;", "getState", "()Ljava/lang/Object;", true, filter_text_state_obj),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$TriState;", "<init>", "(Ljava/lang/String;I)V", true, filter_tristate_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$TriState;", "<init>", "(Ljava/lang/String;IILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, filter_tristate_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$TriState;", "isExcluded", "()Z", true, filter_tristate_is_excluded),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$TriState;", "isIncluded", "()Z", true, filter_tristate_is_included),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Select;", "<init>", "(Ljava/lang/String;[Ljava/lang/String;)V", true, filter_select_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Select;", "<init>", "(Ljava/lang/String;[Ljava/lang/Object;IILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, filter_select_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Select;", "getState", "()Ljava/lang/Object;", true, filter_select_state_obj),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Group;", "<init>", "(Ljava/lang/String;Ljava/util/List;)V", true, filter_group_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Group;", "<init>", "(Ljava/lang/String;Ljava/util/List;IILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, filter_group_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Group;", "getState", "()Ljava/lang/Object;", true, filter_group_state_obj),
    // ---- HttpSource ----
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getBaseUrl", "()Ljava/lang/String;", true, http_source_get_base_url),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getHeaders", "()Lokhttp3/Headers;", true, http_source_get_headers_default),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getLang", "()Ljava/lang/String;", true, http_source_get_lang),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getName", "()Ljava/lang/String;", true, http_source_get_name),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getId", "()J", true, http_source_get_id),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getSupportsLatest", "()Z", true, http_source_get_supports_latest),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "headersBuilder", "()Lokhttp3/Headers$Builder;", true, http_source_headers_builder),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "setUrlWithoutDomain", "(Leu/kanade/tachiyomi/source/model/SManga;Ljava/lang/String;)V", true, http_source_set_url_no_domain_manga),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "setUrlWithoutDomain", "(Leu/kanade/tachiyomi/source/model/SChapter;Ljava/lang/String;)V", true, http_source_set_url_no_domain_chapter),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "mangaDetailsRequest", "(Leu/kanade/tachiyomi/source/model/SManga;)Lokhttp3/Request;", true, http_source_manga_details_request),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "chapterListRequest", "(Leu/kanade/tachiyomi/source/model/SManga;)Lokhttp3/Request;", true, http_source_chapter_list_request),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "pageListRequest", "(Leu/kanade/tachiyomi/source/model/SChapter;)Lokhttp3/Request;", true, http_source_page_list_request),
    // ---- okhttp3 ----
    ne!("Lokhttp3/Request$Builder;", "<init>", "()V", true, request_builder_init),
    ne!("Lokhttp3/Request$Builder;", "url", "(Ljava/lang/String;)Lokhttp3/Request$Builder;", true, request_builder_url),
    ne!("Lokhttp3/Request$Builder;", "method", "(Ljava/lang/String;Lokhttp3/RequestBody;)Lokhttp3/Request$Builder;", true, request_builder_method),
    ne!("Lokhttp3/Request$Builder;", "header", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/Request$Builder;", true, request_builder_header),
    ne!("Lokhttp3/Request$Builder;", "addHeader", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/Request$Builder;", true, request_builder_add_header),
    ne!("Lokhttp3/Request$Builder;", "tag", "(Ljava/lang/Class;)Lokhttp3/Request$Builder;", true, request_builder_tag),
    ne!("Lokhttp3/Request$Builder;", "build", "()Lokhttp3/Request;", true, request_builder_build),
    ne!("Lokhttp3/Request;", "newBuilder", "()Lokhttp3/Request$Builder;", true, request_new_builder),
    ne!("Lokhttp3/Request;", "url", "()Lokhttp3/HttpUrl;", true, request_url),
    ne!("Lokhttp3/Request;", "method", "()Ljava/lang/String;", true, request_method),
    ne!("Lokhttp3/Request;", "header", "(Ljava/lang/String;)Ljava/lang/String;", true, request_header),
    ne!("Lokhttp3/Request;", "tag", "(Ljava/lang/Class;)Ljava/lang/Object;", true, request_tag),
    ne!("Lokhttp3/Headers$Builder;", "<init>", "()V", true, headers_builder_init),
    ne!("Lokhttp3/Headers$Builder;", "add", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/Headers$Builder;", true, headers_builder_add),
    ne!("Lokhttp3/Headers$Builder;", "build", "()Lokhttp3/Headers;", true, headers_builder_build),
    ne!("Lokhttp3/Headers;", "size", "()I", true, headers_size),
    ne!("Lokhttp3/Headers;", "get", "(Ljava/lang/String;)Ljava/lang/String;", true, headers_get),
    ne!("Lokhttp3/Headers;", "toString", "()Ljava/lang/String;", true, headers_to_string),
    ne!("Lokhttp3/Cookie$Companion;", "parse", "(Lokhttp3/HttpUrl;Ljava/lang/String;)Lokhttp3/Cookie;", true, cookie_companion_parse),
    ne!("Lokhttp3/Response;", "code", "()I", true, response_code),
    ne!("Lokhttp3/Response;", "message", "()Ljava/lang/String;", true, response_message),
    ne!("Lokhttp3/Response;", "isSuccessful", "()Z", true, response_is_successful),
    ne!("Lokhttp3/Response;", "headers", "()Lokhttp3/Headers;", true, response_headers),
    ne!("Lokhttp3/Response;", "header", "(Ljava/lang/String;)Ljava/lang/String;", true, response_header),
    ne!("Lokhttp3/Response;", "header$default", "(Lokhttp3/Response;Ljava/lang/String;Ljava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, response_header_default),
    ne!("Lokhttp3/Response;", "body", "()Lokhttp3/ResponseBody;", true, response_body),
    ne!("Lokhttp3/Response;", "request", "()Lokhttp3/Request;", true, response_request),
    ne!("Lokhttp3/Response;", "close", "()V", true, response_close),
    ne!("Lokhttp3/ResponseBody;", "string", "()Ljava/lang/String;", true, response_body_string),
    ne!("Lokhttp3/HttpUrl;", "host", "()Ljava/lang/String;", true, http_url_host),
    ne!("Lokhttp3/HttpUrl;", "scheme", "()Ljava/lang/String;", true, http_url_scheme),
    ne!("Lokhttp3/HttpUrl;", "queryParameter", "(Ljava/lang/String;)Ljava/lang/String;", true, http_url_query_parameter),
    ne!("Lokhttp3/HttpUrl;", "pathSegments", "()Ljava/util/List;", true, http_url_path_segments),
    ne!("Lokhttp3/HttpUrl;", "toString", "()Ljava/lang/String;", true, http_url_to_string),
    ne!("Lokhttp3/HttpUrl$Companion;", "get", "(Ljava/lang/String;)Lokhttp3/HttpUrl;", true, okhttp_http_url_parse),
    // ---- RequestsKt ----
    ne!("Leu/kanade/tachiyomi/network/RequestsKt;", "GET$default", "(Ljava/lang/String;Lokhttp3/Headers;Lokhttp3/CacheControl;ILjava/lang/Object;)Lokhttp3/Request;", false, requests_kt_get_default),
    ne!("Leu/kanade/tachiyomi/network/RequestsKt;", "__host_execute", "(Lokhttp3/Request;)Lokhttp3/Response;", false, keiyoushi_execute),
    // ---- kotlin.text.StringsKt ----
    ne!("Lkotlin/text/StringsKt;", "contains$default", "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;ZILjava/lang/Object;)Z", true, stringskt_contains_default),
    ne!("Lkotlin/text/StringsKt;", "replace$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Ljava/lang/String;", true, stringskt_replace_default),
    ne!("Lkotlin/text/StringsKt;", "trim", "(Ljava/lang/CharSequence;)Ljava/lang/CharSequence;", true, stringskt_trim),
    // ---- org.jsoup ----
    ne!("Leu/kanade/tachiyomi/util/JsoupExtensionsKt;", "asJsoup$default", "(Lokhttp3/Response;Ljava/lang/String;ILjava/lang/Object;)Lorg/jsoup/nodes/Document;", true, jsoup_parse),
    ne!("Lorg/jsoup/nodes/Document;", "select", "(Ljava/lang/String;)Lorg/jsoup/select/Elements;", true, document_select),
    ne!("Lorg/jsoup/nodes/Document;", "text", "()Ljava/lang/String;", true, document_text),
    ne!("Lorg/jsoup/nodes/Element;", "select", "(Ljava/lang/String;)Lorg/jsoup/select/Elements;", true, element_select),
    ne!("Lorg/jsoup/nodes/Element;", "selectFirst", "(Ljava/lang/String;)Lorg/jsoup/nodes/Element;", true, element_select_first),
    ne!("Lorg/jsoup/nodes/Element;", "text", "()Ljava/lang/String;", true, element_text),
    ne!("Lorg/jsoup/nodes/Element;", "attr", "(Ljava/lang/String;)Ljava/lang/String;", true, element_attr),
    ne!("Lorg/jsoup/nodes/Element;", "hasAttr", "(Ljava/lang/String;)Z", true, element_has_attr),
    ne!("Lorg/jsoup/nodes/Element;", "id", "()Ljava/lang/String;", true, element_id_attr),
    ne!("Lorg/jsoup/nodes/Element;", "tagName", "()Ljava/lang/String;", true, element_tag_name),
    ne!("Lorg/jsoup/nodes/Element;", "ownText", "()Ljava/lang/String;", true, element_own_text),
    ne!("Lorg/jsoup/nodes/Element;", "html", "()Ljava/lang/String;", true, element_html),
    ne!("Lorg/jsoup/nodes/Element;", "outerHtml", "()Ljava/lang/String;", true, element_outer_html),
    ne!("Lorg/jsoup/select/Elements;", "first", "()Lorg/jsoup/nodes/Element;", true, elements_first),
    ne!("Lorg/jsoup/select/Elements;", "get", "(I)Lorg/jsoup/nodes/Element;", true, elements_get),
    ne!("Lorg/jsoup/select/Elements;", "get", "(I)Ljava/lang/Object;", true, elements_get),
    ne!("Lorg/jsoup/select/Elements;", "size", "()I", true, elements_size),
    ne!("Lorg/jsoup/select/Elements;", "isEmpty", "()Z", true, elements_is_empty),
    ne!("Lorg/jsoup/select/Elements;", "text", "()Ljava/lang/String;", true, elements_text),
    ne!("Lorg/jsoup/select/Elements;", "eachText", "()Ljava/util/List;", true, elements_each_text),
    ne!("Lorg/jsoup/select/Elements;", "attr", "(Ljava/lang/String;)Ljava/lang/String;", true, elements_attr),
    ne!("Lorg/jsoup/select/Elements;", "eachAttr", "(Ljava/lang/String;)Ljava/util/List;", true, elements_each_attr),
    ne!("Lorg/jsoup/select/Elements;", "select", "(Ljava/lang/String;)Lorg/jsoup/select/Elements;", true, elements_select),
    // ---- kotlin.time.Duration (value class scalar ops) ----
    ne!("Lkotlin/time/Duration;", "minus-LRDsOJo", "(JJ)J", false, keiyoushi_duration_minus),
    ne!("Lkotlin/time/Duration;", "compareTo-LRDsOJo", "(JJ)I", false, keiyoushi_duration_compare),
    ne!("Lkotlin/time/Duration;", "equals-impl0", "(JJ)Z", false, keiyoushi_duration_equals),
    // ---- android prefs ----
    ne!("Landroid/content/Context;", "getSharedPreferences", "(Ljava/lang/String;I)Landroid/content/SharedPreferences;", true, context_get_shared_prefs),
    ne!("Landroid/content/SharedPreferences;", "getBoolean", "(Ljava/lang/String;Z)Z", true, shared_prefs_get_boolean),
    ne!("Landroidx/preference/Preference;", "<init>", "(Landroid/content/Context;)V", true, prefs_obj),
    ne!("Landroidx/preference/Preference;", "setKey", "(Ljava/lang/String;)V", true, prefs_set),
    ne!("Landroidx/preference/Preference;", "setTitle", "(Ljava/lang/CharSequence;)V", true, prefs_set),
    ne!("Landroidx/preference/Preference;", "setSummary", "(Ljava/lang/CharSequence;)V", true, prefs_set),
    ne!("Landroidx/preference/Preference;", "setDefaultValue", "(Ljava/lang/Object;)V", true, prefs_set),
    ne!("Landroidx/preference/PreferenceScreen;", "<init>", "(Landroid/content/Context;)V", true, prefs_obj),
    ne!("Landroidx/preference/PreferenceScreen;", "getContext", "()Landroid/content/Context;", true, prefs_ctx),
    ne!("Landroidx/preference/PreferenceScreen;", "setTitle", "(Ljava/lang/CharSequence;)V", true, prefs_set),
    ne!("Landroidx/preference/SwitchPreferenceCompat;", "<init>", "(Landroid/content/Context;)V", true, prefs_obj),
    ne!("Landroidx/preference/SwitchPreferenceCompat;", "setKey", "(Ljava/lang/String;)V", true, prefs_set),
    ne!("Landroidx/preference/SwitchPreferenceCompat;", "setTitle", "(Ljava/lang/CharSequence;)V", true, prefs_set),
    ne!("Landroidx/preference/SwitchPreferenceCompat;", "setSummary", "(Ljava/lang/CharSequence;)V", true, prefs_set),
    ne!("Landroidx/preference/SwitchPreferenceCompat;", "setDefaultValue", "(Ljava/lang/Object;)V", true, prefs_set),
];
