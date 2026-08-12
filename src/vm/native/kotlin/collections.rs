//! Kotlin collections bridge registrations.
use crate::vm::native::kotlin::sequences::sequence_as_sequence;
use crate::vm::native::*;

pub(super) fn collections_list_of_array(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    list_alloc(vm, items)
}

pub(super) fn collections_list_of_single(vm: &mut Vm, args: &[JValue]) -> R {
    let items = if args[0].is_null_ref() {
        Vec::new()
    } else {
        vec![args[0]]
    };
    list_alloc(vm, items)
}

pub(super) fn kotlin_empty_list(vm: &mut Vm, _args: &[JValue]) -> R {
    list_alloc(vm, Vec::new())
}

pub(super) fn kotlin_list_identity(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(super) fn collections_reversed(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.reverse();
    list_alloc(vm, items)
}

pub(super) fn collections_remove_all(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let predicate = args[1];
    let mut kept = Vec::with_capacity(values.len());
    let mut removed = false;
    for value in values {
        let result = vm
            .invoke_virtual_args(
                predicate,
                "invoke",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                vec![value],
            )
            .map_err(nat_fatal)?;
        if result.as_int() != 0 {
            removed = true;
        } else {
            kept.push(value);
        }
    }
    if let Some(Native::List(items)) = payload_mut(vm, args[0]) {
        *items = kept;
    }
    Ok(JValue::Int(removed as i32))
}

pub(super) fn collections_to_list_iterable(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    list_alloc(vm, values)
}

pub(super) fn collections_sorted(vm: &mut Vm, args: &[JValue]) -> R {
    let mut values = coll_elems(vm, args[0])?;
    values.sort_by(|a, b| java_cmp(vm, *a, *b).unwrap_or(Ordering::Equal));
    list_alloc(vm, values)
}

pub(super) fn collections_as_reversed(vm: &mut Vm, args: &[JValue]) -> R {
    collections_reversed(vm, args)
}

pub(super) fn collections_take(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[1]).max(0) as usize;
    let mut values = coll_elems(vm, args[0])?;
    values.truncate(n);
    list_alloc(vm, values)
}

pub(super) fn collections_drop(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[1]).max(0) as usize;
    let values = coll_elems(vm, args[0])?;
    list_alloc(vm, values.into_iter().skip(n).collect())
}

pub(super) fn collections_plus_iterable(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.extend(coll_elems(vm, args[1])?);
    list_alloc(vm, items)
}

pub(super) fn collections_get_last_index(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    Ok(JValue::Int(values.len().saturating_sub(1) as i32))
}

pub(super) fn collections_plus_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.push(args[1]);
    list_alloc(vm, items)
}

pub(super) fn collections_contains(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    for v in items {
        if java_equals(vm, v, args[1])? {
            return Ok(JValue::Int(1));
        }
    }
    Ok(JValue::Int(0))
}

pub(super) fn collections_first(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    match items.into_iter().next() {
        Some(v) => Ok(v),
        None => Err(no_such_elem(vm)),
    }
}

pub(super) fn collections_first_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(coll_elems(vm, args[0])?
        .into_iter()
        .next()
        .unwrap_or(JValue::Null))
}

pub(super) fn collections_last(vm: &mut Vm, args: &[JValue]) -> R {
    coll_elems(vm, args[0])?
        .into_iter()
        .last()
        .ok_or_else(|| no_such_elem(vm))
}

pub(super) fn collections_get_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let index = int_of(vm, args[1]);
    if index < 0 {
        return Ok(JValue::Null);
    }
    Ok(coll_elems(vm, args[0])?
        .get(index as usize)
        .copied()
        .unwrap_or(JValue::Null))
}

pub(super) fn collections_to_mutable_list(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    list_alloc(vm, items)
}

pub(super) fn collections_flatten(vm: &mut Vm, args: &[JValue]) -> R {
    let mut out = Vec::new();
    for value in coll_elems(vm, args[0])? {
        out.extend(coll_elems(vm, value)?);
    }
    list_alloc(vm, out)
}

