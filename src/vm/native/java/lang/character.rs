//! java.lang.Character host shims.

use crate::vm::native::*;

pub(crate) fn char_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    box_int_value(vm, "Ljava/lang/Character;", args[0])
}

pub(crate) fn char_char_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0])))
}

pub(crate) fn char_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(
        int_of(vm, args[0]) == int_of(vm, args[1]),
    )))
}

pub(crate) fn char_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0])))
}

pub(crate) fn char_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u16;
    Ok(new_str(vm, &u16str(&[c])))
}

pub(crate) fn char_to_string_static(vm: &mut Vm, args: &[JValue]) -> R {
    char_to_string(vm, args)
}

pub(crate) fn char_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(
        int_of(vm, args[0]).cmp(&int_of(vm, args[1])) as i32
    ))
}

pub(crate) fn char_is_digit(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(
        char::from_u32(c).is_some_and(|c| c.is_ascii_digit()),
    )))
}

pub(crate) fn char_is_letter(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(
        char::from_u32(c).is_some_and(|c| c.is_alphabetic()),
    )))
}

pub(crate) fn char_is_letter_or_digit(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(
        char::from_u32(c).is_some_and(|c| c.is_alphanumeric()),
    )))
}

pub(crate) fn char_is_whitespace(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(
        char::from_u32(c).is_some_and(|c| c.is_whitespace()),
    )))
}

pub(crate) fn char_is_upper(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(
        char::from_u32(c).is_some_and(|c| c.is_uppercase()),
    )))
}

pub(crate) fn char_is_lower(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(
        char::from_u32(c).is_some_and(|c| c.is_lowercase()),
    )))
}

pub(crate) fn char_to_upper(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(
        char::from_u32(c)
            .and_then(|c| c.to_uppercase().next())
            .map(|c| c as u32)
            .unwrap_or(c) as u16,
    )))
}

pub(crate) fn char_to_lower(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(
        char::from_u32(c)
            .and_then(|c| c.to_lowercase().next())
            .map(|c| c as u32)
            .unwrap_or(c) as u16,
    )))
}

pub(crate) fn char_is_high_surrogate(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u16;
    Ok(JValue::Int(i32::from((0xD800..=0xDBFF).contains(&c))))
}

pub(crate) fn char_is_low_surrogate(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u16;
    Ok(JValue::Int(i32::from((0xDC00..=0xDFFF).contains(&c))))
}

pub(crate) fn char_get_numeric_value(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    let v = char::from_u32(c)
        .map(|c| c.to_digit(10).map(|d| d as i32).unwrap_or(-1))
        .unwrap_or(-1);
    Ok(JValue::Int(v))
}

/// Native methods for Ljava/lang/Character;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/Character;",
        "valueOf",
        "(C)Ljava/lang/Character;",
        false,
        char_value_of
    ),
    ne!(
        "Ljava/lang/Character;",
        "charValue",
        "()C",
        true,
        char_char_value
    ),
    ne!(
        "Ljava/lang/Character;",
        "equals",
        "(Ljava/lang/Object;)Z",
        true,
        char_equals
    ),
    ne!(
        "Ljava/lang/Character;",
        "hashCode",
        "()I",
        true,
        char_hash_code
    ),
    ne!(
        "Ljava/lang/Character;",
        "toString",
        "()Ljava/lang/String;",
        true,
        char_to_string
    ),
    ne!(
        "Ljava/lang/Character;",
        "toString",
        "(C)Ljava/lang/String;",
        false,
        char_to_string_static
    ),
    ne!(
        "Ljava/lang/Character;",
        "compareTo",
        "(Ljava/lang/Character;)I",
        true,
        char_compare_to
    ),
    ne!(
        "Ljava/lang/Character;",
        "compareTo",
        "(Ljava/lang/Object;)I",
        true,
        char_compare_to
    ),
    ne!(
        "Ljava/lang/Character;",
        "isDigit",
        "(C)Z",
        false,
        char_is_digit
    ),
    ne!(
        "Ljava/lang/Character;",
        "isLetter",
        "(C)Z",
        false,
        char_is_letter
    ),
    ne!(
        "Ljava/lang/Character;",
        "isLetterOrDigit",
        "(C)Z",
        false,
        char_is_letter_or_digit
    ),
    ne!(
        "Ljava/lang/Character;",
        "isWhitespace",
        "(C)Z",
        false,
        char_is_whitespace
    ),
    ne!(
        "Ljava/lang/Character;",
        "isUpperCase",
        "(C)Z",
        false,
        char_is_upper
    ),
    ne!(
        "Ljava/lang/Character;",
        "isLowerCase",
        "(C)Z",
        false,
        char_is_lower
    ),
    ne!(
        "Ljava/lang/Character;",
        "toUpperCase",
        "(C)C",
        false,
        char_to_upper
    ),
    ne!(
        "Ljava/lang/Character;",
        "toLowerCase",
        "(C)C",
        false,
        char_to_lower
    ),
    ne!(
        "Ljava/lang/Character;",
        "isHighSurrogate",
        "(C)Z",
        false,
        char_is_high_surrogate
    ),
    ne!(
        "Ljava/lang/Character;",
        "isLowSurrogate",
        "(C)Z",
        false,
        char_is_low_surrogate
    ),
    ne!(
        "Ljava/lang/Character;",
        "getNumericValue",
        "(C)I",
        false,
        char_get_numeric_value
    ),
];
