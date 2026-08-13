//! Host shims for the OkHttp network stack used by extensions.
//! Requests are never executed; the client/builder classes only carry
//! interceptor lists so extension `<init>` code can run.

pub(crate) const HEADERS: &str = "Lokhttp3/Headers;";
#[cfg_attr(not(feature = "tachiyomi"), allow(dead_code))]
pub(crate) const RESPONSE: &str = "Lokhttp3/Response;";
pub(crate) const REQUEST: &str = "Lokhttp3/Request;";
pub(crate) const HTTP_URL: &str = "Lokhttp3/HttpUrl;";

use log::{info, warn};

use super::*;

// ---------------------------------------------------------------------------

// Bridge entry: executes the request through the registered HTTP callback
// and builds an `okhttp3.Response` object for the extension to parse.
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

/// `Request$Builder.headers(Headers)` — copy all headers from the Headers
/// object into the builder.
pub(crate) fn request_builder_headers(vm: &mut Vm, args: &[JValue]) -> R {
    let src = match payload(vm, args[1]) {
        Some(Native::Headers(es)) => es.clone(),
        _ => return Err(npe(vm)),
    };
    match payload_mut(vm, args[0]) {
        Some(Native::RequestBuilder { headers, .. }) => headers.extend(src),
        _ => return Err(npe(vm)),
    }
    Ok(args[0])
}

/// `Request$Builder.cacheControl(CacheControl)` — materializes the
/// `Cache-Control` header the interceptor chain sees on the final request.
pub(crate) fn request_builder_cache_control(vm: &mut Vm, args: &[JValue]) -> R {
    let (max_age, no_cache) = match payload(vm, args[1]) {
        Some(Native::CacheControl {
            max_age, no_cache, ..
        }) => (*max_age, *no_cache),
        _ => return Err(npe(vm)),
    };
    let mut hdr = String::new();
    if no_cache {
        hdr.push_str("no-cache");
    }
    if max_age >= 0 {
        if !hdr.is_empty() {
            hdr.push_str(", ");
        }
        hdr.push_str(&format!("max-age={max_age}"));
    }
    match payload_mut(vm, args[0]) {
        Some(Native::RequestBuilder { headers, .. }) if !hdr.is_empty() => {
            headers.push(("Cache-Control".to_string(), hdr));
        }
        Some(Native::RequestBuilder { .. }) => {}
        _ => return Err(npe(vm)),
    }
    Ok(args[0])
}

pub(crate) fn request_builder_tag2(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn request_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let (url, method, headers, body) = match payload(vm, args[0]) {
        Some(Native::RequestBuilder {
            url,
            method,
            headers,
            body,
        }) => (url.clone(), method.clone(), headers.clone(), *body),
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
        Some(Native::Request {
            url,
            method,
            headers,
            body,
        }) => (url.clone(), method.clone(), headers.clone(), *body),
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

pub(crate) fn headers_builder_set(vm: &mut Vm, args: &[JValue]) -> R {
    let (name, value) = match (jstr(vm, args[1]), jstr(vm, args[2])) {
        (Ok(name), Ok(value)) => (name, value),
        _ => return Err(npe(vm)),
    };
    let Some(Native::Headers(headers)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    headers.retain(|(existing, _)| !existing.eq_ignore_ascii_case(&name));
    headers.push((name, value));
    Ok(args[0])
}

fn headers_builder_remove_all(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    let Some(Native::Headers(headers)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    headers.retain(|(existing, _)| !existing.eq_ignore_ascii_case(&name));
    Ok(args[0])
}

pub(crate) fn headers_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Headers(headers)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(vm, HEADERS, Native::Headers(headers.clone()))
}

pub(crate) fn headers_new_builder(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Headers(headers)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(
        vm,
        "Lokhttp3/Headers$Builder;",
        Native::Headers(headers.clone()),
    )
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

/// Splits a URL into (lowercased host, path). Path defaults to "/".
/// The host matches what `reqwest::Url::host_str()` returns.
fn url_host_and_path(url: &str) -> (String, String) {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let host = hostport.split(':').next().unwrap_or(hostport);
    (
        host.to_ascii_lowercase(),
        if path.is_empty() {
            "/".into()
        } else {
            path.to_string()
        },
    )
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
    match body {
        Some(bytes) => alloc(
            vm,
            "Lokhttp3/ResponseBody;",
            Native::RespBody(bytes.clone()),
        ),
        None => alloc(vm, "Lokhttp3/ResponseBody;", Native::Str(String::new())),
    }
}

pub(crate) fn response_body_bytes_stream(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = resp_body_bytes(vm, args[0])?;
    alloc(
        vm,
        "Ljava/io/ByteArrayInputStream;",
        Native::ByteArrayInputStream { bytes, pos: 0 },
    )
}

pub(crate) fn response_body_bytes_arr(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = resp_body_bytes(vm, args[0])?;
    let data = bytes.into_iter().map(|b| b as i8).collect::<Vec<_>>();
    let len = data.len();
    alloc_arr(vm, "B", len, move || ArrayData::Byte(data))
}

pub(crate) fn resp_body_bytes(vm: &mut Vm, v: JValue) -> Result<Vec<u8>, NatErr> {
    match payload(vm, v) {
        Some(Native::RespBody(b)) => Ok(b.clone()),
        Some(Native::Str(s)) => Ok(s.as_bytes().to_vec()),
        _ => Err(npe(vm)),
    }
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

pub(crate) fn response_body_len(vm: &mut Vm, args: &[JValue]) -> R {
    let len = match payload(vm, args[0]) {
        Some(Native::RespBody(b)) => b.len(),
        Some(Native::Str(s)) => s.len(),
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Long(len as i64))
}

pub(crate) fn response_body_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::RespBody(b)) => String::from_utf8_lossy(b).into_owned(),
        _ => match jstr(vm, args[0]) {
            Ok(s) => s,
            Err(_) => return Err(npe(vm)),
        },
    };
    Ok(vm.alloc_string(&s))
}

pub(crate) fn http_url_host(vm: &mut Vm, args: &[JValue]) -> R {
    let url = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Err(npe(vm)),
    };
    let host = url
        .split("://")
        .nth(1)
        .and_then(|r| r.split(['/', '?']).next())
        .unwrap_or("");
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
            if k == name.as_str() {
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
    let path = url
        .split("://")
        .nth(1)
        .and_then(|s| s.split(['?', '#']).next())
        .unwrap_or("");
    let path_owned = path.to_string();
    // The authority (host[:port]) precedes the first '/'; path segments
    // start after it.
    let segments = match path_owned.split_once('/') {
        Some((_, rest)) => rest
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| vm.alloc_string(s))
            .collect::<Vec<_>>(),
        None => Vec::new(),
    };
    list_alloc(vm, segments)
}

pub(crate) fn http_url_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(vm.alloc_string(&s))
}

fn http_url_encoded_path_value(url: &str) -> &str {
    let after_authority = url.find("://").map(|index| index + 3).unwrap_or_default();
    let path_start = url[after_authority..]
        .find('/')
        .map(|index| after_authority + index);
    let suffix_start = url[after_authority..]
        .find(['?', '#'])
        .map(|index| after_authority + index)
        .unwrap_or(url.len());
    path_start
        .filter(|start| *start < suffix_start)
        .map(|start| &url[start..suffix_start])
        .unwrap_or("/")
}

pub(crate) fn http_url_encoded_path(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::HttpUrl(url)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let path = http_url_encoded_path_value(url).to_string();
    Ok(new_str(vm, &path))
}

pub(crate) fn http_url_fragment(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::HttpUrl(url)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let fragment = url
        .split_once('#')
        .map(|(_, fragment)| fragment.to_string());
    Ok(fragment
        .map(|fragment| new_str(vm, &fragment))
        .unwrap_or(JValue::Null))
}

pub(crate) fn http_url_port(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::HttpUrl(url)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let scheme = url.split("://").next().unwrap_or_default();
    let authority = url
        .split("://")
        .nth(1)
        .unwrap_or_default()
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let port = authority
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse::<i32>().ok())
        .unwrap_or(if scheme.eq_ignore_ascii_case("https") {
            443
        } else {
            80
        });
    Ok(JValue::Int(port))
}

// ---------------------------------------------------------------------------
// OkHttpClient / FormBody / HttpUrl / builder shims
// ---------------------------------------------------------------------------

pub(crate) fn okhttp_client_new_builder(vm: &mut Vm, _args: &[JValue]) -> R {
    let interceptors = mihon_default_interceptors(vm)?;
    alloc(
        vm,
        "Lokhttp3/OkHttpClient$Builder;",
        Native::OkHttpBuilder {
            interceptors,
            network_interceptors: Vec::new(),
        },
    )
}

/// The three interceptor stubs every mihon default client ships
/// (UncaughtExceptionInterceptor, UserAgentInterceptor,
/// CloudflareInterceptor). Synthetic extensions validate their presence by
/// simple-class-name, then reorder and re-wrap them; each one passes
/// requests through untouched.
pub(crate) fn mihon_default_interceptors(vm: &mut Vm) -> Result<Vec<JValue>, NatErr> {
    let mut out = Vec::new();
    for desc in [
        "Leu/kanade/tachiyomi/network/interceptor/UncaughtExceptionInterceptor;",
        "Leu/kanade/tachiyomi/network/interceptor/UserAgentInterceptor;",
        "Leu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor;",
    ] {
        out.push(alloc(vm, desc, Native::Opaque)?);
    }
    Ok(out)
}

/// Ignores the interceptor and proceeds: used by the default-client stubs
/// and the compression library shims.
pub(crate) fn interceptor_pass_through(vm: &mut Vm, args: &[JValue]) -> R {
    let chain = args[1];
    let request = match payload(vm, chain) {
        Some(Native::Chain { request, .. }) => *request,
        _ => return Err(npe(vm)),
    };
    chain_proceed(vm, &[chain, request])
}

pub(crate) fn compression_interceptor_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// `Request.cacheControl()` — parses the request's `Cache-Control` header
/// into a real CacheControl (max-age in seconds, -1 when absent).
pub(crate) fn request_cache_control(vm: &mut Vm, args: &[JValue]) -> R {
    let headers = match payload(vm, args[0]) {
        Some(Native::Request { headers, .. }) => headers.clone(),
        _ => return Err(npe(vm)),
    };
    let mut max_age = -1i64;
    let mut no_cache = false;
    for (k, v) in &headers {
        if k.eq_ignore_ascii_case("Cache-Control") {
            for part in v.split(',') {
                let part = part.trim();
                if part.eq_ignore_ascii_case("no-cache") || part.eq_ignore_ascii_case("no-store") {
                    no_cache = true;
                }
                if let Some(rest) = part.strip_prefix("max-age=") {
                    if let Ok(n) = rest.trim().parse::<i64>() {
                        max_age = n;
                    }
                }
            }
        }
    }
    alloc(
        vm,
        "Lokhttp3/CacheControl;",
        Native::CacheControl {
            max_age,
            no_cache,
            no_store: match payload(vm, args[0]) {
                Some(Native::CacheControlBuilder { no_store, .. }) => *no_store,
                _ => false,
            },
            max_stale: match payload(vm, args[0]) {
                Some(Native::CacheControlBuilder { max_stale, .. }) => *max_stale,
                _ => 0,
            },
        },
    )
}

pub(crate) fn cache_control_max_age_seconds(vm: &mut Vm, args: &[JValue]) -> R {
    let max_age = match payload(vm, args[0]) {
        Some(Native::CacheControl { max_age, .. }) => *max_age,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(max_age as i32))
}

