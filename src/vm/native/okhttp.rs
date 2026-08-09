//! Host shims for the OkHttp network stack used by extensions.
//! Requests are never executed; the client/builder classes only carry
//! interceptor lists so extension `<init>` code can run.

pub(crate) const HEADERS: &str = "Lokhttp3/Headers;";
#[cfg_attr(not(feature = "tachiyomi"), allow(dead_code))]
pub(crate) const RESPONSE: &str = "Lokhttp3/Response;";
pub(crate) const REQUEST: &str = "Lokhttp3/Request;";
pub(crate) const HTTP_URL: &str = "Lokhttp3/HttpUrl;";

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

pub(crate) fn request_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let (url, method, headers, body) = match payload(vm, args[0]) {
        Some(Native::RequestBuilder {
            url,
            method,
            headers,
            body,
        }) => (url.clone(), method.clone(), headers.clone(), body.clone()),
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
        }) => (url.clone(), method.clone(), headers.clone(), body.clone()),
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
    let path = url
        .split("://")
        .nth(1)
        .and_then(|s| s.split(['?', '#']).next())
        .unwrap_or("");
    let path_owned = path.to_string();
    let segments = path_owned
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| vm.alloc_string(s))
        .collect::<Vec<_>>();
    list_alloc(vm, segments)
}

pub(crate) fn http_url_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::HttpUrl(url)) => url.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(vm.alloc_string(&s))
}

// ---------------------------------------------------------------------------
// OkHttpClient / FormBody / HttpUrl / builder shims
// ---------------------------------------------------------------------------

pub(crate) fn okhttp_client_new_builder(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lokhttp3/OkHttpClient$Builder;",
        Native::OkHttpBuilder {
            interceptors: Vec::new(),
            network_interceptors: Vec::new(),
        },
    )
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

pub(crate) fn okhttp_builder_build(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lokhttp3/OkHttpClient;", Native::Opaque)
}

// ---- request building (FormBody / HttpUrl / RequestsKt) ----

pub(crate) fn lazy_http_url_companion(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lokhttp3/HttpUrl$Companion;")
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

pub(crate) fn okhttp_http_url_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = jstr(vm, args[1]).ok() else {
        return Err(npe(vm));
    };
    alloc(vm, "Lokhttp3/HttpUrl;", Native::HttpUrl(url))
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
// okhttp3 native table
// ---------------------------------------------------------------------------

#[cfg(feature = "okhttp")]
pub(crate) fn okhttp_client_new_call(vm: &mut Vm, args: &[JValue]) -> R {
    let req = match payload(vm, args[1]) {
        Some(Native::Request {
            url,
            method,
            headers,
            body,
        }) => (url.clone(), method.clone(), headers.clone(), *body),
        _ => return Err(npe(vm)),
    };
    let Some(Native::Opaque) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(
        vm,
        "Lokhttp3/Call;",
        Native::Request {
            url: req.0,
            method: req.1,
            headers: req.2,
            body: req.3,
        },
    )
}

#[cfg(feature = "tachiyomi")]
pub(crate) fn okhttp_call_execute(vm: &mut Vm, args: &[JValue]) -> R {
    let (url, method, headers, body) = request_parts(vm, args[0])?;
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

pub(crate) const OKHTTP_TABLE: &[NativeEntry] = &[
    #[cfg(feature = "okhttp")]
    ne!("Lokhttp3/OkHttpClient;", "newCall", "(Lokhttp3/Request;)Lokhttp3/Call;", true, okhttp_client_new_call),
    #[cfg(feature = "tachiyomi")]
    ne!("Lokhttp3/Call;", "execute", "()Lokhttp3/Response;", true, okhttp_call_execute),
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
    ne!("Lokhttp3/HttpUrl$Builder;", "addEncodedQueryParameter", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_builder_add_query),
    ne!("Lokhttp3/HttpUrl$Builder;", "setQueryParameter", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_builder_set_query),
    ne!("Lokhttp3/HttpUrl$Builder;", "build", "()Lokhttp3/HttpUrl;", true, okhttp_http_url_builder_build),
    ne!("Lokhttp3/Request$Builder;", "url", "(Lokhttp3/HttpUrl;)Lokhttp3/Request$Builder;", true, okhttp_request_builder_url),
    ne!("Lokhttp3/HttpUrl$Builder;", "toString", "()Ljava/lang/String;", true, okhttp_http_url_builder_to_string),
];
