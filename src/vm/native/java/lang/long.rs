//! java.lang.Long host shims.

use crate::vm::native::*;

pub(crate) fn long_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    let n = long_of(vm, args[0]);
    boxed(vm, "Ljava/lang/Long;", Native::LongBox(n))
}

pub(crate) fn long_value_of_str(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let n = parse_long_radix(vm, &s, 10)?;
    boxed(vm, "Ljava/lang/Long;", Native::LongBox(n))
}

pub(crate) fn long_int_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(long_of(vm, args[0]) as i32))
}

pub(crate) fn long_long_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0])))
}

pub(crate) fn long_float_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(long_of(vm, args[0]) as f32))
}

pub(crate) fn long_double_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(long_of(vm, args[0]) as f64))
}

pub(crate) fn long_byte_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(long_of(vm, args[0]) as i8 as i32))
}

pub(crate) fn long_short_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(long_of(vm, args[0]) as i16 as i32))
}

pub(crate) fn long_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(
        long_of(vm, args[0]) == long_of(vm, args[1]),
    )))
}

pub(crate) fn long_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    let l = long_of(vm, args[0]);
    Ok(JValue::Int((l ^ (l >> 32)) as i32))
}

pub(crate) fn long_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &long_of(vm, args[0]).to_string()))
}

pub(crate) fn long_to_string_static(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &long_to_string_help(long_of(vm, args[0]), 10)))
}

pub(crate) fn long_to_string_radix(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(
        vm,
        &long_to_string_help(long_of(vm, args[0]), int_of(vm, args[1]) as u32),
    ))
}

pub(crate) fn long_parse_long(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let radix = if args.len() > 1 {
        int_of(vm, args[1]) as u32
    } else {
        10
    };
    parse_long_radix(vm, &s, radix).map(JValue::Long)
}

pub(crate) fn long_to_hex(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &format!("{:x}", long_of(vm, args[0]) as u64)))
}

pub(crate) fn long_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(
        long_of(vm, args[0]).cmp(&long_of(vm, args[1])) as i32
    ))
}

pub(crate) fn long_compare(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(
        long_of(vm, args[0]).cmp(&long_of(vm, args[1])) as i32
    ))
}

pub(crate) fn long_bit_count(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(long_of(vm, args[0]).count_ones() as i32))
}

pub(crate) fn long_signum(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0]).signum()))
}

// ---------------------------------------------------------------------------

/// Native methods for Ljava/lang/Long;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/Long;",
        "valueOf",
        "(J)Ljava/lang/Long;",
        false,
        long_value_of
    ),
    ne!(
        "Ljava/lang/Long;",
        "valueOf",
        "(Ljava/lang/String;)Ljava/lang/Long;",
        false,
        long_value_of_str
    ),
    ne!(
        "Ljava/lang/Long;",
        "parseLong",
        "(Ljava/lang/String;)J",
        false,
        long_parse_long
    ),
    ne!(
        "Ljava/lang/Long;",
        "parseLong",
        "(Ljava/lang/String;I)J",
        false,
        long_parse_long
    ),
    ne!(
        "Ljava/lang/Long;",
        "toString",
        "(J)Ljava/lang/String;",
        false,
        long_to_string_static
    ),
    ne!(
        "Ljava/lang/Long;",
        "toString",
        "(JI)Ljava/lang/String;",
        false,
        long_to_string_radix
    ),
    ne!(
        "Ljava/lang/Long;",
        "toHexString",
        "(J)Ljava/lang/String;",
        false,
        long_to_hex
    ),
    ne!("Ljava/lang/Long;", "compare", "(JJ)I", false, long_compare),
    ne!(
        "Ljava/lang/Long;",
        "bitCount",
        "(J)I",
        false,
        long_bit_count
    ),
    ne!("Ljava/lang/Long;", "signum", "(J)I", false, long_signum),
    ne!("Ljava/lang/Long;", "intValue", "()I", true, long_int_value),
    ne!(
        "Ljava/lang/Long;",
        "longValue",
        "()J",
        true,
        long_long_value
    ),
    ne!(
        "Ljava/lang/Long;",
        "floatValue",
        "()F",
        true,
        long_float_value
    ),
    ne!(
        "Ljava/lang/Long;",
        "doubleValue",
        "()D",
        true,
        long_double_value
    ),
    ne!(
        "Ljava/lang/Long;",
        "byteValue",
        "()B",
        true,
        long_byte_value
    ),
    ne!(
        "Ljava/lang/Long;",
        "shortValue",
        "()S",
        true,
        long_short_value
    ),
    ne!(
        "Ljava/lang/Long;",
        "equals",
        "(Ljava/lang/Object;)Z",
        true,
        long_equals
    ),
    ne!("Ljava/lang/Long;", "hashCode", "()I", true, long_hash_code),
    ne!(
        "Ljava/lang/Long;",
        "toString",
        "()Ljava/lang/String;",
        true,
        long_to_string
    ),
    ne!(
        "Ljava/lang/Long;",
        "compareTo",
        "(Ljava/lang/Long;)I",
        true,
        long_compare_to
    ),
    ne!(
        "Ljava/lang/Long;",
        "compareTo",
        "(Ljava/lang/Object;)I",
        true,
        long_compare_to
    ),
];
