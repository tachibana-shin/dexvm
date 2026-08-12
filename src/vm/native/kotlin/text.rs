//! Kotlin stdlib host shims. Duration raw encoding is milliseconds so that
//! both `getInWholeSeconds` (raw / 1000) and `getInWholeMilliseconds` (raw)
//! round-trip through `toDuration`.
#![allow(dead_code)]

use crate::vm::native::*;

#[path = "collections.rs"]
mod collections;
#[path = "intrinsics.rs"]
mod intrinsics;
#[path = "ranges.rs"]
mod ranges;
#[path = "result.rs"]
mod result;
#[path = "sequences.rs"]
mod sequences;
#[path = "time.rs"]
mod time;
#[path = "tuples.rs"]
mod tuples;
pub(crate) use collections::TABLE as COLLECTIONS_TABLE;
pub(crate) use intrinsics::TABLE as INTRINSICS_TABLE;
pub(crate) use ranges::TABLE as RANGES_TABLE;
#[allow(unused_imports)]
pub(crate) use ranges::{
    int_iterator_has_next, int_iterator_init, int_iterator_next_int, int_range_get_first,
    int_range_get_last, int_range_init, rangeskt_until,
};
pub(crate) use result::TABLE as RESULT_TABLE;
pub(crate) use sequences::TABLE as SEQUENCES_TABLE;
pub(crate) use time::TABLE as TIME_TABLE;
pub(crate) use tuples::TABLE as TUPLES_TABLE;
#[allow(unused_imports)]
pub(crate) use tuples::{
    pair_get_first, pair_get_second, pair_init, triple_component1, triple_component2,
    triple_component3, triple_get_first, triple_get_second, triple_get_third, triple_init,
    tripled_to, tupled_to,
};

// lazy static materializers
// ---------------------------------------------------------------------------

pub(crate) fn opaque_inst(vm: &mut Vm, desc: &str) -> JValue {
    let Ok(class) = vm.ensure_class_by_desc(desc) else {
        return JValue::Null;
    };
    JValue::Obj(vm.arena.alloc(class, Vec::new(), Some(Native::Opaque)))
}

pub(crate) fn lazy_duration_companion(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/time/Duration$Companion;")
}

pub(crate) fn lazy_duration_unit_seconds(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/time/DurationUnit;")
}

pub(crate) fn lazy_duration_unit_days(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/time/DurationUnit;")
}

pub(crate) fn lazy_duration_unit_millis(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/time/DurationUnit;")
}

pub(crate) fn lazy_unit_instance(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/Unit;")
}

pub(crate) fn lazy_global_scope(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlinx/coroutines/GlobalScope;")
}

pub(crate) fn lazy_result_companion(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/Result$Companion;")
}

fn enum_entries(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = coll_elems(vm, args[0])?;
    list_alloc(vm, entries)
}

fn boxing_box_boolean(vm: &mut Vm, args: &[JValue]) -> R {
    boxed(
        vm,
        "Ljava/lang/Boolean;",
        Native::BoolBox(int_of(vm, args[0]) != 0),
    )
}

fn boxing_box_int(vm: &mut Vm, args: &[JValue]) -> R {
    boxed(
        vm,
        "Ljava/lang/Integer;",
        Native::IntBox(int_of(vm, args[0])),
    )
}

// Lazy / LazyKt
// ---------------------------------------------------------------------------

pub(crate) fn lazy_kt_lazy(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Lkotlin/SynchronizedLazyImpl;", Native::Lazy(args[0]))
}

pub(crate) fn lazy_kt_lazy_mode(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Lkotlin/SynchronizedLazyImpl;", Native::Lazy(args[1]))
}

pub(crate) fn lazy_get_value(vm: &mut Vm, args: &[JValue]) -> R {
    let f = match payload(vm, args[0]) {
        Some(Native::Lazy(f)) => *f,
        _ => return Err(npe(vm)),
    };
    if f.is_null_ref() {
        return Err(npe(vm));
    }
    inv_virt(vm, f, "invoke", "()Ljava/lang/Object;", &[])
}

// kotlin.text.Regex (payload reuses Native::Pattern)
// ---------------------------------------------------------------------------

pub(crate) fn regex_init(vm: &mut Vm, args: &[JValue]) -> R {
    let src = jstr(vm, args[1])?;
    let re =
        ::fancy_regex::Regex::new(&src).map_err(|e| iae(vm, format!("bad regex {src}: {e}")))?;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Pattern { re: dst, source } => {
            *dst = re;
            *source = src;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

fn regex_init_option(vm: &mut Vm, args: &[JValue]) -> R {
    let mut src = jstr(vm, args[1])?;
    let option = match payload(vm, args[2]) {
        Some(Native::Enum { name, .. }) => name.as_str(),
        _ => "",
    };
    src = match option {
        "IGNORE_CASE" => format!("(?i:{src})"),
        "MULTILINE" => format!("(?m:{src})"),
        "DOT_MATCHES_ALL" => format!("(?s:{src})"),
        "LITERAL" => fancy_regex::escape(&src).into_owned(),
        _ => src,
    };
    let pattern = new_str(vm, &src);
    regex_init(vm, &[args[0], pattern])
}

pub(crate) fn regex_replace(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let repl = jstr(vm, args[2])?;
    Ok(new_str(vm, &re.replace_all(&text, repl.as_str())))
}

pub(crate) fn regex_replace_function(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let callback = args[2];
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for matched in re.find_iter(&text).flatten() {
        out.push_str(&text[cursor..matched.start()]);
        let match_obj = alloc(
            vm,
            "Lkotlin/text/MatcherMatchResult;",
            Native::Matcher(MatcherState {
                pattern: re.clone(),
                text: text.clone(),
                pos: matched.end(),
                last: Some((matched.start(), matched.end())),
            }),
        )?;
        let replacement = inv_virt(
            vm,
            callback,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[match_obj],
        )?;
        out.push_str(&charseq_of(vm, replacement)?);
        cursor = matched.end();
    }
    out.push_str(&text[cursor..]);
    Ok(new_str(vm, &out))
}

pub(crate) fn regex_matches(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let full = re
        .find(&text)
        .ok()
        .flatten()
        .is_some_and(|m| m.start() == 0 && m.end() == text.len());
    Ok(JValue::Int(i32::from(full)))
}

pub(crate) fn regex_match_entire(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let Some(m) = re.find(&text).ok().flatten() else {
        return Ok(JValue::Null);
    };
    if m.start() != 0 || m.end() != text.len() {
        return Ok(JValue::Null);
    }
    let end = m.end();
    alloc(
        vm,
        "Lkotlin/text/MatcherMatchResult;",
        Native::Matcher(MatcherState {
            pattern: re,
            text,
            pos: end,
            last: Some((0, end)),
        }),
    )
}

pub(crate) fn regex_contains_match_in(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    Ok(JValue::Int(i32::from(re.is_match(&text).unwrap_or(false))))
}

pub(crate) fn regex_find_default(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let start = if int_of(vm, args[3]) & 2 != 0 {
        0
    } else {
        int_of(vm, args[2]).max(0) as usize
    };
    let hit = re
        .find_iter(&text)
        .flatten()
        .find(|matched| matched.start() >= start)
        .map(|matched| (matched.start(), matched.end()));
    let Some((match_start, match_end)) = hit else {
        return Ok(JValue::Null);
    };
    alloc(
        vm,
        "Lkotlin/text/MatcherMatchResult;",
        Native::Matcher(MatcherState {
            pattern: re,
            text,
            pos: match_end,
            last: Some((match_start, match_end)),
        }),
    )
}

pub(crate) fn regex_split(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let limit = int_of(vm, args[2]);
    let raw_parts = if limit > 0 {
        re.splitn(&text, limit as usize)
            .collect::<Result<Vec<_>, _>>()
    } else {
        re.split(&text).collect::<Result<Vec<_>, _>>()
    }
    .map_err(|error| iae(vm, format!("regex split failed: {error}")))?;
    let parts = raw_parts
        .into_iter()
        .map(|part| new_str(vm, part))
        .collect();
    list_alloc(vm, parts)
}

pub(crate) fn regex_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let source = match payload(vm, args[0]) {
        Some(Native::Pattern { source, .. }) => source.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &source))
}