pub(crate) fn cache_control_no_cache(vm: &mut Vm, args: &[JValue]) -> R {
    let no_cache = match payload(vm, args[0]) {
        Some(Native::CacheControl { no_cache, .. }) => *no_cache,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(i32::from(no_cache)))
}

/// `CacheControl$Builder.<init>()` — fresh builder, max-age in seconds.
pub(crate) fn cache_control_builder_init(vm: &mut Vm, args: &[JValue]) -> R {
    let state = Native::CacheControlBuilder {
        max_age: -1,
        no_cache: false,
        no_store: false,
        max_stale: -1,
    };
    if let Some(JValue::Obj(this)) = args.first().copied() {
        vm.arena.objects[this as usize].native = Some(state);
        Ok(JValue::Null)
    } else {
        alloc(vm, "Lokhttp3/CacheControl$Builder;", state)
    }
}

/// `CacheControl$Builder.maxAge-LRDsOJo(J)` — kotlin.time.Duration is the
/// inline long; the VM stores raw milliseconds, so translate to seconds.
pub(crate) fn cache_control_builder_max_age(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = long_of(vm, args[1]);
    match payload_mut(vm, args[0]) {
        Some(Native::CacheControlBuilder { max_age, .. }) => *max_age = millis / 1000,
        _ => return Err(npe(vm)),
    }
    Ok(args[0])
}

/// `CacheControl$Builder.build()` — materialize the parsed CacheControl.
pub(crate) fn cache_control_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let max_age = match payload(vm, args[0]) {
        Some(Native::CacheControlBuilder { max_age, .. }) => *max_age,
        _ => return Err(npe(vm)),
    };
    alloc(
        vm,
        "Lokhttp3/CacheControl;",
        Native::CacheControl {
            max_age,
            no_cache: match payload(vm, args[0]) {
                Some(Native::CacheControlBuilder { no_cache, .. }) => *no_cache,
                _ => return Err(npe(vm)),
            },
            no_store: match payload(vm, args[0]) {
                Some(Native::CacheControlBuilder { no_store, .. }) => *no_store,
                _ => false,
            },
            max_stale: match payload(vm, args[0]) {
                Some(Native::CacheControlBuilder { max_stale, .. }) => *max_stale,
                _ => 0,
            },
        },
    )
}

/// `Call.isCanceled()` — reads the real canceled flag on the Call.
pub(crate) fn call_is_canceled(vm: &mut Vm, args: &[JValue]) -> R {
    let canceled = match payload(vm, args[0]) {
        Some(Native::Call { canceled, .. }) => *canceled,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(i32::from(canceled)))
}

/// `Call.cancel()` — sets the real canceled flag; subsequent executes throw
/// IOException("Canceled") like OkHttp.
pub(crate) fn call_cancel(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Call { canceled, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *canceled = true;
    Ok(JValue::Null)
}

/// `Call.timeout()` — a real per-call Timeout (defaults to the client's
/// call timeout; none set here means zero, matching no configured value).
pub(crate) fn call_timeout(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lokhttp3/Timeout;", Native::Timeout { millis: 0 })
}

/// `Timeout.timeout(long)` — real setter (millis units as okio).
pub(crate) fn timeout_timeout(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = long_of(vm, args[1]);
    let Some(Native::Timeout { millis: dst }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = millis;
    Ok(args[0])
}

/// `Timeout.timeoutMillis()` — real getter.
pub(crate) fn timeout_timeout_millis(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = match payload(vm, args[0]) {
        Some(Native::Timeout { millis }) => *millis,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Long(millis))
}

/// `Response.priorResponse()` — the response before a redirect chain, or
/// null when there is none (the host bridge never redirects).
pub(crate) fn response_prior_response(vm: &mut Vm, args: &[JValue]) -> R {
    let prior = match payload(vm, args[0]) {
        Some(Native::Response { prior, .. }) => *prior,
        _ => return Err(npe(vm)),
    };
    Ok(prior)
}

/// `Response.Builder.priorResponse(Response)` — real setter.
pub(crate) fn response_builder_prior_response(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ResponseBuilder { prior, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *prior = Some(args[1]);
    Ok(args[0])
}

pub(crate) fn okhttp_builder_add_interceptor(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::OkHttpBuilder { interceptors, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    interceptors.push(args[1]);
    Ok(args[0])
}

pub(crate) fn okhttp_builder_add_network_interceptor(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::OkHttpBuilder {
        network_interceptors,
        ..
    }) = payload_mut(vm, args[0])
    else {
        return Err(npe(vm));
    };
    network_interceptors.push(args[1]);
    Ok(args[0])
}

pub(crate) fn okhttp_builder_interceptors(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::OkHttpBuilder { interceptors, .. }) => interceptors.clone(),
        _ => return Err(npe(vm)),
    };
    list_alloc(vm, items)
}

pub(crate) fn okhttp_builder_network_interceptors(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::OkHttpBuilder {
            network_interceptors,
            ..
        }) => network_interceptors.clone(),
        _ => return Err(npe(vm)),
    };
    list_alloc(vm, items)
}

pub(crate) fn okhttp_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let (interceptors, network_interceptors) = match payload(vm, args[0]) {
        Some(Native::OkHttpBuilder {
            interceptors,
            network_interceptors,
        }) => (interceptors.clone(), network_interceptors.clone()),
        _ => return Err(npe(vm)),
    };
    alloc(
        vm,
        "Lokhttp3/OkHttpClient;",
        Native::OkHttpClient {
            interceptors,
            network_interceptors,
        },
    )
}

// ---- request building (FormBody / HttpUrl / RequestsKt) ----

pub(crate) fn lazy_http_url_companion(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lokhttp3/HttpUrl$Companion;")
}

pub(crate) fn lazy_response_body_companion(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lokhttp3/ResponseBody$Companion;")
}

pub(crate) fn lazy_cache_control_force_network(vm: &mut Vm) -> JValue {
    alloc(
        vm,
        "Lokhttp3/CacheControl;",
        Native::CacheControl {
            max_age: -1,
            no_cache: true,
            no_store: false,
            max_stale: -1,
        },
    )
    .expect("CacheControl alloc")
}

pub(crate) fn lazy_cache_control_force_cache(vm: &mut Vm) -> JValue {
    alloc(
        vm,
        "Lokhttp3/CacheControl;",
        Native::CacheControl {
            max_age: -1,
            no_cache: false,
            no_store: false,
            max_stale: i64::MAX,
        },
    )
    .expect("CacheControl alloc")
}

pub(crate) fn lazy_media_type_companion(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lokhttp3/MediaType$Companion;")
}

pub(crate) fn okhttp_form_builder_init(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lokhttp3/FormBody$Builder;",
        Native::FormBody(Vec::new()),
    )
}

pub(crate) fn okhttp_form_builder_add(vm: &mut Vm, args: &[JValue]) -> R {
    let (Some(name), Some(value)) = (jstr(vm, args[1]).ok(), jstr(vm, args[2]).ok()) else {
        return Err(npe(vm));
    };
    let Some(Native::FormBody(fields)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    fields.push((name, value));
    Ok(args[0])
}

pub(crate) fn okhttp_form_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::FormBody(fields)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(vm, "Lokhttp3/FormBody;", Native::FormBody(fields.clone()))
}

pub(crate) fn media_type_get(vm: &mut Vm, args: &[JValue]) -> R {
    let mt = jstr(vm, args[1])?;
    alloc(vm, "Lokhttp3/MediaType;", Native::Str(mt))
}

pub(crate) fn okhttp_http_url_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = jstr(vm, args[1]).ok() else {
        return Err(npe(vm));
    };
    let url = url.trim();
    if !valid_http_url(url) {
        return Ok(JValue::Null);
    }
    alloc(vm, "Lokhttp3/HttpUrl;", Native::HttpUrl(url.to_string()))
}

/// okhttp's `HttpUrl.parse` returns `null` for anything that is not a
/// well-formed URL (missing scheme, missing host, invalid characters).
fn valid_http_url(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once("://") else {
        return false;
    };
    if scheme.is_empty()
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return false;
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty()
}

pub(crate) fn okhttp_http_url_new_builder(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::HttpUrl(url)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(
        vm,
        "Lokhttp3/HttpUrl$Builder;",
        Native::HttpUrl(url.clone()),
    )
}

pub(crate) fn okhttp_http_url_builder_add_query(vm: &mut Vm, args: &[JValue]) -> R {
    let (Some(name), Some(value)) = (jstr(vm, args[1]).ok(), jstr(vm, args[2]).ok()) else {
        return Err(npe(vm));
    };
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if url.contains('?') {
        url.push('&');
    } else {
        url.push('?');
    }
    url.push_str(&name);
    url.push('=');
    url.push_str(&value);
    Ok(args[0])
}

fn encode_path_segment(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(encoded, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    encoded
}

fn append_path_segment(url: &mut String, segment: &str) {
    let suffix_at = url.find(['?', '#']).unwrap_or(url.len());
    let suffix = url.split_off(suffix_at);
    if !url.ends_with('/') {
        url.push('/');
    }
    url.push_str(&encode_path_segment(segment));
    url.push_str(&suffix);
}

pub(crate) fn okhttp_http_url_builder_add_path_segment(vm: &mut Vm, args: &[JValue]) -> R {
    let segment = jstr(vm, args[1])?;
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    append_path_segment(url, &segment);
    Ok(args[0])
}

pub(crate) fn okhttp_http_url_builder_add_path_segments(vm: &mut Vm, args: &[JValue]) -> R {
    let path = jstr(vm, args[1])?;
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    for segment in path.split(['/', '\\']) {
        append_path_segment(url, segment);
    }
    Ok(args[0])
}

pub(crate) fn okhttp_http_url_builder_set_path_segment(vm: &mut Vm, args: &[JValue]) -> R {
    let index = int_of(vm, args[1]);
    let segment = jstr(vm, args[2])?;
    if index < 0 {
        return Err(iae(vm, "unexpected path segment index"));
    }
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let suffix_at = url.find(['?', '#']).unwrap_or(url.len());
    let suffix = url[suffix_at..].to_string();
    let prefix_end = url
        .find("://")
        .map(|scheme| scheme + 3)
        .and_then(|start| url[start..].find('/').map(|slash| start + slash))
        .unwrap_or(suffix_at);
    let prefix = url[..prefix_end].to_string();
    let mut segments: Vec<String> = http_url_encoded_path_value(url)
        .trim_start_matches('/')
        .split('/')
        .map(str::to_string)
        .collect();
    let Some(slot) = segments.get_mut(index as usize) else {
        return Err(iae(vm, "unexpected path segment index"));
    };
    *slot = encode_path_segment(&segment);
    *url = format!("{prefix}/{}{suffix}", segments.join("/"));
    Ok(args[0])
}

pub(crate) fn okhttp_http_url_builder_fragment(vm: &mut Vm, args: &[JValue]) -> R {
    let fragment = if args[1].is_null() {
        None
    } else {
        Some(jstr(vm, args[1])?)
    };
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if let Some(index) = url.find('#') {
        url.truncate(index);
    }
    if let Some(fragment) = fragment {
        url.push('#');
        url.push_str(&fragment.replace(' ', "%20"));
    }
    Ok(args[0])
}

pub(crate) fn okhttp_request_builder_url(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = payload(vm, args[1]).and_then(|n| match n {
        Native::HttpUrl(u) => Some(u.clone()),
        _ => None,
    }) else {
        return Err(npe(vm));
    };
    let Some(Native::RequestBuilder { url: dst, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = url.clone();
    Ok(args[0])
}

pub(crate) fn okhttp_http_url_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::HttpUrl(url)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(vm, "Lokhttp3/HttpUrl;", Native::HttpUrl(url.clone()))
}

pub(crate) fn okhttp_http_url_builder_set_query(vm: &mut Vm, args: &[JValue]) -> R {
    let (Some(name), Some(value)) = (jstr(vm, args[1]).ok(), jstr(vm, args[2]).ok()) else {
        return Err(npe(vm));
    };
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if url.contains('?') {
        url.push('&');
    } else {
        url.push('?');
    }
    url.push_str(&name);
    url.push('=');
    url.push_str(&value);
    Ok(args[0])
}

pub(crate) fn okhttp_http_url_builder_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::HttpUrl(url)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let s = url.clone();
    Ok(vm.alloc_string(&s))
}

// ---------------------------------------------------------------------------
// audit-gap natives: companions, cookies, headers, HttpUrl building, misc
// ---------------------------------------------------------------------------

fn http_url_parts(url: &str) -> Option<(String, String, String, String)> {
    let scheme_end = url.find("://")? + 3;
    let scheme = url[..scheme_end - 3].to_string();
    let rest = &url[scheme_end..];
    let auth_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = rest[..auth_end].to_string();
    let rest = &rest[auth_end..];
    let suffix_at = rest.find(['?', '#']).unwrap_or(rest.len());
    let path = rest[..suffix_at].to_string();
    let suffix = rest[suffix_at..].to_string();
    Some((scheme, authority, path, suffix))
}

fn normalize_dot_segments(url: &str) -> String {
    let Some((scheme, authority, path, suffix)) = http_url_parts(url) else {
        return url.to_string();
    };
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "." | "" => {}
            ".." => {
                segments.pop();
            }
            s => segments.push(s),
        }
    }
    if segments.is_empty() {
        format!("{scheme}://{authority}{suffix}")
    } else {
        format!("{scheme}://{authority}/{}", segments.join("/")) + &suffix
    }
}

