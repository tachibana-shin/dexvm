//! End-to-end: MoeTruyen IMGX v3 GCM image decryption through the real dex
//! interceptor (`La0`) and decryptor (`Lm.c`), with a payload fabricated by
//! an independent reference encryptor mirroring the dex math (`Lm.d`/`Lm.h`
//! key schedule, AAD layout, AES-256-GCM).

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use dexvm::vm::object::{ArrayData, Native};
use dexvm::vm::value::JValue;
use dexvm::{Context, SandboxOptions};

const APK: &str = "fixtures/tachiyomi-vi.moetruyen-v1.6.8.apk";

const EG: &str = "Leu/kanade/tachiyomi/extension/vi/moetruyen/ExtensionGenerated;";
const LA0: &str = "La0;";
const LR0: &str = "Lr0;";
const LP: &str = "Lp;";
const IMG_URL: &str = "https://cdn.moetruyen.example/img/42.webp";
const KEY_STR: &str = "/moetruyen/cdn/grant-key";
const IMG_ID: &str = "chapter-42/page-01";

// ---------------------------------------------------------------------------
// reference encryptor (mirrors the dex: Lm.h, Lm.d, Lm.b, Lm.e, Lm.a)
// ---------------------------------------------------------------------------

fn hmix(x: u32) -> u32 {
    let m = x ^ (x << 13);
    let m = m ^ (m >> 17);
    m ^ (m << 5)
}

fn schedule_of(s: &str) -> Vec<u8> {
    let mut h: u32 = 0x811C_9DC5;
    for &b in s.as_bytes() {
        h ^= u32::from(b);
        h = (u64::from(h) * 16_777_619) as u32;
    }
    if h == 0 {
        h = 0x9E37_79B9;
    }
    let mut out = vec![0u8; 32];
    for i in 0..32u32 {
        if i % 4 == 0 {
            h = hmix(i.wrapping_add(h).wrapping_add(0x9E37_79B9));
        }
        out[i as usize] = ((h >> ((i % 4) * 8)) & 0xFF) as u8;
    }
    out
}

fn grant_string(key_str: &str) -> String {
    // Lm.b(p, keyStr, null) with every optional field of p null:
    // ["IMGX-GRANT-WRAP-v1", "", "", imgId, "", "", "", "", "", trimmedKey]
    [
        "IMGX-GRANT-WRAP-v1",
        "",
        "",
        IMG_ID,
        "",
        "",
        "",
        "",
        "",
        key_str.trim_start_matches('/'),
    ]
    .join(".")
}

fn b64(data: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ABC[(n >> 18) as usize & 63] as char);
        out.push(ABC[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ABC[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ABC[n as usize & 63] as char);
        }
    }
    while !out.len().is_multiple_of(4) {
        out.push('=');
    }
    out
}

fn b64url(data: &[u8]) -> String {
    b64(data).replace('+', "-").replace('/', "_")
}

fn wrapped_key(key: &[u8; 32], key_str: &str) -> String {
    let sched = schedule_of(&grant_string(key_str));
    let w: Vec<u8> = key.iter().zip(&sched).map(|(k, s)| k ^ s).collect();
    b64url(&w)
}

fn imgx_gcm_payload(key: &[u8; 32], iv: &[u8; 12], plain: &[u8]) -> Vec<u8> {
    let w: u32 = 800;
    let h: u32 = 600;
    assert_eq!(wrapped_key(key, KEY_STR).len(), 44, "32-byte key in b64url");

    let aad = format!(
        "IMGX-v3.{}.{}.{}.{}",
        IMG_ID,
        KEY_STR.trim_start_matches('/'),
        w,
        h
    );
    let cipher = Aes256Gcm::new_from_slice(key).unwrap();
    let ct = cipher
        .encrypt(
            Nonce::from_slice(iv),
            Payload {
                msg: plain,
                aad: aad.as_bytes(),
            },
        )
        .expect("reference encrypt");

    // header: "IMGX" + version(1) + w(4 BE) + h(4 BE); iv = next 12 bytes [13..25).
    let mut out = vec![b'I', b'M', b'G', b'X', 0x03];
    out.extend_from_slice(&w.to_be_bytes());
    out.extend_from_slice(&h.to_be_bytes());
    out.extend_from_slice(iv);
    out.extend_from_slice(&ct);
    out
}