// kotlin.collections.CollectionsKt (statics)
// ---------------------------------------------------------------------------

pub(crate) fn collections_list_of_array(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    list_alloc(vm, items)
}

pub(crate) fn collections_list_of_single(vm: &mut Vm, args: &[JValue]) -> R {
    let items = if args[0].is_null_ref() {
        Vec::new()
    } else {
        vec![args[0]]
    };
    list_alloc(vm, items)
}

pub(crate) fn kotlin_empty_list(vm: &mut Vm, _args: &[JValue]) -> R {
    list_alloc(vm, Vec::new())
}

/// `CollectionsKt.build(list)` — the builder is already the final list.
pub(crate) fn kotlin_list_identity(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn stringskt_starts_with_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    let prefix = charseq_of(vm, args[1])?;
    let ignore = args[2].as_int() != 0;
    let ignore_case = if args[3].as_int() & 4 != 0 {
        false
    } else {
        ignore
    };
    let r = if ignore_case {
        s.to_lowercase().starts_with(&prefix.to_lowercase())
    } else {
        s.starts_with(&prefix)
    };
    Ok(JValue::Int(r as i32))
}

pub(crate) fn collections_reversed(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.reverse();
    list_alloc(vm, items)
}

pub(crate) fn collections_remove_all(vm: &mut Vm, args: &[JValue]) -> R {
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
fn collections_to_list_iterable(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    list_alloc(vm, values)
}
fn collections_sorted(vm: &mut Vm, args: &[JValue]) -> R {
    let mut values = coll_elems(vm, args[0])?;
    values.sort_by(|a, b| java_cmp(vm, *a, *b).unwrap_or(Ordering::Equal));
    list_alloc(vm, values)
}

fn collections_as_reversed(vm: &mut Vm, args: &[JValue]) -> R {
    collections_reversed(vm, args)
}
fn collections_take(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[1]).max(0) as usize;
    let mut values = coll_elems(vm, args[0])?;
    values.truncate(n);
    list_alloc(vm, values)
}
fn collections_drop(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[1]).max(0) as usize;
    let values = coll_elems(vm, args[0])?;
    list_alloc(vm, values.into_iter().skip(n).collect())
}

pub(crate) fn collections_plus_iterable(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.extend(coll_elems(vm, args[1])?);
    list_alloc(vm, items)
}
pub(crate) fn collections_get_last_index(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    Ok(JValue::Int(values.len().saturating_sub(1) as i32))
}

pub(crate) fn collections_plus_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.push(args[1]);
    list_alloc(vm, items)
}

pub(crate) fn collections_contains(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    for v in items {
        if java_equals(vm, v, args[1])? {
            return Ok(JValue::Int(1));
        }
    }
    Ok(JValue::Int(0))
}

pub(crate) fn collections_first(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    match items.into_iter().next() {
        Some(v) => Ok(v),
        None => Err(no_such_elem(vm)),
    }
}

pub(crate) fn collections_first_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(coll_elems(vm, args[0])?
        .into_iter()
        .next()
        .unwrap_or(JValue::Null))
}

pub(crate) fn collections_last(vm: &mut Vm, args: &[JValue]) -> R {
    coll_elems(vm, args[0])?
        .into_iter()
        .last()
        .ok_or_else(|| no_such_elem(vm))
}

pub(crate) fn collections_get_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let index = int_of(vm, args[1]);
    if index < 0 {
        return Ok(JValue::Null);
    }
    Ok(coll_elems(vm, args[0])?
        .get(index as usize)
        .copied()
        .unwrap_or(JValue::Null))
}

pub(crate) fn collections_to_mutable_list(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    list_alloc(vm, items)
}

fn sequence_as_sequence(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    alloc(vm, "Lkotlin/sequences/Sequence;", Native::List(values))
}

fn sequence_to_list(vm: &mut Vm, args: &[JValue]) -> R {
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
        let result = inv_virt(
            vm,
            f,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[value],
        )?;
        if int_of(vm, result) != 0 {
            out.push(value);
        }
    }
    alloc(vm, "Lkotlin/sequences/Sequence;", Native::List(out))
}

fn collections_flatten(vm: &mut Vm, args: &[JValue]) -> R {
    let mut out = Vec::new();
    for value in coll_elems(vm, args[0])? {
        out.extend(coll_elems(vm, value)?);
    }
    list_alloc(vm, out)
}

pub(crate) fn collections_add_all(vm: &mut Vm, args: &[JValue]) -> R {
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

pub(crate) fn collections_throw_index_overflow(vm: &mut Vm, _args: &[JValue]) -> R {
    Err(NatErr::Throw(vm.throwable_of(
        "Ljava/lang/ArithmeticException;",
        "Index overflow has happened.",
    )))
}

pub(crate) fn collections_size_or_default(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    let def = int_of(vm, args[1]);
    let n = items.len() as i32;
    Ok(JValue::Int(if n < 10 { n } else { def }))
}

/// kotlin.collections.joinToString with the compiler-generated `$default`
/// marker: (iterable, separator, prefix, postfix, limit, truncated,
/// transform, mask, marker).
pub(crate) fn collections_join_to_string_default(vm: &mut Vm, args: &[JValue]) -> R {
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

// kotlin.text
// ---------------------------------------------------------------------------

/// `StringsKt.isBlank(CharSequence)`.
pub(crate) fn stringskt_is_blank(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    Ok(JValue::Int(i32::from(s.trim().is_empty())))
}

/// `StringsKt.toIntOrNull(String)` — boxed Integer or null.
pub(crate) fn stringskt_to_int_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    match s.trim().parse::<i32>() {
        Ok(n) => boxed(vm, "Ljava/lang/Integer;", Native::IntBox(n)),
        Err(_) => Ok(JValue::Null),
    }
}

fn stringskt_to_float_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    match value.trim().parse::<f32>() {
        Ok(value) => boxed(vm, "Ljava/lang/Float;", Native::FloatBox(value)),
        Err(_) => Ok(JValue::Null),
    }
}

fn stringskt_to_int_radix_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let radix = int_of(vm, args[1]);
    if !(2..=36).contains(&radix) {
        return Err(iae(vm, format!("radix {radix} was not in 2..36")));
    }
    match i32::from_str_radix(value.trim(), radix as u32) {
        Ok(value) => boxed(vm, "Ljava/lang/Integer;", Native::IntBox(value)),
        Err(_) => Ok(JValue::Null),
    }
}

fn stringskt_to_long_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    match value.trim().parse::<i64>() {
        Ok(value) => boxed(vm, "Ljava/lang/Long;", Native::LongBox(value)),
        Err(_) => Ok(JValue::Null),
    }
}

fn chars_from_array(vm: &mut Vm, value: JValue) -> Result<Vec<char>, NatErr> {
    match payload(vm, value) {
        Some(Native::Array(ArrayData::Char(chars))) => Ok(chars
            .iter()
            .map(|value| char::from_u32(u32::from(*value)).unwrap_or('\u{fffd}'))
            .collect()),
        _ => Err(npe(vm)),
    }
}

fn stringskt_trim_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let chars = chars_from_array(vm, args[1])?;
    Ok(new_str(vm, value.trim_matches(|ch| chars.contains(&ch))))
}

