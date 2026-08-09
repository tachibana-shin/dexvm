//! java.lang.Double host shims.

use crate::vm::native::*;

pub(crate) fn double_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    let d = double_of(vm, args[0]);
    boxed(vm, "Ljava/lang/Double;", Native::DoubleBox(d))
}

pub(crate) fn double_value_of_str(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let d = parse_double(vm, &s)?;
    boxed(vm, "Ljava/lang/Double;", Native::DoubleBox(d))
}

pub(crate) fn double_parse_double(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    parse_double(vm, &s).map(JValue::Double)
}

pub(crate) fn double_int_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(double_of(vm, args[0]) as i32))
}

pub(crate) fn double_long_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(double_of(vm, args[0]) as i64))
}

pub(crate) fn double_float_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(double_of(vm, args[0]) as f32))
}

pub(crate) fn double_double_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(double_of(vm, args[0])))
}

pub(crate) fn double_byte_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(double_of(vm, args[0]) as i8 as i32))
}

pub(crate) fn double_short_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(double_of(vm, args[0]) as i16 as i32))
}

pub(crate) fn double_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(
        double_of(vm, args[0]).to_bits() == double_of(vm, args[1]).to_bits(),
    )))
}

pub(crate) fn double_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    let b = double_of(vm, args[0]).to_bits();
    Ok(JValue::Int((b ^ (b >> 32)) as i32))
}

pub(crate) fn double_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &fmt_f64(double_of(vm, args[0]))))
}

pub(crate) fn double_to_string_static(vm: &mut Vm, args: &[JValue]) -> R {
    double_to_string(vm, args)
}

pub(crate) fn double_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    double_compare(vm, args)
}

pub(crate) fn double_compare(vm: &mut Vm, args: &[JValue]) -> R {
    let a = double_of(vm, args[0]);
    let b = double_of(vm, args[1]);
    let r = if a.is_nan() || b.is_nan() {
        if a.is_nan() && b.is_nan() {
            0
        } else if a.is_nan() {
            1
        } else {
            -1
        }
    } else if a < b {
        -1
    } else if a > b {
        1
    } else {
        a.partial_cmp(&b).unwrap_or(Ordering::Equal) as i32
    };
    Ok(JValue::Int(r))
}

pub(crate) fn double_is_nan(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(double_of(vm, args[0]).is_nan())))
}

pub(crate) fn double_is_infinite(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(double_of(vm, args[0]).is_infinite())))
}

pub(crate) fn double_to_long_bits(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(double_of(vm, args[0]).to_bits() as i64))
}

pub(crate) fn double_long_bits_to_double(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(f64::from_bits(long_of(vm, args[0]) as u64)))
}

/// Native methods for Ljava/lang/Double;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/Double;",
        "valueOf",
        "(D)Ljava/lang/Double;",
        false,
        double_value_of
    ),
    ne!(
        "Ljava/lang/Double;",
        "valueOf",
        "(Ljava/lang/String;)Ljava/lang/Double;",
        false,
        double_value_of_str
    ),
    ne!(
        "Ljava/lang/Double;",
        "parseDouble",
        "(Ljava/lang/String;)D",
        false,
        double_parse_double
    ),
    ne!(
        "Ljava/lang/Double;",
        "intValue",
        "()I",
        true,
        double_int_value
    ),
    ne!(
        "Ljava/lang/Double;",
        "longValue",
        "()J",
        true,
        double_long_value
    ),
    ne!(
        "Ljava/lang/Double;",
        "floatValue",
        "()F",
        true,
        double_float_value
    ),
    ne!(
        "Ljava/lang/Double;",
        "doubleValue",
        "()D",
        true,
        double_double_value
    ),
    ne!(
        "Ljava/lang/Double;",
        "byteValue",
        "()B",
        true,
        double_byte_value
    ),
    ne!(
        "Ljava/lang/Double;",
        "shortValue",
        "()S",
        true,
        double_short_value
    ),
    ne!(
        "Ljava/lang/Double;",
        "equals",
        "(Ljava/lang/Object;)Z",
        true,
        double_equals
    ),
    ne!(
        "Ljava/lang/Double;",
        "hashCode",
        "()I",
        true,
        double_hash_code
    ),
    ne!(
        "Ljava/lang/Double;",
        "toString",
        "()Ljava/lang/String;",
        true,
        double_to_string
    ),
    ne!(
        "Ljava/lang/Double;",
        "toString",
        "(D)Ljava/lang/String;",
        false,
        double_to_string_static
    ),
    ne!(
        "Ljava/lang/Double;",
        "compareTo",
        "(Ljava/lang/Double;)I",
        true,
        double_compare_to
    ),
    ne!(
        "Ljava/lang/Double;",
        "compareTo",
        "(Ljava/lang/Object;)I",
        true,
        double_compare_to
    ),
    ne!(
        "Ljava/lang/Double;",
        "compare",
        "(DD)I",
        false,
        double_compare
    ),
    ne!("Ljava/lang/Double;", "isNaN", "(D)Z", false, double_is_nan),
    ne!(
        "Ljava/lang/Double;",
        "isInfinite",
        "(D)Z",
        false,
        double_is_infinite
    ),
    ne!(
        "Ljava/lang/Double;",
        "doubleToLongBits",
        "(D)J",
        false,
        double_to_long_bits
    ),
    ne!(
        "Ljava/lang/Double;",
        "doubleToRawLongBits",
        "(D)J",
        false,
        double_to_long_bits
    ),
    ne!(
        "Ljava/lang/Double;",
        "longBitsToDouble",
        "(J)D",
        false,
        double_long_bits_to_double
    ),
];