fn http_resolve(base: &str, link: &str) -> String {
    if link.contains("://") {
        return normalize_dot_segments(link);
    }
    if link.starts_with("//") {
        let scheme = base.split("://").next().unwrap_or("http").to_string();
        return normalize_dot_segments(&format!("{scheme}:{link}"));
    }
    let Some((scheme, authority, path, _)) = http_url_parts(base) else {
        return link.to_string();
    };
    let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let joined = if link.starts_with('/') {
        format!("{scheme}://{authority}{link}")
    } else {
        format!("{scheme}://{authority}{dir}/{link}")
    };
    normalize_dot_segments(&joined)
}

fn query_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => (pair.to_string(), String::new()),
        })
        .collect()
}

fn pairs_to_query(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn split_query_fragment(suffix: &str) -> (String, String) {
    let query = suffix.strip_prefix('?').unwrap_or("");
    match query.split_once('#') {
        Some((query, fragment)) => (query.to_string(), format!("#{fragment}")),
        None => (query.to_string(), String::new()),
    }
}

fn append_raw_path_segment(url: &mut String, segment: &str) {
    let suffix_at = url.find(['?', '#']).unwrap_or(url.len());
    let suffix = url.split_off(suffix_at);
    if !url.ends_with('/') {
        url.push('/');
    }
    url.push_str(segment);
    url.push_str(&suffix);
}

fn http_url_host_opt(vm: &Vm, v: JValue) -> Option<String> {
    let url = match payload(vm, v) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return None,
    };
    Some(
        url.split("://")
            .nth(1)?
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .to_string(),
    )
}

pub(crate) fn response_body_companion_create_source(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = match payload(vm, args[1]) {
        Some(Native::OkioBuf { bytes, pos }) => bytes[*pos..].to_vec(),
        _ => Vec::new(),
    };
    alloc(vm, "Lokhttp3/ResponseBody;", Native::RespBody(bytes))
}

pub(crate) fn response_body_companion_create_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    alloc(vm, "Lokhttp3/ResponseBody;", Native::RespBody(bytes))
}

pub(crate) fn response_body_companion_create_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[1])?;
    alloc(
        vm,
        "Lokhttp3/ResponseBody;",
        Native::RespBody(s.into_bytes()),
    )
}

pub(crate) fn response_body_content_type(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn cookie_name(vm: &mut Vm, args: &[JValue]) -> R {
    let name = match payload(vm, args[0]) {
        Some(Native::Cookie { name, .. }) => name.clone(),
        _ => return Ok(JValue::Null),
    };
    Ok(vm.alloc_string(&name))
}

pub(crate) fn cookie_value(vm: &mut Vm, args: &[JValue]) -> R {
    let value = match payload(vm, args[0]) {
        Some(Native::Cookie { value, .. }) => value.clone(),
        _ => return Ok(JValue::Null),
    };
    Ok(vm.alloc_string(&value))
}

pub(crate) fn cookie_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let (name, value) = match payload(vm, args[0]) {
        Some(Native::Cookie { name, value, .. }) => (name.clone(), value.clone()),
        _ => (String::new(), String::new()),
    };
    Ok(vm.alloc_string(&format!("{name}={value}")))
}

pub(crate) fn cookie_expires_at(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Long(0))
}

pub(crate) fn cookie_matches(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(1))
}

pub(crate) fn cookie_builder_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Cookie {
        name: String::new(),
        value: String::new(),
    });
    Ok(JValue::Null)
}

fn cookie_builder_set(vm: &mut Vm, args: &[JValue], set_value: bool) -> R {
    let s = jstr(vm, args[1])?;
    let Some(Native::Cookie { name, value }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if set_value {
        *value = s;
    } else {
        *name = s;
    }
    Ok(args[0])
}

pub(crate) fn cookie_builder_name(vm: &mut Vm, args: &[JValue]) -> R {
    cookie_builder_set(vm, args, false)
}

pub(crate) fn cookie_builder_value(vm: &mut Vm, args: &[JValue]) -> R {
    cookie_builder_set(vm, args, true)
}

pub(crate) fn cookie_builder_domain(_vm: &mut Vm, args: &[JValue]) -> R {
    warn!("Cookie$Builder.domain(): cookie attributes are not modeled");
    Ok(args[0])
}

pub(crate) fn cookie_builder_path(_vm: &mut Vm, args: &[JValue]) -> R {
    warn!("Cookie$Builder.path(): cookie attributes are not modeled");
    Ok(args[0])
}

pub(crate) fn cookie_builder_expires_at(_vm: &mut Vm, args: &[JValue]) -> R {
    warn!("Cookie$Builder.expiresAt(): cookie attributes are not modeled");
    Ok(args[0])
}

pub(crate) fn cookie_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let (name, value) = match payload(vm, args[0]) {
        Some(Native::Cookie { name, value }) => (name.clone(), value.clone()),
        _ => (String::new(), String::new()),
    };
    alloc(vm, "Lokhttp3/Cookie;", Native::Cookie { name, value })
}

/// Cookie persistence is host-owned (see [`Context::set_host_headers`]);
/// the in-VM jar stays empty.
pub(crate) fn cookie_jar_load_for_request(vm: &mut Vm, _args: &[JValue]) -> R {
    list_alloc(vm, Vec::new())
}

pub(crate) fn cookie_jar_save_from_response(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// `OkHttpClient.cookieJar()` — Cookie persistence is host-owned, so the
/// jar is an inert placeholder.
pub(crate) fn okhttp_client_cookie_jar(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lokhttp3/CookieJar;", Native::Opaque)
}

pub(crate) fn okhttp_client_interceptors(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::OkHttpClient { interceptors, .. }) => interceptors.clone(),
        _ => return Err(npe(vm)),
    };
    list_alloc(vm, items)
}

pub(crate) fn okhttp_builder_self(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn headers_companion_of_array(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Obj(items))) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut headers = Vec::new();
    let mut iter = items.iter();
    while let (Some(k), Some(v)) = (iter.next(), iter.next()) {
        headers.push((jstr(vm, *k)?, jstr(vm, *v)?));
    }
    alloc(vm, HEADERS, Native::Headers(headers))
}

pub(crate) fn headers_companion_of_map(vm: &mut Vm, args: &[JValue]) -> R {
    let pairs = match payload(vm, args[1]) {
        Some(Native::Map(pairs)) => pairs.clone(),
        _ => return Err(npe(vm)),
    };
    let mut headers = Vec::new();
    for (k, v) in pairs {
        headers.push((jstr(vm, k)?, jstr(vm, v)?));
    }
    alloc(vm, HEADERS, Native::Headers(headers))
}

pub(crate) fn headers_builder_add_line(vm: &mut Vm, args: &[JValue]) -> R {
    let line = jstr(vm, args[1])?;
    let Some((name, value)) = line.split_once(':') else {
        return Err(iae(vm, "Header must contain a ':'"));
    };
    let Some(Native::Headers(headers)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    headers.push((name.trim().to_string(), value.trim().to_string()));
    Ok(args[0])
}

pub(crate) fn headers_builder_add_all(vm: &mut Vm, args: &[JValue]) -> R {
    let src = match payload(vm, args[1]) {
        Some(Native::Headers(src)) => src.clone(),
        _ => return Err(npe(vm)),
    };
    let Some(Native::Headers(headers)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    headers.extend(src);
    Ok(args[0])
}

pub(crate) fn headers_iterator(vm: &mut Vm, args: &[JValue]) -> R {
    let headers = match payload(vm, args[0]) {
        Some(Native::Headers(headers)) => headers.clone(),
        _ => return Err(npe(vm)),
    };
    let lines: Vec<JValue> = headers
        .iter()
        .map(|(k, v)| vm.alloc_string(&format!("{k}: {v}")))
        .collect();
    let list = list_alloc(vm, lines)?;
    alloc(
        vm,
        "Ljava/util/Iterator;",
        Native::Iter(IterKind::List {
            list: list.as_obj(),
            idx: 0,
        }),
    )
}

pub(crate) fn headers_names(vm: &mut Vm, args: &[JValue]) -> R {
    let headers = match payload(vm, args[0]) {
        Some(Native::Headers(headers)) => headers.clone(),
        _ => return Err(npe(vm)),
    };
    let mut seen: Vec<String> = Vec::new();
    let mut names = Vec::new();
    for (k, _) in &headers {
        if !seen.iter().any(|s| s.eq_ignore_ascii_case(k)) {
            seen.push(k.clone());
            names.push(vm.alloc_string(k));
        }
    }
    set_alloc(vm, names)
}

pub(crate) fn headers_to_multimap(vm: &mut Vm, args: &[JValue]) -> R {
    let headers = match payload(vm, args[0]) {
        Some(Native::Headers(headers)) => headers.clone(),
        _ => return Err(npe(vm)),
    };
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for (k, v) in &headers {
        match groups.iter_mut().find(|(g, _)| g.eq_ignore_ascii_case(k)) {
            Some((_, vals)) => vals.push(v.clone()),
            None => groups.push((k.clone(), vec![v.clone()])),
        }
    }
    let mut pairs = Vec::new();
    for (k, vals) in groups {
        let items = vals.into_iter().map(|v| vm.alloc_string(&v)).collect();
        pairs.push((vm.alloc_string(&k), list_alloc(vm, items)?));
    }
    alloc(vm, "Ljava/util/LinkedHashMap;", Native::Map(pairs))
}

pub(crate) fn http_url_query_parameter_names(vm: &mut Vm, args: &[JValue]) -> R {
    let url = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Err(npe(vm)),
    };
    let query = url
        .split_once('?')
        .map(|(_, q)| q.split('#').next().unwrap_or(""))
        .unwrap_or("");
    let mut names = Vec::new();
    let mut seen = Vec::new();
    for (k, _) in query_pairs(query) {
        if !seen.contains(&k) {
            seen.push(k.clone());
            names.push(vm.alloc_string(&k));
        }
    }
    set_alloc(vm, names)
}

pub(crate) fn http_url_query_parameter_values(vm: &mut Vm, args: &[JValue]) -> R {
    let name = match jstr(vm, args[1]) {
        Ok(name) => name,
        Err(_) => return list_alloc(vm, Vec::new()),
    };
    let url = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Err(npe(vm)),
    };
    let query = url
        .split_once('?')
        .map(|(_, q)| q.split('#').next().unwrap_or(""))
        .unwrap_or("");
    let values: Vec<JValue> = query_pairs(query)
        .into_iter()
        .filter(|(k, _)| k == &name)
        .map(|(_, v)| vm.alloc_string(&v))
        .collect();
    list_alloc(vm, values)
}