fn stringskt_trim_end_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let chars = chars_from_array(vm, args[1])?;
    Ok(new_str(
        vm,
        value.trim_end_matches(|ch| chars.contains(&ch)),
    ))
}

fn stringskt_remove_surrounding(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = charseq_of(vm, args[1])?;
    let stripped = value
        .strip_prefix(&delimiter)
        .and_then(|value| value.strip_suffix(&delimiter))
        .unwrap_or(&value);
    Ok(new_str(vm, stripped))
}

fn setskt_to_set(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?;
    let array = alloc_arr(vm, "Ljava/lang/Object;", values.len(), move || {
        ArrayData::Obj(values)
    })?;
    setskt_set_of(vm, &[array])
}

fn setskt_plus(vm: &mut Vm, args: &[JValue]) -> R {
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

fn arrayskt_contains(vm: &mut Vm, args: &[JValue]) -> R {
    for value in coll_elems(vm, args[0])? {
        if java_equals(vm, value, args[1])? {
            return Ok(JValue::Int(1));
        }
    }
    Ok(JValue::Int(0))
}

fn collections_filter_not_null(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?
        .into_iter()
        .filter(|value| !value.is_null_ref())
        .collect();
    list_alloc(vm, values)
}

fn mapskt_map_capacity(vm: &mut Vm, args: &[JValue]) -> R {
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

fn setskt_empty_set(vm: &mut Vm, _args: &[JValue]) -> R {
    set_alloc(vm, Vec::new())
}

fn collections_distinct(vm: &mut Vm, args: &[JValue]) -> R {
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

fn collections_last_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(coll_elems(vm, args[0])?
        .last()
        .copied()
        .unwrap_or(JValue::Null))
}

fn collections_sorted_with(vm: &mut Vm, args: &[JValue]) -> R {
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

fn stringskt_contains(vm: &mut Vm, args: &[JValue]) -> R {
    let haystack = charseq_of(vm, args[0])?;
    let needle = charseq_of(vm, args[1])?;
    let found = if args[2].as_int() != 0 {
        haystack.to_lowercase().contains(&needle.to_lowercase())
    } else {
        haystack.contains(&needle)
    };
    Ok(JValue::Int(i32::from(found)))
}

fn stringskt_starts_with(vm: &mut Vm, args: &[JValue]) -> R {
    let value = charseq_of(vm, args[0])?;
    let prefix = charseq_of(vm, args[1])?;
    let result = if args[2].as_int() != 0 {
        value.to_lowercase().starts_with(&prefix.to_lowercase())
    } else {
        value.starts_with(&prefix)
    };
    Ok(JValue::Int(i32::from(result)))
}

fn stringskt_ends_with(vm: &mut Vm, args: &[JValue]) -> R {
    let value = charseq_of(vm, args[0])?;
    let suffix = charseq_of(vm, args[1])?;
    let result = if args[2].as_int() != 0 {
        value.to_lowercase().ends_with(&suffix.to_lowercase())
    } else {
        value.ends_with(&suffix)
    };
    Ok(JValue::Int(i32::from(result)))
}

fn stringskt_ends_with_default(vm: &mut Vm, args: &[JValue]) -> R {
    let ignore_case = if int_of(vm, args[3]) & 2 != 0 {
        JValue::Int(0)
    } else {
        args[2]
    };
    stringskt_ends_with(vm, &[args[0], args[1], ignore_case])
}

fn stringskt_remove_prefix(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let prefix = charseq_of(vm, args[1])?;
    Ok(new_str(vm, value.strip_prefix(&prefix).unwrap_or(&value)))
}

fn stringskt_remove_suffix(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let suffix = charseq_of(vm, args[1])?;
    Ok(new_str(vm, value.strip_suffix(&suffix).unwrap_or(&value)))
}

fn stringskt_substring_before_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = jstr(vm, args[1])?;
    let missing = if int_of(vm, args[3]) & 2 != 0 {
        value.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        value
            .find(&delimiter)
            .map(|index| &value[..index])
            .unwrap_or(&missing),
    ))
}

fn split_literal(value: &str, delimiters: &[String], ignore_case: bool, limit: i32) -> Vec<String> {
    let mut output = Vec::new();
    let mut offset = 0;
    while offset <= value.len() && (limit <= 0 || output.len() + 1 < limit as usize) {
        let rest = &value[offset..];
        let folded = ignore_case.then(|| rest.to_lowercase());
        let hit = delimiters
            .iter()
            .filter(|delimiter| !delimiter.is_empty())
            .filter_map(|delimiter| {
                let index = if let Some(folded) = &folded {
                    folded.find(&delimiter.to_lowercase())
                } else {
                    rest.find(delimiter)
                }?;
                Some((index, delimiter.len()))
            })
            .min_by_key(|(index, _)| *index);
        let Some((index, delimiter_len)) = hit else {
            break;
        };
        output.push(rest[..index].to_string());
        offset += index + delimiter_len;
    }
    output.push(value[offset..].to_string());
    output
}

fn stringskt_split_strings_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = charseq_of(vm, args[0])?;
    let delimiters = coll_elems(vm, args[1])?
        .into_iter()
        .map(|value| jstr(vm, value))
        .collect::<Result<Vec<_>, _>>()?;
    let mask = int_of(vm, args[4]);
    let ignore_case = mask & 2 == 0 && args[2].as_int() != 0;
    let limit = if mask & 4 != 0 {
        0
    } else {
        int_of(vm, args[3])
    };
    let parts = split_literal(&value, &delimiters, ignore_case, limit)
        .into_iter()
        .map(|part| new_str(vm, &part))
        .collect();
    list_alloc(vm, parts)
}

fn stringskt_split_chars_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = charseq_of(vm, args[0])?;
    let delimiters = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Char(chars))) => chars
            .iter()
            .map(|value| {
                char::from_u32(u32::from(*value))
                    .unwrap_or('\u{fffd}')
                    .to_string()
            })
            .collect::<Vec<_>>(),
        _ => return Err(npe(vm)),
    };
    let mask = int_of(vm, args[4]);
    let ignore_case = mask & 2 == 0 && args[2].as_int() != 0;
    let limit = if mask & 4 != 0 {
        0
    } else {
        int_of(vm, args[3])
    };
    let parts = split_literal(&value, &delimiters, ignore_case, limit)
        .into_iter()
        .map(|part| new_str(vm, &part))
        .collect();
    list_alloc(vm, parts)
}

fn setskt_set_of(vm: &mut Vm, args: &[JValue]) -> R {
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

fn collections_list_of_not_null(vm: &mut Vm, args: &[JValue]) -> R {
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

fn arrayskt_plus_bytes(vm: &mut Vm, args: &[JValue]) -> R {
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

fn mapskt_map_of(vm: &mut Vm, args: &[JValue]) -> R {
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

fn mapskt_empty_map(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/LinkedHashMap;", Native::Map(Vec::new()))
}

fn mapskt_to_list(vm: &mut Vm, args: &[JValue]) -> R {
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

fn charskt_is_whitespace(_vm: &mut Vm, args: &[JValue]) -> R {
    let value = char::from_u32(args[0].as_int() as u32).unwrap_or('\u{fffd}');
    Ok(JValue::Int(i32::from(value.is_whitespace())))
}

fn charskt_titlecase(vm: &mut Vm, args: &[JValue]) -> R {
    let value = char::from_u32(args[0].as_int() as u32).unwrap_or('\u{fffd}');
    Ok(new_str(vm, &value.to_uppercase().collect::<String>()))
}

fn comparisons_compare_values(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(java_cmp(vm, args[0], args[1])? as i32))
}

fn stringskt_substring_before_last_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = jstr(vm, args[1])?;
    let missing = if int_of(vm, args[3]) & 2 != 0 {
        value.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        value
            .rfind(&delimiter)
            .map(|index| &value[..index])
            .unwrap_or(&missing),
    ))
}

fn stringskt_substring_after_last_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = jstr(vm, args[1])?;
    let missing = if int_of(vm, args[3]) & 2 != 0 {
        value.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        value
            .rfind(&delimiter)
            .map(|index| &value[index + delimiter.len()..])
            .unwrap_or(&missing),
    ))
}

