//! Kotlin sequence bridge registrations.
use crate::vm::native::*;

use super::collections::{
    collections_first_or_null, collections_join_to_string_default, collections_last_or_null,
    collections_max_or_null, setskt_to_set,
};

pub(super) fn sequence_as_sequence(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    alloc(vm, "Lkotlin/sequences/Sequence;", Native::List(values))
}
pub(super) fn sequence_to_list(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    list_alloc(vm, values)
}
fn sequence_map(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let f = args[1];
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        out.push(inv_virt(
            vm,
            f,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[value],
        )?);
    }
    alloc(vm, "Lkotlin/sequences/Sequence;", Native::List(out))
}
fn sequence_filter(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let f = args[1];
    let mut out = Vec::new();
    for value in values {
        let keep = inv_virt(
            vm,
            f,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[value],
        )?;
        if int_of(vm, keep) != 0 {
            out.push(value);
        }
    }
    alloc(vm, "Lkotlin/sequences/Sequence;", Native::List(out))
}
fn sequence_map_indexed(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let f = args[1];
    let mut out = Vec::with_capacity(values.len());
    for (i, value) in values.into_iter().enumerate() {
        out.push(inv_virt(
            vm,
            f,
            "invoke",
            "(ILjava/lang/Object;)Ljava/lang/Object;",
            &[JValue::Int(i as i32), value],
        )?);
    }
    alloc(vm, "Lkotlin/sequences/Sequence;", Native::List(out))
}
fn sequence_map_not_null(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let f = args[1];
    let mut out = Vec::new();
    for value in values {
        let mapped = inv_virt(
            vm,
            f,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[value],
        )?;
        if !mapped.is_null_ref() {
            out.push(mapped);
        }
    }
    alloc(vm, "Lkotlin/sequences/Sequence;", Native::List(out))
}

const SEQ_CLASS: &str = "Lkotlin/sequences/Sequence;";

fn seq_alloc(vm: &mut Vm, items: Vec<JValue>) -> R {
    alloc(vm, SEQ_CLASS, Native::List(items))
}

