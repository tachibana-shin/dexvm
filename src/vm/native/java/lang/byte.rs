//! java.lang.Byte host shims.

use crate::vm::native::*;

pub(crate) fn byte_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    box_int_value(vm, "Ljava/lang/Byte;", args[0])
}

pub(crate) fn byte_parse_byte(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let radix = if args.len() > 1 {
        int_of(vm, args[1]) as u32
    } else {
        10
    };
    let n = parse_int_radix(vm, &s, radix)?;
    if n < i32::from(i8::MIN) || n > i32::from(i8::MAX) {
        return Err(nfe(vm, format!("Value out of range: \"{s}\"")));
    }
    Ok(JValue::Int(n))
}

pub(crate) fn byte_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &int_of(vm, args[0]).to_string()))
}

pub(crate) fn byte_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).cmp(&int_of(vm, args[1])) as i32))
}


/// Native methods for Ljava/lang/Byte;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/lang/Byte;", "valueOf", "(B)Ljava/lang/Byte;", false, byte_value_of),
    ne!("Ljava/lang/Byte;", "parseByte", "(Ljava/lang/String;)B", false, byte_parse_byte),
    ne!("Ljava/lang/Byte;", "parseByte", "(Ljava/lang/String;I)B", false, byte_parse_byte),
    ne!("Ljava/lang/Byte;", "toString", "(B)Ljava/lang/String;", false, byte_to_string),
    ne!("Ljava/lang/Byte;", "intValue", "()I", true, integer_int_value),
    ne!("Ljava/lang/Byte;", "shortValue", "()S", true, integer_short_value),
    ne!("Ljava/lang/Byte;", "byteValue", "()B", true, integer_byte_value),
    ne!("Ljava/lang/Byte;", "equals", "(Ljava/lang/Object;)Z", true, integer_equals),
    ne!("Ljava/lang/Byte;", "hashCode", "()I", true, integer_hash_code),
    ne!("Ljava/lang/Byte;", "toString", "()Ljava/lang/String;", true, byte_to_string),
    ne!("Ljava/lang/Byte;", "compareTo", "(Ljava/lang/Byte;)I", true, byte_compare_to),
    ne!("Ljava/lang/Byte;", "compareTo", "(Ljava/lang/Object;)I", true, byte_compare_to),
];
