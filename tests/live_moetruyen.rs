//! Live Moetruyen test: drives the REAL extension code through the VM —
//! source ctor, network client lazy, okhttp Call.execute machinery and the
//! La0 interceptor — with a fully mocked HTTP driver serving stable IMGX
//! fixtures. Runs offline on CI (moetruyen.net blocks some regions and
//! serves flaky data), while still exercising every real dex object and
//! native that production image requests touch.

mod common;
use common::*;

use dexvm::keiyoushi::Keiyoushi;
use dexvm::vm::object::Native;
use dexvm::vm::value::JValue;

/// A stable, realistic image payload: real magic + a hand-picked byte run.
fn pic_plain() -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend((0..64).map(|i| (i * 7 + 3) as u8)); // 64 bytes of image data
    v
}

/// New SPs created fresh per invocation: name/lang/supports_latest come
/// from real dex code (`getName` returns the constant "MoeTruyen").
#[test]
fn source_api_smoke() {
    init_logger();
    let mut ext = Keiyoushi::open(APK).expect("open apk");
    let srcs = ext.sources().expect("sources");
    assert!(!srcs.is_empty(), "moetruyen apk must yield at least one source");

    let name = ext.source_name(&srcs[0]).expect("source_name");
    let lang = ext.source_lang(&srcs[0]).expect("source_lang");
    assert_eq!(name, "MoeTruyen");
    assert_eq!(lang, "vi");
    assert!(ext.supports_latest(&srcs[0]).expect("supports_latest"));
}

/// The full production image path with a stable mock CDN:
/// Request -> Call.execute() -> client interceptor chain (real dex La0)
/// -> mock fetch of IMGX bytes -> real Lm.c GCM decrypt -> plaintext.
#[test]
fn image_pipeline_with_mock_http() {
    init_logger();
    let mock = imgx_gcm_payload(&[7u8; 32], &[9u8; 12], &pic_plain());
    let mut ctx = open();
    ctx.set_http(move |_req| {
        dexvm::keiyoushi::HttpResp::ok_bytes(mock.clone())
    });

    // The extension boot: EG.<init>() -> lazy client with the La0
    // interceptor registered (Lu.getValue builds it).
    let eg = init(&mut ctx, EG);
    if let Err(e) = ctx.invoke_on(eg.as_obj(), "<init>", "()V", &[]) {
        if let dexvm::vm::error::JvmError::Uncaught(t) = e {
            let c = ctx.vm().object_class(JValue::Obj(t)).unwrap();
            println!("THROW CLASS: {}", ctx.vm().class_desc_str(c));
        }
        panic!("eg ctor: {}", err_str(&mut ctx, e));
    }
    attach_rule(&mut ctx, eg, IMG_URL);

    // The real network client, straight from the extension.
    let client = ctx
        .invoke_on(eg.as_obj(), "getClient", "()Lokhttp3/OkHttpClient;", &[])
        .map_err(|e| err_str(&mut ctx, e))
        .expect("getClient");

    let req = alloc_request(&mut ctx, IMG_URL);
    let call = ctx
        .invoke_on(client.as_obj(), "newCall", "(Lokhttp3/Request;)Lokhttp3/Call;", &[req])
        .expect("newCall");
    let resp = match ctx.invoke_on(call.as_obj(), "execute", "()Lokhttp3/Response;", &[]) {
        Ok(r) => r,
        Err(dexvm::vm::error::JvmError::Uncaught(t)) => {
            let class_id = ctx.vm().object_class(JValue::Obj(t)).unwrap();
            let cls = ctx.vm().class_desc_str(class_id);
            println!("EXEC-THROW CLASS: {cls}");
            panic!("execute: {}", throwable_text(&mut ctx, t));
        }
        Err(e) => panic!("execute: {e}"),
    };

    assert_eq!(response_body(&mut ctx, resp), pic_plain());
}

fn throwable_text(ctx: &mut dexvm::Context, t: u32) -> String {
    match ctx.vm().payload_of(JValue::Obj(t)) {
        Some(Native::Throwable { message, .. }) => message.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

/// The same path but the CDN serves an unsupported IMGX version: the real
/// La0 raises instead of decrypting, and the error survives Call.execute.
#[test]
fn image_pipeline_rejects_bad_version() {
    init_logger();
    let mut bad = vec![b'I', b'M', b'G', b'X', 0x01]; // v1: unsupported (m.c only accepts 2 and 3)
    bad.extend_from_slice(&800u32.to_be_bytes());
    bad.extend_from_slice(&600u32.to_be_bytes());
    bad.extend_from_slice(&[0u8; 800]); // payload so the header is readable
    let mut ctx = open();
    ctx.set_http(move |_req| {
        dexvm::keiyoushi::HttpResp::ok_bytes(bad.clone())
    });

    let eg = init(&mut ctx, EG);
    ctx.invoke_on(eg.as_obj(), "<init>", "()V", &[])
        .expect("eg ctor");
    attach_rule(&mut ctx, eg, IMG_URL);

    let client = ctx
        .invoke_on(eg.as_obj(), "getClient", "()Lokhttp3/OkHttpClient;", &[])
        .expect("getClient");

    let req = alloc_request(&mut ctx, IMG_URL);
    let call = ctx
        .invoke_on(client.as_obj(), "newCall", "(Lokhttp3/Request;)Lokhttp3/Call;", &[req])
        .expect("newCall");
    let err = ctx
        .invoke_on(call.as_obj(), "execute", "()Lokhttp3/Response;", &[])
        .map_err(|e| err_str(&mut ctx, e))
        .expect_err("bad version must be rejected");
    assert!(
        err.contains("Unsupported IMGX version"),
        "got: {err}"
    );
}

/// Sanity: the client returned by the extension really carries the La0
/// interceptor in its chain (proves the mock path is the real path).
#[test]
fn extension_client_ships_interceptor() {
    init_logger();
    let mut ctx = open();
    let eg = init(&mut ctx, EG);
    ctx.invoke_on(eg.as_obj(), "<init>", "()V", &[])
        .expect("eg ctor");
    attach_rule(&mut ctx, eg, IMG_URL);

    let client = ctx
        .invoke_on(eg.as_obj(), "getClient", "()Lokhttp3/OkHttpClient;", &[])
        .expect("getClient");
    match ctx.vm().payload_of(client) {
        Some(Native::OkHttpClient { interceptors, .. }) => {
            assert!(
                !interceptors.is_empty(),
                "the extension's client must carry its interceptor"
            );
            let has_a0 = interceptors.iter().any(|i| {
                let class_id = ctx.vm().object_class(*i).unwrap();
                ctx.vm().class_desc_str(class_id) == "a0"
            });
            assert!(has_a0, "interceptor chain must contain the real La0");
        }
        other => panic!("client payload wrong: {:?}", std::mem::discriminant(&other)),
    }
}


#[test]
fn probe_chm() {
    init_logger();
    let mut ctx = open();
    let m = init(&mut ctx, "Ljava/util/concurrent/ConcurrentHashMap;");
    let r = ctx.invoke_on(m.as_obj(), "<init>", "()V", &[]);
    match r {
        Ok(_) => println!("CHM init OK"),
        Err(e) => println!("CHM init ERR: {e:?}"),
    }
    let h = init(&mut ctx, "Ljava/util/HashMap;");
    let r2 = ctx.invoke_on(h.as_obj(), "<init>", "()V", &[]);
    println!("HashMap init: {:?}", r2.is_ok());
}
