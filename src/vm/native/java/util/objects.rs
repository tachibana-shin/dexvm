//! java.util.Objects host shims.

use crate::vm::native::*;

// java.util.Objects (all static)
// ---------------------------------------------------------------------------

pub(crate) fn objects_equals(vm: &mut Vm, args: &[JValue]) -> R {
    java_equals(vm, args[0], args[1]).map(|b| JValue::Int(i32::from(b)))
}

pub(crate) fn objects_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(java_hash(vm, args[0])))
}

pub(crate) fn objects_hash(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Obj(items))) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut h: i32 = 1;
    for v in items {
        h = h.wrapping_mul(31).wrapping_add(java_hash(vm, v));
    }
    Ok(JValue::Int(h))
}

pub(crate) fn objects_require_non_null(vm: &mut Vm, args: &[JValue]) -> R {
    if args[0].is_null() {
        let msg = if args.len() > 1 && !args[1].is_null() {
            jstr(vm, args[1])?
        } else {
            "null".to_string()
        };
        Err(NatErr::Throw(
            vm.throwable_of("Ljava/lang/NullPointerException;", msg),
        ))
    } else {
        Ok(args[0])
    }
}

pub(crate) fn objects_require_non_null_else(_vm: &mut Vm, args: &[JValue]) -> R {
    if args[0].is_null() {
        Ok(args[1])
    } else {
        Ok(args[0])
    }
}

pub(crate) fn objects_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = to_string_of(vm, args[0])?;
    Ok(new_str(vm, &s))
}

pub(crate) fn objects_to_string_def(vm: &mut Vm, args: &[JValue]) -> R {
    if args[0].is_null() {
        let s = jstr(vm, args[1])?;
        Ok(new_str(vm, &s))
    } else {
        let s = to_string_of(vm, args[0])?;
        Ok(new_str(vm, &s))
    }
}

pub(crate) fn objects_is_null(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(args[0].is_null())))
}

pub(crate) fn objects_non_null(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(!args[0].is_null())))
}

// ---------------------------------------------------------------------------

/// Native methods for Ljava/util/Objects;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/Objects;",
        "equals",
        "(Ljava/lang/Object;Ljava/lang/Object;)Z",
        false,
        objects_equals
    ),
    ne!(
        "Ljava/util/Objects;",
        "hashCode",
        "(Ljava/lang/Object;)I",
        false,
        objects_hash_code
    ),
    ne!(
        "Ljava/util/Objects;",
        "hash",
        "([Ljava/lang/Object;)I",
        false,
        objects_hash
    ),
    ne!(
        "Ljava/util/Objects;",
        "requireNonNull",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        false,
        objects_require_non_null
    ),
    ne!(
        "Ljava/util/Objects;",
        "requireNonNull",
        "(Ljava/lang/Object;Ljava/lang/String;)Ljava/lang/Object;",
        false,
        objects_require_non_null
    ),
    ne!(
        "Ljava/util/Objects;",
        "requireNonNullElse",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        false,
        objects_require_non_null_else
    ),
    ne!(
        "Ljava/util/Objects;",
        "toString",
        "(Ljava/lang/Object;)Ljava/lang/String;",
        false,
        objects_to_string
    ),
    ne!(
        "Ljava/util/Objects;",
        "toString",
        "(Ljava/lang/Object;Ljava/lang/String;)Ljava/lang/String;",
        false,
        objects_to_string_def
    ),
    ne!(
        "Ljava/util/Objects;",
        "isNull",
        "(Ljava/lang/Object;)Z",
        false,
        objects_is_null
    ),
    ne!(
        "Ljava/util/Objects;",
        "nonNull",
        "(Ljava/lang/Object;)Z",
        false,
        objects_non_null
    ),
];