pub(super) fn collections_add_all(vm: &mut Vm, args: &[JValue]) -> R {
    let additions = coll_elems(vm, args[1])?;
    if additions.is_empty() {
        return Ok(JValue::Int(0));
    }
    match payload_mut(vm, args[0]) {
        Some(Native::List(items) | Native::Set(items)) => items.extend(additions),
        _ => return Err(iae(vm, "not a mutable collection")),
    }
    Ok(JValue::Int(1))
}

pub(super) fn collections_throw_index_overflow(vm: &mut Vm, _args: &[JValue]) -> R {
    Err(NatErr::Throw(vm.throwable_of(
        "Ljava/lang/ArithmeticException;",
        "Index overflow has happened.",
    )))
}

pub(super) fn collections_size_or_default(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let def = int_of(vm, args[1]);
    let n = items.len() as i32;
    Ok(JValue::Int(if n < 10 { n } else { def }))
}

pub(super) fn collections_join_to_string_default(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let mask = if args.len() > 7 {
        int_of(vm, args[7])
    } else {
        0
    };
    let has = |bit: i32| (mask >> bit) & 1 == 0;
    let separator = if has(0) {
        charseq_of(vm, args[1])?
    } else {
        ", ".to_string()
    };
    let prefix = if has(1) {
        charseq_of(vm, args[2])?
    } else {
        String::new()
    };
    let postfix = if has(2) {
        charseq_of(vm, args[3])?
    } else {
        String::new()
    };
    let limit = if has(3) { int_of(vm, args[4]) } else { -1 };
    let truncated = if has(4) {
        charseq_of(vm, args[5])?
    } else {
        "...".to_string()
    };
    let transform = if has(5) { args[6] } else { JValue::Null };
    let mut out = String::new();
    out.push_str(&prefix);
    let n = items.len();
    let cut = limit >= 0 && (n as i32) > limit;
    let shown = if cut { limit as usize } else { n };
    for (i, v) in items.iter().take(shown).enumerate() {
        if i > 0 {
            out.push_str(&separator);
        }
        let s = if transform.is_null_ref() {
            charseq_of(vm, *v)?
        } else {
            let r = inv_virt(
                vm,
                transform,
                "invoke",
                "(Ljava/lang/Object;)Ljava/lang/Object;",
                &[*v],
            )?;
            charseq_of(vm, r)?
        };
        out.push_str(&s);
    }
    if cut {
        out.push_str(&separator);
        out.push_str(&truncated);
    }
    out.push_str(&postfix);
    Ok(new_str(vm, &out))
}

pub(super) fn setskt_to_set(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let array = alloc_arr(vm, "Ljava/lang/Object;", values.len(), move || {
        ArrayData::Obj(values)
    })?;
    setskt_set_of(vm, &[array])
}

pub(super) fn setskt_plus(vm: &mut Vm, args: &[JValue]) -> R {
    let mut values = coll_elems(vm, args[0])?;
    let mut exists = false;
    for value in &values {
        if java_equals(vm, *value, args[1])? {
            exists = true;
            break;
        }
    }
    if !exists {
        values.push(args[1]);
    }
    set_alloc(vm, values)
}

pub(super) fn arrayskt_contains(vm: &mut Vm, args: &[JValue]) -> R {
    for value in coll_elems(vm, args[0])? {
        if java_equals(vm, value, args[1])? {
            return Ok(JValue::Int(1));
        }
    }
    Ok(JValue::Int(0))
}

pub(super) fn collections_filter_not_null(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?
        .into_iter()
        .filter(|value| !value.is_null_ref())
        .collect();
    list_alloc(vm, values)
}

pub(super) fn mapskt_map_capacity(vm: &mut Vm, args: &[JValue]) -> R {
    let expected = int_of(vm, args[0]);
    let capacity = if expected < 3 {
        expected + 1
    } else if expected < 1_073_741_824 {
        expected / 3 * 4 + 1
    } else {
        i32::MAX
    };
    Ok(JValue::Int(capacity))
}

pub(super) fn setskt_empty_set(vm: &mut Vm, _args: &[JValue]) -> R {
    set_alloc(vm, Vec::new())
}

