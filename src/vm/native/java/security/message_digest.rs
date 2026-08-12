//! java.security.MessageDigest host shims (SHA-256/384/512, SHA-1, MD5).
//! Backing digests come from the RustCrypto crates.

use crate::vm::native::*;
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};

pub(crate) const ALGO_SHA256: u8 = 0;
pub(crate) const ALGO_SHA1: u8 = 1;
pub(crate) const ALGO_MD5: u8 = 2;
pub(crate) const ALGO_SHA384: u8 = 3;
pub(crate) const ALGO_SHA512: u8 = 4;

fn algo_code(name: &str) -> Option<u8> {
    Some(match name.to_uppercase().as_str() {
        "SHA-256" => ALGO_SHA256,
        "SHA-1" => ALGO_SHA1,
        "MD5" => ALGO_MD5,
        "SHA-384" => ALGO_SHA384,
        "SHA-512" => ALGO_SHA512,
        _ => return None,
    })
}

pub(crate) fn algo_name(code: u8) -> &'static str {
    match code {
        ALGO_SHA256 => "SHA-256",
        ALGO_SHA1 => "SHA-1",
        ALGO_MD5 => "MD5",
        ALGO_SHA384 => "SHA-384",
        _ => "SHA-512",
    }
}

pub(crate) fn digest_of(algo: u8, data: &[u8]) -> Vec<u8> {
    match algo {
        ALGO_SHA256 => Sha256::digest(data).to_vec(),
        ALGO_SHA1 => Sha1::digest(data).to_vec(),
        ALGO_MD5 => Md5::digest(data).to_vec(),
        ALGO_SHA384 => Sha384::digest(data).to_vec(),
        _ => Sha512::digest(data).to_vec(),
    }
}

pub(crate) fn bytes_of_jvalue(vm: &Vm, v: JValue) -> Option<Vec<u8>> {
    match payload(vm, v) {
        Some(Native::Array(ArrayData::Byte(bs))) => Some(bs.iter().map(|&b| b as u8).collect()),
        _ => None,
    }
}

pub(crate) fn byte_array(vm: &mut Vm, bytes: Vec<u8>) -> Result<JValue, NatErr> {
    let data = bytes.into_iter().map(|b| b as i8).collect::<Vec<_>>();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

pub(crate) fn md_get_instance(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[0])?;
    let algo = algo_code(&name).ok_or_else(|| {
        iae(
            vm,
            format!("Invalid algorithm {name}, expected MD5, SHA-1, SHA-256, SHA-384 or SHA-512"),
        )
    })?;
    alloc(
        vm,
        "Ljava/security/MessageDigest;",
        Native::MessageDigest {
            algo,
            buf: Vec::new(),
        },
    )
}

pub(crate) fn md_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    let algo = algo_code(&name).ok_or_else(|| {
        iae(
            vm,
            format!("Invalid algorithm {name}, expected MD5, SHA-1, SHA-256, SHA-384 or SHA-512"),
        )
    })?;
    let JValue::Obj(id) = args[0] else {
        return Err(npe(vm));
    };
    vm.arena.objects[id as usize].native = Some(Native::MessageDigest {
        algo,
        buf: Vec::new(),
    });
    Ok(JValue::Null)
}

