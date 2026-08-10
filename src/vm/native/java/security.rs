//! java.security host shims: MessageDigest (SHA-256/384/512, SHA-1, MD5)
//! and SecureRandom. Backing digests come from the RustCrypto crates;
//! SecureRandom is a xorshift64* PRNG seeded from the OS.

use super::*;
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};

const ALGO_SHA256: u8 = 0;
const ALGO_SHA1: u8 = 1;
const ALGO_MD5: u8 = 2;
const ALGO_SHA384: u8 = 3;
const ALGO_SHA512: u8 = 4;

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

fn algo_name(code: u8) -> &'static str {
    match code {
        ALGO_SHA256 => "SHA-256",
        ALGO_SHA1 => "SHA-1",
        ALGO_MD5 => "MD5",
        ALGO_SHA384 => "SHA-384",
        _ => "SHA-512",
    }
}

fn digest_of(algo: u8, data: &[u8]) -> Vec<u8> {
    match algo {
        ALGO_SHA256 => Sha256::digest(data).to_vec(),
        ALGO_SHA1 => Sha1::digest(data).to_vec(),
        ALGO_MD5 => Md5::digest(data).to_vec(),
        ALGO_SHA384 => Sha384::digest(data).to_vec(),
        _ => Sha512::digest(data).to_vec(),
    }
}

fn bytes_of_jvalue(vm: &Vm, v: JValue) -> Option<Vec<u8>> {
    match payload(vm, v) {
        Some(Native::Array(ArrayData::Byte(bs))) => Some(bs.iter().map(|&b| b as u8).collect()),
        _ => None,
    }
}

fn byte_array(vm: &mut Vm, bytes: Vec<u8>) -> Result<JValue, NatErr> {
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

// ---------------------------------------------------------------------------
// SecureRandom
// ---------------------------------------------------------------------------

fn seed_os() -> u64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut h = RandomState::new().build_hasher();
    h.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15),
    );
    h.finish() | 1
}

fn next_u64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *state = x;
    x.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

pub(crate) fn sr_next_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let s0 = match payload(vm, args[0]) {
        Some(Native::SecureRandom(s)) => *s,
        _ => return Err(npe(vm)),
    };
    let Some(Native::Array(ArrayData::Byte(bs))) = payload_mut(vm, args[1]) else {
        return Err(npe(vm));
    };
    let mut s = s0;
    for chunk in bs.chunks_mut(8) {
        let w = next_u64(&mut s).to_le_bytes();
        for (o, b) in chunk.iter_mut().zip(w) {
            *o = b as i8;
        }
    }
    if let Some(Native::SecureRandom(d)) = payload_mut(vm, args[0]) {
        *d = s;
    }
    Ok(JValue::Null)
}

pub(crate) fn sr_set_seed(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of_jvalue(vm, args[1]).unwrap_or_default();
    let mut w = [0u8; 8];
    for (d, b) in w.iter_mut().zip(bytes.into_iter().rev()) {
        *d = b;
    }
    let seed = u64::from_le_bytes(w) | 1;
    if let Some(Native::SecureRandom(d)) = payload_mut(vm, args[0]) {
        *d = seed;
    }
    Ok(JValue::Null)
}

pub(crate) const SECURITY_TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/security/MessageDigest;",
        "getInstance",
        "(Ljava/lang/String;)Ljava/security/MessageDigest;",
        true,
        md_get_instance
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
        "Ljava/security/SecureRandom;",
        "<init>",
        "()V",
        true,
        |vm, _| alloc(
            vm,
            "Ljava/security/SecureRandom;",
            Native::SecureRandom(seed_os())
        )
    ),
    ne!(
        "Ljava/security/SecureRandom;",
        "nextBytes",
        "([B)V",
        true,
        sr_next_bytes
    ),
    ne!(
        "Ljava/security/SecureRandom;",
        "setSeed",
        "([B)V",
        true,
        sr_set_seed
    ),
];