fn stringskt_last_index_of_default(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let needle = charseq_of(vm, args[1])?;
    let start = if int_of(vm, args[4]) & 4 != 0 {
        text.len()
    } else {
        int_of(vm, args[2]).max(0) as usize
    };
    let hay = &text[..start.min(text.len())];
    let found = if int_of(vm, args[3]) != 0 {
        hay.to_lowercase().rfind(&needle.to_lowercase())
    } else {
        hay.rfind(&needle)
    };
    Ok(JValue::Int(found.map_or(-1, |i| i as i32)))
}

fn stringskt_trim_indent(vm: &mut Vm, args: &[JValue]) -> R {
    let text = jstr(vm, args[0])?;
    let lines: Vec<&str> = text.lines().collect();
    let nonblank: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| !l.trim().is_empty())
        .collect();
    let indent = nonblank
        .iter()
        .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
        .min()
        .unwrap_or(0);
    let out = lines
        .into_iter()
        .map(|l| {
            l.chars()
                .skip(indent)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(new_str(vm, &out))
}

fn stringskt_substring_after_last_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = char::from_u32(int_of(vm, args[1]) as u32).unwrap_or('\0');
    let missing = if int_of(vm, args[3]) & 2 != 0 {
        value.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        value
            .rfind(delimiter)
            .map(|index| &value[index + delimiter.len_utf8()..])
            .unwrap_or(&missing),
    ))
}

fn stringskt_substring_before_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = char::from_u32(int_of(vm, args[1]) as u32).unwrap_or('\0');
    let missing = if int_of(vm, args[3]) & 2 != 0 {
        value.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        value
            .find(delimiter)
            .map(|index| &value[..index])
            .unwrap_or(&missing),
    ))
}

fn stringskt_equals(vm: &mut Vm, args: &[JValue]) -> R {
    let left = jstr(vm, args[0])?;
    let right = jstr(vm, args[1])?;
    let equals = if int_of(vm, args[2]) != 0 {
        left.to_lowercase() == right.to_lowercase()
    } else {
        left == right
    };
    Ok(JValue::Int(i32::from(equals)))
}

fn stringskt_index_of_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let needle = char::from_u32(int_of(vm, args[1]) as u32).unwrap_or('\0');
    let start = if int_of(vm, args[4]) & 2 != 0 {
        0
    } else {
        int_of(vm, args[2]).max(0) as usize
    };
    let ignore_case = int_of(vm, args[4]) & 4 == 0 && int_of(vm, args[3]) != 0;
    let suffix = text.get(start..).unwrap_or("");
    let found = if ignore_case {
        suffix
            .char_indices()
            .find(|(_, ch)| ch.to_lowercase().to_string() == needle.to_lowercase().to_string())
            .map(|(index, _)| start + index)
    } else {
        suffix.find(needle).map(|index| start + index)
    };
    Ok(JValue::Int(found.map_or(-1, |index| index as i32)))
}

fn stringskt_index_of_string_default(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let needle = jstr(vm, args[1])?;
    let start = if int_of(vm, args[4]) & 2 != 0 {
        0
    } else {
        int_of(vm, args[2]).max(0) as usize
    };
    let ignore_case = int_of(vm, args[4]) & 4 == 0 && int_of(vm, args[3]) != 0;
    let suffix = text.get(start..).unwrap_or("");
    let found = if ignore_case {
        suffix
            .to_lowercase()
            .find(&needle.to_lowercase())
            .map(|index| start + index)
    } else {
        suffix.find(&needle).map(|index| start + index)
    };
    Ok(JValue::Int(found.map_or(-1, |index| index as i32)))
}

fn rangeskt_coerce_at_least(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(args[0].as_int().max(args[1].as_int())))
}

/// `RangesKt.coerceIn(Int, Int, Int)`.
pub(crate) fn rangeskt_coerce_in(vm: &mut Vm, args: &[JValue]) -> R {
    let v = int_of(vm, args[0]);
    let lo = int_of(vm, args[1]);
    let hi = int_of(vm, args[2]);
    Ok(JValue::Int(v.max(lo).min(hi)))
}

pub(crate) fn charskt_check_radix(vm: &mut Vm, args: &[JValue]) -> R {
    let radix = int_of(vm, args[0]);
    if !(2..=36).contains(&radix) {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/IllegalArgumentException;",
            format!("radix {radix} was not in range 2..36"),
        )));
    }
    Ok(JValue::Int(radix))
}

pub(crate) fn kotlin_random_default_next_int(vm: &mut Vm, args: &[JValue]) -> R {
    let (from, until) = if args.len() >= 2 {
        (args[0].as_int(), args[1].as_int())
    } else {
        (0, args[0].as_int())
    };
    if until <= from {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/IllegalArgumentException;",
            "empty random range",
        )));
    }
    let span = (until - from) as u32;
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    Ok(JValue::Int(from + (seed % span) as i32))
}

/// `MatchResult.getValue` — the whole matched text of the last match on a
/// regex-backed value.
pub(crate) fn match_result_get_value(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::Matcher(ms)) => {
            let Some((start, end)) = ms.last else {
                return Ok(new_str(vm, ""));
            };
            ms.text.get(start..end).unwrap_or("").to_string()
        }
        _ => return Ok(new_str(vm, "")),
    };
    Ok(new_str(vm, &s))
}

pub(crate) fn match_result_destructured_to_list(vm: &mut Vm, args: &[JValue]) -> R {
    let value = match_result_get_value(vm, args)?;
    list_alloc(vm, vec![value])
}

pub(crate) fn match_group_get_value(vm: &mut Vm, args: &[JValue]) -> R {
    let value = match payload(vm, args[0]) {
        Some(Native::Str(value)) => value.clone(),
        _ => String::new(),
    };
    Ok(new_str(vm, &value))
}

pub(crate) fn kotlin_instant_to_epoch_millis(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::EpochMillis(m)) => Ok(JValue::Long(*m)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn kotlin_instant_now(vm: &mut Vm, _args: &[JValue]) -> R {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| iae(vm, "clock before epoch"))?
        .as_millis() as i64;
    alloc(vm, "Lkotlin/time/Instant;", Native::EpochMillis(millis))
}

pub(crate) fn kotlin_instant_minus(vm: &mut Vm, args: &[JValue]) -> R {
    let base = match payload(vm, args[0]) {
        Some(Native::EpochMillis(m)) => *m,
        _ => return Err(npe(vm)),
    };
    alloc(
        vm,
        "Lkotlin/time/Instant;",
        Native::EpochMillis(base.saturating_sub(long_of(vm, args[1]))),
    )
}