pub(crate) fn md_update(vm: &mut Vm, args: &[JValue]) -> R {
    let input = bytes_of_jvalue(vm, args[1]).ok_or_else(|| npe(vm))?;
    let Some(Native::MessageDigest { buf, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    buf.extend_from_slice(&input);
    Ok(JValue::Null)
}

pub(crate) fn md_update_range(vm: &mut Vm, args: &[JValue]) -> R {
    let input = bytes_of_jvalue(vm, args[1]).ok_or_else(|| npe(vm))?;
    let off = int_of(vm, args[2]).max(0) as usize;
    let len = int_of(vm, args[3]).max(0) as usize;
    let Some(Native::MessageDigest { buf, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let end = off.saturating_add(len).min(input.len());
    buf.extend_from_slice(&input[off.min(input.len())..end]);
    Ok(JValue::Null)
}

fn md_finalize(vm: &mut Vm, args: &[JValue], extra: &[u8]) -> Result<JValue, NatErr> {
    let (algo, buf) = match payload(vm, args[0]) {
        Some(Native::MessageDigest { algo, buf }) => (*algo, buf.clone()),
        _ => return Err(npe(vm)),
    };
    let mut data = buf;
    data.extend_from_slice(extra);
    let out = digest_of(algo, &data);
    if let Some(Native::MessageDigest { buf, .. }) = payload_mut(vm, args[0]) {
        buf.clear();
    }
    byte_array(vm, out)
}

pub(crate) fn md_digest(vm: &mut Vm, args: &[JValue]) -> R {
    md_finalize(vm, args, &[])
}

pub(crate) fn md_digest_input(vm: &mut Vm, args: &[JValue]) -> R {
    let input = bytes_of_jvalue(vm, args[1]).ok_or_else(|| npe(vm))?;
    md_finalize(vm, args, &input)
}

pub(crate) fn md_reset(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::MessageDigest { buf, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    buf.clear();
    Ok(JValue::Null)
}

pub(crate) fn md_get_algorithm(vm: &mut Vm, args: &[JValue]) -> R {
    let algo = match payload(vm, args[0]) {
        Some(Native::MessageDigest { algo, .. }) => *algo,
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, algo_name(algo)))
}

fn md_get_digest_length(vm: &mut Vm, args: &[JValue]) -> R {
    let algo = match payload(vm, args[0]) {
        Some(Native::MessageDigest { algo, .. }) => *algo,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(match algo {
        ALGO_MD5 => 16,
        ALGO_SHA1 => 20,
        ALGO_SHA256 => 32,
        ALGO_SHA384 => 48,
        _ => 64,
    }))
}

fn md_digest_into(vm: &mut Vm, args: &[JValue]) -> R {
    let (algo, input) = match payload(vm, args[0]) {
        Some(Native::MessageDigest { algo, buf }) => (*algo, buf.clone()),
        _ => return Err(npe(vm)),
    };
    let digest = digest_of(algo, &input);
    let offset = usize::try_from(int_of(vm, args[2])).unwrap_or(usize::MAX);
    let requested = usize::try_from(int_of(vm, args[3])).unwrap_or(0);
    let digest_len = digest.len();
    {
        let Some(Native::Array(ArrayData::Byte(output))) = payload_mut(vm, args[1]) else {
            return Err(npe(vm));
        };
        if requested < digest_len || offset.saturating_add(digest_len) > output.len() {
            return Err(iae(vm, "digest output buffer is too small"));
        }
        for (dst, src) in output[offset..offset + digest_len].iter_mut().zip(digest) {
            *dst = src as i8;
        }
    }
    if let Some(Native::MessageDigest { buf, .. }) = payload_mut(vm, args[0]) {
        buf.clear();
    }
    Ok(JValue::Int(digest_len as i32))
}

/// Native methods for Ljava/security/MessageDigest;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/security/MessageDigest;",
        "getInstance",
        "(Ljava/lang/String;)Ljava/security/MessageDigest;",
        false,
        md_get_instance
    ),
    ne!(
        "Ljava/security/MessageDigest;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        md_init
    ),
    ne!(
        "Ljava/security/MessageDigest;",
        "update",
        "([B)V",
        true,
        md_update
    ),
    ne!(
        "Ljava/security/MessageDigest;",
        "update",
        "([BII)V",
        true,
        md_update_range
    ),
    ne!(
        "Ljava/security/MessageDigest;",
        "digest",
        "()[B",
        true,
        md_digest
    ),
    ne!(
        "Ljava/security/MessageDigest;",
        "digest",
        "([B)[B",
        true,
        md_digest_input
    ),
    ne!(
        "Ljava/security/MessageDigest;",
        "reset",
        "()V",
        true,
        md_reset
    ),
    ne!(
        "Ljava/security/MessageDigest;",
        "getAlgorithm",
        "()Ljava/lang/String;",
        true,
        md_get_algorithm
    ),
    ne!(
        "Ljava/security/MessageDigest;",
        "getDigestLength",
        "()I",
        true,
        md_get_digest_length
    ),
    ne!(
        "Ljava/security/MessageDigest;",
        "digest",
        "([BII)I",
        true,
        md_digest_into
    ),
];