// ---------------------------------------------------------------------------
// VM wiring helpers
// ---------------------------------------------------------------------------

fn open() -> Context {
    let data = std::fs::read(APK).unwrap();
    Context::new_with(&data, SandboxOptions::allow_all()).unwrap()
}

fn ensure(ctx: &mut Context, desc: &str) -> u32 {
    ctx.vm().ensure_class_by_desc(desc).unwrap()
}

fn init(ctx: &mut Context, desc: &str) -> JValue {
    let cid = ensure(ctx, desc);
    JValue::Obj(ctx.vm().alloc_instance(cid).unwrap())
}

fn str_arg(ctx: &mut Context, s: &str) -> JValue {
    ctx.vm().alloc_string(s)
}

fn bytes_arg(ctx: &mut Context, bytes: &[u8]) -> JValue {
    let data = bytes.iter().map(|&b| b as i8).collect::<Vec<_>>();
    ctx.vm()
        .alloc_native("[B", Native::Array(ArrayData::Byte(data)))
        .expect("alloc [B")
}

fn alloc_request(ctx: &mut Context, url: &str) -> JValue {
    let b = init(ctx, "Lokhttp3/Request$Builder;");
    ctx.invoke_on(b.as_obj(), "<init>", "()V", &[]).unwrap();
    let u = str_arg(ctx, url);
    ctx.invoke_on(
        b.as_obj(),
        "url",
        "(Ljava/lang/String;)Lokhttp3/Request$Builder;",
        &[u],
    )
    .unwrap();
    ctx.invoke_on(b.as_obj(), "build", "()Lokhttp3/Request;", &[])
        .unwrap()
}

/// Builds the full object graph: EG (h: url -> r0(b=key, d=p(c=img, j=key))),
/// a0 = La0(EG), and the interceptor chain around request `url`.
fn wired_ctx(ctx: &mut Context, url: &str) -> JValue {
    let key_str = str_arg(ctx, KEY_STR);
    let img_id = str_arg(ctx, IMG_ID);
    let j = str_arg(ctx, &wrapped_key(&[7u8; 32], KEY_STR));

    // p: all fields null except c (image id, AAD) and j (wrapped key).
    let p = init(ctx, LP);
    ctx.invoke_on(
        p.as_obj(),
        "<init>",
        "(ILjava/lang/Integer;Ljava/lang/String;Ljava/lang/String;Ljava/lang/Long;Ljava/lang/Long;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Int(0x3FF),
            JValue::Null,
            JValue::Null,
            img_id,
            JValue::Null,
            JValue::Null,
            JValue::Null,
            JValue::Null,
            JValue::Null,
            JValue::Null,
            j,
            JValue::Null,
        ],
    )
    .unwrap();

    // r0: a=0, b=keyStr, c=null, d=p (flags 15 = masks 7|8).
    let r0 = init(ctx, LR0);
    ctx.invoke_on(
        r0.as_obj(),
        "<init>",
        "(IILjava/lang/String;Ljava/lang/String;Lp;)V",
        &[JValue::Int(15), JValue::Int(0), key_str, JValue::Null, p],
    )
    .unwrap();

    // EG instance: h = {IMG_URL -> r0}, everything else default.
    let eg = init(ctx, EG);
    let map = init(ctx, "Ljava/util/LinkedHashMap;");
    ctx.invoke_on(
        map.as_obj(),
        "<init>",
        "(IFZ)V",
        &[JValue::Int(16), JValue::Float(1.0), JValue::Int(1)],
    )
    .unwrap();
    let u = str_arg(ctx, url);
    ctx.invoke_on(
        map.as_obj(),
        "put",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        &[u, r0],
    )
    .unwrap();
    assert!(ctx.vm().instance_field_set(eg.as_obj(), "h", map));

    let la0 = init(ctx, LA0);
    ctx.invoke_on(
        la0.as_obj(),
        "<init>",
        "(Leu/kanade/tachiyomi/extension/vi/moetruyen/ExtensionGenerated;)V",
        &[eg],
    )
    .unwrap();

    la0
}