/// Kotlin's ISO-8601 parser used by extension date filters.  This accepts the
/// common UTC form (`YYYY-MM-DDTHH:MM:SS[.fraction]Z`) and returns null for
/// malformed/unsupported values, matching `parseOrNull`.
pub(crate) fn kotlin_instant_parse_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let text = jstr(vm, args[1]).unwrap_or_default();
    let Some((date, time)) = text.split_once('T') else {
        return Ok(JValue::Null);
    };
    let mut d = date.split('-');
    let (Ok(y), Ok(m), Ok(day)) = (
        d.next().unwrap_or("").parse::<i64>(),
        d.next().unwrap_or("").parse::<i64>(),
        d.next().unwrap_or("").parse::<i64>(),
    ) else {
        return Ok(JValue::Null);
    };
    let time = time.strip_suffix('Z').unwrap_or(time);
    let (clock, frac) = time.split_once('.').map_or((time, ""), |v| v);
    let mut c = clock.split(':');
    let (Ok(h), Ok(min), Ok(sec)) = (
        c.next().unwrap_or("").parse::<i64>(),
        c.next().unwrap_or("").parse::<i64>(),
        c.next().unwrap_or("").parse::<i64>(),
    ) else {
        return Ok(JValue::Null);
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) || h > 23 || min > 59 || sec > 60 {
        return Ok(JValue::Null);
    }
    let (y2, m2) = (y - i64::from(m <= 2), if m <= 2 { m + 12 } else { m });
    let days =
        365 * y2 + y2 / 4 - y2 / 100 + y2 / 400 + (153 * (m2 - 3) + 2) / 5 + day - 1 - 719468;
    let millis = (days * 86_400 + h * 3600 + min * 60 + sec) * 1000
        + frac
            .chars()
            .take(3)
            .collect::<String>()
            .parse::<i64>()
            .unwrap_or(0)
            * [100, 10, 1][frac.len().min(3).saturating_sub(1)];
    alloc(vm, "Lkotlin/time/Instant;", Native::EpochMillis(millis))
}

pub(crate) fn kotlin_reflection_class(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

// java.net.URI
// ---------------------------------------------------------------------------

pub(crate) fn uri_init(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[1])?;
    let obj = args[0].as_obj();
    vm.arena.objects[obj as usize].native = Some(Native::URI(s));
    Ok(JValue::Null)
}

pub(crate) fn uri_get_host(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::URI(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    let authority = s
        .split("://")
        .last()
        .and_then(|a| a.strip_prefix("//").or(Some(a)))
        .unwrap_or(&s);
    let host = authority.split(['/', ':']).next().unwrap_or("").to_string();
    Ok(new_str(vm, &host))
}

// kotlin.comparisons.ComparisonsKt
// ---------------------------------------------------------------------------

pub(crate) fn comparisons_max_of(vm: &mut Vm, args: &[JValue]) -> R {
    let a = args[0];
    let b = args[1];
    match java_cmp(vm, a, b)? {
        Ordering::Less => Ok(b),
        _ => Ok(a),
    }
}

// ---------------------------------------------------------------------------
pub(crate) fn strings_append_array(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[1]) {
        Some(Native::Array(data)) => {
            let mut v = Vec::new();
            for i in 0..data.len() {
                v.push(data.get(i));
            }
            v
        }
        _ => return Err(npe(vm)),
    };
    let mut s = String::new();
    for item in items {
        if let Ok(t) = jstr(vm, item) {
            s.push_str(&t);
        }
    }
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&s);
    Ok(args[0])
}

// kotlin.text.StringsKt synthetic default-arg shims (mask bit 2 = ignoreCase default false)
// ---------------------------------------------------------------------------

// kotlin.text.StringsKt synthetic default-arg shims (mask bit 2 = ignoreCase default false)
fn stringskt_contains_default(vm: &mut Vm, args: &[JValue]) -> R {
    let haystack = charseq_of(vm, args[0])?;
    let needle = charseq_of(vm, args[1])?;
    let ignore = args[2].as_int() != 0;
    let ignore_case = if args[3].as_int() & 2 != 0 {
        false
    } else {
        ignore
    };
    let found = if ignore_case {
        haystack.to_lowercase().contains(&needle.to_lowercase())
    } else {
        haystack.contains(&needle)
    };
    Ok(JValue::Int(found as i32))
}

fn stringskt_contains_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let ch = char::from_u32(args[1].as_int() as u32).unwrap_or('\u{fffd}');
    let ignore = args[2].as_int() != 0 && args[3].as_int() & 4 == 0;
    let found = if ignore {
        text.to_lowercase().contains(ch.to_ascii_lowercase())
    } else {
        text.contains(ch)
    };
    Ok(JValue::Int(found as i32))
}

fn stringskt_take(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let n = int_of(vm, args[1]).max(0) as usize;
    Ok(new_str(vm, &s.chars().take(n).collect::<String>()))
}
fn stringskt_pad_start(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let n = int_of(vm, args[1]).max(0) as usize;
    let pad = char::from_u32(int_of(vm, args[2]) as u32).unwrap_or(' ');
    let len = s.chars().count();
    let mut out = std::iter::repeat_n(pad, n.saturating_sub(len)).collect::<String>();
    out.push_str(&s);
    Ok(new_str(vm, &out))
}
fn stringskt_drop_last(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let n = int_of(vm, args[1]).max(0) as usize;
    Ok(new_str(
        vm,
        &s.chars()
            .take(s.chars().count().saturating_sub(n))
            .collect::<String>(),
    ))
}
fn stringskt_replace_first_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    let from = charseq_of(vm, args[1])?;
    let to = charseq_of(vm, args[2])?;
    let ignore = args[3].as_int() != 0 && args[4].as_int() & 4 == 0;
    let pos = if ignore {
        s.to_lowercase().find(&from.to_lowercase())
    } else {
        s.find(&from)
    };
    let out = pos
        .map(|i| format!("{}{}{}", &s[..i], to, &s[i + from.len()..]))
        .unwrap_or(s);
    Ok(new_str(vm, &out))
}

fn stringskt_replace_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    let from = charseq_of(vm, args[1])?;
    let to = charseq_of(vm, args[2])?;
    let ignore = args[3].as_int() != 0;
    let ignore_case = if args[4].as_int() & 4 != 0 {
        false
    } else {
        ignore
    };
    let r = if ignore_case {
        regex_replace_case_insensitive(&s, &from, &to)
    } else {
        s.replace(&from, &to)
    };
    alloc(vm, "Ljava/lang/String;", Native::Str(r))
}

// Char/Char variant: String.replace(oldChar, newChar, ignoreCase) — the
// compiler emits the `$default` synthetic for the trailing `ignoreCase`.
fn stringskt_replace_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let old = char::from_u32(int_of(vm, args[1]) as u32).unwrap_or('\u{FFFD}');
    let new = char::from_u32(int_of(vm, args[2]) as u32).unwrap_or('\u{FFFD}');
    let r = s.replace(old, &new.to_string());
    alloc(vm, "Ljava/lang/String;", Native::Str(r))
}

// kotlin.text.StringsKt.trimStart(String, charArray) — strips every leading
// char present in the (sparse) trim character array.
fn stringskt_trim_start(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let trim = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Char(chars))) => chars.clone(),
        _ => return Err(npe(vm)),
    };
    let r = s.trim_start_matches(|c: char| trim.contains(&(c as u16)));
    alloc(vm, "Ljava/lang/String;", Native::Str(r.to_string()))
}

fn stringskt_trim_start_charseq(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    Ok(new_str(vm, s.trim_start()))
}

fn suspend_lambda_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

fn null_out_spilled_variable(_vm: &mut Vm, args: &[JValue]) -> R {
    // Debug-only coroutine spilling marker; release builds may retain the
    // value, which is semantically invisible to dex code.
    Ok(args[0])
}

/// `IntrinsicsKt.getCOROUTINE_SUSPENDED()` — the sentinel that marks a
/// suspension point. Everything in this runtime runs synchronously, so the
/// value is only ever compared against; a fresh opaque instance suffices.
fn coroutines_suspended(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(opaque_inst(vm, "Ljava/lang/Object;"))
}

/// `ResultKt.throwOnFailure(Object)` — raises the exception when the value
/// is a `createFailure` marker; otherwise a no-op.
fn resultkt_throw_on_failure(vm: &mut Vm, args: &[JValue]) -> R {
    if let Some(Native::ResultFailure(t)) = payload(vm, args[0]) {
        return Err(NatErr::Throw(t.as_obj()));
    }
    Ok(JValue::Null)
}

