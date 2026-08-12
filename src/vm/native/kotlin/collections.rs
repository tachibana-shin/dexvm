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

// kotlin.collections audit-gap bridges
// ---------------------------------------------------------------------------

/// Shared random-index picker: `nextInt(n)` through the Random receiver.
pub(super) fn kotlin_random_index(vm: &mut Vm, random: JValue, n: i32) -> Result<i32, NatErr> {
    if n <= 0 {
        return Err(iae(vm, "Random range is empty."));
    }
    let r = inv_virt(vm, random, "nextInt", "(I)I", &[JValue::Int(n)])?;
    Ok(int_of(vm, r).max(0).min(n - 1))
}

pub(super) fn collections_random(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let index = kotlin_random_index(vm, args[1], items.len() as i32)?;
    Ok(items[index as usize])
}

pub(super) fn collections_random_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    if items.is_empty() {
        return Ok(JValue::Null);
    }
    let index = kotlin_random_index(vm, args[1], items.len() as i32)?;
    Ok(items[index as usize])
}

/// `CollectionsKt.sortWith(List, Comparator)` — in-place insertion sort.
pub(super) fn collections_sort_with_mut(vm: &mut Vm, args: &[JValue]) -> R {
    let mut values = match payload(vm, args[0]) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(iae(vm, "not a mutable list")),
    };
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
    let Some(Native::List(dst)) = payload_mut(vm, args[0]) else {
        return Err(iae(vm, "not a mutable list"));
    };
    *dst = values;
    Ok(JValue::Null)
}

pub(super) fn collections_drop_last(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[1]).max(0) as usize;
    let values = coll_elems(vm, args[0])?;
    let keep = values.len().saturating_sub(n);
    list_alloc(vm, values.into_iter().take(keep).collect())
}

/// `CollectionsKt.asReversedMutable(List)` — a reversed copy. (The real
/// method returns a live view; the VM materializes a copy instead.)
pub(super) fn collections_as_reversed_mutable(vm: &mut Vm, args: &[JValue]) -> R {
    collections_reversed(vm, args)
}

pub(super) fn collections_max_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let mut best: Option<JValue> = None;
    for value in coll_elems(vm, args[0])? {
        best = Some(match best {
            None => value,
            Some(b) => match java_cmp(vm, b, value)? {
                Ordering::Less => value,
                _ => b,
            },
        });
    }
    Ok(best.unwrap_or(JValue::Null))
}

pub(super) fn collections_min_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let mut best: Option<JValue> = None;
    for value in coll_elems(vm, args[0])? {
        best = Some(match best {
            None => value,
            Some(b) => match java_cmp(vm, b, value)? {
                Ordering::Greater => value,
                _ => b,
            },
        });
    }
    Ok(best.unwrap_or(JValue::Null))
}

pub(super) fn collections_min_or_throw(vm: &mut Vm, args: &[JValue]) -> R {
    let mut best: Option<JValue> = None;
    for value in coll_elems(vm, args[0])? {
        best = Some(match best {
            None => value,
            Some(b) => match java_cmp(vm, b, value)? {
                Ordering::Greater => value,
                _ => b,
            },
        });
    }
    best.ok_or_else(|| no_such_elem(vm))
}

pub(super) fn collections_single(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    match items.len() {
        1 => Ok(items.pop().unwrap()),
        0 => Err(no_such_elem(vm)),
        _ => Err(iae(vm, "Collection has more than one element.")),
    }
}

pub(super) fn collections_single_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    if items.len() == 1 {
        Ok(items.pop().unwrap())
    } else {
        Ok(JValue::Null)
    }
}

pub(super) fn collections_to_char_array(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let chars = items
        .iter()
        .map(|v| int_of(vm, *v) as u16)
        .collect::<Vec<u16>>();
    alloc_arr(vm, "C", chars.len(), move || ArrayData::Char(chars))
}

pub(super) fn collections_to_int_array(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let ints = items.iter().map(|v| int_of(vm, *v)).collect::<Vec<i32>>();
    alloc_arr(vm, "I", ints.len(), move || ArrayData::Int(ints))
}

pub(super) fn collections_to_byte_array(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let bytes = items
        .iter()
        .map(|v| int_of(vm, *v) as i8)
        .collect::<Vec<i8>>();
    alloc_arr(vm, "B", bytes.len(), move || ArrayData::Byte(bytes))
}

/// `CollectionsKt.reverse(List)` — in-place.
pub(super) fn collections_reverse(vm: &mut Vm, args: &[JValue]) -> R {
    let mut values = match payload(vm, args[0]) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(iae(vm, "not a mutable list")),
    };
    values.reverse();
    let Some(Native::List(dst)) = payload_mut(vm, args[0]) else {
        return Err(iae(vm, "not a mutable list"));
    };
    *dst = values;
    Ok(JValue::Null)
}

pub(super) fn collections_chunked(vm: &mut Vm, args: &[JValue]) -> R {
    let size = int_of(vm, args[1]);
    if size <= 0 {
        return Err(iae(vm, format!("Size must be greater than zero: {size}")));
    }
    let items = coll_elems(vm, args[0])?;
    let mut out = Vec::new();
    for chunk in items.chunks(size as usize) {
        out.push(list_alloc(vm, chunk.to_vec())?);
    }
    list_alloc(vm, out)
}