fn throwable_message(ctx: &mut Context, id: u32) -> String {
    match ctx.vm().payload_of(JValue::Obj(id)) {
        Some(Native::Throwable { message, .. }) => message.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

fn run_intercept(ctx: &mut Context, la0: JValue, url: &str) -> Result<JValue, String> {
    let req = alloc_request(ctx, url);
    let chain = ctx
        .vm()
        .alloc_native(
            "Lokhttp3/Interceptor$Chain;",
            Native::Chain {
                interceptors: vec![la0],
                pos: 0,
                request: req,
                call: JValue::Null,
            },
        )
        .expect("alloc chain");
    ctx.invoke_on(
        la0.as_obj(),
        "intercept",
        "(Lokhttp3/Interceptor$Chain;)Lokhttp3/Response;",
        &[chain],
    )
    .map_err(|e| match e {
        dexvm::vm::error::JvmError::Uncaught(id) => {
            format!("uncaught: {}", throwable_message(ctx, id))
        }
        other => format!("{other}"),
    })
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

fn init_logger() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();
    });
}

#[test]
fn imgx_gcm_decrypt_end_to_end() {
    init_logger();
    let mut ctx = open();
    let payload = imgx_gcm_payload(&[7u8; 32], &[9u8; 12], &[0x10, 0x20, 0x30]);
    ctx.set_http(move |_req| dexvm::keiyoushi::HttpResp::ok_bytes(payload.clone()));

    let la0 = wired_ctx(&mut ctx, IMG_URL);
    let resp = run_intercept(&mut ctx, la0, IMG_URL).expect("intercept should decrypt");

    match ctx.vm().payload_of(resp) {
        Some(Native::Response {
            body: Some(bytes), ..
        }) => {
            assert_eq!(
                bytes,
                &[0x10, 0x20, 0x30],
                "decrypted bytes must round-trip"
            );
        }
        _ => panic!("expected Response with body_bytes, got a different payload"),
    }
}

#[test]
fn imgx_unsupported_version_throws() {
    init_logger();
    let mut ctx = open();
    let mut bad = imgx_gcm_payload(&[7u8; 32], &[9u8; 12], &[1, 2, 3]);
    bad[4] = b'Z';
    ctx.set_http(move |_req| dexvm::keiyoushi::HttpResp::ok_bytes(bad.clone()));

    let la0 = wired_ctx(&mut ctx, IMG_URL);
    let err =
        run_intercept(&mut ctx, la0, IMG_URL).expect_err("unsupported IMGX version must throw");
    let msg = err.to_string();
    assert!(msg.contains("Unsupported IMGX version"), "got: {msg}");
}

#[test]
fn imgx_wrong_key_fails() {
    init_logger();
    let mut ctx = open();
    // encrypted with a different key: the grant's key (7s) no longer matches.
    let payload = imgx_gcm_payload(&[8u8; 32], &[9u8; 12], &[1, 2, 3]);
    ctx.set_http(move |_req| dexvm::keiyoushi::HttpResp::ok_bytes(payload.clone()));

    let la0 = wired_ctx(&mut ctx, IMG_URL);
    // wired_ctx wraps with key = 7s every time; encrypt with 8s -> mismatch.
    let err = run_intercept(&mut ctx, la0, IMG_URL).expect_err("GCM tag mismatch must throw");
    let msg = err.to_string();
    assert!(msg.contains("GCM"), "got: {msg}");
}

