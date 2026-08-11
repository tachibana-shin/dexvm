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
fn headers_set_and_new_builder_replace_case_insensitively() {
    with_vm(|vm| {
        let builder = headers_builder_init(vm, &[]).unwrap();
        let name = s(vm, "X-Test");
        let first = s(vm, "one");
        headers_builder_add(vm, &[builder, name, first]).unwrap();
        let name = s(vm, "x-test");
        let second = s(vm, "two");
        headers_builder_set(vm, &[builder, name, second]).unwrap();
        let headers = headers_builder_build(vm, &[builder]).unwrap();
        assert_eq!(int_of(headers_size(vm, &[headers]).unwrap()), 1);
        let copied = headers_new_builder(vm, &[headers]).unwrap();
        let copied_headers = headers_builder_build(vm, &[copied]).unwrap();
        let name = s(vm, "X-TEST");
        assert_eq!(s_of!(vm, headers_get(vm, &[copied_headers, name])), "two");
    });
}

#[test]
fn http_url_path_builder_encodes_and_replaces_segments() {
    with_vm(|vm| {
        let raw = s(vm, "https://example.com/base?q=1#frag");
        let url = okhttp_http_url_parse(vm, &[JValue::Null, raw]).unwrap();
        let builder = okhttp_http_url_new_builder(vm, &[url]).unwrap();
        let segment = s(vm, "a/b c");
        okhttp_http_url_builder_add_path_segment(vm, &[builder, segment]).unwrap();
        let replacement = s(vm, "new value");
        okhttp_http_url_builder_set_path_segment(vm, &[builder, JValue::Int(0), replacement])
            .unwrap();
        let built = okhttp_http_url_builder_build(vm, &[builder]).unwrap();
        assert_eq!(
            s_of!(vm, http_url_to_string(vm, &[built])),
            "https://example.com/new%20value/a%2Fb%20c?q=1#frag"
        );
        assert_eq!(
            s_of!(vm, http_url_encoded_path(vm, &[built])),
            "/new%20value/a%2Fb%20c"
        );
        assert_eq!(s_of!(vm, http_url_fragment(vm, &[built])), "frag");
    });
}