/// `ContinuationImpl.<init>(Continuation)` — base ctor of every state
/// machine frame; the VM keeps no dispatch state of its own.
fn continuation_impl_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

// kotlin.collections.ArraysKt.copyOfRange(byte[], from, to)
fn arrayskt_copy_of_range(vm: &mut Vm, args: &[JValue]) -> R {
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

fn arrayskt_int_to_list(vm: &mut Vm, args: &[JValue]) -> R {
    let values = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Int(v))) => v.iter().map(|x| JValue::Int(*x)).collect(),
        _ => return Err(npe(vm)),
    };
    list_alloc(vm, values)
}

fn progression_last_element(_vm: &mut Vm, args: &[JValue]) -> R {
    let first = args[0].as_int();
    let last = args[1].as_int();
    let step = args[2].as_int();
    if step == 0 {
        return Ok(JValue::Int(last));
    }
    let r = if step > 0 {
        last - (last - first).rem_euclid(step)
    } else {
        last + (first - last).rem_euclid(-step)
    };
    Ok(JValue::Int(r))
}

fn regex_replace_first(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let repl = jstr(vm, args[2])?;
    let out = if let Some(m) = re.find(&text).ok().flatten() {
        format!("{}{}{}", &text[..m.start()], repl, &text[m.end()..])
    } else {
        text
    };
    Ok(new_str(vm, &out))
}

fn strings_encode_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let bytes = s.into_bytes();
    alloc_arr(vm, "B", bytes.len(), move || {
        ArrayData::Byte(bytes.into_iter().map(|b| b as i8).collect())
    })
}

// kotlin.UInt / UByte `constructor-impl`: identity (already raw ints).
// Static: the value arrives as the only argument.
fn uint_constructor_impl(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

fn ubyte_constructor_impl(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

// kotlin.io.TextStreamsKt.readText(Reader) — drains the Reader through
// repeated virtual `read([CII)I` calls (any Reader the VM can invoke),
// assembling the chars into one String. A null reader is raised as an
// IllegalStateException (kotlin.UninitializedPropertyAccessException is
// not a registered shim, so this is its closest registered sibling).
fn textstreamskt_read_text(vm: &mut Vm, args: &[JValue]) -> R {
    let reader = args[0];
    if reader.is_null() {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/IllegalStateException;",
            "Uninitialized property access: null Reader in readText",
        )));
    }
    const CAP: i32 = 4096;
    if let Some(Native::Reader(text)) = payload(vm, reader) {
        return alloc(vm, "Ljava/lang/String;", Native::Str(text.clone()));
    }
    let buf = alloc_arr(vm, "C", CAP as usize, || {
        ArrayData::Char(vec![0u16; CAP as usize])
    })?;
    let mut out: Vec<u16> = Vec::new();
    loop {
        let n = vm
            .invoke_virtual_args(
                reader,
                "read",
                "([CII)I",
                vec![buf, JValue::Int(0), JValue::Int(CAP)],
            )
            .map_err(nat_fatal)?;
        let n = int_of(vm, n);
        if n <= 0 {
            break;
        }
        if let Some(Native::Array(ArrayData::Char(chars))) = payload(vm, buf) {
            out.extend_from_slice(&chars[..n as usize]);
        }
    }
    let s = String::from_utf16_lossy(&out);
    alloc(vm, "Ljava/lang/String;", Native::Str(s))
}

// kotlin.io.CloseableKt.closeFinally(source, cause). With a primary failure,
// a close failure is suppressed; otherwise it propagates.
fn closeablekt_close_finally(vm: &mut Vm, args: &[JValue]) -> R {
    let source = args[0];
    if source.is_null() {
        return Ok(JValue::Null);
    }
    let result = vm
        .invoke_virtual_args(source, "close", "()V", vec![])
        .map_err(nat_fatal);
    if args[1].is_null() {
        result
    } else {
        // Throwable.addSuppressed is not observable in the current throwable
        // model, but the primary exception must remain the one that wins.
        Ok(JValue::Null)
    }
}

fn stringskt_trim(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    alloc(vm, "Ljava/lang/String;", Native::Str(s.trim().to_string()))
}

fn stringskt_substring_after_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    let delim = charseq_of(vm, args[1])?;
    let missing = if args[3].as_int() & 2 != 0 {
        s.clone()
    } else {
        charseq_of(vm, args[2])?
    };
    let r = if delim.is_empty() {
        missing.to_string()
    } else {
        match s.find(&delim) {
            Some(i) => s[i + delim.len()..].to_string(),
            None => missing.to_string(),
        }
    };
    alloc(vm, "Ljava/lang/String;", Native::Str(r))
}

fn stringskt_substring_after_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let delim = char::from_u32(int_of(vm, args[1]) as u32).unwrap_or('\u{FFFD}');
    let missing = if args[3].as_int() & 2 != 0 {
        s.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        s.find(delim)
            .map_or(missing, |i| s[i + delim.len_utf8()..].to_string())
            .as_str(),
    ))
}

fn stringskt_substring_after(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    let delim = charseq_of(vm, args[1])?;
    let missing = charseq_of(vm, args[2])?;
    let out = s
        .find(&delim)
        .map(|i| s[i + delim.len()..].to_string())
        .unwrap_or(missing);
    Ok(new_str(vm, &out))
}

fn regex_replace_case_insensitive(s: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(idx) = rest.to_lowercase().find(&from.to_lowercase()) {
        out.push_str(&rest[..idx]);
        out.push_str(to);
        rest = &rest[idx + from.len()..];
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------
// kotlin.time.Duration value-class methods (host stdlib)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// kotlin.time.Duration value-class methods (host stdlib)
// ---------------------------------------------------------------------------

pub(crate) fn keiyoushi_duration_minus(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0]) - long_of(vm, args[1])))
}

pub(crate) fn keiyoushi_duration_compare(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(
        long_of(vm, args[0]).cmp(&long_of(vm, args[1])) as i32
    ))
}

pub(crate) fn keiyoushi_duration_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(
        long_of(vm, args[0]) == long_of(vm, args[1]),
    )))
}

pub(crate) fn duration_box(vm: &mut Vm, args: &[JValue]) -> R {
    let raw = long_of(vm, args[0]);
    alloc(vm, "Lkotlin/time/Duration;", Native::Duration(raw))
}

pub(crate) fn duration_unbox(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Duration(raw)) => Ok(JValue::Long(*raw)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn duration_nanos_impl(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0]).saturating_mul(1_000_000)))
}

/// `Duration.getInWholeMilliseconds-impl(J)J`; raw unit is milliseconds.
pub(crate) fn duration_millis_impl(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0])))
}

pub(crate) fn duration_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    let a = match payload(vm, args[0]) {
        Some(Native::Duration(raw)) => *raw,
        _ => return Err(npe(vm)),
    };
    let b = match payload(vm, args[1]) {
        Some(Native::Duration(raw)) => *raw,
        _ => long_of(vm, args[1]),
    };
    Ok(JValue::Int(a.cmp(&b) as i32))
}

pub(crate) fn comparisons_max_of3(vm: &mut Vm, args: &[JValue]) -> R {
    let mut best = args[0];
    for v in [args[1], args[2]] {
        if java_cmp(vm, v, best)? == Ordering::Greater {
            best = v;
        }
    }
    Ok(best)
}

// ---------------------------------------------------------------------------
// kotlin stdlib native table
// ---------------------------------------------------------------------------

