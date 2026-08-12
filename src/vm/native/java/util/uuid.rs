//! java.util.UUID host shim: random (v4) UUIDs, stored as their canonical
//! string form.

use crate::vm::native::*;

fn os_seed() -> u64 {
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

fn random_uuid_string() -> String {
    let mut state = os_seed();
    let hi = next_u64(&mut state);
    let lo = next_u64(&mut state);
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&hi.to_be_bytes());
    bytes[8..].copy_from_slice(&lo.to_be_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 10xx
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}

fn uuid_random(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/UUID;", Native::Str(random_uuid_string()))
}

fn uuid_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &s))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/UUID;",
        "randomUUID",
        "()Ljava/util/UUID;",
        false,
        uuid_random
    ),
    ne!(
        "Ljava/util/UUID;",
        "toString",
        "()Ljava/lang/String;",
        true,
        uuid_to_string
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_uuid_has_v4_shape() {
        let s = random_uuid_string();
        let parts: Vec<&str> = s.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(
            [
                parts[0].len(),
                parts[1].len(),
                parts[2].len(),
                parts[3].len(),
                parts[4].len()
            ],
            [8, 4, 4, 4, 12]
        );
        assert_eq!(&parts[2][..1], "4");
        assert!(matches!(
            parts[3].chars().next().unwrap(),
            '8' | '9' | 'a' | 'b'
        ));
    }
}
