use crate::vm::native::*;

mod arraydeque;
mod arraylist;
mod arrays;
mod atomic;
mod base64;
mod calendar;
mod collections;
mod condition;
mod count_down_latch;
mod date;
mod hashmap;
mod hashset;
mod inflater;
mod iterator;
mod linkedhashmap;
mod linkedhashset;
mod list_iterator;
mod locale;
mod map_entry;
mod objects;
mod random;
mod reentrant_lock;
mod regex;
mod resourcebundle;
mod timezone;
mod uuid;
mod zip_stream;

pub(crate) use arraylist::*;
pub(crate) use hashmap::*;
pub(crate) use hashset::*;
pub(crate) use iterator::*;
pub(crate) use locale::*;
pub(crate) use reentrant_lock::*;

// ---------------------------------------------------------------------------
// shared collection helpers
// ---------------------------------------------------------------------------

pub(crate) fn coll_elems(vm: &mut Vm, v: JValue) -> Result<Vec<JValue>, NatErr> {
    match v {
        JValue::Obj(_) => match payload(vm, v) {
            Some(Native::List(items)) => Ok(items.clone()),
            Some(Native::Set(items)) => Ok(items.clone()),
            Some(Native::SFilterList(items)) => Ok(items.clone()),
            Some(Native::Array(ArrayData::Obj(items))) => Ok(items.clone()),
            #[cfg(feature = "jsoup")]
            Some(Native::JsoupElements { doc, ids }) => {
                let doc = doc.clone();
                let ids = ids.clone();
                ids.into_iter()
                    .map(|id| {
                        alloc(
                            vm,
                            "Lorg/jsoup/nodes/Element;",
                            Native::JsoupElement {
                                doc: doc.clone(),
                                id,
                            },
                        )
                    })
                    .collect()
            }
            _ => Err(iae(vm, "not a collection")),
        },
        JValue::Null => Err(npe(vm)),
        _ => Err(iae(vm, "not a collection")),
    }
}

pub(crate) fn list_alloc(vm: &mut Vm, items: Vec<JValue>) -> Result<JValue, NatErr> {
    if items
        .iter()
        .any(|v| matches!(v, JValue::Int(i) if *i == 0))
    {
        eprintln!("DEXTRACE list_alloc WITH INT(0): {items:?}");
    if let Some(f) = vm.frames.last() {
        let cls = &vm.classes[f.class as usize];
        let name = cls
            .methods
            .get(f.slot as usize)
            .map(|m| vm.str_of(m.name))
            .unwrap_or("?");
        eprintln!(
            "DEXTRACE   caller: {}.{}",
            vm.class_desc_str(f.class),
            name
        );
    }
    eprintln!("{}", std::backtrace::Backtrace::force_capture());
    }
    alloc(vm, "Ljava/util/ArrayList;", Native::List(items))
}

pub(crate) fn set_alloc(vm: &mut Vm, items: Vec<JValue>) -> Result<JValue, NatErr> {
    alloc(vm, "Ljava/util/HashSet;", Native::Set(items))
}

// ---------------------------------------------------------------------------
// AbstractCollection / AbstractMap: dispatch by backing payload so a List-
// or Set-shaped shim class that falls through to the shared Abstract*
// superclass still resolves to the right concrete implementation.
// ---------------------------------------------------------------------------

pub(crate) fn coll_generic_add(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Set(_)) => set_add(vm, args),
        _ => list_add(vm, args),
    }
}

pub(crate) fn coll_generic_remove(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Set(_)) => set_remove(vm, args),
        _ => list_remove_obj(vm, args),
    }
}

pub(crate) fn coll_generic_clear(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Set(_)) => set_clear(vm, args),
        _ => list_clear(vm, args),
    }
}

pub(crate) fn coll_generic_contains(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Set(_)) => set_contains(vm, args),
        _ => list_contains(vm, args),
    }
}

pub(crate) fn coll_generic_add_all(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Set(_)) => set_add_all(vm, args),
        _ => list_add_all(vm, args),
    }
}

pub(crate) fn coll_generic_contains_all(vm: &mut Vm, args: &[JValue]) -> R {
    let needle = coll_elems(vm, args[1])?;
    let haystack = coll_elems(vm, args[0])?;
    for n in needle {
        let mut found = false;
        for h in &haystack {
            if java_equals(vm, *h, n)? {
                found = true;
                break;
            }
        }
        if !found {
            return Ok(JValue::Int(0));
        }
    }
    Ok(JValue::Int(1))
}

/// All java.util native tables, grouped for `register`.
pub(crate) const UTIL_TABLE: &[&[NativeEntry]] = &[
    arraydeque::TABLE,
    arraylist::TABLE,
    arraylist::COW_TABLE,
    arrays::TABLE,
    atomic::TABLE,
    base64::TABLE,
    calendar::TABLE,
    count_down_latch::TABLE,
    collections::TABLE,
    hashmap::CHM_TABLE,
    hashmap::ABSTRACT_MAP_TABLE,
    condition::TABLE,
    date::TABLE,
    hashmap::TABLE,
    hashset::TABLE,
    inflater::TABLE,
    iterator::TABLE,
    list_iterator::TABLE,
    linkedhashmap::TABLE,
    linkedhashset::TABLE,
    locale::TABLE,
    map_entry::TABLE,
    objects::TABLE,
    random::TABLE,
    resourcebundle::TABLE,
    reentrant_lock::TABLE,
    regex::matcher::TABLE,
    regex::pattern::TABLE,
    timezone::TABLE,
    uuid::TABLE,
    zip_stream::TABLE,
];