pub(crate) const KOTLIN_TABLE: &[NativeEntry] = &[
    ne!("Lkotlin/NoWhenBranchMatchedException;", "<init>", "()V", true, object_noop),
    ne!("Lkotlin/enums/EnumEntriesKt;", "enumEntries", "([Ljava/lang/Enum;)Lkotlin/enums/EnumEntries;", false, enum_entries),
    ne!("Lkotlin/coroutines/jvm/internal/Boxing;", "boxBoolean", "(Z)Ljava/lang/Boolean;", false, boxing_box_boolean),
    ne!("Lkotlin/coroutines/jvm/internal/Boxing;", "boxInt", "(I)Ljava/lang/Integer;", false, boxing_box_int),
    ne!("Lkotlin/Lazy;", "getValue", "()Ljava/lang/Object;", true, lazy_get_value),
    ne!("Lkotlin/LazyKt;", "lazy", "(Lkotlin/jvm/functions/Function0;)Lkotlin/Lazy;", false, lazy_kt_lazy),
    ne!("Lkotlin/LazyKt;", "lazy", "(Lkotlin/LazyThreadSafetyMode;Lkotlin/jvm/functions/Function0;)Lkotlin/Lazy;", false, lazy_kt_lazy_mode),
    ne!("Lkotlin/text/Regex;", "<init>", "(Ljava/lang/String;)V", true, regex_init),
    ne!("Lkotlin/text/Regex;", "<init>", "(Ljava/lang/String;Lkotlin/text/RegexOption;)V", true, regex_init_option),
    ne!("Lkotlin/text/Regex;", "replace", "(Ljava/lang/CharSequence;Ljava/lang/String;)Ljava/lang/String;", true, regex_replace),
    ne!("Lkotlin/text/Regex;", "replace", "(Ljava/lang/CharSequence;Lkotlin/jvm/functions/Function1;)Ljava/lang/String;", true, regex_replace_function),
    ne!("Lkotlin/text/Regex;", "matches", "(Ljava/lang/CharSequence;)Z", true, regex_matches),
    ne!("Lkotlin/text/Regex;", "matchEntire", "(Ljava/lang/CharSequence;)Lkotlin/text/MatchResult;", true, regex_match_entire),
    ne!("Lkotlin/text/Regex;", "containsMatchIn", "(Ljava/lang/CharSequence;)Z", true, regex_contains_match_in),
    ne!("Lkotlin/text/Regex;", "find$default", "(Lkotlin/text/Regex;Ljava/lang/CharSequence;IILjava/lang/Object;)Lkotlin/text/MatchResult;", false, regex_find_default),
    ne!("Lkotlin/text/Regex;", "findAll$default", "(Lkotlin/text/Regex;Ljava/lang/CharSequence;IILjava/lang/Object;)Lkotlin/sequences/Sequence;", false, regex_find_default),
    ne!("Lkotlin/text/Regex;", "split", "(Ljava/lang/CharSequence;I)Ljava/util/List;", true, regex_split),
    ne!("Lkotlin/text/Regex;", "toString", "()Ljava/lang/String;", true, regex_to_string),
    ne!("Lkotlin/text/StringsKt;", "append", "(Ljava/lang/StringBuilder;[Ljava/lang/String;)Ljava/lang/StringBuilder;", false, strings_append_array),
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
    ne!("Lkotlin/text/StringsKt;", "startsWith$default", "(Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Z", false, stringskt_starts_with_default),
    ne!("Lkotlin/collections/CollectionsKt;", "collectionSizeOrDefault", "(Ljava/lang/Iterable;I)I", false, collections_size_or_default),
    ne!("Lkotlin/collections/CollectionsKt;", "joinToString$default", "(Ljava/lang/Iterable;Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;ILjava/lang/CharSequence;Lkotlin/jvm/functions/Function1;ILjava/lang/Object;)Ljava/lang/String;", false, collections_join_to_string_default),
    ne!("Lkotlin/collections/CollectionsKt;", "joinTo$default", "(Ljava/lang/Iterable;Ljava/lang/Appendable;Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;ILjava/lang/CharSequence;Lkotlin/jvm/functions/Function1;ILjava/lang/Object;)Ljava/lang/Appendable;", false, collections_join_to_string_default),
    ne!("Lkotlin/text/StringsKt;", "isBlank", "(Ljava/lang/CharSequence;)Z", false, stringskt_is_blank),
    ne!("Lkotlin/text/StringsKt;", "toIntOrNull", "(Ljava/lang/String;)Ljava/lang/Integer;", false, stringskt_to_int_or_null),
    ne!("Lkotlin/text/StringsKt;", "toFloatOrNull", "(Ljava/lang/String;)Ljava/lang/Float;", false, stringskt_to_float_or_null),
    ne!("Lkotlin/text/StringsKt;", "toIntOrNull", "(Ljava/lang/String;I)Ljava/lang/Integer;", false, stringskt_to_int_radix_or_null),
    ne!("Lkotlin/text/StringsKt;", "toLongOrNull", "(Ljava/lang/String;)Ljava/lang/Long;", false, stringskt_to_long_or_null),
    ne!("Lkotlin/text/StringsKt;", "trim", "(Ljava/lang/String;[C)Ljava/lang/String;", false, stringskt_trim_chars),
    ne!("Lkotlin/text/StringsKt;", "trimEnd", "(Ljava/lang/String;[C)Ljava/lang/String;", false, stringskt_trim_end_chars),
    ne!("Lkotlin/text/StringsKt;", "removeSurrounding", "(Ljava/lang/String;Ljava/lang/CharSequence;)Ljava/lang/String;", false, stringskt_remove_surrounding),
    ne!("Lkotlin/text/StringsKt;", "contains", "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;Z)Z", false, stringskt_contains),
    ne!("Lkotlin/text/StringsKt;", "startsWith", "(Ljava/lang/String;Ljava/lang/String;Z)Z", false, stringskt_starts_with),
    ne!("Lkotlin/text/StringsKt;", "endsWith", "(Ljava/lang/String;Ljava/lang/String;Z)Z", false, stringskt_ends_with),
    ne!("Lkotlin/text/StringsKt;", "endsWith$default", "(Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Z", false, stringskt_ends_with_default),
    ne!("Lkotlin/text/StringsKt;", "removePrefix", "(Ljava/lang/String;Ljava/lang/CharSequence;)Ljava/lang/String;", false, stringskt_remove_prefix),
    ne!("Lkotlin/text/StringsKt;", "removeSuffix", "(Ljava/lang/String;Ljava/lang/CharSequence;)Ljava/lang/String;", false, stringskt_remove_suffix),
    ne!("Lkotlin/text/StringsKt;", "substringBefore$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_before_default),
    ne!("Lkotlin/text/StringsKt;", "split$default", "(Ljava/lang/CharSequence;[Ljava/lang/String;ZIILjava/lang/Object;)Ljava/util/List;", false, stringskt_split_strings_default),
    ne!("Lkotlin/text/StringsKt;", "split$default", "(Ljava/lang/CharSequence;[CZIILjava/lang/Object;)Ljava/util/List;", false, stringskt_split_chars_default),
    ne!("Lkotlin/text/StringsKt;", "substringBeforeLast$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_before_last_default),
    ne!("Lkotlin/text/StringsKt;", "substringAfterLast$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_after_last_default),
    ne!("Lkotlin/text/StringsKt;", "substringAfterLast$default", "(Ljava/lang/String;CLjava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_after_last_char_default),
    ne!("Lkotlin/text/StringsKt;", "lastIndexOf$default", "(Ljava/lang/CharSequence;Ljava/lang/String;IZILjava/lang/Object;)I", false, stringskt_last_index_of_default),
    ne!("Lkotlin/text/StringsKt;", "trimIndent", "(Ljava/lang/String;)Ljava/lang/String;", false, stringskt_trim_indent),
    ne!("Lkotlin/text/StringsKt;", "substringBefore$default", "(Ljava/lang/String;CLjava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_before_char_default),
    ne!("Lkotlin/text/StringsKt;", "equals", "(Ljava/lang/String;Ljava/lang/String;Z)Z", false, stringskt_equals),
    ne!("Lkotlin/text/StringsKt;", "indexOf$default", "(Ljava/lang/CharSequence;CIZILjava/lang/Object;)I", false, stringskt_index_of_char_default),
    ne!("Lkotlin/text/StringsKt;", "indexOf$default", "(Ljava/lang/CharSequence;Ljava/lang/String;IZILjava/lang/Object;)I", false, stringskt_index_of_string_default),
    ne!("Lkotlin/text/CharsKt;", "isWhitespace", "(C)Z", false, charskt_is_whitespace),
    ne!("Lkotlin/text/CharsKt;", "checkRadix", "(I)I", false, charskt_check_radix),
    ne!("Lkotlin/text/CharsKt;", "titlecase", "(CLjava/util/Locale;)Ljava/lang/String;", false, charskt_titlecase),
    ne!("Lkotlin/ranges/RangesKt;", "coerceIn", "(III)I", false, rangeskt_coerce_in),
    ne!("Lkotlin/ranges/RangesKt;", "coerceAtLeast", "(II)I", false, rangeskt_coerce_at_least),
    ne!("Lkotlin/text/MatchResult;", "getValue", "()Ljava/lang/String;", true, match_result_get_value),
    ne!("Lkotlin/text/MatchResult$Destructured;", "toList", "()Ljava/util/List;", true, match_result_destructured_to_list),
    ne!("Lkotlin/text/MatchGroup;", "getValue", "()Ljava/lang/String;", true, match_group_get_value),
    ne!("Lkotlin/jvm/internal/Reflection;", "getOrCreateKotlinClass", "(Ljava/lang/Class;)Lkotlin/reflect/KClass;", false, kotlin_reflection_class),
    ne!("Lkotlin/jvm/internal/Reflection;", "typeOf", "(Ljava/lang/Class;)Lkotlin/reflect/KType;", false, kotlin_reflection_class),
    ne!("Lkotlin/text/MatcherMatchResult;", "getValue", "()Ljava/lang/String;", true, match_result_get_value),
    ne!("Lkotlin/comparisons/ComparisonsKt;", "maxOf", "(Ljava/lang/Comparable;Ljava/lang/Comparable;)Ljava/lang/Comparable;", false, comparisons_max_of),
    ne!("Lkotlin/jvm/internal/DefaultConstructorMarker;", "<init>", "()V", true, object_noop),
    ne!("Lkotlin/jvm/internal/Lambda;", "<init>", "(I)V", true, object_noop),
    ne!("Lkotlin/text/StringsKt;", "contains$default", "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;ZILjava/lang/Object;)Z", false, stringskt_contains_default),
    ne!("Lkotlin/text/StringsKt;", "contains$default", "(Ljava/lang/CharSequence;CZILjava/lang/Object;)Z", false, stringskt_contains_char_default),
    ne!("Lkotlin/random/Random$Default;", "nextInt", "(I)I", false, kotlin_random_default_next_int),
    ne!("Lkotlin/random/Random$Default;", "nextInt", "(II)I", false, kotlin_random_default_next_int),
    ne!("Lkotlin/text/StringsKt;", "substringAfter", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;", false, stringskt_substring_after),
    ne!("Lkotlin/text/StringsKt;", "substringAfter$default", "(Ljava/lang/String;CLjava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_after_char_default),
    ne!("Lkotlin/text/StringsKt;", "take", "(Ljava/lang/String;I)Ljava/lang/String;", false, stringskt_take),
    ne!("Lkotlin/text/StringsKt;", "padStart", "(Ljava/lang/String;IC)Ljava/lang/String;", false, stringskt_pad_start),
    ne!("Lkotlin/text/StringsKt;", "dropLast", "(Ljava/lang/String;I)Ljava/lang/String;", false, stringskt_drop_last),
    ne!("Lkotlin/text/StringsKt;", "replaceFirst$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Ljava/lang/String;", false, stringskt_replace_first_default),
    ne!("Lkotlin/jvm/internal/FunctionReferenceImpl;", "<init>", "(ILjava/lang/Object;Ljava/lang/Class;Ljava/lang/String;Ljava/lang/String;I)V", true, object_noop),
    ne!("Lkotlin/text/StringsKt;", "replace$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Ljava/lang/String;", false, stringskt_replace_default),
    ne!("Lkotlin/text/StringsKt;", "replace$default", "(Ljava/lang/String;CCZILjava/lang/Object;)Ljava/lang/String;", false, stringskt_replace_char_default),
    ne!("Lkotlin/text/StringsKt;", "trimStart", "(Ljava/lang/String;[C)Ljava/lang/String;", false, stringskt_trim_start),
    ne!("Lkotlin/text/StringsKt;", "trimStart", "(Ljava/lang/CharSequence;)Ljava/lang/CharSequence;", false, stringskt_trim_start_charseq),
    ne!("Lkotlin/collections/ArraysKt;", "copyOfRange", "([BII)[B", false, arrayskt_copy_of_range),
    ne!("Lkotlin/collections/ArraysKt;", "toList", "([I)Ljava/util/List;", false, arrayskt_int_to_list),
    ne!("Lkotlin/internal/ProgressionUtilKt;", "getProgressionLastElement", "(III)I", false, progression_last_element),
    ne!("Lkotlin/text/Regex;", "replaceFirst", "(Ljava/lang/CharSequence;Ljava/lang/String;)Ljava/lang/String;", true, regex_replace_first),
    ne!("Lkotlin/text/StringsKt;", "encodeToByteArray", "(Ljava/lang/String;)[B", false, strings_encode_bytes),
    ne!("Lkotlin/coroutines/jvm/internal/SuspendLambda;", "<init>", "(ILkotlin/coroutines/Continuation;)V", true, suspend_lambda_init),
    ne!("Lkotlin/coroutines/jvm/internal/ContinuationImpl;", "<init>", "(Lkotlin/coroutines/Continuation;)V", true, continuation_impl_init),
    ne!("Lkotlin/coroutines/jvm/internal/SpillingKt;", "nullOutSpilledVariable", "(Ljava/lang/Object;)Ljava/lang/Object;", false, null_out_spilled_variable),
    ne!("Lkotlin/coroutines/intrinsics/IntrinsicsKt;", "getCOROUTINE_SUSPENDED", "()Ljava/lang/Object;", false, coroutines_suspended),
    ne!("Lkotlin/ResultKt;", "throwOnFailure", "(Ljava/lang/Object;)V", false, resultkt_throw_on_failure),
    ne!("Lkotlin/UInt;", "constructor-impl", "(I)I", false, uint_constructor_impl),
    ne!("Lkotlin/UByte;", "constructor-impl", "(B)B", false, ubyte_constructor_impl),
    ne!("Lkotlin/io/CloseableKt;", "closeFinally", "(Ljava/io/Closeable;Ljava/lang/Throwable;)V", false, closeablekt_close_finally),
    ne!("Lkotlin/io/TextStreamsKt;", "readText", "(Ljava/io/Reader;)Ljava/lang/String;", false, textstreamskt_read_text),
    ne!("Lkotlin/text/StringsKt;", "substringAfter$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_after_default),
    ne!("Lkotlin/text/StringsKt;", "trim", "(Ljava/lang/CharSequence;)Ljava/lang/CharSequence;", false, stringskt_trim),
    ne!("Lkotlin/comparisons/ComparisonsKt;", "maxOf", "(Ljava/lang/Comparable;Ljava/lang/Comparable;Ljava/lang/Comparable;)Ljava/lang/Comparable;", false, comparisons_max_of3),
    ne!("Lkotlin/comparisons/ComparisonsKt;", "compareValues", "(Ljava/lang/Comparable;Ljava/lang/Comparable;)I", false, comparisons_compare_values),
];

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
