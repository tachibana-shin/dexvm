//! java.security.SecureRandom host shim. A xorshift64* PRNG seeded from the
//! OS.

use crate::vm::native::*;

pub(crate) fn seed_os() -> u64 {
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
    let bytes = super::message_digest::bytes_of_jvalue(vm, args[1]).unwrap_or_default();
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

/// Native methods for Ljava/security/SecureRandom;
pub(crate) const TABLE: &[NativeEntry] = &[
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
