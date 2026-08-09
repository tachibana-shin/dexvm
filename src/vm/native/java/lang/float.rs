//! java.lang.Float host shims.

use crate::vm::native::*;

pub(crate) fn float_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    let f = float_of(vm, args[0]);
    boxed(vm, "Ljava/lang/Float;", Native::FloatBox(f))
}

pub(crate) fn float_value_of_str(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let f = parse_float(vm, &s)?;
    boxed(vm, "Ljava/lang/Float;", Native::FloatBox(f))
}

pub(crate) fn float_parse_float(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    parse_float(vm, &s).map(JValue::Float)
}

pub(crate) fn float_int_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(float_of(vm, args[0]) as i32))
}

pub(crate) fn float_long_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(float_of(vm, args[0]) as i64))
}

pub(crate) fn float_float_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(float_of(vm, args[0])))
}

pub(crate) fn float_double_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(f64::from(float_of(vm, args[0]))))
}

pub(crate) fn float_byte_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(float_of(vm, args[0]) as i8 as i32))
}

pub(crate) fn float_short_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(float_of(vm, args[0]) as i16 as i32))
}

pub(crate) fn float_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(
        float_of(vm, args[0]).to_bits() == float_of(vm, args[1]).to_bits(),
    )))
}

pub(crate) fn float_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(float_of(vm, args[0]).to_bits() as i32))
}

pub(crate) fn float_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &fmt_f32(float_of(vm, args[0]))))
}

pub(crate) fn float_to_string_static(vm: &mut Vm, args: &[JValue]) -> R {
    float_to_string(vm, args)
}

pub(crate) fn float_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    float_compare(vm, args)
}

pub(crate) fn float_compare(vm: &mut Vm, args: &[JValue]) -> R {
    let a = float_of(vm, args[0]);
    let b = float_of(vm, args[1]);
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

pub(crate) fn float_is_nan(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(float_of(vm, args[0]).is_nan())))
}

pub(crate) fn float_is_infinite(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(float_of(vm, args[0]).is_infinite())))
}

pub(crate) fn float_to_int_bits(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(float_of(vm, args[0]).to_bits() as i32))
}

pub(crate) fn float_int_bits_to_float(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(f32::from_bits(int_of(vm, args[0]) as u32)))
}


/// Native methods for Ljava/lang/Float;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/lang/Float;", "valueOf", "(F)Ljava/lang/Float;", false, float_value_of),
    ne!("Ljava/lang/Float;", "valueOf", "(Ljava/lang/String;)Ljava/lang/Float;", false, float_value_of_str),
    ne!("Ljava/lang/Float;", "parseFloat", "(Ljava/lang/String;)F", false, float_parse_float),
    ne!("Ljava/lang/Float;", "intValue", "()I", true, float_int_value),
    ne!("Ljava/lang/Float;", "longValue", "()J", true, float_long_value),
    ne!("Ljava/lang/Float;", "floatValue", "()F", true, float_float_value),
    ne!("Ljava/lang/Float;", "doubleValue", "()D", true, float_double_value),
    ne!("Ljava/lang/Float;", "byteValue", "()B", true, float_byte_value),
    ne!("Ljava/lang/Float;", "shortValue", "()S", true, float_short_value),
    ne!("Ljava/lang/Float;", "equals", "(Ljava/lang/Object;)Z", true, float_equals),
    ne!("Ljava/lang/Float;", "hashCode", "()I", true, float_hash_code),
    ne!("Ljava/lang/Float;", "toString", "()Ljava/lang/String;", true, float_to_string),
    ne!("Ljava/lang/Float;", "toString", "(F)Ljava/lang/String;", false, float_to_string_static),
    ne!("Ljava/lang/Float;", "compareTo", "(Ljava/lang/Float;)I", true, float_compare_to),
    ne!("Ljava/lang/Float;", "compareTo", "(Ljava/lang/Object;)I", true, float_compare_to),
    ne!("Ljava/lang/Float;", "compare", "(FF)I", false, float_compare),
    ne!("Ljava/lang/Float;", "isNaN", "(F)Z", false, float_is_nan),
    ne!("Ljava/lang/Float;", "isInfinite", "(F)Z", false, float_is_infinite),
    ne!("Ljava/lang/Float;", "floatToIntBits", "(F)I", false, float_to_int_bits),
    ne!("Ljava/lang/Float;", "floatToRawIntBits", "(F)I", false, float_to_int_bits),
    ne!("Ljava/lang/Float;", "intBitsToFloat", "(I)F", false, float_int_bits_to_float),
];
