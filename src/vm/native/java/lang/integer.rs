//! java.lang.Integer host shims.

use crate::vm::native::*;

pub(crate) fn box_int_value(vm: &mut Vm, desc: &str, v: JValue) -> R {
    let n = int_of(vm, v);
    let native = match desc {
        "Ljava/lang/Integer;" => Native::IntBox(n),
        "Ljava/lang/Short;" => Native::ShortBox(n as i16),
        "Ljava/lang/Byte;" => Native::ByteBox(n as i8),
        "Ljava/lang/Character;" => Native::CharBox(n as u16),
        _ => return Err(iae(vm, "bad box class")),
    };
    boxed(vm, desc, native)
}

pub(crate) fn box_int(vm: &mut Vm, desc: &str, args: &[JValue], i: usize) -> R {
    box_int_value(vm, desc, args[i])
}

pub(crate) fn integer_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    box_int(vm, "Ljava/lang/Integer;", args, 0)
}

pub(crate) fn integer_value_of_str(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let n = parse_int_radix(vm, &s, 10)?;
    box_int_value(vm, "Ljava/lang/Integer;", JValue::Int(n))
}

pub(crate) fn integer_int_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0])))
}

pub(crate) fn integer_long_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(i64::from(int_of(vm, args[0]))))
}

pub(crate) fn integer_float_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(int_of(vm, args[0]) as f32))
}

pub(crate) fn integer_double_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(f64::from(int_of(vm, args[0]))))
}

pub(crate) fn integer_byte_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]) as i8 as i32))
}

pub(crate) fn integer_short_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]) as i16 as i32))
}

pub(crate) fn integer_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(
        int_of(vm, args[0]) == int_of(vm, args[1]),
    )))
}

pub(crate) fn integer_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0])))
}

pub(crate) fn integer_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[0]);
    Ok(new_str(vm, &n.to_string()))
}

pub(crate) fn integer_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(
        int_of(vm, args[0]).cmp(&int_of(vm, args[1])) as i32
    ))
}

pub(crate) fn integer_to_string_static(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &int_to_string(int_of(vm, args[0]), 10)))
}

pub(crate) fn integer_to_string_radix(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(
        vm,
        &int_to_string(int_of(vm, args[0]), int_of(vm, args[1]) as u32),
    ))
}

pub(crate) fn integer_parse_int(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let radix = if args.len() > 1 {
        int_of(vm, args[1]) as u32
    } else {
        10
    };
    parse_int_radix(vm, &s, radix).map(JValue::Int)
}

pub(crate) fn integer_to_hex(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &format!("{:x}", int_of(vm, args[0]) as u32)))
}

pub(crate) fn integer_to_binary(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &format!("{:b}", int_of(vm, args[0]) as u32)))
}

pub(crate) fn integer_to_octal(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &format!("{:o}", int_of(vm, args[0]) as u32)))
}

pub(crate) fn integer_compare(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(
        int_of(vm, args[0]).cmp(&int_of(vm, args[1])) as i32
    ))
}

pub(crate) fn integer_bit_count(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).count_ones() as i32))
}

pub(crate) fn integer_highest_one_bit(vm: &mut Vm, args: &[JValue]) -> R {
    let v = int_of(vm, args[0]);
    if v == 0 {
        Ok(JValue::Int(0))
    } else {
        Ok(JValue::Int(1i32 << (31 - v.leading_zeros())))
    }
}

pub(crate) fn integer_signum(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).signum()))
}

/// Native methods for Ljava/lang/Integer;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/Integer;",
        "hashCode",
        "(I)I",
        false,
        integer_hash_code
    ),
    ne!(
        "Ljava/lang/Integer;",
        "valueOf",
        "(I)Ljava/lang/Integer;",
        false,
        integer_value_of
    ),
    ne!(
        "Ljava/lang/Integer;",
        "valueOf",
        "(Ljava/lang/String;)Ljava/lang/Integer;",
        false,
        integer_value_of_str
    ),
    ne!(
        "Ljava/lang/Integer;",
        "parseInt",
        "(Ljava/lang/String;)I",
        false,
        integer_parse_int
    ),
    ne!(
        "Ljava/lang/Integer;",
        "parseInt",
        "(Ljava/lang/String;I)I",
        false,
        integer_parse_int
    ),
    ne!(
        "Ljava/lang/Integer;",
        "toString",
        "(I)Ljava/lang/String;",
        false,
        integer_to_string_static
    ),
    ne!(
        "Ljava/lang/Integer;",
        "toString",
        "(II)Ljava/lang/String;",
        false,
        integer_to_string_radix
    ),
    ne!(
        "Ljava/lang/Integer;",
        "toHexString",
        "(I)Ljava/lang/String;",
        false,
        integer_to_hex
    ),
    ne!(
        "Ljava/lang/Integer;",
        "toBinaryString",
        "(I)Ljava/lang/String;",
        false,
        integer_to_binary
    ),
    ne!(
        "Ljava/lang/Integer;",
        "toOctalString",
        "(I)Ljava/lang/String;",
        false,
        integer_to_octal
    ),
    ne!(
        "Ljava/lang/Integer;",
        "compare",
        "(II)I",
        false,
        integer_compare
    ),
    ne!(
        "Ljava/lang/Integer;",
        "bitCount",
        "(I)I",
        false,
        integer_bit_count
    ),
    ne!(
        "Ljava/lang/Integer;",
        "highestOneBit",
        "(I)I",
        false,
        integer_highest_one_bit
    ),
    ne!(
        "Ljava/lang/Integer;",
        "signum",
        "(I)I",
        false,
        integer_signum
    ),
    ne!(
        "Ljava/lang/Integer;",
        "intValue",
        "()I",
        true,
        integer_int_value
    ),
    ne!(
        "Ljava/lang/Integer;",
        "longValue",
        "()J",
        true,
        integer_long_value
    ),
    ne!(
        "Ljava/lang/Integer;",
        "floatValue",
        "()F",
        true,
        integer_float_value
    ),
    ne!(
        "Ljava/lang/Integer;",
        "doubleValue",
        "()D",
        true,
        integer_double_value
    ),
    ne!(
        "Ljava/lang/Integer;",
        "byteValue",
        "()B",
        true,
        integer_byte_value
    ),
    ne!(
        "Ljava/lang/Integer;",
        "shortValue",
        "()S",
        true,
        integer_short_value
    ),
    ne!(
        "Ljava/lang/Integer;",
        "equals",
        "(Ljava/lang/Object;)Z",
        true,
        integer_equals
    ),
    ne!(
        "Ljava/lang/Integer;",
        "hashCode",
        "()I",
        true,
        integer_hash_code
    ),
    ne!(
        "Ljava/lang/Integer;",
        "toString",
        "()Ljava/lang/String;",
        true,
        integer_to_string
    ),
    ne!(
        "Ljava/lang/Integer;",
        "compareTo",
        "(Ljava/lang/Integer;)I",
        true,
        integer_compare_to
    ),
    ne!(
        "Ljava/lang/Integer;",
        "compareTo",
        "(Ljava/lang/Object;)I",
        true,
        integer_compare_to
    ),
];
