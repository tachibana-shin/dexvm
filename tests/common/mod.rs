//! Shared helpers for Moetruyen tests: the reference IMGX encryptor
//! (the exact inverse of the on-device `Lm.c` decryptor) and VM wiring
//! utilities that both `moetruyen_decrypt.rs` and `live_moetruyen.rs` use.

#![allow(dead_code)] // Each integration-test crate uses a different subset.

use std::sync::Once;

use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use dexvm::permission::{FilesystemPermission, NetworkPermission, Permission};
use dexvm::vm::object::{ArrayData, Native};
use dexvm::vm::value::JValue;
use dexvm::Context;

pub const APK: &str = "fixtures/tachiyomi-vi.moetruyen-v1.6.8.apk";

pub const EG: &str = "Leu/kanade/tachiyomi/extension/vi/moetruyen/ExtensionGenerated;";
pub const LA0: &str = "La0;";
pub const LR0: &str = "Lr0;";
pub const LP: &str = "Lp;";

/// Constant image URL + rule identity shared by the fixture payload
/// builder and the HTTP mock, so both sides always agree.
pub const IMG_URL: &str = "https://cdn.moetruyen.example/img/42.webp";
pub const KEY_STR: &str = "/moetruyen/cdn/grant-key";
pub const IMG_ID: &str = "chapter-42/page-01";

/// Installs the `log` backend once per process (no-op afterwards). With
/// `RUST_LOG=info cargo test ... -- --nocapture` all `DBG`/`ERR`/`INV`
/// instrumentation becomes visible; without it the logger stays silent.
pub fn init_logger() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("off")).init();
    });
}

pub fn hmix(x: u32) -> u32 {
    let m = x ^ (x << 13);
    let m = m ^ (m >> 17);
    m ^ (m << 5)
}

/// Reference schedule_of — mirrors `Lc.d` and `Lc.e` in dex.
pub fn schedule_of(s: &str) -> Vec<u8> {
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

/// Reference grant_string — mirrors `Lm.b(p, keyStr, null)` with every
/// optional field of p null: ["IMGX-GRANT-WRAP-v1", "", "", imgId, "", "",
/// "", "", "", trimmedKey].
pub fn grant_string(key_str: &str) -> String {
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

pub fn b64(data: &[u8]) -> String {
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

pub fn b64url(data: &[u8]) -> String {
    b64(data).replace('+', "-").replace('/', "_")
}

/// Reference wrapped_key — mirrors `Lm.a(KEY, p)` where the grant string
/// is the sole AAD and the schedule is XORed over the key.
pub fn wrapped_key(key: &[u8; 32], key_str: &str) -> String {
    let sched = schedule_of(&grant_string(key_str));
    let w: Vec<u8> = key.iter().zip(&sched).map(|(k, s)| k ^ s).collect();
    b64url(&w)
}

/// Reference payload the CDN is mocked to serve: the IMGX header +
/// AES-256-GCM ciphertext the real `Lm.c` (native javax.crypto) decrypts.
pub fn imgx_gcm_payload(key: &[u8; 32], iv: &[u8; 12], plain: &[u8]) -> Vec<u8> {
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

pub fn open() -> Context {
    let mut ctx = Context::open(APK).unwrap();
    // These are extension-flow tests, not sandbox tests: their fixture HTTP
    // callback and cache directory are intentionally in scope.
    ctx.grant(Permission::Filesystem(FilesystemPermission::Any));
    ctx.grant(Permission::Network(NetworkPermission::Any));
    ctx
}

pub fn ensure(ctx: &mut Context, desc: &str) -> u32 {
    ctx.vm().ensure_class_by_desc(desc).unwrap()
}

pub fn init(ctx: &mut Context, desc: &str) -> JValue {
    let cid = ensure(ctx, desc);
    JValue::Obj(ctx.vm().alloc_instance(cid).unwrap())
}

pub fn str_arg(ctx: &mut Context, s: &str) -> JValue {
    ctx.vm().alloc_string(s)
}

pub fn bytes_arg(ctx: &mut Context, bytes: &[u8]) -> JValue {
    let data = bytes.iter().map(|&b| b as i8).collect::<Vec<_>>();
    ctx.vm()
        .alloc_native("[B", Native::Array(ArrayData::Byte(data)))
        .expect("alloc [B")
}

pub fn alloc_request(ctx: &mut Context, url: &str) -> JValue {
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

/// Wires the rule graph into extension instance `eg` exactly like the
/// extension's own page-parsing loop does (eg.h: url -> r0(b=keyStr,
/// d=p(c=IMG_ID, j=wrappedKey))). Constant key [7;32], constant rule
/// identity from IMG_URL/KEY_STR/IMG_ID.
pub fn attach_rule(ctx: &mut Context, eg: JValue, url: &str) -> JValue {
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

    r0
}

pub fn throwable_message(ctx: &mut Context, id: u32) -> String {
    match ctx.vm().payload_of(JValue::Obj(id)) {
        Some(Native::Throwable { message, .. }) => message.clone().unwrap_or_default(),
        _ => match ctx.invoke_on(
            JValue::Obj(id).as_obj(),
            "getMessage",
            "()Ljava/lang/String;",
            &[],
        ) {
            Ok(m) => ctx.vm().str_of(m.as_obj()).to_string(),
            Err(_) => String::new(),
        },
    }
}

/// Body bytes out of a Response payload (the plaintext image).
pub fn response_body(ctx: &mut Context, resp: JValue) -> Vec<u8> {
    match ctx.vm().payload_of(resp) {
        Some(Native::Response { body: Some(b), .. }) => b,
        Some(Native::Response { body: None, .. }) => Vec::new(),
        _ => panic!("not a Response payload"),
    }
}

/// Convert an invoke error into a readable string, resolving the uncaught
/// throwable message like `uncaught: <msg>`.
pub fn err_str(ctx: &mut Context, e: dexvm::vm::error::JvmError) -> String {
    match e {
        dexvm::vm::error::JvmError::Uncaught(id) => {
            format!("uncaught: {}", throwable_message(ctx, id))
        }
        other => format!("{other}"),
    }
}
