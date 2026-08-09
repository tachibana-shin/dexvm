//! java.util.Iterator host shims.

use crate::vm::native::*;

pub(crate) fn iter_has_next(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Iter(k)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let (coll, idx) = match k {
        IterKind::List { list, idx } => (JValue::Obj(*list), *idx),
        IterKind::Set { set, idx } => (JValue::Obj(*set), *idx),
        IterKind::MapEntries { map, idx } | IterKind::MapKeys { map, idx } | IterKind::MapValues { map, idx } => {
            (JValue::Obj(*map), *idx)
        }
    };
    let len = match payload(vm, coll) {
        Some(Native::List(items)) => items.len(),
        Some(Native::Set(items)) => items.len(),
        #[cfg(feature = "keiyoushi")]
        Some(Native::SFilterList(items)) => items.len(),
        Some(Native::Map(entries)) => entries.len(),
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(i32::from(idx < len)))
}

pub(crate) fn iter_next(vm: &mut Vm, args: &[JValue]) -> R {
    let k = match payload(vm, args[0]) {
        Some(Native::Iter(k)) => k.clone(),
        _ => return Err(npe(vm)),
    };
    let (coll, idx) = match &k {
        IterKind::List { list, idx } => (JValue::Obj(*list), *idx),
        IterKind::Set { set, idx } => (JValue::Obj(*set), *idx),
        IterKind::MapEntries { map, idx } | IterKind::MapKeys { map, idx } | IterKind::MapValues { map, idx } => {
            (JValue::Obj(*map), *idx)
        }
    };
    let item = match &k {
        IterKind::List { .. } => match payload(vm, coll) {
            Some(Native::List(items)) => items
                .get(idx)
                .copied()
                .ok_or_else(|| no_such_elem(vm)),
            #[cfg(feature = "keiyoushi")]
            Some(Native::SFilterList(items)) => items
                .get(idx)
                .copied()
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
        IterKind::Set { .. } => match payload(vm, coll) {
            Some(Native::Set(items)) => items
                .get(idx)
                .copied()
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
        IterKind::MapEntries { map, .. } => {
            alloc(vm, "Ljava/util/Map$Entry;", Native::MapEntry { map: *map, idx })
        }
        IterKind::MapKeys { .. } => match payload(vm, coll) {
            Some(Native::Map(entries)) => entries
                .get(idx)
                .map(|(k, _)| *k)
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
        IterKind::MapValues { .. } => match payload(vm, coll) {
            Some(Native::Map(entries)) => entries
                .get(idx)
                .map(|(_, v)| *v)
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
    }?;
    let next_idx = idx + 1;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Iter(k2) => match k2 {
            IterKind::List { idx: i, .. }
            | IterKind::Set { idx: i, .. }
            | IterKind::MapEntries { idx: i, .. }
            | IterKind::MapKeys { idx: i, .. }
            | IterKind::MapValues { idx: i, .. } => *i = next_idx,
        },
        _ => return Err(npe(vm)),
    }
    Ok(item)
}

pub(crate) fn iter_remove(_vm: &mut Vm, _args: &[JValue]) -> R {
    Err(uoe(_vm, "Iterator.remove"))
}


/// Native methods for Ljava/util/Iterator;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/util/Iterator;", "hasNext", "()Z", true, iter_has_next),
    ne!("Ljava/util/Iterator;", "next", "()Ljava/lang/Object;", true, iter_next),
    ne!("Ljava/util/Iterator;", "remove", "()V", true, iter_remove),
];