pub(crate) fn http_url_path_size(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::HttpUrl(url)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let count = http_url_encoded_path_value(url)
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .count();
    Ok(JValue::Int(count as i32))
}

pub(crate) fn http_url_encoded_query(vm: &mut Vm, args: &[JValue]) -> R {
    let url = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Err(npe(vm)),
    };
    let query = url
        .split_once('?')
        .map(|(_, q)| q.split('#').next().unwrap_or(""))
        .unwrap_or("")
        .to_string();
    Ok(vm.alloc_string(&query))
}

pub(crate) fn http_url_resolve(vm: &mut Vm, args: &[JValue]) -> R {
    let base = match payload(vm, args[0]) {
        Some(Native::HttpUrl(base)) => base.clone(),
        _ => return Err(npe(vm)),
    };
    let link = match jstr(vm, args[1]) {
        Ok(link) => link,
        Err(_) => return Ok(JValue::Null),
    };
    let resolved = http_resolve(&base, &link);
    alloc(vm, "Lokhttp3/HttpUrl;", Native::HttpUrl(resolved))
}

pub(crate) fn http_url_new_builder_string(vm: &mut Vm, args: &[JValue]) -> R {
    let base = match payload(vm, args[0]) {
        Some(Native::HttpUrl(base)) => base.clone(),
        _ => return Err(npe(vm)),
    };
    let link = match jstr(vm, args[1]) {
        Ok(link) => link,
        Err(_) => return Err(npe(vm)),
    };
    let resolved = http_resolve(&base, &link);
    alloc(vm, "Lokhttp3/HttpUrl$Builder;", Native::HttpUrl(resolved))
}

pub(crate) fn http_url_uri(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Err(npe(vm)),
    };
    alloc(vm, "Ljava/net/URI;", Native::URI(s))
}

pub(crate) fn http_url_url(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::HttpUrl(_)) => {}
        _ => return Err(npe(vm)),
    }
    alloc(vm, "Ljava/net/URL;", Native::Opaque)
}

pub(crate) fn http_url_top_private_domain(vm: &mut Vm, args: &[JValue]) -> R {
    let host = http_url_host_opt(vm, args[0]).unwrap_or_default();
    let domain = host.strip_prefix("www.").unwrap_or(&host).to_string();
    Ok(new_str(vm, &domain))
}

pub(crate) fn http_url_encoded_fragment(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::HttpUrl(url)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let fragment = url
        .split_once('#')
        .map(|(_, fragment)| fragment.to_string());
    Ok(fragment
        .map(|fragment| new_str(vm, &fragment))
        .unwrap_or(JValue::Null))
}

pub(crate) fn http_url_is_https(vm: &mut Vm, args: &[JValue]) -> R {
    let url = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(i32::from(url.starts_with("https://"))))
}

pub(crate) fn http_url_builder_scheme(vm: &mut Vm, args: &[JValue]) -> R {
    let scheme = jstr(vm, args[1])?;
    if scheme != "http" && scheme != "https" {
        return Err(iae(vm, "unexpected scheme"));
    }
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if let Some(end) = url.find("://") {
        url.replace_range(..end, &scheme);
    }
    Ok(args[0])
}

pub(crate) fn http_url_builder_host(vm: &mut Vm, args: &[JValue]) -> R {
    let host = jstr(vm, args[1])?;
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some((scheme, authority, path, suffix)) = http_url_parts(url) else {
        return Err(iae(vm, "unexpected url"));
    };
    let port = authority
        .rsplit_once(':')
        .map(|(_, p)| format!(":{p}"))
        .unwrap_or_default();
    *url = format!("{scheme}://{host}{port}{path}{suffix}");
    Ok(args[0])
}

pub(crate) fn http_url_builder_port(vm: &mut Vm, args: &[JValue]) -> R {
    let port = int_of(vm, args[1]);
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some((scheme, authority, path, suffix)) = http_url_parts(url) else {
        return Err(iae(vm, "unexpected url"));
    };
    let host = authority
        .split(':')
        .next()
        .unwrap_or(&authority)
        .to_string();
    *url = format!("{scheme}://{host}:{port}{path}{suffix}");
    Ok(args[0])
}

fn http_url_builder_set_query_impl(vm: &mut Vm, args: &[JValue]) -> R {
    let new_query = if args[1].is_null() {
        None
    } else {
        Some(jstr(vm, args[1])?)
    };
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some((scheme, authority, path, suffix)) = http_url_parts(url) else {
        return Err(iae(vm, "unexpected url"));
    };
    let (_, fragment) = split_query_fragment(&suffix);
    let query = new_query.map(|q| format!("?{q}")).unwrap_or_default();
    *url = format!("{scheme}://{authority}{path}{query}{fragment}");
    Ok(args[0])
}

pub(crate) fn http_url_builder_query(vm: &mut Vm, args: &[JValue]) -> R {
    http_url_builder_set_query_impl(vm, args)
}

pub(crate) fn http_url_builder_encoded_query(vm: &mut Vm, args: &[JValue]) -> R {
    http_url_builder_set_query_impl(vm, args)
}

pub(crate) fn http_url_builder_remove_all_query_parameters(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some((scheme, authority, path, suffix)) = http_url_parts(url) else {
        return Err(iae(vm, "unexpected url"));
    };
    let (query, fragment) = split_query_fragment(&suffix);
    let kept: Vec<(String, String)> = query_pairs(&query)
        .into_iter()
        .filter(|(k, _)| k != &name)
        .collect();
    let query = if kept.is_empty() {
        String::new()
    } else {
        format!("?{}", pairs_to_query(&kept))
    };
    *url = format!("{scheme}://{authority}{path}{query}{fragment}");
    Ok(args[0])
}

pub(crate) fn http_url_builder_set_encoded_query_parameter(vm: &mut Vm, args: &[JValue]) -> R {
    let (name, value) = (jstr(vm, args[1])?, jstr(vm, args[2])?);
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some((scheme, authority, path, suffix)) = http_url_parts(url) else {
        return Err(iae(vm, "unexpected url"));
    };
    let (query, fragment) = split_query_fragment(&suffix);
    let mut kept: Vec<(String, String)> = query_pairs(&query)
        .into_iter()
        .filter(|(k, _)| k != &name)
        .collect();
    kept.push((name, value));
    *url = format!(
        "{scheme}://{authority}{path}?{}{fragment}",
        pairs_to_query(&kept)
    );
    Ok(args[0])
}

pub(crate) fn http_url_builder_encoded_path(vm: &mut Vm, args: &[JValue]) -> R {
    let path = jstr(vm, args[1])?;
    if !path.is_empty() && !path.starts_with('/') {
        return Err(iae(vm, format!("unexpected path: {path}")));
    }
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some((scheme, authority, _, suffix)) = http_url_parts(url) else {
        return Err(iae(vm, "unexpected url"));
    };
    *url = format!("{scheme}://{authority}{path}{suffix}");
    Ok(args[0])
}

pub(crate) fn http_url_builder_add_encoded_path_segment(vm: &mut Vm, args: &[JValue]) -> R {
    let segment = jstr(vm, args[1])?;
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    append_raw_path_segment(url, &segment);
    Ok(args[0])
}

pub(crate) fn http_url_builder_add_encoded_path_segments(vm: &mut Vm, args: &[JValue]) -> R {
    let path = jstr(vm, args[1])?;
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    for segment in path.split(['/', '\\']) {
        append_raw_path_segment(url, segment);
    }
    Ok(args[0])
}

