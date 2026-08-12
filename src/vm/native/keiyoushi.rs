//! Host shims for the eu.kanade.tachiyomi (mihon/keiyoushi) extension API surface.
//!
//! HTTP requests are not executed by the VM: the bridge registers an HTTP
//! callback on [`Vm::http`] and triggers it through the
//! `RequestsKt.__host_execute` native. The OkHttp / jsoup / android shims
//! live in sibling modules (okhttp.rs, jsoup.rs, android.rs, kotlin.rs).

use super::*;
use crate::permission::{NetworkPermission, Permission};
use std::rc::Rc;

pub(crate) const SMANGA: &str = "Leu/kanade/tachiyomi/source/model/SManga;";
pub(crate) const SCHAPTER: &str = "Leu/kanade/tachiyomi/source/model/SChapter;";
pub(crate) const PAGE: &str = "Leu/kanade/tachiyomi/source/model/Page;";
pub(crate) const FILTER_LIST: &str = "Leu/kanade/tachiyomi/source/model/FilterList;";
pub(crate) const FILTER: &str = "Leu/kanade/tachiyomi/source/model/Filter;";
pub(crate) const HEADERS: &str = "Lokhttp3/Headers;";
pub(crate) const RESPONSE: &str = "Lokhttp3/Response;";
pub(crate) const REQUEST: &str = "Lokhttp3/Request;";

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
    /// Raw response payload; text bodies arrive as UTF-8 bytes.
    pub body: Option<Vec<u8>>,
}

impl HttpResp {
    pub fn ok(body: impl Into<String>) -> Self {
        HttpResp {
            code: 200,
            message: "OK".into(),
            headers: Vec::new(),
            body: Some(body.into().into_bytes()),
        }
    }
    pub fn ok_bytes(bytes: Vec<u8>) -> Self {
        HttpResp {
            code: 200,
            message: "OK".into(),
            headers: Vec::new(),
            body: Some(bytes),
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

pub(crate) fn keiyoushi_execute(vm: &mut Vm, args: &[JValue]) -> R {
    let (url, method, headers, body) = request_parts(vm, args[0])?;
    check_network_url(vm, &url)?;
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
            prior: JValue::Null,
        },
    )
}

pub(crate) fn check_network_url(vm: &mut Vm, url: &str) -> Result<(), NatErr> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| nat_fatal(JvmError::Resolution(format!("invalid URL: {url}"))))?;
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .rsplit('@')
        .next()
        .unwrap_or_default();
    if authority.is_empty() {
        return Err(nat_fatal(JvmError::Resolution(format!(
            "invalid URL: {url}"
        ))));
    }
    let has_port = if authority.starts_with('[') {
        authority
            .find(']')
            .is_some_and(|end| authority.as_bytes().get(end + 1) == Some(&b':'))
    } else {
        authority
            .rsplit_once(':')
            .is_some_and(|(_, port)| port.chars().all(|c| c.is_ascii_digit()))
    };
    let target = if has_port {
        authority.to_owned()
    } else {
        match scheme.to_ascii_lowercase().as_str() {
            "http" => format!("{authority}:80"),
            "https" => format!("{authority}:443"),
            _ => authority.to_owned(),
        }
    };
    check_native_permission(vm, &Permission::Network(NetworkPermission::Connect(target)))
}

/// Host callback that returns the stored `lazy_http`? (unused, kept for docs)
#[allow(dead_code)]
pub(crate) type HttpCall = Rc<dyn Fn(&HttpData) -> HttpResp>;

/// Per-host header resolver: given the lowercase request host, returns an
/// optional User-Agent and Cookie header value (see
/// [`Context::set_host_headers`](crate::Context::set_host_headers)).
pub(crate) type HostHeaderFn = Rc<dyn Fn(&str) -> (Option<String>, Option<String>)>;

pub(crate) fn _http_client(vm: &mut Vm) -> Option<HttpCall> {
    vm.http.clone()
}

/// `OkHttpExtensionsKt.awaitSuccess(Call, Continuation)` — the suspend
/// bridge used by every keiyoushi coroutine source. The VM is fully
/// synchronous: run the call's interceptor chain immediately and fail the
/// frame on non-2xx instead of ever suspending.
pub(crate) fn okhttp_await_success(vm: &mut Vm, args: &[JValue]) -> R {
    let resp = okhttp_call_execute(vm, &args[..1])?;
    let code = match payload(vm, resp) {
        Some(Native::Response { code, .. }) => *code,
        _ => return Err(npe(vm)),
    };
    if (200..300).contains(&code) {
        Ok(resp)
    } else {
        Err(ioe(vm, format!("HTTP {code}")))
    }
}

