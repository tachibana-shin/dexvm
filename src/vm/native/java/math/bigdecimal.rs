//! java.math.BigDecimal host shim: the standard (unscaled BigInteger,
//! scale) representation, backed by `num-bigint::BigInt`.

use crate::vm::native::*;
use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

#[derive(Clone)]
struct Dec {
    unscaled: BigInt,
    scale: i32,
}

fn dec_of(vm: &Vm, v: JValue) -> Option<Dec> {
    match payload(vm, v) {
        Some(Native::BigDecimal { unscaled, scale }) => Some(Dec {
            unscaled: unscaled.clone(),
            scale: *scale,
        }),
        _ => None,
    }
}

fn alloc_dec(vm: &mut Vm, dec: Dec) -> R {
    alloc(
        vm,
        "Ljava/math/BigDecimal;",
        Native::BigDecimal {
            unscaled: dec.unscaled,
            scale: dec.scale,
        },
    )
}

fn set_this(vm: &mut Vm, this: JValue, dec: Dec) -> R {
    let JValue::Obj(id) = this else {
        return Err(npe(vm));
    };
    vm.arena.objects[id as usize].native = Some(Native::BigDecimal {
        unscaled: dec.unscaled,
        scale: dec.scale,
    });
    Ok(JValue::Null)
}

/// Parses a plain or exponential decimal string into (unscaled, scale).
fn parse_decimal(s: &str) -> Option<Dec> {
    let s = s.trim();
    let (mantissa, exp) = match s.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i32>().ok()?),
        None => (s, 0),
    };
    let (int_part, frac_part) = match mantissa.split_once('.') {
        Some((i, f)) => (i, f),
        None => (mantissa, ""),
    };
    let digits = format!("{int_part}{frac_part}");
    let unscaled = BigInt::parse_bytes(digits.as_bytes(), 10)?;
    let scale = frac_part.len() as i32 - exp;
    Some(Dec { unscaled, scale })
}

fn bigdecimal_init_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[1])?;
    let dec = parse_decimal(&s).ok_or_else(|| NatErr::Throw(vm.err_nfe(format!("invalid BigDecimal: {s}"))))?;
    set_this(vm, args[0], dec)
}

fn bigdecimal_init_int(vm: &mut Vm, args: &[JValue]) -> R {
    set_this(vm, args[0], Dec { unscaled: BigInt::from(int_of(vm, args[1])), scale: 0 })
}

fn bigdecimal_init_biginteger_scale(vm: &mut Vm, args: &[JValue]) -> R {
    let unscaled = match payload(vm, args[1]) {
        Some(Native::BigInt(b)) => b.clone(),
        _ => return Err(npe(vm)),
    };
    let scale = int_of(vm, args[2]);
    set_this(vm, args[0], Dec { unscaled, scale })
}

/// Scales two decimals to a common scale, returning their unscaled values.
fn align(a: &Dec, b: &Dec) -> (BigInt, BigInt, i32) {
    let scale = a.scale.max(b.scale);
    let ua = &a.unscaled * BigInt::from(10u32).pow((scale - a.scale).max(0) as u32);
    let ub = &b.unscaled * BigInt::from(10u32).pow((scale - b.scale).max(0) as u32);
    (ua, ub, scale)
}

fn bigdecimal_add(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = dec_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let (ua, ub, scale) = align(&a, &b);
    alloc_dec(vm, Dec { unscaled: ua + ub, scale })
}
fn bigdecimal_subtract(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = dec_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let (ua, ub, scale) = align(&a, &b);
    alloc_dec(vm, Dec { unscaled: ua - ub, scale })
}
fn bigdecimal_multiply(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = dec_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    alloc_dec(vm, Dec { unscaled: a.unscaled * b.unscaled, scale: a.scale + b.scale })
}
fn bigdecimal_divide(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = dec_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    if b.unscaled.is_zero() {
        return Err(NatErr::Throw(
            vm.throwable_of("Ljava/lang/ArithmeticException;", "BigDecimal divide by zero"),
        ));
    }
    // Divide at a generous extra precision, then round half-up to that scale.
    let scale = a.scale.max(b.scale) + 16;
    let num = &a.unscaled * BigInt::from(10u32).pow((scale - a.scale + b.scale).max(0) as u32);
    alloc_dec(vm, Dec { unscaled: &num / &b.unscaled, scale })
}
fn bigdecimal_signum(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(JValue::Int(if a.unscaled.is_zero() {
        0
    } else if a.unscaled.is_negative() {
        -1
    } else {
        1
    }))
}
fn bigdecimal_scale(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(JValue::Int(a.scale))
}
fn bigdecimal_set_scale(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let new_scale = int_of(vm, args[1]);
    let unscaled = if new_scale >= a.scale {
        a.unscaled * BigInt::from(10u32).pow((new_scale - a.scale) as u32)
    } else {
        let divisor = BigInt::from(10u32).pow((a.scale - new_scale) as u32);
        // Round half-up.
        let half = &divisor / BigInt::from(2u32);
        let sign = if a.unscaled.is_negative() { -1 } else { 1 };
        (a.unscaled.abs() + half) / divisor * sign
    };
    alloc_dec(vm, Dec { unscaled, scale: new_scale })
}
fn bigdecimal_strip_trailing_zeros(vm: &mut Vm, args: &[JValue]) -> R {
    let mut a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    if a.unscaled.is_zero() {
        return alloc_dec(vm, Dec { unscaled: BigInt::zero(), scale: 0 });
    }
    let ten = BigInt::from(10u32);
    while a.scale > 0 {
        let (q, r) = (&a.unscaled / &ten, &a.unscaled % &ten);
        if !r.is_zero() {
            break;
        }
        a.unscaled = q;
        a.scale -= 1;
    }
    alloc_dec(vm, a)
}
fn bigdecimal_int_value(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let ten = BigInt::from(10u32);
    let v = if a.scale >= 0 {
        &a.unscaled / ten.pow(a.scale as u32)
    } else {
        &a.unscaled * ten.pow((-a.scale) as u32)
    };
    Ok(JValue::Int(v.to_i32().unwrap_or(0)))
}
fn bigdecimal_double_value(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let unscaled = a.unscaled.to_f64().unwrap_or(0.0);
    Ok(JValue::Double(unscaled / 10f64.powi(a.scale)))
}
fn bigdecimal_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let b = dec_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let (ua, ub, _) = align(&a, &b);
    Ok(JValue::Int(ua.cmp(&ub) as i32))
}

