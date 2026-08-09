//! java.util.Iterator host shims.

use crate::vm::native::*;

pub(crate) fn iter_has_next(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Iter(k)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let idx = match &k {
        IterKind::List { idx, .. }
        | IterKind::Set { idx, .. }
        | IterKind::MapEntries { idx, .. }
        | IterKind::MapKeys { idx, .. }
        | IterKind::MapValues { idx, .. } => *idx,
        #[cfg(feature = "jsoup")]
        IterKind::Jsoup { idx, .. } => *idx,
    };
    let len = match &k {
        IterKind::List { list, .. } => match payload(vm, JValue::Obj(*list)) {
            Some(Native::List(items)) => items.len(),
            #[cfg(feature = "jsoup")]
            Some(Native::SFilterList(items)) => items.len(),
            _ => return Err(npe(vm)),
        },
        IterKind::Set { set, .. } => match payload(vm, JValue::Obj(*set)) {
            Some(Native::Set(items)) => items.len(),
            _ => return Err(npe(vm)),
        },
        IterKind::MapEntries { map, .. } | IterKind::MapKeys { map, .. } | IterKind::MapValues { map, .. } => {
            match payload(vm, JValue::Obj(*map)) {
                Some(Native::Map(entries)) => entries.len(),
                _ => return Err(npe(vm)),
            }
        }
        #[cfg(feature = "jsoup")]
        IterKind::Jsoup { ids, .. } => ids.len(),
    };
    Ok(JValue::Int(i32::from(idx < len)))
}

pub(crate) fn iter_next(vm: &mut Vm, args: &[JValue]) -> R {
    let k = match payload(vm, args[0]) {
        Some(Native::Iter(k)) => k.clone(),
        _ => return Err(npe(vm)),
    };
    let item = match &k {
        IterKind::List { list, idx } => match payload(vm, JValue::Obj(*list)) {
            Some(Native::List(items)) => items
                .get(*idx)
                .copied()
                .ok_or_else(|| no_such_elem(vm)),
            #[cfg(feature = "jsoup")]
            Some(Native::SFilterList(items)) => items
                .get(*idx)
                .copied()
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
        IterKind::Set { set, idx } => match payload(vm, JValue::Obj(*set)) {
            Some(Native::Set(items)) => items
                .get(*idx)
                .copied()
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
        IterKind::MapEntries { map, idx } => {
            alloc(vm, "Ljava/util/Map$Entry;", Native::MapEntry { map: *map, idx: *idx })
        }
        IterKind::MapKeys { map, idx } => match payload(vm, JValue::Obj(*map)) {
            Some(Native::Map(entries)) => entries
                .get(*idx)
                .map(|(k, _)| *k)
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
        IterKind::MapValues { map, idx } => match payload(vm, JValue::Obj(*map)) {
            Some(Native::Map(entries)) => entries
                .get(*idx)
                .map(|(_, v)| *v)
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
        #[cfg(feature = "jsoup")]
        IterKind::Jsoup { doc, ids, idx } => match ids.get(*idx) {
            Some(id) => alloc(vm, "Lorg/jsoup/nodes/Element;", Native::JsoupElement { doc: doc.clone(), id: *id }),
            None => Err(no_such_elem(vm)),
        },
    }?;
    let next_idx = match &k {
        IterKind::List { idx, .. }
        | IterKind::Set { idx, .. }
        | IterKind::MapEntries { idx, .. }
        | IterKind::MapKeys { idx, .. }
        | IterKind::MapValues { idx, .. } => *idx,
        #[cfg(feature = "jsoup")]
        IterKind::Jsoup { idx, .. } => *idx,
    } + 1;
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
            #[cfg(feature = "jsoup")]
            IterKind::Jsoup { idx: i, .. } => *i = next_idx,
        },
        _ => return Err(npe(vm)),
    }
    Ok(item)
}

pub(crate) fn iter_remove(_vm: &mut Vm, _args: &[JValue]) -> R {
    Err(uoe(_vm, "Iterator.remove"))
}

#[cfg(feature = "jsoup")]
pub(crate) fn abstract_collection_iterator(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::JsoupElements { doc, ids }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    alloc(
        vm,
        "Ljava/util/Iterator;",
        Native::Iter(IterKind::Jsoup { doc: doc.clone(), ids: ids.clone(), idx: 0 }),
    )
}


/// Native methods for Ljava/util/Iterator;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/util/Iterator;", "hasNext", "()Z", true, iter_has_next),
    ne!("Ljava/util/Iterator;", "next", "()Ljava/lang/Object;", true, iter_next),
    ne!("Ljava/util/Iterator;", "remove", "()V", true, iter_remove),
    #[cfg(feature = "jsoup")]
    ne!("Ljava/util/AbstractCollection;", "iterator", "()Ljava/util/Iterator;", true, abstract_collection_iterator),
];