fn okhttp_await(vm: &mut Vm, args: &[JValue]) -> R {
    okhttp_call_execute(vm, &args[..1])
}

fn okhttp_as_observable_success(vm: &mut Vm, args: &[JValue]) -> R {
    let result = (|| {
        let response = okhttp_call_execute(vm, &args[..1])?;
        let code = match payload(vm, response) {
            Some(Native::Response { code, .. }) => *code,
            _ => return Err(npe(vm)),
        };
        if (200..300).contains(&code) {
            Ok(response)
        } else {
            Err(ioe(vm, format!("HTTP {code}")))
        }
    })();
    rx::rx_from_result(vm, result)
}

fn okhttp_as_observable(vm: &mut Vm, args: &[JValue]) -> R {
    let result = okhttp_call_execute(vm, &args[..1]);
    rx::rx_from_result(vm, result)
}

// ---------------------------------------------------------------------------
// eu.kanade.tachiyomi.source.online.HttpSource defaults
// ---------------------------------------------------------------------------

/// The per-source getters (`getName`, `getLang`, `getBaseUrl`, `getId`,
/// `getSupportsLatest`) are implemented by the extension's own dex class —
/// every concrete mihon source overrides them. The natives below are only
/// reachable when the receiver's whole class chain lacks the override, which
/// on a real device is `AbstractMethodError`. Surface that instead of
/// inventing host-side constants (which would silently lie about sources
/// like "Comic Fury" vs "Akuma").
fn http_source_abstract(vm: &mut Vm, args: &[JValue], name: &str) -> R {
    let receiver = match args.first() {
        Some(JValue::Obj(o)) => vm
            .class_desc_str(vm.arena.objects[*o as usize].class)
            .to_string(),
        _ => String::new(),
    };
    Err(NatErr::Fatal(JvmError::Resolution(format!(
        "{name}: no override on {} (HttpSource stub method; abstract on real devices)",
        receiver
    ))))
}

pub(crate) fn http_source_get_base_url(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_abstract(vm, args, "getBaseUrl")
}

pub(crate) fn http_source_get_headers_default(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, HEADERS, Native::Headers(Vec::new()))
}

pub(crate) fn http_source_headers_builder(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lokhttp3/Headers$Builder;", Native::Headers(Vec::new()))
}

fn http_source_fetch(
    vm: &mut Vm,
    receiver: JValue,
    request_name: &str,
    request_sig: &str,
    request_args: &[JValue],
    parse_name: &str,
    parse_sig: &str,
) -> R {
    let result = (|| {
        let request = inv_virt(vm, receiver, request_name, request_sig, request_args)?;
        let response = keiyoushi_execute(vm, &[request])?;
        inv_virt(vm, receiver, parse_name, parse_sig, &[response])
    })();
    rx::rx_from_result(vm, result)
}

fn http_source_fetch_search(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_fetch(
        vm,
        args[0],
        "searchMangaRequest",
        "(ILjava/lang/String;Leu/kanade/tachiyomi/source/model/FilterList;)Lokhttp3/Request;",
        &args[1..4],
        "searchMangaParse",
        "(Lokhttp3/Response;)Leu/kanade/tachiyomi/source/model/MangasPage;",
    )
}

fn http_source_fetch_popular(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_fetch(
        vm,
        args[0],
        "popularMangaRequest",
        "(I)Lokhttp3/Request;",
        &args[1..2],
        "popularMangaParse",
        "(Lokhttp3/Response;)Leu/kanade/tachiyomi/source/model/MangasPage;",
    )
}

fn http_source_fetch_image_url(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_fetch(
        vm,
        args[0],
        "imageUrlRequest",
        "(Leu/kanade/tachiyomi/source/model/Page;)Lokhttp3/Request;",
        &args[1..2],
        "imageUrlParse",
        "(Lokhttp3/Response;)Ljava/lang/String;",
    )
}