fn sequence_filter_not(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let f = args[1];
    let mut out = Vec::new();
    for value in values {
        let keep = inv_virt(
            vm,
            f,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[value],
        )?;
        if int_of(vm, keep) == 0 {
            out.push(value);
        }
    }
    seq_alloc(vm, out)
}
fn sequence_filter_not_null(vm: &mut Vm, args: &[JValue]) -> R {
    let out = coll_elems(vm, args[0])?
        .into_iter()
        .filter(|v| !v.is_null_ref())
        .collect();
    seq_alloc(vm, out)
}
fn sequence_distinct(vm: &mut Vm, args: &[JValue]) -> R {
    let mut out: Vec<JValue> = Vec::new();
    for value in coll_elems(vm, args[0])? {
        let mut found = false;
        for existing in &out {
            if java_equals(vm, *existing, value)? {
                found = true;
                break;
            }
        }
        if !found {
            out.push(value);
        }
    }
    seq_alloc(vm, out)
}
fn sequence_distinct_by(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let f = args[1];
    let mut keys: Vec<JValue> = Vec::new();
    let mut out = Vec::new();
    for value in values {
        let key = inv_virt(
            vm,
            f,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[value],
        )?;
        let mut found = false;
        for existing in &keys {
            if java_equals(vm, *existing, key)? {
                found = true;
                break;
            }
        }
        if !found {
            keys.push(key);
            out.push(value);
        }
    }
    seq_alloc(vm, out)
}
fn sequence_sorted_with(vm: &mut Vm, args: &[JValue]) -> R {
    let mut values = coll_elems(vm, args[0])?;
    let comparator = args[1];
    let mut err: Option<NatErr> = None;
    values.sort_by(|a, b| {
        match inv_virt(
            vm,
            comparator,
            "compare",
            "(Ljava/lang/Object;Ljava/lang/Object;)I",
            &[*a, *b],
        ) {
            Ok(r) => r.as_int().cmp(&0),
            Err(e) => {
                err = Some(e);
                Ordering::Equal
            }
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    seq_alloc(vm, values)
}
fn sequence_take(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[1]).max(0) as usize;
    let mut values = coll_elems(vm, args[0])?;
    values.truncate(n);
    seq_alloc(vm, values)
}
fn sequence_drop(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[1]).max(0) as usize;
    let values = coll_elems(vm, args[0])?;
    seq_alloc(vm, values.into_iter().skip(n).collect())
}
fn sequence_take_while(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let f = args[1];
    let mut out = Vec::new();
    for value in values {
        let keep = inv_virt(
            vm,
            f,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[value],
        )?;
        if int_of(vm, keep) == 0 {
            break;
        }
        out.push(value);
    }
    seq_alloc(vm, out)
}
fn sequence_flatten(vm: &mut Vm, args: &[JValue]) -> R {
    let mut out = Vec::new();
    for value in coll_elems(vm, args[0])? {
        out.extend(coll_elems(vm, value)?);
    }
    seq_alloc(vm, out)
}
fn sequence_flat_map_iterable(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let f = args[1];
    let mut out = Vec::new();
    for value in values {
        let mapped = inv_virt(
            vm,
            f,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[value],
        )?;
        out.extend(coll_elems(vm, mapped)?);
    }
    seq_alloc(vm, out)
}
fn sequence_zip(vm: &mut Vm, args: &[JValue]) -> R {
    let a = coll_elems(vm, args[0])?;
    let b = coll_elems(vm, args[1])?;
    let mut out = Vec::with_capacity(a.len().min(b.len()));
    for (x, y) in a.into_iter().zip(b) {
        out.push(alloc(vm, "Lkotlin/Pair;", Native::Pair(x, y))?);
    }
    seq_alloc(vm, out)
}
fn sequence_zip_transform(vm: &mut Vm, args: &[JValue]) -> R {
    let a = coll_elems(vm, args[0])?;
    let b = coll_elems(vm, args[1])?;
    let f = args[2];
    let mut out = Vec::with_capacity(a.len().min(b.len()));
    for (x, y) in a.into_iter().zip(b) {
        out.push(inv_virt(
            vm,
            f,
            "invoke",
            "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
            &[x, y],
        )?);
    }
    seq_alloc(vm, out)
}
fn sequence_sequence_of(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    seq_alloc(vm, values)
}
fn sequence_empty(vm: &mut Vm, _args: &[JValue]) -> R {
    seq_alloc(vm, Vec::new())
}
fn sequence_as_iterable(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}
fn sequence_count(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(coll_elems(vm, args[0])?.len() as i32))
}
fn sequence_element_at(vm: &mut Vm, args: &[JValue]) -> R {
    let idx = int_of(vm, args[1]);
    let values = coll_elems(vm, args[0])?;
    if idx < 0 || idx as usize >= values.len() {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/IndexOutOfBoundsException;",
            format!("index {idx}"),
        )));
    }
    Ok(values[idx as usize])
}
fn sequence_to_collection(vm: &mut Vm, args: &[JValue]) -> R {
    let extra = coll_elems(vm, args[0])?;
    let Some(n) = payload_mut(vm, args[1]) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) | Native::Set(dst) => dst.extend(extra),
        _ => return Err(iae(vm, "not a mutable collection")),
    }
    Ok(args[1])
}
const CAP: usize = 100_000;
fn sequence_generate_with_seed(vm: &mut Vm, args: &[JValue]) -> R {
    let seed = args[0];
    let f = args[1];
    let mut out = Vec::new();
    let mut cur = seed;
    while !cur.is_null_ref() && out.len() < CAP {
        out.push(cur);
        cur = inv_virt(
            vm,
            f,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[cur],
        )?;
    }
    seq_alloc(vm, out)
}
fn sequence_generate(vm: &mut Vm, args: &[JValue]) -> R {
    let f = args[0];
    let mut out = Vec::new();
    loop {
        let v = inv_virt(vm, f, "invoke", "()Ljava/lang/Object;", &[])?;
        if v.is_null_ref() || out.len() >= CAP {
            break;
        }
        out.push(v);
    }
    seq_alloc(vm, out)
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "map",
        "(Lkotlin/sequences/Sequence;Lkotlin/jvm/functions/Function1;)Lkotlin/sequences/Sequence;",
        false,
        sequence_map
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "filter",
        "(Lkotlin/sequences/Sequence;Lkotlin/jvm/functions/Function1;)Lkotlin/sequences/Sequence;",
        false,
        sequence_filter
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "toList",
        "(Lkotlin/sequences/Sequence;)Ljava/util/List;",
        false,
        sequence_to_list
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "mapIndexed",
        "(Lkotlin/sequences/Sequence;Lkotlin/jvm/functions/Function2;)Lkotlin/sequences/Sequence;",
        false,
        sequence_map_indexed
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "mapNotNull",
        "(Lkotlin/sequences/Sequence;Lkotlin/jvm/functions/Function1;)Lkotlin/sequences/Sequence;",
        false,
        sequence_map_not_null
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "filterNot",
        "(Lkotlin/sequences/Sequence;Lkotlin/jvm/functions/Function1;)Lkotlin/sequences/Sequence;",
        false,
        sequence_filter_not
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "filterNotNull",
        "(Lkotlin/sequences/Sequence;)Lkotlin/sequences/Sequence;",
        false,
        sequence_filter_not_null
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "distinct",
        "(Lkotlin/sequences/Sequence;)Lkotlin/sequences/Sequence;",
        false,
        sequence_distinct
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "distinctBy",
        "(Lkotlin/sequences/Sequence;Lkotlin/jvm/functions/Function1;)Lkotlin/sequences/Sequence;",
        false,
        sequence_distinct_by
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "sortedWith",
        "(Lkotlin/sequences/Sequence;Ljava/util/Comparator;)Lkotlin/sequences/Sequence;",
        false,
        sequence_sorted_with
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "take",
        "(Lkotlin/sequences/Sequence;I)Lkotlin/sequences/Sequence;",
        false,
        sequence_take
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "drop",
        "(Lkotlin/sequences/Sequence;I)Lkotlin/sequences/Sequence;",
        false,
        sequence_drop
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "takeWhile",
        "(Lkotlin/sequences/Sequence;Lkotlin/jvm/functions/Function1;)Lkotlin/sequences/Sequence;",
        false,
        sequence_take_while
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "flatten",
        "(Lkotlin/sequences/Sequence;)Lkotlin/sequences/Sequence;",
        false,
        sequence_flatten
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "flatMapIterable",
        "(Lkotlin/sequences/Sequence;Lkotlin/jvm/functions/Function1;)Lkotlin/sequences/Sequence;",
        false,
        sequence_flat_map_iterable
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "zip",
        "(Lkotlin/sequences/Sequence;Lkotlin/sequences/Sequence;)Lkotlin/sequences/Sequence;",
        false,
        sequence_zip
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "zip",
        "(Lkotlin/sequences/Sequence;Lkotlin/sequences/Sequence;Lkotlin/jvm/functions/Function2;)Lkotlin/sequences/Sequence;",
        false,
        sequence_zip_transform
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "sequenceOf",
        "([Ljava/lang/Object;)Lkotlin/sequences/Sequence;",
        false,
        sequence_sequence_of
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "emptySequence",
        "()Lkotlin/sequences/Sequence;",
        false,
        sequence_empty
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "asIterable",
        "(Lkotlin/sequences/Sequence;)Ljava/lang/Iterable;",
        false,
        sequence_as_iterable
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "count",
        "(Lkotlin/sequences/Sequence;)I",
        false,
        sequence_count
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "elementAt",
        "(Lkotlin/sequences/Sequence;I)Ljava/lang/Object;",
        false,
        sequence_element_at
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "firstOrNull",
        "(Lkotlin/sequences/Sequence;)Ljava/lang/Object;",
        false,
        collections_first_or_null
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "lastOrNull",
        "(Lkotlin/sequences/Sequence;)Ljava/lang/Object;",
        false,
        collections_last_or_null
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "maxOrNull",
        "(Lkotlin/sequences/Sequence;)Ljava/lang/Comparable;",
        false,
        collections_max_or_null
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "toSet",
        "(Lkotlin/sequences/Sequence;)Ljava/util/Set;",
        false,
        setskt_to_set
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "toSortedSet",
        "(Lkotlin/sequences/Sequence;)Ljava/util/SortedSet;",
        false,
        setskt_to_set
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "toCollection",
        "(Lkotlin/sequences/Sequence;Ljava/util/Collection;)Ljava/util/Collection;",
        false,
        sequence_to_collection
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "joinToString$default",
        "(Lkotlin/sequences/Sequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;ILjava/lang/CharSequence;Lkotlin/jvm/functions/Function1;ILjava/lang/Object;)Ljava/lang/String;",
        false,
        collections_join_to_string_default
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "generateSequence",
        "(Ljava/lang/Object;Lkotlin/jvm/functions/Function1;)Lkotlin/sequences/Sequence;",
        false,
        sequence_generate_with_seed
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "generateSequence",
        "(Lkotlin/jvm/functions/Function0;)Lkotlin/sequences/Sequence;",
        false,
        sequence_generate
    ),
];
