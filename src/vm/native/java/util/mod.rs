use crate::vm::native::*;

mod arraydeque;
mod arraylist;
mod arrays;
mod collections;
mod condition;
mod date;
mod hashmap;
mod hashset;
mod iterator;
mod list_iterator;
mod linkedhashmap;
mod linkedhashset;
mod locale;
mod map_entry;
mod objects;
mod random;
mod reentrant_lock;
mod regex;
mod timezone;

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
            Some(Native::Array(ArrayData::Obj(items))) => Ok(items.clone()),
            #[cfg(feature = "keiyoushi")]
            Some(Native::JsoupElements { doc, ids }) => {
                let doc = doc.clone();
                let ids = ids.clone();
                ids.into_iter()
                    .map(|id| {
                        alloc(
                            vm,
                            "Lorg/jsoup/nodes/Element;",
                            Native::JsoupElement { doc: doc.clone(), id },
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
    alloc(vm, "Ljava/util/ArrayList;", Native::List(items))
}

pub(crate) fn set_alloc(vm: &mut Vm, items: Vec<JValue>) -> Result<JValue, NatErr> {
    alloc(vm, "Ljava/util/HashSet;", Native::Set(items))
}

/// All java.util native tables, grouped for `register`.
pub(crate) const UTIL_TABLE: &[&[NativeEntry]] = &[
    arraydeque::TABLE,
    arraylist::TABLE,
    arrays::TABLE,
    collections::TABLE,
    condition::TABLE,
    date::TABLE,
    hashmap::TABLE,
    hashset::TABLE,
    iterator::TABLE,
    list_iterator::TABLE,
    linkedhashmap::TABLE,
    linkedhashset::TABLE,
    locale::TABLE,
    map_entry::TABLE,
    objects::TABLE,
    random::TABLE,
    reentrant_lock::TABLE,
    regex::matcher::TABLE,
    regex::pattern::TABLE,
    timezone::TABLE,
];