fn http_source_prepare_new_chapter(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

fn http_source_fetch_details(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_fetch(
        vm,
        args[0],
        "mangaDetailsRequest",
        "(Leu/kanade/tachiyomi/source/model/SManga;)Lokhttp3/Request;",
        &args[1..2],
        "mangaDetailsParse",
        "(Lokhttp3/Response;)Leu/kanade/tachiyomi/source/model/SManga;",
    )
}

fn http_source_fetch_chapters(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_fetch(
        vm,
        args[0],
        "chapterListRequest",
        "(Leu/kanade/tachiyomi/source/model/SManga;)Lokhttp3/Request;",
        &args[1..2],
        "chapterListParse",
        "(Lokhttp3/Response;)Ljava/util/List;",
    )
}

fn http_source_fetch_pages(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_fetch(
        vm,
        args[0],
        "pageListRequest",
        "(Leu/kanade/tachiyomi/source/model/SChapter;)Lokhttp3/Request;",
        &args[1..2],
        "pageListParse",
        "(Lokhttp3/Response;)Ljava/util/List;",
    )
}

pub(crate) fn http_source_get_lang(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_abstract(vm, args, "getLang")
}

pub(crate) fn http_source_get_name(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_abstract(vm, args, "getName")
}

pub(crate) fn http_source_get_id(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_abstract(vm, args, "getId")
}

pub(crate) fn http_source_get_supports_latest(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_abstract(vm, args, "getSupportsLatest")
}

fn set_url_relative(p: &mut Native, url: &str) {
    let target = match p {
        Native::SManga { url: u, .. } | Native::SChapter { url: u, .. } => u,
        _ => return,
    };
    *target = if let Some(rest) = url.strip_prefix("http://") {
        match rest.find('/') {
            Some(i) => rest[i..].to_string(),
            None => "/".to_string(),
        }
    } else if let Some(rest) = url.strip_prefix("https://") {
        match rest.find('/') {
            Some(i) => rest[i..].to_string(),
            None => "/".to_string(),
        }
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
    set_url_relative(n, &url);
    Ok(JValue::Null)
}

pub(crate) fn http_source_set_url_no_domain_chapter(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = jstr(vm, args[2]).ok() else {
        return Ok(JValue::Null);
    };
    let Some(n) = payload_mut(vm, args[1]) else {
        return Err(npe(vm));
    };
    set_url_relative(n, &url);
    Ok(JValue::Null)
}

/// Extracts the URL of an SManga or SChapter object, or a plain string.
fn obj_url(vm: &mut Vm, v: JValue) -> Option<String> {
    match payload(vm, v) {
        Some(Native::SManga { url, .. }) | Some(Native::SChapter { url, .. }) => Some(url.clone()),
        _ => jstr(vm, v).ok(),
    }
}

fn http_source_get_request(vm: &mut Vm, src: JValue, obj: JValue) -> R {
    let Some(url) = obj_url(vm, obj) else {
        return Err(npe(vm));
    };
    let full = if url.starts_with("http") {
        url
    } else {
        format!("{}{url}", source_base_url(vm, src)?)
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

/// Host default for `mangaDetailsRequest` when the extension does not
/// override it: `GET baseUrl + manga.url`.
pub(crate) fn http_source_manga_details_request(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_get_request(vm, args[0], args[1])
}

pub(crate) fn http_source_chapter_list_request(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_get_request(vm, args[0], args[1])
}

pub(crate) fn http_source_page_list_request(vm: &mut Vm, args: &[JValue]) -> R {
    http_source_get_request(vm, args[0], args[1])
}

/// Resolves the source instance's `baseUrl` by invoking its (possibly
/// R8-renamed) bytecode `getBaseUrl` override.
fn source_base_url(vm: &mut Vm, src: JValue) -> Result<String, NatErr> {
    use crate::dex::insn::InvokeKind;
    use crate::vm::MethodRef;
    let JValue::Obj(o) = src else {
        return Err(nat_fatal(crate::vm::error::JvmError::Resolution(
            "source base url: not a source instance".into(),
        )));
    };
    let mref = MethodRef {
        name: vm.intern("getBaseUrl"),
        sig: vm.intern("()Ljava/lang/String;"),
        ret: 0,
        args: Vec::new(),
        class_desc: 0,
    };
    let target = vm
        .resolve_target(InvokeKind::Virtual, &mref, Some(o), 0)
        .map_err(nat_fatal)?;
    match vm.call_target(target, vec![src]) {
        Ok(JValue::Obj(s)) => Ok(jstr(vm, JValue::Obj(s)).unwrap_or_default()),
        Ok(_) => Ok(String::new()),
        Err(e) => Err(nat_fatal(e)),
    }
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
        memo: JValue::Null,
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
        Some(Native::SManga {
            update_strategy, ..
        }) => Ok(*update_strategy),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn smanga_set_update_strategy(vm: &mut Vm, args: &[JValue]) -> R {
    match payload_mut(vm, args[0]) {
        Some(Native::SManga {
            update_strategy, ..
        }) => *update_strategy = args[1],
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn smanga_set_initialized(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn smanga_get_memo(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SManga { memo, .. }) => Ok(*memo),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn smanga_set_memo(vm: &mut Vm, args: &[JValue]) -> R {
    match payload_mut(vm, args[0]) {
        Some(Native::SManga { memo, .. }) => *memo = args[1],
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
        chapter_number: 0.0,
        memo: JValue::Null,
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

pub(crate) fn schapter_get_chapter_number(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SChapter { chapter_number, .. }) => Ok(JValue::Float(*chapter_number)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn schapter_set_chapter_number(vm: &mut Vm, args: &[JValue]) -> R {
    let v = float_of(vm, args[1]);
    match payload_mut(vm, args[0]) {
        Some(Native::SChapter { chapter_number, .. }) => *chapter_number = v,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn schapter_get_memo(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SChapter { memo, .. }) => Ok(*memo),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn schapter_set_memo(vm: &mut Vm, args: &[JValue]) -> R {
    match payload_mut(vm, args[0]) {
        Some(Native::SChapter { memo, .. }) => *memo = args[1],
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
        Native::SMangasPage {
            mangas: dst,
            has_next: dst_next,
        } => {
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
    list_alloc(vm, items)
}

pub(crate) fn mangas_page_has_next(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SMangasPage { has_next, .. }) => Ok(JValue::Int(i32::from(*has_next))),
        _ => Err(npe(vm)),
    }
}

fn smanga_update_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::SMangaUpdate {
        manga: args[1],
        chapters: args[2],
    });
    Ok(JValue::Null)
}

fn smanga_update_get_manga(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SMangaUpdate { manga, .. }) => Ok(*manga),
        _ => Err(npe(vm)),
    }
}

fn smanga_update_get_chapters(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SMangaUpdate { chapters, .. }) => Ok(*chapters),
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

pub(crate) fn filter_list_init_list(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[1]) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
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
    list_alloc(vm, items)
}

pub(crate) fn filter_list_iterator(vm: &mut Vm, args: &[JValue]) -> R {
    let list = args[0].as_obj();
    alloc(
        vm,
        "Ljava/util/Iterator;",
        Native::Iter(IterKind::List { list, idx: 0 }),
    )
}

fn filter_list_is_empty(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SFilterList(items)) => Ok(JValue::Int(i32::from(items.is_empty()))),
        _ => Err(npe(vm)),
    }
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

/// `Filter$CheckBox.<init>(String, Z, I, DefaultConstructorMarker)` — the
/// default-args synthetic constructor; forwards to the `(String, Z)` form.
pub(crate) fn filter_checkbox_init_synth(vm: &mut Vm, args: &[JValue]) -> R {
    filter_init_checked(vm, args)
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
        (
            Native::SFilter {
                name,
                state,
                is_checked,
                children,
                options,
                text_value,
            },
            Native::SFilter {
                name: n2,
                state: st2,
                is_checked: ic2,
                children: ch2,
                options: op2,
                text_value: tv2,
            },
        ) => {
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

pub(crate) fn filter_select_set_state(vm: &mut Vm, args: &[JValue]) -> R {
    let state = match payload(vm, args[1]) {
        Some(Native::IntBox(v)) => *v,
        _ => int_of(vm, args[1]),
    };
    match payload_mut(vm, args[0]) {
        Some(Native::SFilter { state: slot, .. }) => *slot = state,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn filter_checkbox_set_state(vm: &mut Vm, args: &[JValue]) -> R {
    let checked = match payload(vm, args[1]) {
        Some(Native::BoolBox(v)) => *v,
        _ => int_of(vm, args[1]) != 0,
    };
    match payload_mut(vm, args[0]) {
        Some(Native::SFilter {
            is_checked: slot, ..
        }) => *slot = checked,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn filter_select_state_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let state = match payload(vm, args[0]) {
        Some(Native::SFilter { state, .. }) => *state,
        _ => return Ok(JValue::Null),
    };
    box_int_value(vm, "Ljava/lang/Integer;", JValue::Int(state))
}

fn filter_select_get_values(vm: &mut Vm, args: &[JValue]) -> R {
    let options = match payload(vm, args[0]) {
        Some(Native::SFilter { options, .. }) => options.clone(),
        _ => return Err(npe(vm)),
    };
    alloc_arr(vm, "Ljava/lang/Object;", options.len(), move || {
        ArrayData::Obj(options)
    })
}

fn sort_selection_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::SortSelection {
        index: int_of(vm, args[1]),
        ascending: int_of(vm, args[2]) != 0,
    });
    Ok(JValue::Null)
}

fn sort_selection_get_index(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SortSelection { index, .. }) => Ok(JValue::Int(*index)),
        _ => Err(npe(vm)),
    }
}

fn sort_selection_get_ascending(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SortSelection { ascending, .. }) => Ok(JValue::Int(i32::from(*ascending))),
        _ => Err(npe(vm)),
    }
}

fn filter_sort_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    let options = match payload(vm, args[2]) {
        Some(Native::Array(ArrayData::Obj(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    set_filter_payload(
        vm,
        args[0],
        Native::SFilter {
            name,
            state: 0,
            is_checked: false,
            children: vec![args[3]],
            options,
            text_value: String::new(),
        },
    )
}

fn filter_sort_get_state(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::SFilter { children, .. }) => {
            Ok(children.first().copied().unwrap_or(JValue::Null))
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn filter_checkbox_state_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let checked = match payload(vm, args[0]) {
        Some(Native::SFilter { is_checked, .. }) => *is_checked,
        _ => return Ok(JValue::Null),
    };
    boxed(vm, "Ljava/lang/Boolean;", Native::BoolBox(checked))
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
            Some(Native::SFilter {
                state, is_checked, ..
            }) => {
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

fn filter_tristate_state_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let state = filter_get_state(vm, args)?;
    box_int_value(vm, "Ljava/lang/Integer;", state)
}

pub(crate) fn requests_kt_get_default(vm: &mut Vm, args: &[JValue]) -> R {
    let url = match payload(vm, args[0]) {
        Some(Native::HttpUrl(u)) => u.clone(),
        _ => match jstr(vm, args[0]) {
            Ok(s) => s,
            Err(_) => return Err(npe(vm)),
        },
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

// ---------------------------------------------------------------------------
// eu.kanade network helpers (from OkHttp shim)
// ---------------------------------------------------------------------------

pub(crate) fn http_source_get_network(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Leu/kanade/tachiyomi/network/NetworkHelper;",
        Native::Opaque,
    )
}

pub(crate) fn network_helper_get_client(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lokhttp3/OkHttpClient;", Native::Opaque)
}

pub(crate) fn requests_kt_post_default(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = jstr(vm, args[0]).ok() else {
        return Err(npe(vm));
    };
    let body = args[2];
    alloc(
        vm,
        "Lokhttp3/Request;",
        Native::Request {
            url,
            method: "POST".into(),
            headers: Vec::new(),
            body: Some(body),
        },
    )
}

// ---------------------------------------------------------------------------
// KEIYOUSHI_TABLE
// ---------------------------------------------------------------------------

pub const KEIYOUSHI_TABLE: &[NativeEntry] = &[
    ne!("Leu/kanade/tachiyomi/network/RequestsKt;", "GET", "(Ljava/lang/String;Lokhttp3/Headers;Lokhttp3/CacheControl;)Lokhttp3/Request;", false, requests_kt_get_default),
    ne!("Leu/kanade/tachiyomi/network/RequestsKt;", "GET", "(Lokhttp3/HttpUrl;Lokhttp3/Headers;Lokhttp3/CacheControl;)Lokhttp3/Request;", false, requests_kt_get_default),
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
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "setInitialized", "(Z)V", true, smanga_set_initialized),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "getMemo", "()Lkotlinx/serialization/json/JsonObject;", true, smanga_get_memo),
    ne!("Leu/kanade/tachiyomi/source/model/SManga;", "setMemo", "(Lkotlinx/serialization/json/JsonObject;)V", true, smanga_set_memo),
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
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "getChapter_number", "()F", true, schapter_get_chapter_number),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "setChapter_number", "(F)V", true, schapter_set_chapter_number),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "getScanlator", "()Ljava/lang/String;", true, schapter_get_scanlator),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "setScanlator", "(Ljava/lang/String;)V", true, schapter_set_scanlator),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "getMemo", "()Lkotlinx/serialization/json/JsonObject;", true, schapter_get_memo),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter;", "setMemo", "(Lkotlinx/serialization/json/JsonObject;)V", true, schapter_set_memo),
    ne!("Leu/kanade/tachiyomi/source/model/SChapter$Companion;", "create", "()Leu/kanade/tachiyomi/source/model/SChapter;", true, schapter_companion_create),
    ne!("Leu/kanade/tachiyomi/source/model/Page;", "<init>", "(ILjava/lang/String;Ljava/lang/String;Landroid/net/Uri;ILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, page_init),
    ne!("Leu/kanade/tachiyomi/source/model/Page;", "getUrl", "()Ljava/lang/String;", true, page_get_url),
    ne!("Leu/kanade/tachiyomi/source/model/Page;", "getName", "()Ljava/lang/String;", true, page_get_name),
    ne!("Leu/kanade/tachiyomi/source/model/Page;", "getImageUrl", "()Ljava/lang/String;", true, page_get_image_url),
    ne!("Leu/kanade/tachiyomi/source/model/Page;", "getIndex", "()I", true, page_get_index),
    ne!("Leu/kanade/tachiyomi/source/model/MangasPage;", "<init>", "(Ljava/util/List;Z)V", true, mangas_page_init),
    ne!("Leu/kanade/tachiyomi/source/model/MangasPage;", "getMangas", "()Ljava/util/List;", true, mangas_page_get_mangas),
    ne!("Leu/kanade/tachiyomi/source/model/MangasPage;", "hasNextPage", "()Z", true, mangas_page_has_next),
    ne!("Leu/kanade/tachiyomi/source/model/MangasPage;", "getHasNextPage", "()Z", true, mangas_page_has_next),
    ne!("Leu/kanade/tachiyomi/source/model/SMangaUpdate;", "<init>", "(Leu/kanade/tachiyomi/source/model/SManga;Ljava/util/List;)V", true, smanga_update_init),
    ne!("Leu/kanade/tachiyomi/source/model/SMangaUpdate;", "getManga", "()Leu/kanade/tachiyomi/source/model/SManga;", true, smanga_update_get_manga),
    ne!("Leu/kanade/tachiyomi/source/model/SMangaUpdate;", "getChapters", "()Ljava/util/List;", true, smanga_update_get_chapters),
    ne!("Leu/kanade/tachiyomi/source/model/FilterList;", "<init>", "([Leu/kanade/tachiyomi/source/model/Filter;)V", true, filter_list_init),
    ne!("Leu/kanade/tachiyomi/source/model/FilterList;", "<init>", "(Ljava/util/List;)V", true, filter_list_init_list),
    ne!("Leu/kanade/tachiyomi/source/model/FilterList;", "getFilters", "()Ljava/util/List;", true, filter_list_get_filters),
    ne!("Leu/kanade/tachiyomi/source/model/FilterList;", "iterator", "()Ljava/util/Iterator;", true, filter_list_iterator),
    ne!("Leu/kanade/tachiyomi/source/model/FilterList;", "isEmpty", "()Z", true, filter_list_is_empty),
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
    ne!("Leu/kanade/tachiyomi/source/model/Filter$TriState;", "getState", "()Ljava/lang/Object;", true, filter_tristate_state_obj),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Select;", "<init>", "(Ljava/lang/String;[Ljava/lang/String;)V", true, filter_select_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Select;", "<init>", "(Ljava/lang/String;[Ljava/lang/Object;I)V", true, filter_select_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Select;", "<init>", "(Ljava/lang/String;[Ljava/lang/Object;IILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, filter_select_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Select;", "getState", "()Ljava/lang/Object;", true, filter_select_state_obj),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Select;", "getValues", "()[Ljava/lang/Object;", true, filter_select_get_values),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Select;", "setState", "(Ljava/lang/Object;)V", true, filter_select_set_state),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Sort$Selection;", "<init>", "(IZ)V", true, sort_selection_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Sort$Selection;", "getIndex", "()I", true, sort_selection_get_index),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Sort$Selection;", "getAscending", "()Z", true, sort_selection_get_ascending),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Sort;", "<init>", "(Ljava/lang/String;[Ljava/lang/String;Leu/kanade/tachiyomi/source/model/Filter$Sort$Selection;)V", true, filter_sort_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Sort;", "getState", "()Ljava/lang/Object;", true, filter_sort_get_state),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$CheckBox;", "<init>", "(Ljava/lang/String;Z)V", true, filter_init_checked),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$CheckBox;", "<init>", "(Ljava/lang/String;ZILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, filter_checkbox_init_synth),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$CheckBox;", "getState", "()Ljava/lang/Object;", true, filter_checkbox_state_obj),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$CheckBox;", "setState", "(Ljava/lang/Object;)V", true, filter_checkbox_set_state),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Group;", "<init>", "(Ljava/lang/String;Ljava/util/List;)V", true, filter_group_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Group;", "<init>", "(Ljava/lang/String;Ljava/util/List;IILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, filter_group_init),
    ne!("Leu/kanade/tachiyomi/source/model/Filter$Group;", "getState", "()Ljava/lang/Object;", true, filter_group_state_obj),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getBaseUrl", "()Ljava/lang/String;", true, http_source_get_base_url),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getHeaders", "()Lokhttp3/Headers;", true, http_source_get_headers_default),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getLang", "()Ljava/lang/String;", true, http_source_get_lang),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getName", "()Ljava/lang/String;", true, http_source_get_name),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getId", "()J", true, http_source_get_id),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getSupportsLatest", "()Z", true, http_source_get_supports_latest),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "fetchSearchManga", "(ILjava/lang/String;Leu/kanade/tachiyomi/source/model/FilterList;)Lrx/Observable;", true, http_source_fetch_search),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "fetchPopularManga", "(I)Lrx/Observable;", true, http_source_fetch_popular),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "fetchMangaDetails", "(Leu/kanade/tachiyomi/source/model/SManga;)Lrx/Observable;", true, http_source_fetch_details),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "fetchChapterList", "(Leu/kanade/tachiyomi/source/model/SManga;)Lrx/Observable;", true, http_source_fetch_chapters),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "fetchPageList", "(Leu/kanade/tachiyomi/source/model/SChapter;)Lrx/Observable;", true, http_source_fetch_pages),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "fetchImageUrl", "(Leu/kanade/tachiyomi/source/model/Page;)Lrx/Observable;", true, http_source_fetch_image_url),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "prepareNewChapter", "(Leu/kanade/tachiyomi/source/model/SChapter;Leu/kanade/tachiyomi/source/model/SManga;)V", true, http_source_prepare_new_chapter),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "headersBuilder", "()Lokhttp3/Headers$Builder;", true, http_source_headers_builder),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "setUrlWithoutDomain", "(Leu/kanade/tachiyomi/source/model/SManga;Ljava/lang/String;)V", true, http_source_set_url_no_domain_manga),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "setUrlWithoutDomain", "(Leu/kanade/tachiyomi/source/model/SChapter;Ljava/lang/String;)V", true, http_source_set_url_no_domain_chapter),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "mangaDetailsRequest", "(Leu/kanade/tachiyomi/source/model/SManga;)Lokhttp3/Request;", true, http_source_manga_details_request),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "chapterListRequest", "(Leu/kanade/tachiyomi/source/model/SManga;)Lokhttp3/Request;", true, http_source_chapter_list_request),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "pageListRequest", "(Leu/kanade/tachiyomi/source/model/SChapter;)Lokhttp3/Request;", true, http_source_page_list_request),
    ne!("Leu/kanade/tachiyomi/network/RequestsKt;", "GET$default", "(Ljava/lang/String;Lokhttp3/Headers;Lokhttp3/CacheControl;ILjava/lang/Object;)Lokhttp3/Request;", false, requests_kt_get_default),
    ne!("Leu/kanade/tachiyomi/network/RequestsKt;", "GET$default", "(Lokhttp3/HttpUrl;Lokhttp3/Headers;Lokhttp3/CacheControl;ILjava/lang/Object;)Lokhttp3/Request;", false, requests_kt_get_default),
    ne!("Leu/kanade/tachiyomi/network/RequestsKt;", "__host_execute", "(Lokhttp3/Request;)Lokhttp3/Response;", false, keiyoushi_execute),
    ne!("Leu/kanade/tachiyomi/network/OkHttpExtensionsKt;", "awaitSuccess", "(Lokhttp3/Call;Lkotlin/coroutines/Continuation;)Ljava/lang/Object;", false, okhttp_await_success),
    ne!("Leu/kanade/tachiyomi/network/OkHttpExtensionsKt;", "await", "(Lokhttp3/Call;Lkotlin/coroutines/Continuation;)Ljava/lang/Object;", false, okhttp_await),
    ne!("Leu/kanade/tachiyomi/network/OkHttpExtensionsKt;", "asObservableSuccess", "(Lokhttp3/Call;)Lrx/Observable;", false, okhttp_as_observable_success),
    ne!("Leu/kanade/tachiyomi/network/OkHttpExtensionsKt;", "asObservable", "(Lokhttp3/Call;)Lrx/Observable;", false, okhttp_as_observable),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getNetwork", "()Leu/kanade/tachiyomi/network/NetworkHelper;", true, http_source_get_network),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getClient", "()Lokhttp3/OkHttpClient;", true, network_helper_get_client),
    ne!("Leu/kanade/tachiyomi/network/NetworkHelper;", "getClient", "()Lokhttp3/OkHttpClient;", true, network_helper_get_client),
    ne!("Leu/kanade/tachiyomi/network/RequestsKt;", "POST$default", "(Ljava/lang/String;Lokhttp3/Headers;Lokhttp3/RequestBody;Lokhttp3/CacheControl;ILjava/lang/Object;)Lokhttp3/Request;", false, requests_kt_post_default),
    ne!(
        "Leu/kanade/tachiyomi/util/JsoupExtensionsKt;",
        "asJsoup$default",
        "(Lokhttp3/Response;Ljava/lang/String;ILjava/lang/Object;)Lorg/jsoup/nodes/Document;",
        false,
        crate::vm::native::jsoup::jsoup_parse
    ),
    ne!(
        "Leu/kanade/tachiyomi/network/interceptor/UncaughtExceptionInterceptor;",
        "intercept",
        "(Lokhttp3/Interceptor$Chain;)Lokhttp3/Response;",
        true,
        crate::vm::native::okhttp::interceptor_pass_through
    ),
    ne!(
        "Leu/kanade/tachiyomi/network/interceptor/UserAgentInterceptor;",
        "intercept",
        "(Lokhttp3/Interceptor$Chain;)Lokhttp3/Response;",
        true,
        crate::vm::native::okhttp::interceptor_pass_through
    ),
    ne!(
        "Leu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor;",
        "intercept",
        "(Lokhttp3/Interceptor$Chain;)Lokhttp3/Response;",
        true,
        crate::vm::native::okhttp::interceptor_pass_through
    ),
    ne!(
        "Lcom/squareup/zstd/okio/OkioZstd;",
        "zstdDecompress",
        "(Lokio/Source;)Lokio/Source;",
        false,
        crate::vm::native::serialization::zstd_identity
    ),
    ne!(
        "Lcom/squareup/zstd/okio/OkioZstd;",
        "zstdCompress",
        "(Lokio/Sink;)Lokio/Sink;",
        false,
        crate::vm::native::serialization::zstd_identity
    ),
    ne!(
        "Ltachiyomi/decoder/ImageDecoder;",
        "getWidth",
        "()I",
        true,
        crate::vm::native::serialization::image_decoder_get_dimension
    ),
    ne!(
        "Ltachiyomi/decoder/ImageDecoder;",
        "getHeight",
        "()I",
        true,
        crate::vm::native::serialization::image_decoder_get_dimension
    ),
    ne!(
        "Ltachiyomi/decoder/ImageDecoder;",
        "recycle",
        "()V",
        true,
        crate::vm::native::serialization::image_decoder_recycle
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    #[test]
    fn network_bridge_enforces_host_and_port_permissions() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut ctx = Context::new(&data).unwrap();
        let vm = ctx.vm();
        assert!(matches!(
            check_network_url(vm, "https://api.example/path"),
            Err(NatErr::Throw(_))
        ));
        vm.perms
            .grant(Permission::Network(NetworkPermission::Connect(
                "api.example:443".into(),
            )));
        check_network_url(vm, "https://api.example/path").unwrap();
        assert!(check_network_url(vm, "https://api.example:8443/path").is_err());
        assert!(check_network_url(vm, "https://evil.example/path").is_err());
    }

    #[test]
    fn manga_and_chapter_memo_round_trip() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut ctx = Context::new(&data).unwrap();
        let vm = ctx.vm();
        let manga = alloc(vm, SMANGA, empty_smanga()).unwrap();
        let chapter = alloc(vm, SCHAPTER, empty_schapter()).unwrap();
        let memo = alloc(
            vm,
            "Lkotlinx/serialization/json/JsonObject;",
            Native::Json(crate::vm::object::JsonVal::Object(Vec::new())),
        )
        .unwrap();
        smanga_set_memo(vm, &[manga, memo]).unwrap();
        schapter_set_memo(vm, &[chapter, memo]).unwrap();
        assert_eq!(smanga_get_memo(vm, &[manga]).unwrap(), memo);
        assert_eq!(schapter_get_memo(vm, &[chapter]).unwrap(), memo);
    }

    #[test]
    fn filter_subclass_states_are_mutable() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut ctx = Context::new(&data).unwrap();
        let vm = ctx.vm();
        let select = alloc(
            vm,
            "Leu/kanade/tachiyomi/source/model/Filter$Select;",
            filter_new("x".into(), false, 0),
        )
        .unwrap();
        filter_select_set_state(vm, &[select, JValue::Int(2)]).unwrap();
        assert_eq!(filter_get_state(vm, &[select]).unwrap(), JValue::Int(2));
        let check = alloc(
            vm,
            "Leu/kanade/tachiyomi/source/model/Filter$CheckBox;",
            filter_new("x".into(), false, 0),
        )
        .unwrap();
        filter_checkbox_set_state(vm, &[check, JValue::Int(1)]).unwrap();
        let state = filter_checkbox_state_obj(vm, &[check]).unwrap();
        assert!(bool_of(vm, state));
    }
}
