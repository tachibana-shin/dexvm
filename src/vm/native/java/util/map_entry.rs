//! java.util.Map$Entry host shims.

use crate::vm::native::*;

// ---- java.util.Map$Entry / java.util.Iterator ----

pub(crate) fn entry_get_key(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::MapEntry { map, idx }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some(Native::Map(entries)) = payload(vm, JValue::Obj(*map)) else {
        return Err(npe(vm));
    };
    match entries.get(*idx) {
        Some((k, _)) => Ok(*k),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn entry_get_value(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::MapEntry { map, idx }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some(Native::Map(entries)) = payload(vm, JValue::Obj(*map)) else {
        return Err(npe(vm));
    };
    match entries.get(*idx) {
        Some((_, v)) => Ok(*v),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn entry_set_value(vm: &mut Vm, args: &[JValue]) -> R {
    let (map, idx) = match payload(vm, args[0]) {
        Some(Native::MapEntry { map, idx }) => (*map, *idx),
        _ => return Err(npe(vm)),
    };
    let old = match payload(vm, JValue::Obj(map)) {
        Some(Native::Map(entries)) => entries.get(idx).map(|(_, v)| *v).unwrap_or(JValue::Null),
        _ => return Err(npe(vm)),
    };
    let val = args[1];
    let Some(n) = payload_mut(vm, JValue::Obj(map)) else {
        return Err(npe(vm));
    };
    match n {
        Native::Map(dst) => {
            if let Some(e) = dst.get_mut(idx) {
                e.1 = val;
            }
        }
        _ => return Err(npe(vm)),
    }
    Ok(old)
}

/// Native methods for Ljava/util/Map$Entry;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/Map$Entry;",
        "getKey",
        "()Ljava/lang/Object;",
        true,
        entry_get_key
    ),
    ne!(
        "Ljava/util/Map$Entry;",
        "getValue",
        "()Ljava/lang/Object;",
        true,
        entry_get_value
    ),
    ne!(
        "Ljava/util/Map$Entry;",
        "setValue",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        entry_set_value
    ),
];
