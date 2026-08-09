//! java.util.Random host shims.

use crate::vm::native::*;

// java.util.Random
// ---------------------------------------------------------------------------

pub(crate) fn rand_next(seed: &mut u64) -> u64 {
    let mut x = *seed;
    if x == 0 {
        x = 0x9E37_79B9_7F4A_7C15;
    }
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *seed = x;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

pub(crate) fn random_init(vm: &mut Vm, args: &[JValue]) -> R {
    let seed = (now_millis() as u64) ^ next_random_u64();
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(dst) => *dst = seed,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn random_init_seed(vm: &mut Vm, args: &[JValue]) -> R {
    let seed = long_of(vm, args[1]) as u64;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(dst) => *dst = seed,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn random_next_int(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => Ok(JValue::Int((rand_next(seed) >> 32) as i32)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_int_bound(vm: &mut Vm, args: &[JValue]) -> R {
    let bound = int_of(vm, args[1]);
    if bound <= 0 {
        return Err(iae(vm, "bound must be positive"));
    }
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => {
            let v = (rand_next(seed) >> 32) as u32;
            Ok(JValue::Int((v % bound as u32) as i32))
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_long(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => Ok(JValue::Long(rand_next(seed) as i64)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_double(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => {
            let v = (rand_next(seed) >> 11) as f64 * (1.0 / ((1u64 << 53) as f64));
            Ok(JValue::Double(v))
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_float(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => {
            let v = ((rand_next(seed) >> 40) as f32) * (1.0 / ((1u32 << 24) as f32));
            Ok(JValue::Float(v))
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_boolean(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => Ok(JValue::Int(i32::from((rand_next(seed) >> 63) != 0))),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let mut seed = match payload_mut(vm, args[0]) {
        Some(Native::Random(seed)) => *seed,
        _ => return Err(npe(vm)),
    };
    let Some(Native::Array(ArrayData::Byte(bs))) = payload_mut(vm, args[1]) else {
        return Err(npe(vm));
    };
    let mut w = rand_next(&mut seed);
    let n_bytes = bs.len();
    for (i, b) in bs.iter_mut().enumerate() {
        *b = (w & 0xff) as i8;
        w >>= 8;
        if w == 0 && i + 1 < n_bytes {
            w = rand_next(&mut seed);
        }
    }
    Ok(JValue::Null)
}

pub(crate) fn random_set_seed(vm: &mut Vm, args: &[JValue]) -> R {
    let seed = long_of(vm, args[1]) as u64;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(dst) => *dst = seed,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

/// Native methods for Ljava/util/Random;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/util/Random;", "<init>", "()V", true, random_init),
    ne!(
        "Ljava/util/Random;",
        "<init>",
        "(J)V",
        true,
        random_init_seed
    ),
    ne!(
        "Ljava/util/Random;",
        "nextInt",
        "()I",
        true,
        random_next_int
    ),
    ne!(
        "Ljava/util/Random;",
        "nextInt",
        "(I)I",
        true,
        random_next_int_bound
    ),
    ne!(
        "Ljava/util/Random;",
        "nextLong",
        "()J",
        true,
        random_next_long
    ),
    ne!(
        "Ljava/util/Random;",
        "nextDouble",
        "()D",
        true,
        random_next_double
    ),
    ne!(
        "Ljava/util/Random;",
        "nextFloat",
        "()F",
        true,
        random_next_float
    ),
    ne!(
        "Ljava/util/Random;",
        "nextBoolean",
        "()Z",
        true,
        random_next_boolean
    ),
    ne!(
        "Ljava/util/Random;",
        "nextBytes",
        "([B)V",
        true,
        random_next_bytes
    ),
    ne!(
        "Ljava/util/Random;",
        "setSeed",
        "(J)V",
        true,
        random_set_seed
    ),
];