pub(crate) fn http_url_builder_remove_path_segment(vm: &mut Vm, args: &[JValue]) -> R {
    let index = int_of(vm, args[1]);
    if index < 0 {
        return Err(iae(vm, "unexpected path segment index"));
    }
    let Some(Native::HttpUrl(url)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some((scheme, authority, path, suffix)) = http_url_parts(url) else {
        return Err(iae(vm, "unexpected url"));
    };
    let mut segments: Vec<String> = path
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if index as usize >= segments.len() {
        return Err(iae(vm, "unexpected path segment index"));
    }
    segments.remove(index as usize);
    let new_path = if segments.is_empty() {
        String::new()
    } else {
        format!("/{}", segments.join("/"))
    };
    *url = format!("{scheme}://{authority}{new_path}{suffix}");
    Ok(args[0])
}

pub(crate) fn request_headers(vm: &mut Vm, args: &[JValue]) -> R {
    let headers = match payload(vm, args[0]) {
        Some(Native::Request { headers, .. }) => headers.clone(),
        _ => return Err(npe(vm)),
    };
    alloc(vm, HEADERS, Native::Headers(headers))
}

pub(crate) fn request_body_get(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Request { body, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(body.unwrap_or(JValue::Null))
}

pub(crate) fn request_builder_remove_header(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    let Some(Native::RequestBuilder { headers, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    headers.retain(|(existing, _)| !existing.eq_ignore_ascii_case(&name));
    Ok(args[0])
}

pub(crate) fn request_builder_get(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::RequestBuilder { method, body, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *method = "GET".to_string();
    *body = None;
    Ok(args[0])
}

pub(crate) fn request_builder_head(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::RequestBuilder { method, body, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *method = "HEAD".to_string();
    *body = None;
    Ok(args[0])
}

pub(crate) fn request_builder_put(vm: &mut Vm, args: &[JValue]) -> R {
    let method = new_str(vm, "PUT");
    request_builder_method(vm, &[args[0], method, args[1]])
}

pub(crate) fn request_builder_delete_default(vm: &mut Vm, args: &[JValue]) -> R {
    let method = new_str(vm, "DELETE");
    request_builder_method(vm, &[args[0], method, args[1]])
}

pub(crate) fn response_peek_body(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = match payload(vm, args[0]) {
        Some(Native::Response { body, .. }) => body.clone().unwrap_or_default(),
        _ => return Err(npe(vm)),
    };
    alloc(vm, "Lokhttp3/ResponseBody;", Native::RespBody(bytes))
}

pub(crate) fn response_is_redirect(vm: &mut Vm, args: &[JValue]) -> R {
    let code = match payload(vm, args[0]) {
        Some(Native::Response { code, .. }) => *code,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(i32::from((300..400).contains(&code))))
}

pub(crate) fn response_headers_string(vm: &mut Vm, args: &[JValue]) -> R {
    let name = match jstr(vm, args[1]) {
        Ok(name) => name,
        Err(_) => return list_alloc(vm, Vec::new()),
    };
    let headers = match payload(vm, args[0]) {
        Some(Native::Response { headers, .. }) => headers.clone(),
        _ => return Err(npe(vm)),
    };
    let values: Vec<JValue> = headers
        .iter()
        .filter(|(k, _)| k.eq_ignore_ascii_case(&name))
        .map(|(_, v)| vm.alloc_string(v))
        .collect();
    list_alloc(vm, values)
}

pub(crate) fn response_builder_protocol(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn response_builder_remove_header(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    let Some(Native::ResponseBuilder { headers, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    headers.retain(|(existing, _)| !existing.eq_ignore_ascii_case(&name));
    Ok(args[0])
}

pub(crate) fn response_builder_request(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ResponseBuilder { request, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *request = Some(args[1]);
    Ok(args[0])
}

pub(crate) fn response_builder_headers(vm: &mut Vm, args: &[JValue]) -> R {
    let src = match payload(vm, args[1]) {
        Some(Native::Headers(src)) => src.clone(),
        _ => return Err(npe(vm)),
    };
    let Some(Native::ResponseBuilder { headers, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *headers = src;
    Ok(args[0])
}

pub(crate) fn media_type_type(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    let t = s.split('/').next().unwrap_or(&s).to_string();
    Ok(new_str(vm, &t))
}

pub(crate) fn media_type_subtype(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    let st = s.split('/').nth(1).unwrap_or("").to_string();
    Ok(new_str(vm, &st))
}

pub(crate) fn media_type_companion_parse(vm: &mut Vm, args: &[JValue]) -> R {
    match jstr(vm, args[1]) {
        Ok(mt) => alloc(vm, "Lokhttp3/MediaType;", Native::Str(mt)),
        Err(_) => Ok(JValue::Null),
    }
}

pub(crate) fn form_body_size(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::FormBody(fields)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(fields.len() as i32))
}

pub(crate) fn form_body_name(vm: &mut Vm, args: &[JValue]) -> R {
    let index = int_of(vm, args[1]);
    let Some(Native::FormBody(fields)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some((name, _)) = fields.get(index as usize) else {
        return Err(ioobe(vm, index));
    };
    Ok(vm.alloc_string(&name.clone()))
}

pub(crate) fn form_body_value(vm: &mut Vm, args: &[JValue]) -> R {
    let index = int_of(vm, args[1]);
    let Some(Native::FormBody(fields)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some((_, value)) = fields.get(index as usize) else {
        return Err(ioobe(vm, index));
    };
    Ok(vm.alloc_string(&value.clone()))
}

pub(crate) fn form_body_content_length(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::FormBody(fields)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let len = fields
        .iter()
        .map(|(k, v)| k.len() + v.len() + 1)
        .sum::<usize>()
        .saturating_sub(1);
    Ok(JValue::Long(len as i64))
}

pub(crate) fn form_body_content_type(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lokhttp3/MediaType;",
        Native::Str("application/x-www-form-urlencoded".into()),
    )
}

pub(crate) fn request_body_content_length(vm: &mut Vm, args: &[JValue]) -> R {
    let len = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.len(),
        Some(Native::RespBody(b)) => b.len(),
        _ => 0,
    };
    Ok(JValue::Long(len as i64))
}

pub(crate) fn request_body_content_type(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn request_body_write_to(vm: &mut Vm, args: &[JValue]) -> R {
    let data = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.as_bytes().to_vec(),
        Some(Native::RespBody(b)) => b.clone(),
        _ => return Err(npe(vm)),
    };
    match payload_mut(vm, args[1]) {
        Some(Native::OkioSink { bytes, .. }) => bytes.extend_from_slice(&data),
        Some(Native::OkioBuf { bytes, .. }) => bytes.extend_from_slice(&data),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn request_body_companion_create_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    alloc(vm, "Lokhttp3/RequestBody;", Native::RespBody(bytes))
}

pub(crate) fn cache_control_builder_no_cache(vm: &mut Vm, args: &[JValue]) -> R {
    match payload_mut(vm, args[0]) {
        Some(Native::CacheControlBuilder { no_cache, .. }) => *no_cache = true,
        _ => return Err(npe(vm)),
    }
    Ok(args[0])
}

pub(crate) fn cache_control_builder_no_store(vm: &mut Vm, args: &[JValue]) -> R {
    match payload_mut(vm, args[0]) {
        Some(Native::CacheControlBuilder { no_store, .. }) => *no_store = true,
        _ => return Err(npe(vm)),
    }
    Ok(args[0])
}

pub(crate) fn cache_control_builder_max_stale(vm: &mut Vm, args: &[JValue]) -> R {
    let value = long_of(vm, args[1]).max(0);
    match payload_mut(vm, args[0]) {
        Some(Native::CacheControlBuilder { max_stale, .. }) => *max_stale = value,
        _ => return Err(npe(vm)),
    }
    Ok(args[0])
}

pub(crate) fn cache_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Cache { closed: false });
    warn!("Lokhttp3/Cache: on-disk response caching is not implemented");
    Ok(JValue::Null)
}

pub(crate) fn dispatcher_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Dispatcher {
        max_requests: 64,
        max_requests_per_host: 5,
    });
    Ok(JValue::Null)
}

pub(crate) fn dispatcher_set_max_requests(vm: &mut Vm, args: &[JValue]) -> R {
    let value = int_of(vm, args[1]).max(1);
    match payload_mut(vm, args[0]) {
        Some(Native::Dispatcher { max_requests, .. }) => *max_requests = value,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn dispatcher_set_max_requests_per_host(vm: &mut Vm, args: &[JValue]) -> R {
    let value = int_of(vm, args[1]).max(1);
    match payload_mut(vm, args[0]) {
        Some(Native::Dispatcher {
            max_requests_per_host,
            ..
        }) => *max_requests_per_host = value,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn cache_close(vm: &mut Vm, args: &[JValue]) -> R {
    match payload_mut(vm, args[0]) {
        Some(Native::Cache { closed }) => *closed = true,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn dns_lookup(vm: &mut Vm, _args: &[JValue]) -> R {
    warn!("Dns.lookup(): no InetAddresses are modeled; the host resolves via HttpUrl");
    list_alloc(vm, Vec::new())
}

pub(crate) fn close_quietly(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn credentials_basic_default(vm: &mut Vm, args: &[JValue]) -> R {
    let (user, pass) = match (jstr(vm, args[1]), jstr(vm, args[2])) {
        (Ok(user), Ok(pass)) => (user, pass),
        _ => return Err(npe(vm)),
    };
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    let token = STANDARD.encode(format!("{user}:{pass}"));
    Ok(new_str(vm, &format!("Basic {token}")))
}

pub(crate) fn multipart_builder_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Opaque);
    Ok(JValue::Null)
}

pub(crate) fn multipart_builder_add_form_data_part(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn multipart_builder_set_type(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn multipart_builder_build(_vm: &mut Vm, _args: &[JValue]) -> R {
    warn!("MultipartBody: multipart/form-data bodies are not forwarded to the host");
    Ok(JValue::Null)
}

pub(crate) fn http_url_query(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::HttpUrl(url)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let query = url
        .split_once('?')
        .map(|(_, q)| q.split('#').next().unwrap_or("").to_string())
        .unwrap_or_default();
    if query.is_empty() {
        Ok(JValue::Null)
    } else {
        Ok(vm.alloc_string(&query))
    }
}

pub(crate) fn response_body_companion_create_source_default(vm: &mut Vm, args: &[JValue]) -> R {
    response_body_companion_create_source(vm, &[args[0], args[1], args[2], args[3]])
}

pub(crate) fn request_body_companion_create_string_default(vm: &mut Vm, args: &[JValue]) -> R {
    request_body_create_string(vm, &[args[0], args[1]])
}

pub(crate) fn request_body_companion_create_bytes_default(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let mask = int_of(vm, args[5]);
    let offset = if mask & 4 != 0 {
        0
    } else {
        int_of(vm, args[3])
    };
    let byte_count = if mask & 8 != 0 {
        bytes.len().saturating_sub(offset.max(0) as usize)
    } else {
        int_of(vm, args[4]).max(0) as usize
    };
    let data: Vec<u8> = bytes
        .iter()
        .skip(offset.max(0) as usize)
        .take(byte_count)
        .copied()
        .collect();
    alloc(vm, "Lokhttp3/RequestBody;", Native::RespBody(data))
}

// ---------------------------------------------------------------------------
// okhttp3 native table
// ---------------------------------------------------------------------------
/// Lokhttp3/brotli/Brotli.INSTANCE & co (compression algorithms).
pub fn lazy_brotli_inst(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lokhttp3/brotli/Brotli;")
}
pub fn lazy_gzip_inst(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lokhttp3/Gzip;")
}
pub fn lazy_zstd_inst(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lokhttp3/zstd/Zstd;")
}

#[cfg(feature = "okhttp")]
pub(crate) fn okhttp_client_new_call(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::OkHttpClient { .. }) | Some(Native::Opaque) => {}
        _ => return Err(npe(vm)),
    };
    alloc(
        vm,
        "Lokhttp3/Call;",
        Native::Call {
            request: args[1],
            client: args[0],
            canceled: false,
        },
    )
}

#[cfg(feature = "tachiyomi")]
pub(crate) fn okhttp_call_execute(vm: &mut Vm, args: &[JValue]) -> R {
    let call = args[0];
    let (request, client) = match payload(vm, call) {
        Some(Native::Call {
            request,
            client,
            canceled,
        }) => {
            if *canceled {
                return Err(ioe(vm, "Canceled"));
            }
            (*request, *client)
        }
        // legacy: Call payload was a bare Request
        Some(Native::Request { .. }) => (call, JValue::Null),
        _ => return Err(npe(vm)),
    };
    let interceptors = match payload(vm, client) {
        Some(Native::OkHttpClient {
            interceptors,
            network_interceptors,
            ..
        }) => {
            let mut v = interceptors.clone();
            v.extend(network_interceptors.clone());
            v
        }
        _ => Vec::new(),
    };
    if interceptors.is_empty() {
        return host_execute(vm, request);
    }
    let chain = alloc(
        vm,
        "Lokhttp3/Interceptor$Chain;",
        Native::Chain {
            interceptors,
            pos: 0,
            request,
            call,
        },
    )?;
    let first = match payload(vm, chain) {
        Some(Native::Chain { interceptors, .. }) => interceptors[0],
        _ => unreachable!(),
    };
    let resp = vm
        .invoke_virtual_args(
            first,
            "intercept",
            "(Lokhttp3/Interceptor$Chain;)Lokhttp3/Response;",
            vec![chain],
        )
        .map_err(nat_fatal)?;
    Ok(resp)
}

#[cfg(feature = "tachiyomi")]
fn okhttp_call_enqueue(vm: &mut Vm, args: &[JValue]) -> R {
    let call = args[0];
    let callback = args[1];
    match okhttp_call_execute(vm, &[call]) {
        Ok(response) => {
            inv_virt(
                vm,
                callback,
                "onResponse",
                "(Lokhttp3/Call;Lokhttp3/Response;)V",
                &[call, response],
            )?;
        }
        Err(NatErr::Throw(error)) => {
            inv_virt(
                vm,
                callback,
                "onFailure",
                "(Lokhttp3/Call;Ljava/io/IOException;)V",
                &[call, JValue::Obj(error)],
            )?;
        }
        Err(error) => return Err(error),
    }
    Ok(JValue::Null)
}

/// Runs the real host HTTP request for `request` and wraps it as a Response.
/// Before dispatch, the registered host header resolver (see
/// [`Context::set_host_headers`](crate::Context::set_host_headers)) supplies
/// the User-Agent and Cookie header values for the request host, unless the
/// request already sets them.
fn host_execute(vm: &mut Vm, request: JValue) -> R {
    let (url, method, mut headers, body) = request_parts(vm, request)?;
    crate::vm::native::keiyoushi::check_network_url(vm, &url)?;
    let host = url_host_and_path(&url).0;
    if let Some(resolve) = &vm.host_headers {
        let (ua, cookie) = resolve(&host);
        if let Some(ua) = ua {
            if !headers
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("User-Agent"))
            {
                headers.push(("User-Agent".to_string(), ua));
            }
        }
        if let Some(cookie) = cookie {
            if !headers
                .iter()
                .any(|(k, _)| k.eq_ignore_ascii_case("Cookie"))
            {
                headers.push(("Cookie".to_string(), cookie));
            }
        }
    }
    info!("DBG HOST fetch {method} {url} hdrs={}", headers.len());
    let body_str = form_body_to_string(vm, &body);
    let Some(http) = vm.http.clone() else {
        return Err(uoe(vm, "no HTTP client registered for this SourceEngine"));
    };
    let resp = http(&crate::vm::native::keiyoushi::HttpData {
        url,
        method,
        headers,
        body: body_str,
    });
    if std::env::var("DEXVM_TRACE").is_ok() {
        let preview: String = resp
            .body
            .as_deref()
            .unwrap_or_default()
            .iter()
            .take(60)
            .map(|b| char::from(*b))
            .collect();
        eprintln!(
            "DEXVM_TRACE http {} code={} body={}",
            resp.code,
            resp.message,
            preview.escape_default().take(120).collect::<String>()
        );
    }
    alloc(
        vm,
        RESPONSE,
        Native::Response {
            code: resp.code,
            message: resp.message,
            headers: resp.headers,
            body: resp.body,
            request,
            prior: JValue::Null,
        },
    )
}

// ---------------------------------------------------------------------------
// interceptor chains
// ---------------------------------------------------------------------------

const INTERCEPT_SIG: &str = "(Lokhttp3/Interceptor$Chain;)Lokhttp3/Response;";

pub(crate) fn chain_request(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Chain { request, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(*request)
}

pub(crate) fn chain_call(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Chain { call, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(*call)
}

pub(crate) fn chain_connection(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn chain_proceed(vm: &mut Vm, args: &[JValue]) -> R {
    let request = args[1];
    let mut next = None;
    {
        let Some(Native::Chain {
            interceptors,
            pos,
            request: rq,
            ..
        }) = payload_mut(vm, args[0])
        else {
            return Err(npe(vm));
        };
        *rq = request;
        if *pos < interceptors.len() {
            next = Some(interceptors[*pos]);
            *pos += 1;
        }
    }
    if next.is_some() {
        let cls = match next {
            Some(JValue::Obj(o)) => vm.class_desc_str(vm.object_class(JValue::Obj(o)).unwrap_or(0)),
            _ => String::new(),
        };
        info!("DBG chain.proceed -> {cls}");
    }
    match next {
        Some(interceptor) => vm
            .invoke_virtual_args(interceptor, "intercept", INTERCEPT_SIG, vec![args[0]])
            .map_err(nat_fatal),
        None => host_execute(vm, request),
    }
}

pub(crate) fn response_new_builder(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Response {
        code,
        message,
        headers,
        request,
        ..
    }) = payload(vm, args[0])
    else {
        return Err(npe(vm));
    };
    let (code, message, headers, request) = (*code, message.clone(), headers.clone(), *request);
    alloc(
        vm,
        "Lokhttp3/Response$Builder;",
        Native::ResponseBuilder {
            code,
            message,
            headers,
            body: None,
            request: Some(request),
            prior: None,
        },
    )
}

pub(crate) fn response_builder_body(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ResponseBuilder { body, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *body = Some(args[1]);
    Ok(args[0])
}

pub(crate) fn response_builder_header(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    let value = jstr(vm, args[2])?;
    let Some(Native::ResponseBuilder { headers, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    headers.retain(|(existing, _)| !existing.eq_ignore_ascii_case(&name));
    headers.push((name, value));
    Ok(args[0])
}

pub(crate) fn response_builder_code(vm: &mut Vm, args: &[JValue]) -> R {
    let value = int_of(vm, args[1]);
    let Some(Native::ResponseBuilder { code, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *code = value;
    Ok(args[0])
}

pub(crate) fn response_builder_message(vm: &mut Vm, args: &[JValue]) -> R {
    let message = jstr(vm, args[1])?;
    let Some(Native::ResponseBuilder { message: dst, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = message;
    Ok(args[0])
}

pub(crate) fn request_body_create_string(vm: &mut Vm, args: &[JValue]) -> R {
    let content = jstr(vm, args[1])?;
    alloc(vm, "Lokhttp3/RequestBody;", Native::Str(content))
}

pub(crate) fn request_builder_post(vm: &mut Vm, args: &[JValue]) -> R {
    let method = new_str(vm, "POST");
    request_builder_method(vm, &[args[0], method, args[1]])
}

pub(crate) fn response_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let (code, message, headers, body, request, prior) = match payload(vm, args[0]) {
        Some(Native::ResponseBuilder {
            code,
            message,
            headers,
            body,
            request,
            prior,
        }) => (
            *code,
            message.clone(),
            headers.clone(),
            *body,
            *request,
            *prior,
        ),
        _ => return Err(npe(vm)),
    };
    let body = match body {
        Some(b) => match payload(vm, b) {
            Some(Native::RespBody(bs)) => Some(bs.clone()),
            Some(Native::Str(s)) => Some(s.clone().into_bytes()),
            _ => vm.object_bytes(b),
        },
        None => None,
    };
    alloc(
        vm,
        RESPONSE,
        Native::Response {
            code,
            message,
            headers,
            body,
            request: request.unwrap_or(JValue::Null),
            prior: prior.unwrap_or(JValue::Null),
        },
    )
}

pub(crate) const OKHTTP_TABLE: &[NativeEntry] = &[
    #[cfg(feature = "okhttp")]
    ne!("Lokhttp3/OkHttpClient;", "newCall", "(Lokhttp3/Request;)Lokhttp3/Call;", true, okhttp_client_new_call),
    #[cfg(feature = "tachiyomi")]
    ne!("Lokhttp3/Call;", "execute", "()Lokhttp3/Response;", true, okhttp_call_execute),
    #[cfg(feature = "tachiyomi")]
    ne!("Lokhttp3/Call;", "enqueue", "(Lokhttp3/Callback;)V", true, okhttp_call_enqueue),
    ne!("Lokhttp3/Request$Builder;", "<init>", "()V", true, request_builder_init),
    ne!("Lokhttp3/Request$Builder;", "url", "(Ljava/lang/String;)Lokhttp3/Request$Builder;", true, request_builder_url),
    ne!("Lokhttp3/Request$Builder;", "method", "(Ljava/lang/String;Lokhttp3/RequestBody;)Lokhttp3/Request$Builder;", true, request_builder_method),
    ne!("Lokhttp3/Request$Builder;", "header", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/Request$Builder;", true, request_builder_header),
    ne!("Lokhttp3/Request$Builder;", "addHeader", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/Request$Builder;", true, request_builder_add_header),
    ne!("Lokhttp3/Request$Builder;", "headers", "(Lokhttp3/Headers;)Lokhttp3/Request$Builder;", true, request_builder_headers),
    ne!("Lokhttp3/Request$Builder;", "cacheControl", "(Lokhttp3/CacheControl;)Lokhttp3/Request$Builder;", true, request_builder_cache_control),
    ne!("Lokhttp3/Request$Builder;", "tag", "(Ljava/lang/Class;)Lokhttp3/Request$Builder;", true, request_builder_tag),
    ne!("Lokhttp3/Request$Builder;", "tag", "(Ljava/lang/Class;Ljava/lang/Object;)Lokhttp3/Request$Builder;", true, request_builder_tag2),
    ne!("Lokhttp3/Request$Builder;", "build", "()Lokhttp3/Request;", true, request_builder_build),
    ne!("Lokhttp3/Request;", "cacheControl", "()Lokhttp3/CacheControl;", true, request_cache_control),
    ne!("Lokhttp3/CacheControl;", "maxAgeSeconds", "()I", true, cache_control_max_age_seconds),
    ne!("Lokhttp3/CacheControl;", "noCache", "()Z", true, cache_control_no_cache),
    ne!("Lokhttp3/CacheControl$Builder;", "<init>", "()V", true, cache_control_builder_init),
    ne!("Lokhttp3/CacheControl$Builder;", "maxAge-LRDsOJo", "(J)Lokhttp3/CacheControl$Builder;", true, cache_control_builder_max_age),
    ne!("Lokhttp3/CacheControl$Builder;", "build", "()Lokhttp3/CacheControl;", true, cache_control_builder_build),
    ne!("Lokhttp3/Request;", "newBuilder", "()Lokhttp3/Request$Builder;", true, request_new_builder),
    ne!("Lokhttp3/Request;", "url", "()Lokhttp3/HttpUrl;", true, request_url),
    ne!("Lokhttp3/Request;", "method", "()Ljava/lang/String;", true, request_method),
    ne!("Lokhttp3/Request;", "header", "(Ljava/lang/String;)Ljava/lang/String;", true, request_header),
    ne!("Lokhttp3/Request;", "tag", "(Ljava/lang/Class;)Ljava/lang/Object;", true, request_tag),
    ne!("Lokhttp3/Headers$Builder;", "<init>", "()V", true, headers_builder_init),
    ne!("Lokhttp3/Headers$Builder;", "add", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/Headers$Builder;", true, headers_builder_add),
    ne!("Lokhttp3/Headers$Builder;", "set", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/Headers$Builder;", true, headers_builder_set),
    ne!("Lokhttp3/Headers$Builder;", "removeAll", "(Ljava/lang/String;)Lokhttp3/Headers$Builder;", true, headers_builder_remove_all),
    ne!("Lokhttp3/Headers$Builder;", "build", "()Lokhttp3/Headers;", true, headers_builder_build),
    ne!("Lokhttp3/Headers;", "newBuilder", "()Lokhttp3/Headers$Builder;", true, headers_new_builder),
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
    ne!("Lokhttp3/ResponseBody;", "byteStream", "()Ljava/io/InputStream;", true, response_body_bytes_stream),
    ne!("Lokhttp3/ResponseBody;", "bytes", "()[B", true, response_body_bytes_arr),
    ne!("Lokhttp3/ResponseBody;", "contentLength", "()J", true, response_body_len),
    ne!("Lokhttp3/ResponseBody;", "close", "()V", true, response_close),
    ne!("Lokhttp3/ResponseBody;", "source", "()Lokio/BufferedSource;", true, okio_source_response_body),
    ne!("Lokhttp3/Interceptor$Chain;", "request", "()Lokhttp3/Request;", true, chain_request),
    ne!("Lokhttp3/Interceptor$Chain;", "proceed", "(Lokhttp3/Request;)Lokhttp3/Response;", true, chain_proceed),
    ne!("Lokhttp3/Interceptor$Chain;", "call", "()Lokhttp3/Call;", true, chain_call),
    ne!("Lokhttp3/Call;", "isCanceled", "()Z", true, call_is_canceled),
    ne!("Lokhttp3/Call;", "cancel", "()V", true, call_cancel),
    ne!("Lokhttp3/Call;", "timeout", "()Lokhttp3/Timeout;", true, call_timeout),
    ne!("Lokhttp3/Timeout;", "timeout", "(J)Lokhttp3/Timeout;", true, timeout_timeout),
    ne!("Lokhttp3/Timeout;", "timeoutMillis", "()J", true, timeout_timeout_millis),
    ne!("Lokhttp3/Response;", "priorResponse", "()Lokhttp3/Response;", true, response_prior_response),
    ne!("Lokhttp3/Interceptor$Chain;", "connection", "()Lokhttp3/Connection;", true, chain_connection),
    ne!("Lokhttp3/Response;", "newBuilder", "()Lokhttp3/Response$Builder;", true, response_new_builder),
    ne!("Lokhttp3/Response$Builder;", "body", "(Lokhttp3/ResponseBody;)Lokhttp3/Response$Builder;", true, response_builder_body),
    ne!("Lokhttp3/Response$Builder;", "header", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/Response$Builder;", true, response_builder_header),
    ne!("Lokhttp3/Response$Builder;", "code", "(I)Lokhttp3/Response$Builder;", true, response_builder_code),
    ne!("Lokhttp3/Response$Builder;", "message", "(Ljava/lang/String;)Lokhttp3/Response$Builder;", true, response_builder_message),
    ne!("Lokhttp3/Response$Builder;", "priorResponse", "(Lokhttp3/Response;)Lokhttp3/Response$Builder;", true, response_builder_prior_response),
    ne!("Lokhttp3/Response$Builder;", "build", "()Lokhttp3/Response;", true, response_builder_build),
    ne!("Lokhttp3/HttpUrl;", "host", "()Ljava/lang/String;", true, http_url_host),
    // default-client interceptor stubs (mihon 0.17+ extensions validate them)
    ne!("Lokhttp3/CompressionInterceptor;", "<init>", "([Lokhttp3/CompressionInterceptor$DecompressionAlgorithm;)V", true, compression_interceptor_init),
    ne!("Lokhttp3/CompressionInterceptor;", "intercept", INTERCEPT_SIG, true, interceptor_pass_through),
    ne!("Lokhttp3/brotli/BrotliInterceptor;", "intercept", INTERCEPT_SIG, true, interceptor_pass_through),
    ne!("Lokhttp3/HttpUrl;", "scheme", "()Ljava/lang/String;", true, http_url_scheme),
    ne!("Lokhttp3/HttpUrl;", "queryParameter", "(Ljava/lang/String;)Ljava/lang/String;", true, http_url_query_parameter),
    ne!("Lokhttp3/HttpUrl;", "pathSegments", "()Ljava/util/List;", true, http_url_path_segments),
    ne!("Lokhttp3/HttpUrl;", "encodedPath", "()Ljava/lang/String;", true, http_url_encoded_path),
    ne!("Lokhttp3/HttpUrl;", "fragment", "()Ljava/lang/String;", true, http_url_fragment),
    ne!("Lokhttp3/HttpUrl;", "port", "()I", true, http_url_port),
    ne!("Lokhttp3/HttpUrl;", "toString", "()Ljava/lang/String;", true, http_url_to_string),
    ne!("Lokhttp3/HttpUrl$Companion;", "get", "(Ljava/lang/String;)Lokhttp3/HttpUrl;", true, okhttp_http_url_parse),
    ne!("Lokhttp3/MediaType$Companion;", "get", "(Ljava/lang/String;)Lokhttp3/MediaType;", true, media_type_get),
    ne!("Lokhttp3/OkHttpClient;", "newBuilder", "()Lokhttp3/OkHttpClient$Builder;", true, okhttp_client_new_builder),
    ne!("Lokhttp3/OkHttpClient$Builder;", "addInterceptor", "(Lokhttp3/Interceptor;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_add_interceptor),
    ne!("Lokhttp3/OkHttpClient$Builder;", "addNetworkInterceptor", "(Lokhttp3/Interceptor;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_add_network_interceptor),
    ne!("Lokhttp3/OkHttpClient$Builder;", "interceptors", "()Ljava/util/List;", true, okhttp_builder_interceptors),
    ne!("Lokhttp3/OkHttpClient$Builder;", "networkInterceptors", "()Ljava/util/List;", true, okhttp_builder_network_interceptors),
    ne!("Lokhttp3/OkHttpClient$Builder;", "build", "()Lokhttp3/OkHttpClient;", true, okhttp_builder_build),
    ne!("Lokhttp3/FormBody$Builder;", "<init>", "(Ljava/nio/charset/Charset;ILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, okhttp_form_builder_init),
    ne!("Lokhttp3/FormBody$Builder;", "add", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/FormBody$Builder;", true, okhttp_form_builder_add),
    ne!("Lokhttp3/FormBody$Builder;", "build", "()Lokhttp3/FormBody;", true, okhttp_form_builder_build),
    ne!("Lokhttp3/HttpUrl$Companion;", "parse", "(Ljava/lang/String;)Lokhttp3/HttpUrl;", true, okhttp_http_url_parse),
    ne!("Lokhttp3/HttpUrl;", "newBuilder", "()Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_new_builder),
    ne!("Lokhttp3/HttpUrl$Builder;", "addQueryParameter", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_builder_add_query),
    ne!("Lokhttp3/HttpUrl$Builder;", "addPathSegment", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_builder_add_path_segment),
    ne!("Lokhttp3/HttpUrl$Builder;", "addPathSegments", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_builder_add_path_segments),
    ne!("Lokhttp3/HttpUrl$Builder;", "setPathSegment", "(ILjava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_builder_set_path_segment),
    ne!("Lokhttp3/HttpUrl$Builder;", "fragment", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_builder_fragment),
    ne!("Lokhttp3/RequestBody$Companion;", "create", "(Ljava/lang/String;Lokhttp3/MediaType;)Lokhttp3/RequestBody;", true, request_body_create_string),
    ne!("Lokhttp3/Request$Builder;", "post", "(Lokhttp3/RequestBody;)Lokhttp3/Request$Builder;", true, request_builder_post),
    ne!("Lokhttp3/HttpUrl$Builder;", "addEncodedQueryParameter", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_builder_add_query),
    ne!("Lokhttp3/HttpUrl$Builder;", "setQueryParameter", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_builder_set_query),
    ne!("Lokhttp3/HttpUrl$Builder;", "build", "()Lokhttp3/HttpUrl;", true, okhttp_http_url_builder_build),
    ne!("Lokhttp3/Request$Builder;", "url", "(Lokhttp3/HttpUrl;)Lokhttp3/Request$Builder;", true, okhttp_request_builder_url),
    ne!("Lokhttp3/HttpUrl$Builder;", "toString", "()Ljava/lang/String;", true, okhttp_http_url_builder_to_string),
    // ---- audit-gap natives: companions, cookies, HttpUrl building, client config ----
    ne!("Lokhttp3/ResponseBody$Companion;", "create", "(Lokio/BufferedSource;Lokhttp3/MediaType;J)Lokhttp3/ResponseBody;", true, response_body_companion_create_source),
    ne!("Lokhttp3/ResponseBody$Companion;", "create", "([BLokhttp3/MediaType;)Lokhttp3/ResponseBody;", true, response_body_companion_create_bytes),
    ne!("Lokhttp3/ResponseBody$Companion;", "create", "(Ljava/lang/String;Lokhttp3/MediaType;)Lokhttp3/ResponseBody;", true, response_body_companion_create_string),
    ne!("Lokhttp3/ResponseBody$Companion;", "create$default", "(Lokhttp3/ResponseBody$Companion;Lokio/BufferedSource;Lokhttp3/MediaType;JILjava/lang/Object;)Lokhttp3/ResponseBody;", false, response_body_companion_create_source_default),
    ne!("Lokhttp3/RequestBody$Companion;", "create", "([BLokhttp3/MediaType;)Lokhttp3/RequestBody;", true, request_body_companion_create_bytes),
    ne!("Lokhttp3/RequestBody$Companion;", "create$default", "(Lokhttp3/RequestBody$Companion;[BLokhttp3/MediaType;IIILjava/lang/Object;)Lokhttp3/RequestBody;", false, request_body_companion_create_bytes_default),
    ne!("Lokhttp3/RequestBody$Companion;", "create$default", "(Lokhttp3/RequestBody$Companion;Ljava/lang/String;Lokhttp3/MediaType;ILjava/lang/Object;)Lokhttp3/RequestBody;", false, request_body_companion_create_string_default),
    ne!("Lokhttp3/Cookie;", "name", "()Ljava/lang/String;", true, cookie_name),
    ne!("Lokhttp3/Cookie;", "value", "()Ljava/lang/String;", true, cookie_value),
    ne!("Lokhttp3/Cookie;", "toString", "()Ljava/lang/String;", true, cookie_to_string),
    ne!("Lokhttp3/Cookie;", "expiresAt", "()J", true, cookie_expires_at),
    ne!("Lokhttp3/Cookie;", "matches", "(Lokhttp3/HttpUrl;)Z", true, cookie_matches),
    ne!("Lokhttp3/CookieJar;", "loadForRequest", "(Lokhttp3/HttpUrl;)Ljava/util/List;", true, cookie_jar_load_for_request),
    ne!("Lokhttp3/CookieJar;", "saveFromResponse", "(Lokhttp3/HttpUrl;Ljava/util/List;)V", true, cookie_jar_save_from_response),
    ne!("Lokhttp3/Cookie$Builder;", "<init>", "()V", true, cookie_builder_init),
    ne!("Lokhttp3/Cookie$Builder;", "build", "()Lokhttp3/Cookie;", true, cookie_builder_build),
    ne!("Lokhttp3/Cookie$Builder;", "name", "(Ljava/lang/String;)Lokhttp3/Cookie$Builder;", true, cookie_builder_name),
    ne!("Lokhttp3/Cookie$Builder;", "value", "(Ljava/lang/String;)Lokhttp3/Cookie$Builder;", true, cookie_builder_value),
    ne!("Lokhttp3/Cookie$Builder;", "domain", "(Ljava/lang/String;)Lokhttp3/Cookie$Builder;", true, cookie_builder_domain),
    ne!("Lokhttp3/Cookie$Builder;", "path", "(Ljava/lang/String;)Lokhttp3/Cookie$Builder;", true, cookie_builder_path),
    ne!("Lokhttp3/Cookie$Builder;", "expiresAt", "(J)Lokhttp3/Cookie$Builder;", true, cookie_builder_expires_at),
    ne!("Lokhttp3/Headers$Companion;", "of", "([Ljava/lang/String;)Lokhttp3/Headers;", true, headers_companion_of_array),
    ne!("Lokhttp3/Headers$Companion;", "of", "(Ljava/util/Map;)Lokhttp3/Headers;", true, headers_companion_of_map),
    ne!("Lokhttp3/Headers$Builder;", "add", "(Ljava/lang/String;)Lokhttp3/Headers$Builder;", true, headers_builder_add_line),
    ne!("Lokhttp3/Headers$Builder;", "addAll", "(Lokhttp3/Headers;)Lokhttp3/Headers$Builder;", true, headers_builder_add_all),
    ne!("Lokhttp3/Headers;", "iterator", "()Ljava/util/Iterator;", true, headers_iterator),
    ne!("Lokhttp3/Headers;", "names", "()Ljava/util/Set;", true, headers_names),
    ne!("Lokhttp3/Headers;", "toMultimap", "()Ljava/util/Map;", true, headers_to_multimap),
    ne!("Lokhttp3/MultipartBody$Builder;", "<init>", "(Ljava/lang/String;ILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, multipart_builder_init),
    ne!("Lokhttp3/MultipartBody$Builder;", "addFormDataPart", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/MultipartBody$Builder;", true, multipart_builder_add_form_data_part),
    ne!("Lokhttp3/MultipartBody$Builder;", "setType", "(Lokhttp3/MediaType;)Lokhttp3/MultipartBody$Builder;", true, multipart_builder_set_type),
    ne!("Lokhttp3/MultipartBody$Builder;", "build", "()Lokhttp3/MultipartBody;", true, multipart_builder_build),
    ne!("Lokhttp3/Credentials;", "basic$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/nio/charset/Charset;ILjava/lang/Object;)Ljava/lang/String;", false, credentials_basic_default),
    ne!("Lokhttp3/Cache;", "<init>", "(Ljava/io/File;J)V", true, cache_init),
    ne!("Lokhttp3/Cache;", "close", "()V", true, cache_close),
    ne!("Lokhttp3/Dispatcher;", "<init>", "()V", true, dispatcher_init),
    ne!("Lokhttp3/Dispatcher;", "setMaxRequests", "(I)V", true, dispatcher_set_max_requests),
    ne!("Lokhttp3/Dispatcher;", "setMaxRequestsPerHost", "(I)V", true, dispatcher_set_max_requests_per_host),
    ne!("Lokhttp3/Dns;", "lookup", "(Ljava/lang/String;)Ljava/util/List;", true, dns_lookup),
    ne!("Lokhttp3/internal/_UtilCommonKt;", "closeQuietly", "(Ljava/io/Closeable;)V", false, close_quietly),
    ne!("Lokhttp3/OkHttpClient;", "cookieJar", "()Lokhttp3/CookieJar;", true, okhttp_client_cookie_jar),
    ne!("Lokhttp3/OkHttpClient;", "interceptors", "()Ljava/util/List;", true, okhttp_client_interceptors),
    ne!("Lokhttp3/OkHttpClient$Builder;", "readTimeout-LRDsOJo", "(J)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "connectTimeout-LRDsOJo", "(J)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "writeTimeout-LRDsOJo", "(J)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "callTimeout-LRDsOJo", "(J)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "followRedirects", "(Z)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "followSslRedirects", "(Z)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "retryOnConnectionFailure", "(Z)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "hostnameVerifier", "(Ljavax/net/ssl/HostnameVerifier;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "sslSocketFactory", "(Ljavax/net/ssl/SSLSocketFactory;Ljavax/net/ssl/X509TrustManager;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "cookieJar", "(Lokhttp3/CookieJar;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "dns", "(Lokhttp3/Dns;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "cache", "(Lokhttp3/Cache;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "dispatcher", "(Lokhttp3/Dispatcher;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "authenticator", "(Lokhttp3/Authenticator;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/OkHttpClient$Builder;", "protocols", "(Ljava/util/List;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_self),
    ne!("Lokhttp3/Response$Builder;", "protocol", "(Lokhttp3/Protocol;)Lokhttp3/Response$Builder;", true, response_builder_protocol),
    ne!("Lokhttp3/Response$Builder;", "removeHeader", "(Ljava/lang/String;)Lokhttp3/Response$Builder;", true, response_builder_remove_header),
    ne!("Lokhttp3/Response$Builder;", "request", "(Lokhttp3/Request;)Lokhttp3/Response$Builder;", true, response_builder_request),
    ne!("Lokhttp3/Response$Builder;", "headers", "(Lokhttp3/Headers;)Lokhttp3/Response$Builder;", true, response_builder_headers),
    ne!("Lokhttp3/Response;", "peekBody", "(J)Lokhttp3/ResponseBody;", true, response_peek_body),
    ne!("Lokhttp3/Response;", "isRedirect", "()Z", true, response_is_redirect),
    ne!("Lokhttp3/Response;", "headers", "(Ljava/lang/String;)Ljava/util/List;", true, response_headers_string),
    ne!("Lokhttp3/Response;", "header", "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;", true, response_header_default),
    ne!("Lokhttp3/ResponseBody;", "contentType", "()Lokhttp3/MediaType;", true, response_body_content_type),
    ne!("Lokhttp3/HttpUrl;", "queryParameterNames", "()Ljava/util/Set;", true, http_url_query_parameter_names),
    ne!("Lokhttp3/HttpUrl;", "queryParameterValues", "(Ljava/lang/String;)Ljava/util/List;", true, http_url_query_parameter_values),
    ne!("Lokhttp3/HttpUrl;", "resolve", "(Ljava/lang/String;)Lokhttp3/HttpUrl;", true, http_url_resolve),
    ne!("Lokhttp3/HttpUrl;", "pathSize", "()I", true, http_url_path_size),
    ne!("Lokhttp3/HttpUrl;", "encodedQuery", "()Ljava/lang/String;", true, http_url_encoded_query),
    ne!("Lokhttp3/HttpUrl;", "encodedPathSegments", "()Ljava/util/List;", true, http_url_path_segments),
    ne!("Lokhttp3/HttpUrl;", "query", "()Ljava/lang/String;", true, http_url_query),
    ne!("Lokhttp3/HttpUrl;", "topPrivateDomain", "()Ljava/lang/String;", true, http_url_top_private_domain),
    ne!("Lokhttp3/HttpUrl;", "url", "()Ljava/net/URL;", true, http_url_url),
    ne!("Lokhttp3/HttpUrl;", "encodedFragment", "()Ljava/lang/String;", true, http_url_encoded_fragment),
    ne!("Lokhttp3/HttpUrl;", "isHttps", "()Z", true, http_url_is_https),
    ne!("Lokhttp3/HttpUrl;", "uri", "()Ljava/net/URI;", true, http_url_uri),
    ne!("Lokhttp3/HttpUrl;", "newBuilder", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, http_url_new_builder_string),
    ne!("Lokhttp3/HttpUrl$Builder;", "addEncodedPathSegments", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, http_url_builder_add_encoded_path_segments),
    ne!("Lokhttp3/HttpUrl$Builder;", "addEncodedPathSegment", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, http_url_builder_add_encoded_path_segment),
    ne!("Lokhttp3/HttpUrl$Builder;", "encodedPath", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, http_url_builder_encoded_path),
    ne!("Lokhttp3/HttpUrl$Builder;", "host", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, http_url_builder_host),
    ne!("Lokhttp3/HttpUrl$Builder;", "port", "(I)Lokhttp3/HttpUrl$Builder;", true, http_url_builder_port),
    ne!("Lokhttp3/HttpUrl$Builder;", "removeAllQueryParameters", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, http_url_builder_remove_all_query_parameters),
    ne!("Lokhttp3/HttpUrl$Builder;", "setEncodedQueryParameter", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, http_url_builder_set_encoded_query_parameter),
    ne!("Lokhttp3/HttpUrl$Builder;", "removePathSegment", "(I)Lokhttp3/HttpUrl$Builder;", true, http_url_builder_remove_path_segment),
    ne!("Lokhttp3/HttpUrl$Builder;", "query", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, http_url_builder_query),
    ne!("Lokhttp3/HttpUrl$Builder;", "encodedQuery", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, http_url_builder_encoded_query),
    ne!("Lokhttp3/HttpUrl$Builder;", "scheme", "(Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, http_url_builder_scheme),
    ne!("Lokhttp3/MediaType$Companion;", "parse", "(Ljava/lang/String;)Lokhttp3/MediaType;", true, media_type_companion_parse),
    ne!("Lokhttp3/MediaType;", "type", "()Ljava/lang/String;", true, media_type_type),
    ne!("Lokhttp3/MediaType;", "subtype", "()Ljava/lang/String;", true, media_type_subtype),
    ne!("Lokhttp3/Request;", "headers", "()Lokhttp3/Headers;", true, request_headers),
    ne!("Lokhttp3/Request;", "body", "()Lokhttp3/RequestBody;", true, request_body_get),
    ne!("Lokhttp3/Request;", "tag", "()Ljava/lang/Object;", true, request_tag),
    ne!("Lokhttp3/Request;", "tag", "(Lkotlin/reflect/KClass;)Ljava/lang/Object;", true, request_tag),
    ne!("Lokhttp3/Request$Builder;", "removeHeader", "(Ljava/lang/String;)Lokhttp3/Request$Builder;", true, request_builder_remove_header),
    ne!("Lokhttp3/Request$Builder;", "head", "()Lokhttp3/Request$Builder;", true, request_builder_head),
    ne!("Lokhttp3/Request$Builder;", "get", "()Lokhttp3/Request$Builder;", true, request_builder_get),
    ne!("Lokhttp3/Request$Builder;", "put", "(Lokhttp3/RequestBody;)Lokhttp3/Request$Builder;", true, request_builder_put),
    ne!("Lokhttp3/Request$Builder;", "delete$default", "(Lokhttp3/Request$Builder;Lokhttp3/RequestBody;ILjava/lang/Object;)Lokhttp3/Request$Builder;", false, request_builder_delete_default),
    ne!("Lokhttp3/Request$Builder;", "tag", "(Ljava/lang/Object;)Lokhttp3/Request$Builder;", true, request_builder_tag),
    ne!("Lokhttp3/Request$Builder;", "tag", "(Lkotlin/reflect/KClass;Ljava/lang/Object;)Lokhttp3/Request$Builder;", true, request_builder_tag2),
    ne!("Lokhttp3/FormBody$Builder;", "addEncoded", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/FormBody$Builder;", true, okhttp_form_builder_add),
    ne!("Lokhttp3/FormBody;", "value", "(I)Ljava/lang/String;", true, form_body_value),
    ne!("Lokhttp3/FormBody;", "name", "(I)Ljava/lang/String;", true, form_body_name),
    ne!("Lokhttp3/FormBody;", "size", "()I", true, form_body_size),
    ne!("Lokhttp3/FormBody;", "contentLength", "()J", true, form_body_content_length),
    ne!("Lokhttp3/FormBody;", "contentType", "()Lokhttp3/MediaType;", true, form_body_content_type),
    ne!("Lokhttp3/RequestBody;", "contentLength", "()J", true, request_body_content_length),
    ne!("Lokhttp3/RequestBody;", "contentType", "()Lokhttp3/MediaType;", true, request_body_content_type),
    ne!("Lokhttp3/RequestBody;", "writeTo", "(Lokio/BufferedSink;)V", true, request_body_write_to),
    ne!("Lokhttp3/CacheControl$Builder;", "maxStale-LRDsOJo", "(J)Lokhttp3/CacheControl$Builder;", true, cache_control_builder_max_stale),
    ne!("Lokhttp3/CacheControl$Builder;", "maxStale", "(ILjava/util/concurrent/TimeUnit;)Lokhttp3/CacheControl$Builder;", true, cache_control_builder_max_stale),
    ne!("Lokhttp3/CacheControl$Builder;", "noCache", "()Lokhttp3/CacheControl$Builder;", true, cache_control_builder_no_cache),
    ne!("Lokhttp3/CacheControl$Builder;", "noStore", "()Lokhttp3/CacheControl$Builder;", true, cache_control_builder_no_store),
];

#[cfg(test)]
mod tests;