pub(super) fn collections_index_of(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    for (i, value) in items.iter().enumerate() {
        if java_equals(vm, *value, args[1])? {
            return Ok(JValue::Int(i as i32));
        }
    }
    Ok(JValue::Int(-1))
}

/// `CollectionsKt.retainAll(List, Function1)` — keep matching elements.
pub(super) fn collections_retain_all(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let predicate = args[1];
    let mut kept = Vec::new();
    let mut changed = false;
    for value in values {
        let result = inv_virt(
            vm,
            predicate,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[value],
        )?;
        if int_of(vm, result) != 0 {
            kept.push(value);
        } else {
            changed = true;
        }
    }
    let Some(Native::List(dst)) = payload_mut(vm, args[0]) else {
        return Err(iae(vm, "not a mutable list"));
    };
    *dst = kept;
    Ok(JValue::Int(i32::from(changed)))
}

pub(super) fn collections_get_indices(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let last = if items.is_empty() {
        -1
    } else {
        items.len() as i32 - 1
    };
    alloc(vm, "Lkotlin/ranges/IntRange;", Native::IntRange(0, last))
}

pub(super) fn collections_intersect(vm: &mut Vm, args: &[JValue]) -> R {
    let first = coll_elems(vm, args[0])?;
    let second = coll_elems(vm, args[1])?;
    let mut out = Vec::new();
    for value in first {
        let mut in_second = false;
        for other in &second {
            if java_equals(vm, value, *other)? {
                in_second = true;
                break;
            }
        }
        if !in_second {
            continue;
        }
        let mut dup = false;
        for existing in &out {
            if java_equals(vm, value, *existing)? {
                dup = true;
                break;
            }
        }
        if !dup {
            out.push(value);
        }
    }
    set_alloc(vm, out)
}

pub(super) fn collections_union(vm: &mut Vm, args: &[JValue]) -> R {
    let mut out = Vec::new();
    for source in [args[0], args[1]] {
        for value in coll_elems(vm, source)? {
            let mut dup = false;
            for existing in &out {
                if java_equals(vm, value, *existing)? {
                    dup = true;
                    break;
                }
            }
            if !dup {
                out.push(value);
            }
        }
    }
    set_alloc(vm, out)
}

pub(super) fn collections_shuffled(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos()) as u64;
    let mut state = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    for i in (1..items.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let j = (state % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
    list_alloc(vm, items)
}

pub(super) fn collections_take_last(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[1]).max(0) as usize;
    let items = coll_elems(vm, args[0])?;
    let skip = items.len().saturating_sub(n);
    list_alloc(vm, items.into_iter().skip(skip).collect())
}

pub(super) fn collections_array_list_of(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    list_alloc(vm, items)
}

pub(super) fn collections_throw_count_overflow(vm: &mut Vm, _args: &[JValue]) -> R {
    Err(NatErr::Throw(vm.throwable_of(
        "Ljava/lang/ArithmeticException;",
        "Count overflow has happened.",
    )))
}

pub(super) fn collections_zip(vm: &mut Vm, args: &[JValue]) -> R {
    let a = coll_elems(vm, args[0])?;
    let b = coll_elems(vm, args[1])?;
    let mut out = Vec::new();
    for (x, y) in a.into_iter().zip(b) {
        out.push(alloc(vm, "Lkotlin/Pair;", Native::Pair(x, y))?);
    }
    list_alloc(vm, out)
}

pub(super) fn collections_with_index(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let mut out = Vec::new();
    for (index, value) in items.into_iter().enumerate() {
        out.push(alloc(
            vm,
            "Lkotlin/collections/IndexedValue;",
            Native::Pair(JValue::Int(index as i32), value),
        )?);
    }
    list_alloc(vm, out)
}

pub(super) fn collections_sorted_descending(vm: &mut Vm, args: &[JValue]) -> R {
    let mut values = coll_elems(vm, args[0])?;
    values.sort_by(|a, b| java_cmp(vm, *b, *a).unwrap_or(Ordering::Equal));
    list_alloc(vm, values)
}

pub(super) fn collections_binary_search_default(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let key = args[1];
    let mask = int_of(vm, args[4]);
    let from = if mask & 2 != 0 {
        0
    } else {
        int_of(vm, args[2]).max(0) as usize
    };
    let to = if mask & 4 != 0 {
        items.len()
    } else {
        int_of(vm, args[3]).max(0) as usize
    };
    let mut lo = from;
    let mut hi = to;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        match java_cmp(vm, items[mid], key)? {
            Ordering::Less => lo = mid + 1,
            Ordering::Greater => hi = mid,
            Ordering::Equal => {
                let mut first = mid;
                while first > from && java_cmp(vm, items[first - 1], key)? == Ordering::Equal {
                    first -= 1;
                }
                return Ok(JValue::Int(first as i32));
            }
        }
    }
    Ok(JValue::Int(-(lo as i32) - 1))
}

pub(super) fn collections_any(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(!coll_elems(vm, args[0])?.is_empty())))
}