#[test]
fn response_builder_mutators_update_response() {
    with_vm(|vm| {
        let request = request_of(vm, "https://example.com", "GET");
        let response = alloc(
            vm,
            RESPONSE,
            Native::Response {
                code: 200,
                message: "OK".into(),
                headers: Vec::new(),
                body: None,
                request,
                prior: JValue::Null,
            },
        )
        .unwrap();
        let builder = response_new_builder(vm, &[response]).unwrap();
        response_builder_code(vm, &[builder, JValue::Int(201)]).unwrap();
        let message = s(vm, "Created");
        response_builder_message(vm, &[builder, message]).unwrap();
        let name = s(vm, "X-Test");
        let value = s(vm, "yes");
        response_builder_header(vm, &[builder, name, value]).unwrap();
        let built = response_builder_build(vm, &[builder]).unwrap();
        assert_eq!(int_of(response_code(vm, &[built]).unwrap()), 201);
        assert_eq!(s_of!(vm, response_message(vm, &[built])), "Created");
        let name = s(vm, "x-test");
        assert_eq!(s_of!(vm, response_header(vm, &[built, name])), "yes");
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
                body: Some("<html>hi</html>".as_bytes().to_vec()),
                request: req,
                prior: JValue::Null,
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
                body: Some(Vec::new()),
                request: req,
                prior: JValue::Null,
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
        // The builder starts with the three mihon default stubs, then our adds.
        assert_eq!(items.len(), 5);
        for (v, want) in items.iter().zip([
            "eu.kanade.tachiyomi.network.interceptor.UncaughtExceptionInterceptor",
            "eu.kanade.tachiyomi.network.interceptor.UserAgentInterceptor",
            "eu.kanade.tachiyomi.network.interceptor.CloudflareInterceptor",
        ]) {
            let cls = vm.class_desc_str(vm.object_class(*v).unwrap());
            assert_eq!(cls, want);
        }
        let strs: Vec<String> = items[3..].iter().map(|v| jstr(vm, *v).unwrap()).collect();
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

#[test]
fn binary_body_plumbing() {
    with_vm(|vm| {
        // response with raw bytes
        let req = alloc(
            vm,
            "Lokhttp3/Request;",
            Native::Request {
                url: "https://x/img".into(),
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
                body: Some(vec![
                    0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3, 4, 5, 6,
                ]),
                request: req,
                prior: JValue::Null,
            },
        )
        .unwrap();

        // Response.body() -> ResponseBody with RespBody payload
        let body = response_body(vm, &[resp]).unwrap();
        assert!(matches!(payload(vm, body), Some(Native::RespBody(_))));

        // byteStream -> ByteArrayInputStream; BufferedReader-ish read back
        let stream = response_body_bytes_stream(vm, &[body]).unwrap();
        let first = bais_read(vm, &[stream]).unwrap();
        assert_eq!(first, JValue::Int(0x89));
        let b1 = alloc_arr(vm, "B", 4, || ArrayData::Byte(vec![0; 4])).unwrap();
        // read([BII) into a byte array (fills via payload)
        let arr = {
            let cnt = bais_read_buf(vm, &[stream, b1, JValue::Int(0), JValue::Int(3)]).unwrap();
            let bytes = match payload(vm, b1) {
                Some(Native::Array(ArrayData::Byte(bs))) => {
                    bs.iter().map(|&b| b as u8).collect::<Vec<_>>()
                }
                _ => Vec::new(),
            };
            (cnt, bytes)
        };
        assert_eq!(arr.0, JValue::Int(3));
        assert_eq!(arr.1[..3], *b"PNG");

        // okio: source(InputStream) -> BufferedSource over the REMAINING cursor
        let source = okio_source_input_stream(vm, &[stream]).unwrap();
        assert_eq!(
            okio_request(vm, &[source, JValue::Long(4)]).unwrap(),
            JValue::Int(1)
        );
        assert_eq!(
            okio_request(vm, &[source, JValue::Long(99)]).unwrap(),
            JValue::Int(0)
        );
        let buf = okio_get_buffer(vm, &[source]).unwrap();
        assert_eq!(
            okio_buffer_get(vm, &[buf, JValue::Long(0)]).unwrap(),
            JValue::Int(0x0d)
        );
        assert_eq!(
            okio_buffer_get(vm, &[buf, JValue::Long(2)]).unwrap(),
            JValue::Int(0x1a)
        );
        assert_eq!(
            okio_buffer_get(vm, &[buf, JValue::Long(9)]).unwrap(),
            JValue::Int(0x06)
        );
        assert!(okio_buffer_get(vm, &[buf, JValue::Long(10)]).is_err());
        let rest = okio_read_byte_array(vm, &[source]).unwrap();
        let bytes = bytes_of(vm, rest).unwrap();
        assert_eq!(bytes, vec![0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3, 4, 5, 6]);
    });
}

#[cfg(feature = "tachiyomi")]
mod chain_tests {
    use super::*;
    use crate::vm::native::keiyoushi::HttpData;
    use crate::vm::native::register_global;
    use std::rc::Rc;

    static FAKE_TABLE: &[NativeEntry] = &[ne!(
        "Ltest/FakeInterceptor;",
        "intercept",
        "(Lokhttp3/Interceptor$Chain;)Lokhttp3/Response;",
        true,
        fake_intercept
    )];

    fn fake_intercept(vm: &mut Vm, args: &[JValue]) -> R {
        let chain = args[1];
        let req = chain_request(vm, &[chain])?;
        chain_proceed(vm, &[chain, req])
    }

    #[test]
    fn interceptor_chain_runs_before_host() {
        register_global(FAKE_TABLE);
        with_vm(|vm| {
            vm.http = Some(Rc::new(|_r: &HttpData| HttpResp::ok_bytes(vec![9, 8, 7])));
            let b = alloc(
                vm,
                "Lokhttp3/OkHttpClient$Builder;",
                Native::OkHttpBuilder {
                    interceptors: Vec::new(),
                    network_interceptors: Vec::new(),
                },
            )
            .unwrap();
            let fake = alloc(vm, "Ltest/FakeInterceptor;", Native::Opaque).unwrap();
            let b = okhttp_builder_add_interceptor(vm, &[b, fake]).unwrap();
            let client = okhttp_builder_build(vm, &[b]).unwrap();
            let rb = alloc(
                vm,
                "Lokhttp3/Request$Builder;",
                Native::RequestBuilder {
                    url: String::new(),
                    method: String::new(),
                    headers: Vec::new(),
                    body: None,
                },
            )
            .unwrap();
            let url = s(vm, "https://img.example/a");
            let rb = request_builder_url(vm, &[rb, url]).unwrap();
            let req = request_builder_build(vm, &[rb]).unwrap();
            let call = okhttp_client_new_call(vm, &[client, req]).unwrap();
            let resp = okhttp_call_execute(vm, &[call]).unwrap();
            let bytes = match payload(vm, resp) {
                Some(Native::Response { body: Some(b), .. }) => b.clone(),
                _ => panic!("expected byte body"),
            };
            assert_eq!(bytes, vec![9, 8, 7]);
            let rq = response_request(vm, &[resp]).unwrap();
            let u = request_url(vm, &[rq]).unwrap();
            let us = http_url_to_string(vm, &[u]).unwrap();
            let us = jstr(vm, us).unwrap();
            assert_eq!(us, "https://img.example/a");
        });
    }

    #[test]
    fn empty_chain_skips_interceptors() {
        with_vm(|vm| {
            vm.http = Some(Rc::new(|_r: &HttpData| HttpResp::ok_bytes(vec![1])));
            let b = alloc(
                vm,
                "Lokhttp3/OkHttpClient$Builder;",
                Native::OkHttpBuilder {
                    interceptors: Vec::new(),
                    network_interceptors: Vec::new(),
                },
            )
            .unwrap();
            let client = okhttp_builder_build(vm, &[b]).unwrap();
            let rb = alloc(
                vm,
                "Lokhttp3/Request$Builder;",
                Native::RequestBuilder {
                    url: String::new(),
                    method: String::new(),
                    headers: Vec::new(),
                    body: None,
                },
            )
            .unwrap();
            let url = s(vm, "https://x/f");
            let rb = request_builder_url(vm, &[rb, url]).unwrap();
            let req = request_builder_build(vm, &[rb]).unwrap();
            let call = okhttp_client_new_call(vm, &[client, req]).unwrap();
            let resp = okhttp_call_execute(vm, &[call]).unwrap();
            assert!(matches!(
                payload(vm, resp),
                Some(Native::Response { code: 200, body: Some(b), .. }) if b == &vec![1]
            ));
        });
    }
}
