//! java.lang.Enum host shims.

use crate::vm::native::*;

pub(crate) fn enum_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    let ordinal = int_of(vm, args[2]);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Enum {
            name: dst,
            ordinal: o,
        } => {
            *dst = name;
            *o = ordinal;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn enum_name(vm: &mut Vm, args: &[JValue]) -> R {
    let name = match payload(vm, args[0]) {
        Some(Native::Enum { name, .. }) => name.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &name))
}

pub(crate) fn enum_ordinal(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Enum { ordinal, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(*ordinal))
}

pub(crate) fn enum_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    enum_name(vm, args)
}

pub(crate) fn enum_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    let a = match payload(vm, args[0]) {
        Some(Native::Enum { ordinal, .. }) => *ordinal,
        _ => return Err(npe(vm)),
    };
    let b = match payload(vm, args[1]) {
        Some(Native::Enum { ordinal, .. }) => *ordinal,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(a.cmp(&b) as i32))
}

// ---------------------------------------------------------------------------

/// Native methods for Ljava/lang/Enum;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/Enum;",
        "<init>",
        "(Ljava/lang/String;I)V",
        true,
        enum_init
    ),
    ne!(
        "Ljava/lang/Enum;",
        "name",
        "()Ljava/lang/String;",
        true,
        enum_name
    ),
    ne!("Ljava/lang/Enum;", "ordinal", "()I", true, enum_ordinal),
    ne!(
        "Ljava/lang/Enum;",
        "toString",
        "()Ljava/lang/String;",
        true,
        enum_to_string
    ),
    ne!(
        "Ljava/lang/Enum;",
        "compareTo",
        "(Ljava/lang/Enum;)I",
        true,
        enum_compare_to
    ),
    ne!(
        "Ljava/lang/Enum;",
        "compareTo",
        "(Ljava/lang/Object;)I",
        true,
        enum_compare_to
    ),
];