pub(super) fn collections_distinct(vm: &mut Vm, args: &[JValue]) -> R {
    let mut unique = Vec::new();
    for value in coll_elems(vm, args[0])? {
        let mut found = false;
        for existing in &unique {
            if java_equals(vm, *existing, value)? {
                found = true;
                break;
            }
        }
        if !found {
            unique.push(value);
        }
    }
    list_alloc(vm, unique)
}

pub(super) fn collections_last_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(coll_elems(vm, args[0])?
        .last()
        .copied()
        .unwrap_or(JValue::Null))
}

pub(super) fn collections_sorted_with(vm: &mut Vm, args: &[JValue]) -> R {
    let mut values = coll_elems(vm, args[0])?;
    let comparator = args[1];
    for idx in 1..values.len() {
        let mut pos = idx;
        while pos > 0 {
            let result = inv_virt(
                vm,
                comparator,
                "compare",
                "(Ljava/lang/Object;Ljava/lang/Object;)I",
                &[values[pos - 1], values[pos]],
            )?;
            if int_of(vm, result) <= 0 {
                break;
            }
            values.swap(pos - 1, pos);
            pos -= 1;
        }
    }
    list_alloc(vm, values)
}

pub(super) fn setskt_set_of(vm: &mut Vm, args: &[JValue]) -> R {
    let mut unique = Vec::new();
    for value in coll_elems(vm, args[0])? {
        let mut exists = false;
        for existing in &unique {
            if java_equals(vm, *existing, value)? {
                exists = true;
                break;
            }
        }
        if !exists {
            unique.push(value);
        }
    }
    set_alloc(vm, unique)
}

pub(super) fn collections_list_of_not_null(vm: &mut Vm, args: &[JValue]) -> R {
    let values = if matches!(payload(vm, args[0]), Some(Native::Array(_))) {
        coll_elems(vm, args[0])?
    } else {
        vec![args[0]]
    };
    list_alloc(
        vm,
        values
            .into_iter()
            .filter(|value| !value.is_null())
            .collect(),
    )
}

pub(super) fn arrayskt_plus_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let mut left = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Byte(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    let right = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Byte(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    left.extend(right);
    alloc_arr(vm, "B", left.len(), move || ArrayData::Byte(left))
}

pub(super) fn mapskt_map_of(vm: &mut Vm, args: &[JValue]) -> R {
    let pairs = coll_elems(vm, args[0])?;
    let mut entries = Vec::with_capacity(pairs.len());
    for pair in pairs {
        match payload(vm, pair) {
            Some(Native::Pair(key, value)) => entries.push((*key, *value)),
            _ => return Err(iae(vm, "mapOf element is not a Pair")),
        }
    }
    alloc(vm, "Ljava/util/LinkedHashMap;", Native::Map(entries))
}

pub(super) fn mapskt_empty_map(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/LinkedHashMap;", Native::Map(Vec::new()))
}

pub(super) fn mapskt_to_list(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let mut pairs = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        pairs.push(alloc(vm, "Lkotlin/Pair;", Native::Pair(key, value))?);
    }
    list_alloc(vm, pairs)
}

pub(super) fn arrayskt_copy_of_range(vm: &mut Vm, args: &[JValue]) -> R {
    let from = int_of(vm, args[1]).max(0) as usize;
    let to = int_of(vm, args[2]).max(0) as usize;
    let data = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Byte(bs))) => bs.clone(),
        _ => return Err(npe(vm)),
    };
    let end = to.min(data.len());
    let start = from.min(end);
    let slice = data[start..end].to_vec();
    alloc_arr(vm, "B", slice.len(), move || ArrayData::Byte(slice))
}

