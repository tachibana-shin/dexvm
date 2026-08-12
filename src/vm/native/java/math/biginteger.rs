//! java.math.BigInteger host shim backed by `num-bigint::BigInt`.

use crate::vm::native::*;
use num_bigint::{BigInt, Sign};
use num_traits::{Signed, ToPrimitive, Zero};

fn big_of(vm: &Vm, v: JValue) -> Option<BigInt> {
    match payload(vm, v) {
        Some(Native::BigInt(b)) => Some(b.clone()),
        _ => None,
    }
}

fn alloc_big(vm: &mut Vm, value: BigInt) -> R {
    alloc(vm, "Ljava/math/BigInteger;", Native::BigInt(value))
}

fn set_this(vm: &mut Vm, this: JValue, value: BigInt) -> R {
    let JValue::Obj(id) = this else {
        return Err(npe(vm));
    };
    vm.arena.objects[id as usize].native = Some(Native::BigInt(value));
    Ok(JValue::Null)
}

fn biginteger_init_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Byte(bs))) => bs.iter().map(|&b| b as u8).collect::<Vec<_>>(),
        _ => return Err(npe(vm)),
    };
    let value = BigInt::from_signed_bytes_be(&bytes);
    set_this(vm, args[0], value)
}

fn biginteger_init_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[1])?;
    let radix = if args.len() > 2 { int_of(vm, args[2]) } else { 10 };
    let value = BigInt::parse_bytes(s.trim().as_bytes(), radix as u32)
        .ok_or_else(|| NatErr::Throw(vm.err_nfe(format!("invalid BigInteger: {s}"))))?;
    set_this(vm, args[0], value)
}

fn biginteger_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    alloc_big(vm, BigInt::from(long_of(vm, args[0])))
}

fn biginteger_add(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = big_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    alloc_big(vm, a + b)
}
fn biginteger_subtract(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = big_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    alloc_big(vm, a - b)
}
fn biginteger_multiply(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = big_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    alloc_big(vm, a * b)
}
fn biginteger_divide(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = big_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    if b.is_zero() {
        return Err(NatErr::Throw(
            vm.throwable_of("Ljava/lang/ArithmeticException;", "BigInteger divide by zero"),
        ));
    }
    alloc_big(vm, a / b)
}
/// Java/Kotlin `mod` (always non-negative), distinct from `remainder`.
fn biginteger_mod(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let m = big_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    if m.sign() != Sign::Plus {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/ArithmeticException;",
            "BigInteger: modulus not positive",
        )));
    }
    let r = &a % &m;
    alloc_big(vm, if r.is_negative() { r + m } else { r })
}
fn biginteger_remainder(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = big_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    alloc_big(vm, a % b)
}
fn biginteger_neg(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    alloc_big(vm, -a)
}
fn biginteger_abs(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    alloc_big(vm, a.abs())
}
fn biginteger_pow(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let e = int_of(vm, args[1]).max(0) as u32;
    alloc_big(vm, a.pow(e))
}
fn biginteger_mod_pow(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let e = big_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let m = big_of(vm, args[2]).ok_or_else(|| npe(vm))?;
    alloc_big(vm, a.modpow(&e, &m))
}
fn biginteger_mod_inverse(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let m = big_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    // Extended Euclidean algorithm.
    let (mut old_r, mut r) = (a.clone(), m.clone());
    let (mut old_s, mut s) = (BigInt::from(1), BigInt::from(0));
    while !r.is_zero() {
        let q = &old_r / &r;
        let new_r = &old_r - &q * &r;
        old_r = std::mem::replace(&mut r, new_r);
        let new_s = &old_s - &q * &s;
        old_s = std::mem::replace(&mut s, new_s);
    }
    if old_r != BigInt::from(1) {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/ArithmeticException;",
            "BigInteger not invertible",
        )));
    }
    let inv = ((old_s % &m) + &m) % &m;
    alloc_big(vm, inv)
}
fn biginteger_shift_left(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let n = int_of(vm, args[1]);
    alloc_big(vm, if n >= 0 { a << n as u32 } else { a >> (-n) as u32 })
}
fn biginteger_shift_right(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let n = int_of(vm, args[1]);
    alloc_big(vm, if n >= 0 { a >> n as u32 } else { a << (-n) as u32 })
}
fn biginteger_signum(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(JValue::Int(if a.is_zero() {
        0
    } else if a.is_negative() {
        -1
    } else {
        1
    }))
}
fn biginteger_test_bit(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let n = int_of(vm, args[1]).max(0) as u64;
    Ok(JValue::Int(i32::from(a.bit(n))))
}
fn biginteger_bit_length(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(JValue::Int(a.bits() as i32))
}
fn biginteger_int_value(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(JValue::Int(a.to_i32().unwrap_or_else(|| {
        (&a & BigInt::from(0xFFFF_FFFFu64)).to_u32().unwrap_or(0) as i32
    })))
}
fn biginteger_long_value(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(JValue::Long(a.to_i64().unwrap_or(0)))
}
fn biginteger_double_value(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(JValue::Double(a.to_f64().unwrap_or(0.0)))
}
fn biginteger_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = big_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    Ok(JValue::Int(a.cmp(&b) as i32))
}
fn biginteger_equals(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = match big_of(vm, args[1]) {
        Some(b) => b,
        None => return Ok(JValue::Int(0)),
    };
    Ok(JValue::Int(i32::from(a == b)))
}
fn biginteger_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(new_str(vm, &a.to_string()))
}
fn biginteger_to_string_radix(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let radix = int_of(vm, args[1]) as u32;
    Ok(new_str(vm, &a.to_str_radix(radix)))
}
fn biginteger_to_byte_array(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let bytes = a.to_signed_bytes_be();
    let data: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}
