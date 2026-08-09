//! java.lang.Short host shims.

use crate::vm::native::*;

pub(crate) fn short_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    box_int_value(vm, "Ljava/lang/Short;", args[0])
}

pub(crate) fn short_parse_short(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let radix = if args.len() > 1 {
        int_of(vm, args[1]) as u32
    } else {
        10
    };
    let n = parse_int_radix(vm, &s, radix)?;
    if n < i32::from(i16::MIN) || n > i32::from(i16::MAX) {
        return Err(nfe(vm, format!("Value out of range: \"{s}\"")));
    }
    Ok(JValue::Int(n))
}

pub(crate) fn short_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &int_of(vm, args[0]).to_string()))
}

pub(crate) fn short_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(
        int_of(vm, args[0]).cmp(&int_of(vm, args[1])) as i32
    ))
}

/// Native methods for Ljava/lang/Short;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/Short;",
        "valueOf",
        "(S)Ljava/lang/Short;",
        false,
        short_value_of
    ),
    ne!(
        "Ljava/lang/Short;",
        "parseShort",
        "(Ljava/lang/String;)S",
        false,
        short_parse_short
    ),
    ne!(
        "Ljava/lang/Short;",
        "parseShort",
        "(Ljava/lang/String;I)S",
        false,
        short_parse_short
    ),
    ne!(
        "Ljava/lang/Short;",
        "toString",
        "(S)Ljava/lang/String;",
        false,
        short_to_string
    ),
    ne!(
        "Ljava/lang/Short;",
        "intValue",
        "()I",
        true,
        integer_int_value
    ),
    ne!(
        "Ljava/lang/Short;",
        "shortValue",
        "()S",
        true,
        integer_short_value
    ),
    ne!(
        "Ljava/lang/Short;",
        "byteValue",
        "()B",
        true,
        integer_byte_value
    ),
    ne!(
        "Ljava/lang/Short;",
        "equals",
        "(Ljava/lang/Object;)Z",
        true,
        integer_equals
    ),
    ne!(
        "Ljava/lang/Short;",
        "hashCode",
        "()I",
        true,
        integer_hash_code
    ),
    ne!(
        "Ljava/lang/Short;",
        "toString",
        "()Ljava/lang/String;",
        true,
        short_to_string
    ),
    ne!(
        "Ljava/lang/Short;",
        "compareTo",
        "(Ljava/lang/Short;)I",
        true,
        short_compare_to
    ),
    ne!(
        "Ljava/lang/Short;",
        "compareTo",
        "(Ljava/lang/Object;)I",
        true,
        short_compare_to
    ),
];