pub(super) fn arrayskt_int_to_list(vm: &mut Vm, args: &[JValue]) -> R {
    let values = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Int(v))) => v.iter().map(|x| JValue::Int(*x)).collect(),
        _ => return Err(npe(vm)),
    };
    list_alloc(vm, values)
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Lkotlin/collections/CollectionsKt;", "listOf", "([Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_array),
    ne!("Lkotlin/collections/CollectionsKt;", "listOf", "(Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_single),
    ne!("Lkotlin/collections/CollectionsKt;", "mutableListOf", "([Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_array),
    ne!("Lkotlin/collections/CollectionsKt;", "emptyList", "()Ljava/util/List;", false, kotlin_empty_list),
    ne!("Lkotlin/collections/CollectionsKt;", "createListBuilder", "()Ljava/util/List;", false, kotlin_empty_list),
    ne!("Lkotlin/collections/CollectionsKt;", "createListBuilder", "(I)Ljava/util/List;", false, kotlin_empty_list),
    ne!("Lkotlin/collections/CollectionsKt;", "build", "(Ljava/util/List;)Ljava/util/List;", false, kotlin_list_identity),
    ne!("Lkotlin/collections/CollectionsKt;", "plus", "(Ljava/util/Collection;Ljava/lang/Iterable;)Ljava/util/List;", false, collections_plus_iterable),
    ne!("Lkotlin/collections/CollectionsKt;", "plus", "(Ljava/util/Collection;Ljava/lang/Object;)Ljava/util/List;", false, collections_plus_obj),
    ne!("Lkotlin/collections/CollectionsKt;", "contains", "(Ljava/lang/Iterable;Ljava/lang/Object;)Z", false, collections_contains),
    ne!("Lkotlin/collections/CollectionsKt;", "first", "(Ljava/lang/Iterable;)Ljava/lang/Object;", false, collections_first),
    ne!("Lkotlin/collections/CollectionsKt;", "first", "(Ljava/util/List;)Ljava/lang/Object;", false, collections_first),
    ne!("Lkotlin/collections/CollectionsKt;", "firstOrNull", "(Ljava/util/List;)Ljava/lang/Object;", false, collections_first_or_null),
    ne!("Lkotlin/collections/CollectionsKt;", "last", "(Ljava/util/List;)Ljava/lang/Object;", false, collections_last),
    ne!("Lkotlin/collections/CollectionsKt;", "getOrNull", "(Ljava/util/List;I)Ljava/lang/Object;", false, collections_get_or_null),
    ne!("Lkotlin/collections/CollectionsKt;", "toMutableList", "(Ljava/util/Collection;)Ljava/util/List;", false, collections_to_mutable_list),
    ne!("Lkotlin/collections/CollectionsKt;", "addAll", "(Ljava/util/Collection;Ljava/lang/Iterable;)Z", false, collections_add_all),
    ne!("Lkotlin/collections/CollectionsKt;", "throwIndexOverflow", "()V", false, collections_throw_index_overflow),
    ne!("Lkotlin/collections/CollectionsKt;", "listOfNotNull", "(Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_not_null),
    ne!("Lkotlin/collections/CollectionsKt;", "listOfNotNull", "([Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_not_null),
    ne!("Lkotlin/collections/CollectionsKt;", "distinct", "(Ljava/lang/Iterable;)Ljava/util/List;", false, collections_distinct),
    ne!("Lkotlin/collections/CollectionsKt;", "filterNotNull", "(Ljava/lang/Iterable;)Ljava/util/List;", false, collections_filter_not_null),
    ne!("Lkotlin/collections/CollectionsKt;", "lastOrNull", "(Ljava/util/List;)Ljava/lang/Object;", false, collections_last_or_null),
    ne!("Lkotlin/collections/CollectionsKt;", "sortedWith", "(Ljava/lang/Iterable;Ljava/util/Comparator;)Ljava/util/List;", false, collections_sorted_with),
    ne!("Lkotlin/collections/SetsKt;", "setOf", "([Ljava/lang/Object;)Ljava/util/Set;", false, setskt_set_of),
    ne!("Lkotlin/collections/SetsKt;", "emptySet", "()Ljava/util/Set;", false, setskt_empty_set),
    ne!("Lkotlin/collections/SetsKt;", "plus", "(Ljava/util/Set;Ljava/lang/Object;)Ljava/util/Set;", false, setskt_plus),
    ne!("Lkotlin/collections/CollectionsKt;", "toSet", "(Ljava/lang/Iterable;)Ljava/util/Set;", false, setskt_to_set),
    ne!("Lkotlin/collections/MapsKt;", "mapOf", "([Lkotlin/Pair;)Ljava/util/Map;", false, mapskt_map_of),
    ne!("Lkotlin/collections/MapsKt;", "emptyMap", "()Ljava/util/Map;", false, mapskt_empty_map),
    ne!("Lkotlin/collections/MapsKt;", "toList", "(Ljava/util/Map;)Ljava/util/List;", false, mapskt_to_list),
    ne!("Lkotlin/collections/MapsKt;", "mapCapacity", "(I)I", false, mapskt_map_capacity),
    ne!("Lkotlin/collections/ArraysKt;", "plus", "([B[B)[B", false, arrayskt_plus_bytes),
    ne!("Lkotlin/collections/ArraysKt;", "contains", "([Ljava/lang/Object;Ljava/lang/Object;)Z", false, arrayskt_contains),
    ne!("Lkotlin/collections/CollectionsKt;", "collectionSizeOrDefault", "(Ljava/lang/Iterable;I)I", false, collections_size_or_default),
    ne!("Lkotlin/collections/CollectionsKt;", "joinToString$default", "(Ljava/lang/Iterable;Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;ILjava/lang/CharSequence;Lkotlin/jvm/functions/Function1;ILjava/lang/Object;)Ljava/lang/String;", false, collections_join_to_string_default),
    ne!("Lkotlin/collections/CollectionsKt;", "joinTo$default", "(Ljava/lang/Iterable;Ljava/lang/Appendable;Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;ILjava/lang/CharSequence;Lkotlin/jvm/functions/Function1;ILjava/lang/Object;)Ljava/lang/Appendable;", false, collections_join_to_string_default),
    ne!("Lkotlin/collections/ArraysKt;", "copyOfRange", "([BII)[B", false, arrayskt_copy_of_range),
    ne!("Lkotlin/collections/ArraysKt;", "toList", "([I)Ljava/util/List;", false, arrayskt_int_to_list),
    ne!(
        "Lkotlin/collections/CollectionsKt;",
        "asSequence",
        "(Ljava/lang/Iterable;)Lkotlin/sequences/Sequence;",
        false,
        sequence_as_sequence
    ),
    ne!(
        "Lkotlin/collections/CollectionsKt;",
        "flatten",
        "(Ljava/lang/Iterable;)Ljava/util/List;",
        false,
        collections_flatten
    ),
    ne!(
        "Lkotlin/collections/CollectionsKt;",
        "toList",
        "(Ljava/lang/Iterable;)Ljava/util/List;",
        false,
        collections_to_list_iterable
    ),
    ne!(
        "Lkotlin/collections/CollectionsKt;",
        "sorted",
        "(Ljava/lang/Iterable;)Ljava/util/List;",
        false,
        collections_sorted
    ),
    ne!(
        "Lkotlin/collections/CollectionsKt;",
        "take",
        "(Ljava/lang/Iterable;I)Ljava/util/List;",
        false,
        collections_take
    ),
    ne!(
        "Lkotlin/collections/CollectionsKt;",
        "drop",
        "(Ljava/lang/Iterable;I)Ljava/util/List;",
        false,
        collections_drop
    ),
    ne!(
        "Lkotlin/collections/CollectionsKt;",
        "asReversed",
        "(Ljava/util/List;)Ljava/util/List;",
        false,
        collections_as_reversed
    ),
    ne!(
        "Lkotlin/collections/CollectionsKt;",
        "reversed",
        "(Ljava/lang/Iterable;)Ljava/util/List;",
        false,
        collections_reversed
    ),
    ne!(
        "Lkotlin/collections/CollectionsKt;",
        "removeAll",
        "(Ljava/util/List;Lkotlin/jvm/functions/Function1;)Z",
        false,
        collections_remove_all
    ),
    ne!(
        "Lkotlin/collections/CollectionsKt;",
        "plus",
        "(Ljava/lang/Iterable;Ljava/lang/Iterable;)Ljava/util/List;",
        false,
        collections_plus_iterable
    ),
    ne!(
        "Lkotlin/collections/CollectionsKt;",
        "getLastIndex",
        "(Ljava/util/List;)I",
        false,
        collections_get_last_index
    ),
];
