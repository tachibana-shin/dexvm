//! Host shims for the mihon extension network API and OkHttp.
//! Requests are never executed; the client/builder classes only carry
//! interceptor lists so extension `<init>` code can run.

use super::*;

pub(crate) fn http_source_get_network(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Leu/kanade/tachiyomi/network/NetworkHelper;", Native::Opaque)
}

pub(crate) fn http_source_get_headers(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lokhttp3/Headers;", Native::Opaque)
}

pub(crate) fn network_helper_get_client(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lokhttp3/OkHttpClient;", Native::Opaque)
}

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
    let Some(Native::OkHttpBuilder { network_interceptors, .. }) = payload_mut(vm, args[0]) else {
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
    collections::list_alloc(vm, items)
}

pub(crate) fn okhttp_builder_network_interceptors(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::OkHttpBuilder { network_interceptors, .. }) => network_interceptors.clone(),
        _ => return Err(npe(vm)),
    };
    collections::list_alloc(vm, items)
}

pub(crate) fn okhttp_builder_build(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lokhttp3/OkHttpClient;", Native::Opaque)
}

// ---- request building (FormBody / HttpUrl / RequestsKt) ----

pub(crate) fn lazy_http_url_companion(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lokhttp3/HttpUrl$Companion;")
}

pub(crate) fn okhttp_form_builder_init(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lokhttp3/FormBody$Builder;", Native::FormBody(Vec::new()))
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
    alloc(vm, "Lokhttp3/HttpUrl$Builder;", Native::HttpUrl(url.clone()))
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

pub(crate) fn okhttp_request_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::RequestBuilder { url, headers, body, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(
        vm,
        "Lokhttp3/Request;",
        Native::Request {
            url: url.clone(),
            method: "GET".into(),
            headers: headers.clone(),
            body: body.clone(),
        },
    )
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
pub(crate) fn requests_kt_post_default(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(url) = jstr(vm, args[0]).ok() else {
        return Err(npe(vm));
    };
    let body = args[2];
    alloc(
        vm,
        "Lokhttp3/Request;",
        Native::Request { url, method: "POST".into(), headers: Vec::new(), body: Some(body) },
    )
}
