//! java.util.Collections host shims.

use crate::vm::native::*;

// ---------------------------------------------------------------------------
// java.util.Collections (all static)
// ---------------------------------------------------------------------------

pub(crate) fn collections_empty_list(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/ArrayList;", Native::List(Vec::new()))
}

pub(crate) fn collections_empty_set(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/HashSet;", Native::Set(Vec::new()))
}

pub(crate) fn collections_empty_map(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/HashMap;", Native::Map(Vec::new()))
}

pub(crate) fn collections_singleton(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/HashSet;", Native::Set(vec![args[0]]))
}

pub(crate) fn collections_singleton_list(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/ArrayList;", Native::List(vec![args[0]]))
}

pub(crate) fn collections_singleton_map(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(
        vm,
        "Ljava/util/HashMap;",
        Native::Map(vec![(args[0], args[1])]),
    )
}

pub(crate) fn collections_identity(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn collections_sort(vm: &mut Vm, args: &[JValue]) -> R {
    let list = args[0].as_obj();
    let items = match payload(vm, JValue::Obj(list)) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut items = items;
    let mut err: Option<NatErr> = None;
    items.sort_by(|a, b| match java_cmp(vm, *a, *b) {
        Ok(o) => o,
        Err(e) => {
            err = Some(e);
            Ordering::Equal
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    let Some(n) = payload_mut(vm, JValue::Obj(list)) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => *dst = items,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn collections_reverse(vm: &mut Vm, args: &[JValue]) -> R {
    let list = args[0].as_obj();
    let items = match payload(vm, JValue::Obj(list)) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut items = items;
    items.reverse();
    let Some(n) = payload_mut(vm, JValue::Obj(list)) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => *dst = items,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn collections_add_all(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Obj(items))) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => dst.extend_from_slice(&items),
        Native::Set(dst) => dst.extend_from_slice(&items),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Int(i32::from(!items.is_empty())))
}

/// Native methods for Ljava/util/Collections;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/Collections;",
        "emptyList",
        "()Ljava/util/List;",
        false,
        collections_empty_list
    ),
    ne!(
        "Ljava/util/Collections;",
        "emptySet",
        "()Ljava/util/Set;",
        false,
        collections_empty_set
    ),
    ne!(
        "Ljava/util/Collections;",
        "emptyMap",
        "()Ljava/util/Map;",
        false,
        collections_empty_map
    ),
    ne!(
        "Ljava/util/Collections;",
        "singleton",
        "(Ljava/lang/Object;)Ljava/util/Set;",
        false,
        collections_singleton
    ),
    ne!(
        "Ljava/util/Collections;",
        "singletonList",
        "(Ljava/lang/Object;)Ljava/util/List;",
        false,
        collections_singleton_list
    ),
    ne!(
        "Ljava/util/Collections;",
        "singletonMap",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/util/Map;",
        false,
        collections_singleton_map
    ),
    ne!(
        "Ljava/util/Collections;",
        "unmodifiableCollection",
        "(Ljava/util/Collection;)Ljava/util/Collection;",
        false,
        collections_identity
    ),
    ne!(
        "Ljava/util/Collections;",
        "unmodifiableList",
        "(Ljava/util/List;)Ljava/util/List;",
        false,
        collections_identity
    ),
    ne!(
        "Ljava/util/Collections;",
        "unmodifiableSet",
        "(Ljava/util/Set;)Ljava/util/Set;",
        false,
        collections_identity
    ),
    ne!(
        "Ljava/util/Collections;",
        "unmodifiableMap",
        "(Ljava/util/Map;)Ljava/util/Map;",
        false,
        collections_identity
    ),
    ne!(
        "Ljava/util/Collections;",
        "synchronizedList",
        "(Ljava/util/List;)Ljava/util/List;",
        false,
        collections_identity
    ),
    ne!(
        "Ljava/util/Collections;",
        "synchronizedSet",
        "(Ljava/util/Set;)Ljava/util/Set;",
        false,
        collections_identity
    ),
    ne!(
        "Ljava/util/Collections;",
        "synchronizedMap",
        "(Ljava/util/Map;)Ljava/util/Map;",
        false,
        collections_identity
    ),
    ne!(
        "Ljava/util/Collections;",
        "sort",
        "(Ljava/util/List;)V",
        false,
        collections_sort
    ),
    ne!(
        "Ljava/util/Collections;",
        "sort",
        "(Ljava/util/List;Ljava/util/Comparator;)V",
        false,
        list_sort_cmp
    ),
    ne!(
        "Ljava/util/Collections;",
        "reverse",
        "(Ljava/util/List;)V",
        false,
        collections_reverse
    ),
    ne!(
        "Ljava/util/Collections;",
        "addAll",
        "(Ljava/util/Collection;[Ljava/lang/Object;)Z",
        false,
        collections_add_all
    ),
];