pub(super) fn collections_windowed_default(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let size = int_of(vm, args[1]).max(1) as usize;
    let step = int_of(vm, args[2]).max(1) as usize;
    let partial = int_of(vm, args[5]) & 8 != 0 || int_of(vm, args[3]) != 0;
    let mut out = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let end = i + size;
        if end <= items.len() || partial {
            out.push(list_alloc(vm, items[i..end.min(items.len())].to_vec())?);
        }
        i += step;
    }
    list_alloc(vm, out)
}

/// `ArraysKt.joinToString$default(byte[], ...)` — the generic handler cannot
/// stringify raw bytes, so pre-convert them to decimal strings first.
pub(super) fn arrayskt_join_bytes_default(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Byte(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    let items: Vec<JValue> = bytes.iter().map(|b| new_str(vm, &b.to_string())).collect();
    let list = list_alloc(vm, items)?;
    collections_join_to_string_default(
        vm,
        &[
            list, args[1], args[2], args[3], args[4], args[5], args[6], args[7], args[8],
        ],
    )
}

pub(super) fn arrayskt_plus_arrays(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.extend(coll_elems(vm, args[1])?);
    alloc_arr(vm, "Ljava/lang/Object;", items.len(), move || {
        ArrayData::Obj(items)
    })
}

pub(super) fn arrayskt_plus_elem(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.push(args[1]);
    alloc_arr(vm, "Ljava/lang/Object;", items.len(), move || {
        ArrayData::Obj(items)
    })
}

pub(super) fn arrayskt_plus_collection(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.extend(coll_elems(vm, args[1])?);
    alloc_arr(vm, "Ljava/lang/Object;", items.len(), move || {
        ArrayData::Obj(items)
    })
}

pub(super) fn arrayskt_plus_byte_elem(vm: &mut Vm, args: &[JValue]) -> R {
    let mut left = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Byte(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    left.push(int_of(vm, args[1]) as i8);
    alloc_arr(vm, "B", left.len(), move || ArrayData::Byte(left))
}

pub(super) fn arrayskt_sum_int(vm: &mut Vm, args: &[JValue]) -> R {
    let sum = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Int(values))) => values.iter().sum::<i32>(),
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(sum))
}

pub(super) fn arrayskt_first_int(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Int(values))) => match values.first() {
            Some(v) => Ok(JValue::Int(*v)),
            None => Err(no_such_elem(vm)),
        },
        _ => Err(npe(vm)),
    }
}

pub(super) fn arrayskt_last_byte(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Byte(values))) => match values.last() {
            Some(v) => Ok(JValue::Int(*v as i32)),
            None => Err(no_such_elem(vm)),
        },
        _ => Err(npe(vm)),
    }
}

pub(super) fn arrayskt_slice_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let (first, last) = match payload(vm, args[1]) {
        Some(Native::IntRange(f, l)) => (*f as usize, *l as usize),
        _ => return Err(npe(vm)),
    };
    let end = (last + 1).min(items.len());
    let start = first.min(end);
    list_alloc(vm, items[start..end].to_vec())
}

pub(super) fn arrayskt_slice_array_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let data = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Byte(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    let (first, last) = match payload(vm, args[1]) {
        Some(Native::IntRange(f, l)) => (*f as usize, *l as usize),
        _ => return Err(npe(vm)),
    };
    let end = (last + 1).min(data.len());
    let start = first.min(end);
    let slice = data[start..end].to_vec();
    alloc_arr(vm, "B", slice.len(), move || ArrayData::Byte(slice))
}

/// `ArraysKt.copyInto(byte[], byte[], int, int, int)`.
pub(super) fn arrayskt_copy_into_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let src = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Byte(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    let dst_offset = int_of(vm, args[2]).max(0) as usize;
    let start = int_of(vm, args[3]).max(0) as usize;
    let end = int_of(vm, args[4]).max(0) as usize;
    let Some(Native::Array(ArrayData::Byte(dst))) = payload_mut(vm, args[1]) else {
        return Err(npe(vm));
    };
    let n = end.min(src.len()).saturating_sub(start.min(src.len()));
    let cap = dst.len().saturating_sub(dst_offset).min(n);
    for i in 0..cap {
        dst[dst_offset + i] = src[start + i];
    }
    Ok(args[1])
}

