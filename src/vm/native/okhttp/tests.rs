//! Unit tests for the okhttp3 host shims (Request/Response/Headers/HttpUrl).

use super::*;
use crate::context::Context;
use crate::SandboxOptions;

fn with_vm<T>(f: impl FnOnce(&mut Vm) -> T) -> T {
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
    f(ctx.vm())
}

fn s(vm: &mut Vm, x: &str) -> JValue {
    vm.alloc_string(x)
}

/// Decode a string result (native runs first, then vm is reborrowed).
macro_rules! s_of {
    ($vm:expr, $call:expr) => {{
        let r = $call.unwrap();
        jstr($vm, r).unwrap()
    }};
}

fn int_of(v: JValue) -> i32 {
    match v {
        JValue::Int(i) => i,
        other => panic!("expected int, got {other:?}"),
    }
}

fn bool_of(v: JValue) -> bool {
    match v {
        JValue::Int(i) => i != 0,
        other => panic!("expected int-bool, got {other:?}"),
    }
}

/// Build a Request via the standard builder chain.
fn request_of(vm: &mut Vm, url: &str, method: &str) -> JValue {
    let b = request_builder_init(vm, &[]).unwrap();
    let u = s(vm, url);
    let b = request_builder_url(vm, &[b, u]).unwrap();
    let m = s(vm, method);
    let b = request_builder_method(vm, &[b, m, JValue::Null]).unwrap();
    request_builder_build(vm, &[b]).unwrap()
}

#[test]
fn request_builder_chain() {
    with_vm(|vm| {
        let req = request_of(vm, "https://api.example.com/manga/5", "POST");
        assert_eq!(s_of!(vm, request_method(vm, &[req])), "POST");
        let url = request_url(vm, &[req]).unwrap();
        assert_eq!(
            s_of!(vm, http_url_to_string(vm, &[url])),
            "https://api.example.com/manga/5"
        );
    });
}

#[test]
fn request_method_normalized_to_uppercase() {
    with_vm(|vm| {
        let req = request_of(vm, "https://api.example.com/x", "get");
        assert_eq!(s_of!(vm, request_method(vm, &[req])), "GET");
    });
}

#[test]
fn request_headers_roundtrip() {
    with_vm(|vm| {
        let b = request_builder_init(vm, &[]).unwrap();
        let u = s(vm, "https://api.example.com/x");
        let b = request_builder_url(vm, &[b, u]).unwrap();

        let k = s(vm, "X-Key");
        let v = s(vm, "abc");
        let b = request_builder_header(vm, &[b, k, v]).unwrap();
        // header() replaces an existing value; addHeader() appends.
        let k = s(vm, "X-Key");
        let v = s(vm, "def");
        let b = request_builder_header(vm, &[b, k, v]).unwrap();
        let k = s(vm, "X-Multi");
        let v = s(vm, "one");
        let b = request_builder_add_header(vm, &[b, k, v]).unwrap();
        let k = s(vm, "X-Multi");
        let v = s(vm, "two");
        let b = request_builder_add_header(vm, &[b, k, v]).unwrap();

        let req = request_builder_build(vm, &[b]).unwrap();
        let k = s(vm, "x-key");
        assert_eq!(s_of!(vm, request_header(vm, &[req, k])), "def");
        let k = s(vm, "x-multi");
        assert_eq!(s_of!(vm, request_header(vm, &[req, k])), "two");
        let k = s(vm, "Missing");
        assert!(request_header(vm, &[req, k]).unwrap().is_null());
    });
}

#[test]
fn request_new_builder_copies() {
    with_vm(|vm| {
        let req = request_of(vm, "https://api.example.com/a", "GET");
        let nb = request_new_builder(vm, &[req]).unwrap();
        let built = request_builder_build(vm, &[nb]).unwrap();
        assert_eq!(s_of!(vm, request_method(vm, &[built])), "GET");
        let url = request_url(vm, &[built]).unwrap();
        assert_eq!(
            s_of!(vm, http_url_to_string(vm, &[url])),
            "https://api.example.com/a"
        );
    });
}

#[test]
fn headers_builder_and_lookup() {
    with_vm(|vm| {
        let hb = headers_builder_init(vm, &[]).unwrap();
        let k = s(vm, "Content-Type");
        let v = s(vm, "application/json");
        let hb = headers_builder_add(vm, &[hb, k, v]).unwrap();
        let k = s(vm, "User-Agent");
        let v = s(vm, "dexvm/1");
        let hb = headers_builder_add(vm, &[hb, k, v]).unwrap();
        let hs = headers_builder_build(vm, &[hb]).unwrap();

        assert_eq!(int_of(headers_size(vm, &[hs]).unwrap()), 2);
        let k = s(vm, "content-type");
        assert_eq!(s_of!(vm, headers_get(vm, &[hs, k])), "application/json");
        let k = s(vm, "nope");
        assert!(headers_get(vm, &[hs, k]).unwrap().is_null());

        let tostr = headers_to_string(vm, &[hs]).unwrap();
        let st = match payload(vm, tostr) {
            Some(Native::Str(st)) => st,
            _ => panic!("expected string"),
        };
        assert!(st.contains("Content-Type: application/json"));
        assert!(st.contains("User-Agent: dexvm/1"));
    });
}

