//! java.text.ParsePosition host shims.

use crate::vm::native::*;

pub(crate) fn parse_position_init(vm: &mut Vm, args: &[JValue]) -> R {
    let idx = int_of(vm, args[1]);
    let Some(Native::ParsePosition(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = idx;
    Ok(JValue::Null)
}

pub(crate) fn parse_position_get_index(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ParsePosition(i)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(*i))
}

pub(crate) fn parse_position_set_index(vm: &mut Vm, args: &[JValue]) -> R {
    let idx = int_of(vm, args[1]);
    let Some(Native::ParsePosition(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = idx;
    Ok(JValue::Null)
}

/// Native methods for Ljava/text/ParsePosition;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/text/ParsePosition;",
        "<init>",
        "(I)V",
        true,
        parse_position_init
    ),
    ne!(
        "Ljava/text/ParsePosition;",
        "getIndex",
        "()I",
        true,
        parse_position_get_index
    ),
    ne!(
        "Ljava/text/ParsePosition;",
        "setIndex",
        "(I)V",
        true,
        parse_position_set_index
    ),
];