pub(super) fn arrayskt_copy_into_bytes_default(vm: &mut Vm, args: &[JValue]) -> R {
    let src = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Byte(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    let mask = int_of(vm, args[5]);
    let end = if mask & 8 != 0 {
        src.len() as i32
    } else {
        int_of(vm, args[4])
    };
    arrayskt_copy_into_bytes(vm, &[args[0], args[1], args[2], args[3], JValue::Int(end)])
}

/// `ArraysKt.copyInto(long[], long[], int, int, int)` and its `$default`.
pub(super) fn arrayskt_copy_into_longs(vm: &mut Vm, args: &[JValue]) -> R {
    let src = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Long(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    let dst_offset = int_of(vm, args[2]).max(0) as usize;
    let start = int_of(vm, args[3]).max(0) as usize;
    let end = int_of(vm, args[4]).max(0) as usize;
    let Some(Native::Array(ArrayData::Long(dst))) = payload_mut(vm, args[1]) else {
        return Err(npe(vm));
    };
    let n = end.min(src.len()).saturating_sub(start.min(src.len()));
    let cap = dst.len().saturating_sub(dst_offset).min(n);
    for i in 0..cap {
        dst[dst_offset + i] = src[start + i];
    }
    Ok(args[1])
}

pub(super) fn arrayskt_copy_into_longs_default(vm: &mut Vm, args: &[JValue]) -> R {
    let src = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Long(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    let mask = int_of(vm, args[5]);
    let end = if mask & 8 != 0 {
        src.len() as i32
    } else {
        int_of(vm, args[4])
    };
    arrayskt_copy_into_longs(vm, &[args[0], args[1], args[2], args[3], JValue::Int(end)])
}

/// `ArraysKt.fill$default(byte[], byte, int, int, int, Object)`.
pub(super) fn arrayskt_fill_bytes_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = int_of(vm, args[1]) as i8;
    let mask = int_of(vm, args[4]);
    let from = if mask & 2 != 0 {
        0
    } else {
        int_of(vm, args[2]).max(0) as usize
    };
    let to = if mask & 4 != 0 {
        usize::MAX
    } else {
        int_of(vm, args[3]).max(0) as usize
    };
    let Some(Native::Array(ArrayData::Byte(dst))) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let end = to.min(dst.len());
    for i in from.min(end)..end {
        dst[i] = value;
    }
    Ok(JValue::Null)
}

pub(super) fn arrayskt_fill_longs_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = long_of(vm, args[1]);
    let mask = int_of(vm, args[4]);
    let from = if mask & 2 != 0 {
        0
    } else {
        int_of(vm, args[2]).max(0) as usize
    };
    let to = if mask & 4 != 0 {
        usize::MAX
    } else {
        int_of(vm, args[3]).max(0) as usize
    };
    let Some(Native::Array(ArrayData::Long(dst))) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let end = to.min(dst.len());
    for i in from.min(end)..end {
        dst[i] = value;
    }
    Ok(JValue::Null)
}

/// `ArraysKt.toList(byte[])` — boxed Byte elements.
pub(super) fn arrayskt_bytes_to_list(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Byte(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    let mut items = Vec::with_capacity(bytes.len());
    for b in bytes {
        items.push(boxed(vm, "Ljava/lang/Byte;", Native::ByteBox(b))?);
    }
    list_alloc(vm, items)
}

pub(super) fn arrayskt_copy_of_range_ints(vm: &mut Vm, args: &[JValue]) -> R {
    let from = int_of(vm, args[1]).max(0) as usize;
    let to = int_of(vm, args[2]).max(0) as usize;
    let data = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Int(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    let end = to.min(data.len());
    let start = from.min(end);
    let slice = data[start..end].to_vec();
    alloc_arr(vm, "I", slice.len(), move || ArrayData::Int(slice))
}

// kotlin.collections.SetsKt audit-gap bridges
// ---------------------------------------------------------------------------

pub(super) fn setskt_set_of_single(vm: &mut Vm, args: &[JValue]) -> R {
    if args[0].is_null_ref() {
        set_alloc(vm, Vec::new())
    } else {
        set_alloc(vm, vec![args[0]])
    }
}

pub(super) fn setskt_plus_iterable(vm: &mut Vm, args: &[JValue]) -> R {
    let mut values = coll_elems(vm, args[0])?;
    for value in coll_elems(vm, args[1])? {
        let mut exists = false;
        for existing in &values {
            if java_equals(vm, *existing, value)? {
                exists = true;
                break;
            }
        }
        if !exists {
            values.push(value);
        }
    }
    set_alloc(vm, values)
}

pub(super) fn setskt_minus_iterable(vm: &mut Vm, args: &[JValue]) -> R {
    let remove = coll_elems(vm, args[1])?;
    let mut kept = Vec::new();
    for value in coll_elems(vm, args[0])? {
        let mut drop = false;
        for other in &remove {
            if java_equals(vm, value, *other)? {
                drop = true;
                break;
            }
        }
        if !drop {
            kept.push(value);
        }
    }
    set_alloc(vm, kept)
}

pub(super) fn setskt_minus_elem(vm: &mut Vm, args: &[JValue]) -> R {
    let mut kept = Vec::new();
    for value in coll_elems(vm, args[0])? {
        if !java_equals(vm, value, args[1])? {
            kept.push(value);
        }
    }
    set_alloc(vm, kept)
}

pub(super) fn setskt_build(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(super) fn setskt_create_builder(vm: &mut Vm, _args: &[JValue]) -> R {
    set_alloc(vm, Vec::new())
}

// kotlin.collections.MapsKt audit-gap bridges
// ---------------------------------------------------------------------------

pub(super) fn mapskt_get_value(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    for (key, value) in entries {
        if java_equals(vm, key, args[1])? {
            return Ok(value);
        }
    }
    Err(NatErr::Throw(vm.throwable_of(
        "Ljava/util/NoSuchElementException;",
        "Key is missing in the map.",
    )))
}

/// `MapsKt.plus(Map, Map)` — merged map.
pub(super) fn mapskt_plus_maps(vm: &mut Vm, args: &[JValue]) -> R {
    let mut entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let extra = match payload(vm, args[1]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    for (key, value) in extra {
        let mut replaced = false;
        for (k, v) in entries.iter_mut() {
            if java_equals(vm, *k, key)? {
                *v = value;
                replaced = true;
                break;
            }
        }
        if !replaced {
            entries.push((key, value));
        }
    }
    alloc(vm, "Ljava/util/LinkedHashMap;", Native::Map(entries))
}

pub(super) fn mapskt_plus_pair(vm: &mut Vm, args: &[JValue]) -> R {
    let mut entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let (key, value) = match payload(vm, args[1]) {
        Some(Native::Pair(key, value)) => (*key, *value),
        _ => return Err(iae(vm, "plus element is not a Pair")),
    };
    let mut replaced = false;
    for (k, v) in entries.iter_mut() {
        if java_equals(vm, *k, key)? {
            *v = value;
            replaced = true;
            break;
        }
    }
    if !replaced {
        entries.push((key, value));
    }
    alloc(vm, "Ljava/util/LinkedHashMap;", Native::Map(entries))
}

pub(super) fn mapskt_to_mutable_map(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    alloc(vm, "Ljava/util/LinkedHashMap;", Native::Map(entries))
}

pub(super) fn mapskt_build(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(super) fn mapskt_create_builder(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/LinkedHashMap;", Native::Map(Vec::new()))
}

pub(super) fn mapskt_map_of_single(vm: &mut Vm, args: &[JValue]) -> R {
    let (key, value) = match payload(vm, args[0]) {
        Some(Native::Pair(key, value)) => (*key, *value),
        _ => return Err(iae(vm, "mapOf element is not a Pair")),
    };
    alloc(
        vm,
        "Ljava/util/LinkedHashMap;",
        Native::Map(vec![(key, value)]),
    )
}

pub(super) fn mapskt_sorted_map_of(vm: &mut Vm, args: &[JValue]) -> R {
    let pairs = coll_elems(vm, args[0])?;
    let mut entries = Vec::with_capacity(pairs.len());
    for pair in pairs {
        match payload(vm, pair) {
            Some(Native::Pair(key, value)) => entries.push((*key, *value)),
            _ => return Err(iae(vm, "mapOf element is not a Pair")),
        }
    }
    entries.sort_by(|(a, _), (b, _)| java_cmp(vm, *a, *b).unwrap_or(Ordering::Equal));
    alloc(vm, "Ljava/util/TreeMap;", Native::Map(entries))
}

// kotlin.collections.ArrayDeque audit-gap bridges
// ---------------------------------------------------------------------------

pub(super) fn arraydeque_init(vm: &mut Vm, args: &[JValue]) -> R {
    let items = if args.len() > 1 && !args[1].is_null() {
        coll_elems(vm, args[1]).unwrap_or_default()
    } else {
        Vec::new()
    };
    let JValue::Obj(id) = args[0] else {
        return Err(npe(vm));
    };
    vm.arena.objects[id as usize].native = Some(Native::ArrayDeque(items));
    Ok(JValue::Null)
}
pub(super) fn arraydeque_first_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(items.first().copied().unwrap_or(JValue::Null))
}
pub(super) fn arraydeque_set(vm: &mut Vm, args: &[JValue]) -> R {
    let i = int_of(vm, args[1]);
    let value = args[2];
    let Some(Native::ArrayDeque(items)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match items.get_mut(i as usize) {
        Some(slot) => {
            let old = *slot;
            *slot = value;
            Ok(old)
        }
        None => Err(ioobe(vm, i)),
    }
}
pub(super) fn arraydeque_is_empty(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(items.is_empty())))
}

pub(super) fn arraydeque_add_last(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(items)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    items.push(args[1]);
    Ok(JValue::Null)
}

pub(super) fn arraydeque_add(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(items)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    items.push(args[1]);
    Ok(JValue::Int(1))
}

pub(super) fn arraydeque_add_first(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(items)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    items.insert(0, args[1]);
    Ok(JValue::Null)
}

pub(super) fn arraydeque_remove_first(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(items)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if items.is_empty() {
        return Err(no_such_elem(vm));
    }
    Ok(items.remove(0))
}

pub(super) fn arraydeque_remove_last(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(items)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match items.pop() {
        Some(v) => Ok(v),
        None => Err(no_such_elem(vm)),
    }
}

pub(super) fn arraydeque_clear(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(items)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    items.clear();
    Ok(JValue::Null)
}

pub(super) fn arraydeque_last_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(items.last().copied().unwrap_or(JValue::Null))
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
    ne!("Lkotlin/collections/CollectionsKt;", "random", "(Ljava/util/Collection;Lkotlin/random/Random;)Ljava/lang/Object;", false, collections_random),
    ne!("Lkotlin/collections/CollectionsKt;", "randomOrNull", "(Ljava/util/Collection;Lkotlin/random/Random;)Ljava/lang/Object;", false, collections_random_or_null),
    ne!("Lkotlin/collections/CollectionsKt;", "sortWith", "(Ljava/util/List;Ljava/util/Comparator;)V", false, collections_sort_with_mut),
    ne!("Lkotlin/collections/CollectionsKt;", "dropLast", "(Ljava/util/List;I)Ljava/util/List;", false, collections_drop_last),
    ne!("Lkotlin/collections/CollectionsKt;", "asReversedMutable", "(Ljava/util/List;)Ljava/util/List;", false, collections_as_reversed_mutable),
    ne!("Lkotlin/collections/CollectionsKt;", "maxOrNull", "(Ljava/lang/Iterable;)Ljava/lang/Comparable;", false, collections_max_or_null),
    ne!("Lkotlin/collections/CollectionsKt;", "minOrNull", "(Ljava/lang/Iterable;)Ljava/lang/Comparable;", false, collections_min_or_null),
    ne!("Lkotlin/collections/CollectionsKt;", "minOrThrow", "(Ljava/lang/Iterable;)Ljava/lang/Comparable;", false, collections_min_or_throw),
    ne!("Lkotlin/collections/CollectionsKt;", "single", "(Ljava/util/List;)Ljava/lang/Object;", false, collections_single),
    ne!("Lkotlin/collections/CollectionsKt;", "single", "(Ljava/lang/Iterable;)Ljava/lang/Object;", false, collections_single),
    ne!("Lkotlin/collections/CollectionsKt;", "singleOrNull", "(Ljava/util/List;)Ljava/lang/Object;", false, collections_single_or_null),
    ne!("Lkotlin/collections/CollectionsKt;", "toCharArray", "(Ljava/util/Collection;)[C", false, collections_to_char_array),
    ne!("Lkotlin/collections/CollectionsKt;", "toIntArray", "(Ljava/util/Collection;)[I", false, collections_to_int_array),
    ne!("Lkotlin/collections/CollectionsKt;", "toByteArray", "(Ljava/util/Collection;)[B", false, collections_to_byte_array),
    ne!("Lkotlin/collections/CollectionsKt;", "reverse", "(Ljava/util/List;)V", false, collections_reverse),
    ne!("Lkotlin/collections/CollectionsKt;", "chunked", "(Ljava/lang/Iterable;I)Ljava/util/List;", false, collections_chunked),
    ne!("Lkotlin/collections/CollectionsKt;", "indexOf", "(Ljava/util/List;Ljava/lang/Object;)I", false, collections_index_of),
    ne!("Lkotlin/collections/CollectionsKt;", "retainAll", "(Ljava/util/List;Lkotlin/jvm/functions/Function1;)Z", false, collections_retain_all),
    ne!("Lkotlin/collections/CollectionsKt;", "getIndices", "(Ljava/util/Collection;)Lkotlin/ranges/IntRange;", false, collections_get_indices),
    ne!("Lkotlin/collections/CollectionsKt;", "intersect", "(Ljava/lang/Iterable;Ljava/lang/Iterable;)Ljava/util/Set;", false, collections_intersect),
    ne!("Lkotlin/collections/CollectionsKt;", "union", "(Ljava/lang/Iterable;Ljava/lang/Iterable;)Ljava/util/Set;", false, collections_union),
    ne!("Lkotlin/collections/CollectionsKt;", "shuffled", "(Ljava/lang/Iterable;)Ljava/util/List;", false, collections_shuffled),
    ne!("Lkotlin/collections/CollectionsKt;", "takeLast", "(Ljava/util/List;I)Ljava/util/List;", false, collections_take_last),
    ne!("Lkotlin/collections/CollectionsKt;", "arrayListOf", "([Ljava/lang/Object;)Ljava/util/ArrayList;", false, collections_array_list_of),
    ne!("Lkotlin/collections/CollectionsKt;", "throwCountOverflow", "()V", false, collections_throw_count_overflow),
    ne!("Lkotlin/collections/CollectionsKt;", "zip", "(Ljava/lang/Iterable;Ljava/lang/Iterable;)Ljava/util/List;", false, collections_zip),
    ne!("Lkotlin/collections/CollectionsKt;", "withIndex", "(Ljava/lang/Iterable;)Ljava/lang/Iterable;", false, collections_with_index),
    ne!("Lkotlin/collections/CollectionsKt;", "sortedDescending", "(Ljava/lang/Iterable;)Ljava/util/List;", false, collections_sorted_descending),
    ne!("Lkotlin/collections/CollectionsKt;", "binarySearch$default", "(Ljava/util/List;Ljava/lang/Comparable;IIILjava/lang/Object;)I", false, collections_binary_search_default),
    ne!("Lkotlin/collections/CollectionsKt;", "any", "(Ljava/lang/Iterable;)Z", false, collections_any),
    ne!("Lkotlin/collections/CollectionsKt;", "windowed$default", "(Ljava/lang/Iterable;IIZILjava/lang/Object;)Ljava/util/List;", false, collections_windowed_default),
    ne!("Lkotlin/collections/CollectionsKt;", "toHashSet", "(Ljava/lang/Iterable;)Ljava/util/HashSet;", false, setskt_to_set),
    ne!("Lkotlin/collections/CollectionsKt;", "toSortedSet", "(Ljava/lang/Iterable;)Ljava/util/SortedSet;", false, setskt_to_set),
    ne!("Lkotlin/collections/CollectionsKt;", "toMutableSet", "(Ljava/lang/Iterable;)Ljava/util/Set;", false, setskt_to_set),
    ne!("Lkotlin/collections/CollectionsKt;", "firstOrNull", "(Ljava/lang/Iterable;)Ljava/lang/Object;", false, collections_first_or_null),
    ne!("Lkotlin/collections/CollectionsKt;", "toMutableList", "(Ljava/lang/Iterable;)Ljava/util/List;", false, collections_to_mutable_list),
    ne!("Lkotlin/collections/ArraysKt;", "joinToString$default", "([BLjava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;ILjava/lang/CharSequence;Lkotlin/jvm/functions/Function1;ILjava/lang/Object;)Ljava/lang/String;", false, arrayskt_join_bytes_default),
    ne!("Lkotlin/collections/ArraysKt;", "joinToString$default", "([Ljava/lang/Object;Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;ILjava/lang/CharSequence;Lkotlin/jvm/functions/Function1;ILjava/lang/Object;)Ljava/lang/String;", false, collections_join_to_string_default),
    ne!("Lkotlin/collections/ArraysKt;", "plus", "([Ljava/lang/Object;[Ljava/lang/Object;)[Ljava/lang/Object;", false, arrayskt_plus_arrays),
    ne!("Lkotlin/collections/ArraysKt;", "plus", "([Ljava/lang/Object;Ljava/lang/Object;)[Ljava/lang/Object;", false, arrayskt_plus_elem),
    ne!("Lkotlin/collections/ArraysKt;", "plus", "([Ljava/lang/Object;Ljava/util/Collection;)[Ljava/lang/Object;", false, arrayskt_plus_collection),
    ne!("Lkotlin/collections/ArraysKt;", "plus", "([BB)[B", false, arrayskt_plus_byte_elem),
    ne!("Lkotlin/collections/ArraysKt;", "sum", "([I)I", false, arrayskt_sum_int),
    ne!("Lkotlin/collections/ArraysKt;", "toList", "([Ljava/lang/Object;)Ljava/util/List;", false, collections_to_list_iterable),
    ne!("Lkotlin/collections/ArraysKt;", "toList", "([B)Ljava/util/List;", false, arrayskt_bytes_to_list),
    ne!("Lkotlin/collections/ArraysKt;", "toSet", "([Ljava/lang/Object;)Ljava/util/Set;", false, setskt_to_set),
    ne!("Lkotlin/collections/ArraysKt;", "asList", "([Ljava/lang/Object;)Ljava/util/List;", false, collections_to_list_iterable),
    ne!("Lkotlin/collections/ArraysKt;", "asSequence", "([Ljava/lang/Object;)Lkotlin/sequences/Sequence;", false, sequence_as_sequence),
    ne!("Lkotlin/collections/ArraysKt;", "first", "([Ljava/lang/Object;)Ljava/lang/Object;", false, collections_first),
    ne!("Lkotlin/collections/ArraysKt;", "first", "([I)I", false, arrayskt_first_int),
    ne!("Lkotlin/collections/ArraysKt;", "getOrNull", "([Ljava/lang/Object;I)Ljava/lang/Object;", false, collections_get_or_null),
    ne!("Lkotlin/collections/ArraysKt;", "last", "([B)B", false, arrayskt_last_byte),
    ne!("Lkotlin/collections/ArraysKt;", "slice", "([Ljava/lang/Object;Lkotlin/ranges/IntRange;)Ljava/util/List;", false, arrayskt_slice_obj),
    ne!("Lkotlin/collections/ArraysKt;", "sliceArray", "([BLkotlin/ranges/IntRange;)[B", false, arrayskt_slice_array_bytes),
    ne!("Lkotlin/collections/ArraysKt;", "zip", "([Ljava/lang/Object;[Ljava/lang/Object;)Ljava/util/List;", false, collections_zip),
    ne!("Lkotlin/collections/ArraysKt;", "copyInto", "([B[BIII)[B", false, arrayskt_copy_into_bytes),
    ne!("Lkotlin/collections/ArraysKt;", "copyInto$default", "([B[BIIIILjava/lang/Object;)[B", false, arrayskt_copy_into_bytes_default),
    ne!("Lkotlin/collections/ArraysKt;", "copyInto", "([J[JIII)[J", false, arrayskt_copy_into_longs),
    ne!("Lkotlin/collections/ArraysKt;", "copyInto$default", "([J[JIIIILjava/lang/Object;)[J", false, arrayskt_copy_into_longs_default),
    ne!("Lkotlin/collections/ArraysKt;", "fill$default", "([BBIIILjava/lang/Object;)V", false, arrayskt_fill_bytes_default),
    ne!("Lkotlin/collections/ArraysKt;", "fill$default", "([JJIIILjava/lang/Object;)V", false, arrayskt_fill_longs_default),
    ne!("Lkotlin/collections/ArraysKt;", "copyOfRange", "([III)[I", false, arrayskt_copy_of_range_ints),
    ne!("Lkotlin/collections/SetsKt;", "setOf", "(Ljava/lang/Object;)Ljava/util/Set;", false, setskt_set_of_single),
    ne!("Lkotlin/collections/SetsKt;", "hashSetOf", "([Ljava/lang/Object;)Ljava/util/HashSet;", false, setskt_set_of),
    ne!("Lkotlin/collections/SetsKt;", "mutableSetOf", "([Ljava/lang/Object;)Ljava/util/Set;", false, setskt_set_of),
    ne!("Lkotlin/collections/SetsKt;", "plus", "(Ljava/util/Set;Ljava/lang/Iterable;)Ljava/util/Set;", false, setskt_plus_iterable),
    ne!("Lkotlin/collections/SetsKt;", "minus", "(Ljava/util/Set;Ljava/lang/Iterable;)Ljava/util/Set;", false, setskt_minus_iterable),
    ne!("Lkotlin/collections/SetsKt;", "minus", "(Ljava/util/Set;Ljava/lang/Object;)Ljava/util/Set;", false, setskt_minus_elem),
    ne!("Lkotlin/collections/SetsKt;", "build", "(Ljava/util/Set;)Ljava/util/Set;", false, setskt_build),
    ne!("Lkotlin/collections/SetsKt;", "createSetBuilder", "()Ljava/util/Set;", false, setskt_create_builder),
    ne!("Lkotlin/collections/MapsKt;", "getValue", "(Ljava/util/Map;Ljava/lang/Object;)Ljava/lang/Object;", false, mapskt_get_value),
    ne!("Lkotlin/collections/MapsKt;", "toMap", "(Ljava/lang/Iterable;)Ljava/util/Map;", false, mapskt_map_of),
    ne!("Lkotlin/collections/MapsKt;", "toMap", "([Lkotlin/Pair;)Ljava/util/Map;", false, mapskt_map_of),
    ne!("Lkotlin/collections/MapsKt;", "toMutableMap", "(Ljava/util/Map;)Ljava/util/Map;", false, mapskt_to_mutable_map),
    ne!("Lkotlin/collections/MapsKt;", "build", "(Ljava/util/Map;)Ljava/util/Map;", false, mapskt_build),
    ne!("Lkotlin/collections/MapsKt;", "createMapBuilder", "()Ljava/util/Map;", false, mapskt_create_builder),
    ne!("Lkotlin/collections/MapsKt;", "mapOf", "(Lkotlin/Pair;)Ljava/util/Map;", false, mapskt_map_of_single),
    ne!("Lkotlin/collections/MapsKt;", "mutableMapOf", "([Lkotlin/Pair;)Ljava/util/Map;", false, mapskt_map_of),
    ne!("Lkotlin/collections/MapsKt;", "plus", "(Ljava/util/Map;Lkotlin/Pair;)Ljava/util/Map;", false, mapskt_plus_pair),
    ne!("Lkotlin/collections/MapsKt;", "plus", "(Ljava/util/Map;Ljava/util/Map;)Ljava/util/Map;", false, mapskt_plus_maps),
    ne!("Lkotlin/collections/MapsKt;", "sortedMapOf", "([Lkotlin/Pair;)Ljava/util/SortedMap;", false, mapskt_sorted_map_of),
    ne!("Lkotlin/collections/MapsKt;", "toSortedMap", "(Ljava/util/Map;)Ljava/util/SortedMap;", false, mapskt_to_mutable_map),
    ne!("Lkotlin/collections/MapsKt;", "linkedMapOf", "([Lkotlin/Pair;)Ljava/util/LinkedHashMap;", false, mapskt_map_of),
    ne!("Lkotlin/collections/MapsKt;", "hashMapOf", "([Lkotlin/Pair;)Ljava/util/HashMap;", false, mapskt_map_of),
    ne!("Lkotlin/collections/ArrayDeque;", "<init>", "()V", true, arraydeque_init),
    ne!("Lkotlin/collections/ArrayDeque;", "<init>", "(I)V", true, arraydeque_init),
    ne!("Lkotlin/collections/ArrayDeque;", "<init>", "(Ljava/util/Collection;)V", true, arraydeque_init),
    ne!("Lkotlin/collections/ArrayDeque;", "firstOrNull", "()Ljava/lang/Object;", true, arraydeque_first_or_null),
    ne!("Lkotlin/collections/ArrayDeque;", "set", "(ILjava/lang/Object;)Ljava/lang/Object;", true, arraydeque_set),
    ne!("Lkotlin/collections/ArrayDeque;", "isEmpty", "()Z", true, arraydeque_is_empty),
    ne!("Lkotlin/collections/ArrayDeque;", "addLast", "(Ljava/lang/Object;)V", true, arraydeque_add_last),
    ne!("Lkotlin/collections/ArrayDeque;", "add", "(Ljava/lang/Object;)Z", true, arraydeque_add),
    ne!("Lkotlin/collections/ArrayDeque;", "addFirst", "(Ljava/lang/Object;)V", true, arraydeque_add_first),
    ne!("Lkotlin/collections/ArrayDeque;", "removeFirst", "()Ljava/lang/Object;", true, arraydeque_remove_first),
    ne!("Lkotlin/collections/ArrayDeque;", "removeLast", "()Ljava/lang/Object;", true, arraydeque_remove_last),
    ne!("Lkotlin/collections/ArrayDeque;", "clear", "()V", true, arraydeque_clear),
    ne!("Lkotlin/collections/ArrayDeque;", "lastOrNull", "()Ljava/lang/Object;", true, arraydeque_last_or_null),
];