fn biginteger_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    let a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let bytes = a.to_signed_bytes_be();
    let mut h: i32 = 0;
    for b in bytes {
        h = h.wrapping_mul(31).wrapping_add(i32::from(b as i8));
    }
    Ok(JValue::Int(h))
}
fn biginteger_gcd(vm: &mut Vm, args: &[JValue]) -> R {
    let mut a = big_of(vm, args[0]).ok_or_else(|| npe(vm))?.abs();
    let mut b = big_of(vm, args[1]).ok_or_else(|| npe(vm))?.abs();
    while !b.is_zero() {
        let r = &a % &b;
        a = std::mem::replace(&mut b, r);
    }
    alloc_big(vm, a)
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/math/BigInteger;", "<init>", "([B)V", true, biginteger_init_bytes),
    ne!("Ljava/math/BigInteger;", "<init>", "(Ljava/lang/String;)V", true, biginteger_init_string),
    ne!("Ljava/math/BigInteger;", "<init>", "(Ljava/lang/String;I)V", true, biginteger_init_string),
    ne!("Ljava/math/BigInteger;", "valueOf", "(J)Ljava/math/BigInteger;", false, biginteger_value_of),
    ne!("Ljava/math/BigInteger;", "add", "(Ljava/math/BigInteger;)Ljava/math/BigInteger;", true, biginteger_add),
    ne!("Ljava/math/BigInteger;", "subtract", "(Ljava/math/BigInteger;)Ljava/math/BigInteger;", true, biginteger_subtract),
    ne!("Ljava/math/BigInteger;", "multiply", "(Ljava/math/BigInteger;)Ljava/math/BigInteger;", true, biginteger_multiply),
    ne!("Ljava/math/BigInteger;", "divide", "(Ljava/math/BigInteger;)Ljava/math/BigInteger;", true, biginteger_divide),
    ne!("Ljava/math/BigInteger;", "mod", "(Ljava/math/BigInteger;)Ljava/math/BigInteger;", true, biginteger_mod),
    ne!("Ljava/math/BigInteger;", "remainder", "(Ljava/math/BigInteger;)Ljava/math/BigInteger;", true, biginteger_remainder),
    ne!("Ljava/math/BigInteger;", "negate", "()Ljava/math/BigInteger;", true, biginteger_neg),
    ne!("Ljava/math/BigInteger;", "abs", "()Ljava/math/BigInteger;", true, biginteger_abs),
    ne!("Ljava/math/BigInteger;", "pow", "(I)Ljava/math/BigInteger;", true, biginteger_pow),
    ne!("Ljava/math/BigInteger;", "modPow", "(Ljava/math/BigInteger;Ljava/math/BigInteger;)Ljava/math/BigInteger;", true, biginteger_mod_pow),
    ne!("Ljava/math/BigInteger;", "modInverse", "(Ljava/math/BigInteger;)Ljava/math/BigInteger;", true, biginteger_mod_inverse),
    ne!("Ljava/math/BigInteger;", "gcd", "(Ljava/math/BigInteger;)Ljava/math/BigInteger;", true, biginteger_gcd),
    ne!("Ljava/math/BigInteger;", "shiftLeft", "(I)Ljava/math/BigInteger;", true, biginteger_shift_left),
    ne!("Ljava/math/BigInteger;", "shiftRight", "(I)Ljava/math/BigInteger;", true, biginteger_shift_right),
    ne!("Ljava/math/BigInteger;", "signum", "()I", true, biginteger_signum),
    ne!("Ljava/math/BigInteger;", "testBit", "(I)Z", true, biginteger_test_bit),
    ne!("Ljava/math/BigInteger;", "bitLength", "()I", true, biginteger_bit_length),
    ne!("Ljava/math/BigInteger;", "intValue", "()I", true, biginteger_int_value),
    ne!("Ljava/math/BigInteger;", "intValueExact", "()I", true, biginteger_int_value),
    ne!("Ljava/math/BigInteger;", "longValue", "()J", true, biginteger_long_value),
    ne!("Ljava/math/BigInteger;", "longValueExact", "()J", true, biginteger_long_value),
    ne!("Ljava/math/BigInteger;", "doubleValue", "()D", true, biginteger_double_value),
    ne!("Ljava/math/BigInteger;", "compareTo", "(Ljava/math/BigInteger;)I", true, biginteger_compare_to),
    ne!("Ljava/math/BigInteger;", "equals", "(Ljava/lang/Object;)Z", true, biginteger_equals),
    ne!("Ljava/math/BigInteger;", "toString", "()Ljava/lang/String;", true, biginteger_to_string),
    ne!("Ljava/math/BigInteger;", "toString", "(I)Ljava/lang/String;", true, biginteger_to_string_radix),
    ne!("Ljava/math/BigInteger;", "toByteArray", "()[B", true, biginteger_to_byte_array),
    ne!("Ljava/math/BigInteger;", "hashCode", "()I", true, biginteger_hash_code),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_pow_matches_known_rsa_style_reduction() {
        let base = BigInt::from(4u32);
        let exp = BigInt::from(13u32);
        let modulus = BigInt::from(497u32);
        assert_eq!(base.modpow(&exp, &modulus), BigInt::from(445u32));
    }

    #[test]
    fn mod_is_always_non_negative_unlike_remainder() {
        let a = BigInt::from(-7i32);
        let m = BigInt::from(3u32);
        let r = &a % &m;
        let mod_result = if r.is_negative() { r + &m } else { r };
        assert_eq!(mod_result, BigInt::from(2u32));
    }
}