// Exercise the Cipher natives directly: encrypt/decrypt round-trip.
#[test]
fn cipher_native_roundtrip() {
    init_logger();
    let mut ctx = open();

    fn cipher(vm: &mut dexvm::vm::Vm) -> JValue {
        vm.alloc_native(
            "Ljavax/crypto/Cipher;",
            Native::CipherState {
                transformation: "AES/GCM/NOPADDING".into(),
                mode: 0,
                key: Vec::new(),
                iv: Vec::new(),
                tag_bits: 0,
                aad: Vec::new(),
            },
        )
        .unwrap()
    }

    fn getc(ctx: &mut Context) -> JValue {
        let c = cipher(ctx.vm());
        let name = str_arg(ctx, "AES/GCM/NoPadding");
        ctx.vm()
            .invoke_static(
                "Ljavax/crypto/Cipher;",
                "getInstance",
                "(Ljava/lang/String;)Ljavax/crypto/Cipher;",
                vec![name],
            )
            .unwrap();
        c
    }

    fn key_spec(ctx: &mut Context) -> JValue {
        let spec = init(ctx, "Ljavax/crypto/spec/SecretKeySpec;");
        let kb = bytes_arg(ctx, &[3u8; 32]);
        let algo = str_arg(ctx, "AES");
        ctx.invoke_on(
            spec.as_obj(),
            "<init>",
            "([BLjava/lang/String;)V",
            &[kb, algo],
        )
        .unwrap();
        spec
    }

    fn gcm_spec(ctx: &mut Context) -> JValue {
        let gcm = init(ctx, "Ljavax/crypto/spec/GCMParameterSpec;");
        let iv = bytes_arg(ctx, &[5u8; 12]);
        ctx.invoke_on(gcm.as_obj(), "<init>", "(I[B)V", &[JValue::Int(128), iv])
            .unwrap();
        gcm
    }

    // encrypt
    let c = getc(&mut ctx);
    {
        let spec = key_spec(&mut ctx);
        let gcm = gcm_spec(&mut ctx);
        ctx.invoke_on(
            c.as_obj(),
            "init",
            "(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V",
            &[JValue::Int(1), spec, gcm],
        )
        .unwrap();
    }
    {
        let aad = bytes_arg(&mut ctx, b"IMGX-v3.test");
        ctx.invoke_on(c.as_obj(), "updateAAD", "([B)V", &[aad])
            .unwrap();
    }
    let ct = {
        let input = bytes_arg(&mut ctx, b"hello imgx");
        ctx.invoke_on(
            c.as_obj(),
            "doFinal",
            "([BII)[B",
            &[input, JValue::Int(0), JValue::Int(10)],
        )
        .expect("native encrypt")
    };

    // decrypt with a fresh cipher
    let c2 = getc(&mut ctx);
    {
        let spec = key_spec(&mut ctx);
        let gcm = gcm_spec(&mut ctx);
        ctx.invoke_on(
            c2.as_obj(),
            "init",
            "(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V",
            &[JValue::Int(2), spec, gcm],
        )
        .unwrap();
    }
    {
        let aad = bytes_arg(&mut ctx, b"IMGX-v3.test");
        ctx.invoke_on(c2.as_obj(), "updateAAD", "([B)V", &[aad])
            .unwrap();
    }
    let pt = {
        let input = ct;
        // "hello imgx" (10 bytes) + 16-byte GCM tag.
        ctx.invoke_on(
            c2.as_obj(),
            "doFinal",
            "([BII)[B",
            &[input, JValue::Int(0), JValue::Int(26)],
        )
        .expect("native decrypt")
    };
    let bytes = match ctx.vm().payload_of(pt) {
        Some(Native::Array(ArrayData::Byte(bs))) => bs.iter().map(|&b| b as u8).collect::<Vec<_>>(),
        _ => panic!("expected byte array result"),
    };
    assert_eq!(bytes, b"hello imgx");
}