#[test]
fn response_accessors() {
    with_vm(|vm| {
        let req = request_of(vm, "https://api.example.com/manga/5", "GET");
        let resp = alloc(
            vm,
            "Lokhttp3/Response;",
            Native::Response {
                code: 200,
                message: "OK".into(),
                headers: vec![("Content-Type".into(), "text/html".into())],
                body: "<html>hi</html>".into(),
                request: req,
            },
        )
        .unwrap();

        assert_eq!(int_of(response_code(vm, &[resp]).unwrap()), 200);
        assert!(bool_of(response_is_successful(vm, &[resp]).unwrap()));
        assert_eq!(s_of!(vm, response_message(vm, &[resp])), "OK");

        // header() finds by name (case-insensitive); header(name, default).
        let k = s(vm, "content-type");
        assert_eq!(s_of!(vm, response_header(vm, &[resp, k])), "text/html");
        let k = s(vm, "missing");
        let def = s(vm, "fallback");
        assert_eq!(
            s_of!(vm, response_header_default(vm, &[resp, k, def])),
            "fallback"
        );

        // body() wraps the payload string; body.string() echoes it.
        let body = response_body(vm, &[resp]).unwrap();
        assert_eq!(
            s_of!(vm, response_body_string(vm, &[body])),
            "<html>hi</html>"
        );

        // request() returns the originating request.
        let back = response_request(vm, &[resp]).unwrap();
        assert_eq!(s_of!(vm, request_method(vm, &[back])), "GET");

        // close() is a no-op.
        let resp2 = response_close(vm, &[]).unwrap();
        assert!(resp2.is_null());
    });
}

#[test]
fn response_error_code_not_successful() {
    with_vm(|vm| {
        let req = request_of(vm, "https://api.example.com/missing", "GET");
        let resp = alloc(
            vm,
            "Lokhttp3/Response;",
            Native::Response {
                code: 404,
                message: "Not Found".into(),
                headers: Vec::new(),
                body: String::new(),
                request: req,
            },
        )
        .unwrap();
        assert!(!bool_of(response_is_successful(vm, &[resp]).unwrap()));
    });
}

#[test]
fn http_url_parsing() {
    with_vm(|vm| {
        let req = request_of(
            vm,
            "https://api.example.com:8443/manga/5?lang=vi&sort=asc",
            "GET",
        );
        let url = request_url(vm, &[req]).unwrap();
        assert_eq!(s_of!(vm, http_url_scheme(vm, &[url])), "https");
        assert_eq!(s_of!(vm, http_url_host(vm, &[url])), "api.example.com:8443");
        let k = s(vm, "lang");
        assert_eq!(s_of!(vm, http_url_query_parameter(vm, &[url, k])), "vi");
        let k = s(vm, "sort");
        assert_eq!(s_of!(vm, http_url_query_parameter(vm, &[url, k])), "asc");
        let k = s(vm, "nope");
        assert!(http_url_query_parameter(vm, &[url, k]).unwrap().is_null());

        // pathSegments: ["manga", "5"] as a List payload.
        let segs = http_url_path_segments(vm, &[url]).unwrap();
        let items: Vec<JValue> = match payload(vm, segs) {
            Some(Native::List(items)) => items.clone(),
            _ => panic!("expected List payload"),
        };
        let strs: Vec<String> = items.into_iter().map(|v| jstr(vm, v).unwrap()).collect();
        assert_eq!(strs, ["manga", "5"]);
    });
}

#[test]
fn okhttp_builder_interceptor_lists() {
    with_vm(|vm| {
        let b = okhttp_client_new_builder(vm, &[]).unwrap();
        let one = s(vm, "interceptor-1");
        let b = okhttp_builder_add_interceptor(vm, &[b, one]).unwrap();
        let two = s(vm, "interceptor-2");
        let b = okhttp_builder_add_interceptor(vm, &[b, two]).unwrap();
        let net = s(vm, "net-1");
        let b = okhttp_builder_add_network_interceptor(vm, &[b, net]).unwrap();

        let inter = okhttp_builder_interceptors(vm, &[b]).unwrap();
        let items: Vec<JValue> = match payload(vm, inter) {
            Some(Native::List(items)) => items.clone(),
            _ => panic!("expected List payload"),
        };
        let strs: Vec<String> = items.into_iter().map(|v| jstr(vm, v).unwrap()).collect();
        assert_eq!(strs, ["interceptor-1", "interceptor-2"]);

        let netlist = okhttp_builder_network_interceptors(vm, &[b]).unwrap();
        let items: Vec<JValue> = match payload(vm, netlist) {
            Some(Native::List(items)) => items.clone(),
            _ => panic!("expected List payload"),
        };
        let strs: Vec<String> = items.into_iter().map(|v| jstr(vm, v).unwrap()).collect();
        assert_eq!(strs, ["net-1"]);

        let _client = okhttp_builder_build(vm, &[b]).unwrap();
    });
}