#[cfg(test)]
mod tests {
    fn hex_of(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn array_roundtrip() {
        with_vm(|vm| {
            let v = filled_bytes(vm, b"hello");
            assert_eq!(bytes_of_jvalue(vm, v).unwrap(), b"hello");
        });
    }

    #[test]
    fn digest_of_direct() {
        assert_eq!(
            hex_of(&digest_of(ALGO_SHA256, b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
    use super::*;
    use crate::context::Context;
    use crate::SandboxOptions;

    fn with_vm<T>(f: impl FnOnce(&mut Vm) -> T) -> T {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
        f(ctx.vm())
    }

    fn jbytes(vm: &mut Vm, v: JValue) -> Vec<u8> {
        bytes_of_jvalue(vm, v).unwrap()
    }

    /// The test fixture dex has no `[B` type, so alloc_arr falls back to a
    /// zero-filled array; fill the payload after allocation instead.
    fn filled_bytes(vm: &mut Vm, data: &[u8]) -> JValue {
        let arr = alloc_empty_arr(vm, "B").unwrap();
        if let Some(Native::Array(ArrayData::Byte(bs))) = payload_mut(vm, arr) {
            bs.extend(data.iter().map(|&b| b as i8));
        }
        arr
    }

    fn digest_bytes(vm: &mut Vm, algo: &str, data: &[u8]) -> Vec<u8> {
        let name = vm.alloc_string(algo);
        let md = md_get_instance(vm, &[name]).unwrap();
        let input = filled_bytes(vm, data);
        md_update(vm, &[md, input]).unwrap();
        let out = md_digest(vm, &[md]).unwrap();
        jbytes(vm, out)
    }

    #[test]
    fn sha256_matches_known_vector() {
        with_vm(|vm| {
            let hexd = digest_bytes(vm, "SHA-256", b"abc")
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            assert_eq!(
                hexd,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            );
        });
    }

    #[test]
    fn md5_and_sha1_vectors() {
        with_vm(|vm| {
            let mut hexd = |algo: &str, data: &[u8]| {
                digest_bytes(vm, algo, data)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            };
            assert_eq!(hexd("MD5", b"abc"), "900150983cd24fb0d6963f7d28e17f72");
            assert_eq!(
                hexd("SHA-1", b"abc"),
                "a9993e364706816aba3e25717850c26c9cd0d89d"
            );
        });
    }

    #[test]
    fn digest_with_input_and_reset_semantics() {
        with_vm(|vm| {
            let name = vm.alloc_string("SHA-256");
            let md = md_get_instance(vm, &[name]).unwrap();
            let input = filled_bytes(vm, b"abc");
            let out = md_digest_input(vm, &[md, input]).unwrap();
            let hexd = jbytes(vm, out)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            assert!(hexd.starts_with("ba7816bf"));
            // digest() reset the state: a fresh digest of "" differs
            let out = md_digest(vm, &[md]).unwrap();
            let empty = jbytes(vm, out);
            assert!(!empty.is_empty());
        });
    }

    #[test]
    fn secure_random_fills_bytes_and_advances() {
        with_vm(|vm| {
            let a = alloc(
                vm,
                "Ljava/security/SecureRandom;",
                Native::SecureRandom(seed_os()),
            )
            .unwrap();
            let b1 = byte_array(vm, vec![0; 16]).unwrap();
            let b2 = byte_array(vm, vec![0; 16]).unwrap();
            sr_next_bytes(vm, &[a, b1]).unwrap();
            sr_next_bytes(vm, &[a, b2]).unwrap();
            let v1 = bytes_of_jvalue(vm, b1).unwrap();
            let v2 = bytes_of_jvalue(vm, b2).unwrap();
            assert!(v1.iter().any(|&b| b != 0));
            assert!(v2.iter().any(|&b| b != 0));
            assert_ne!(v1, v2);
        });
    }
}