fn to_plain_string(dec: &Dec) -> String {
    let neg = dec.unscaled.is_negative();
    let digits = dec.unscaled.abs().to_string();
    let scale = dec.scale;
    let body = if scale <= 0 {
        format!("{digits}{}", "0".repeat((-scale) as usize))
    } else if (scale as usize) < digits.len() {
        let split = digits.len() - scale as usize;
        format!("{}.{}", &digits[..split], &digits[split..])
    } else {
        format!("0.{}{digits}", "0".repeat(scale as usize - digits.len()))
    };
    if neg {
        format!("-{body}")
    } else {
        body
    }
}
fn bigdecimal_to_plain_string(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(new_str(vm, &to_plain_string(&a)))
}
fn bigdecimal_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let a = dec_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(new_str(vm, &to_plain_string(&a)))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/math/BigDecimal;", "<init>", "(Ljava/lang/String;)V", true, bigdecimal_init_string),
    ne!("Ljava/math/BigDecimal;", "<init>", "(I)V", true, bigdecimal_init_int),
    ne!("Ljava/math/BigDecimal;", "<init>", "(Ljava/math/BigInteger;I)V", true, bigdecimal_init_biginteger_scale),
    ne!("Ljava/math/BigDecimal;", "add", "(Ljava/math/BigDecimal;)Ljava/math/BigDecimal;", true, bigdecimal_add),
    ne!("Ljava/math/BigDecimal;", "subtract", "(Ljava/math/BigDecimal;)Ljava/math/BigDecimal;", true, bigdecimal_subtract),
    ne!("Ljava/math/BigDecimal;", "multiply", "(Ljava/math/BigDecimal;)Ljava/math/BigDecimal;", true, bigdecimal_multiply),
    ne!("Ljava/math/BigDecimal;", "divide", "(Ljava/math/BigDecimal;)Ljava/math/BigDecimal;", true, bigdecimal_divide),
    ne!("Ljava/math/BigDecimal;", "divide", "(Ljava/math/BigDecimal;Ljava/math/RoundingMode;)Ljava/math/BigDecimal;", true, bigdecimal_divide),
    ne!("Ljava/math/BigDecimal;", "signum", "()I", true, bigdecimal_signum),
    ne!("Ljava/math/BigDecimal;", "scale", "()I", true, bigdecimal_scale),
    ne!("Ljava/math/BigDecimal;", "setScale", "(I)Ljava/math/BigDecimal;", true, bigdecimal_set_scale),
    ne!("Ljava/math/BigDecimal;", "setScale", "(ILjava/math/RoundingMode;)Ljava/math/BigDecimal;", true, bigdecimal_set_scale),
    ne!("Ljava/math/BigDecimal;", "stripTrailingZeros", "()Ljava/math/BigDecimal;", true, bigdecimal_strip_trailing_zeros),
    ne!("Ljava/math/BigDecimal;", "intValue", "()I", true, bigdecimal_int_value),
    ne!("Ljava/math/BigDecimal;", "doubleValue", "()D", true, bigdecimal_double_value),
    ne!("Ljava/math/BigDecimal;", "compareTo", "(Ljava/math/BigDecimal;)I", true, bigdecimal_compare_to),
    ne!("Ljava/math/BigDecimal;", "toPlainString", "()Ljava/lang/String;", true, bigdecimal_to_plain_string),
    ne!("Ljava/math/BigDecimal;", "toString", "()Ljava/lang/String;", true, bigdecimal_to_string),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_plain_strings_round_trip() {
        for s in ["123.456", "-0.001", "1000", "0.0"] {
            let dec = parse_decimal(s).unwrap();
            assert_eq!(to_plain_string(&dec), s.trim_start_matches('+'));
        }
    }

    #[test]
    fn strip_trailing_zeros_reduces_scale() {
        let mut dec = parse_decimal("1.2300").unwrap();
        let ten = BigInt::from(10u32);
        while dec.scale > 0 && (&dec.unscaled % &ten).is_zero() {
            dec.unscaled = &dec.unscaled / &ten;
            dec.scale -= 1;
        }
        assert_eq!(to_plain_string(&dec), "1.23");
    }
}
