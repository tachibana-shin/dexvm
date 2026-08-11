//! Kotlin stdlib host shims. Duration raw encoding is milliseconds so that
//! both `getInWholeSeconds` (raw / 1000) and `getInWholeMilliseconds` (raw)
//! round-trip through `toDuration`.

use super::*;

// lazy static materializers
// ---------------------------------------------------------------------------

pub(crate) fn opaque_inst(vm: &mut Vm, desc: &str) -> JValue {
    let class = vm.ensure_class_by_desc(desc).expect("kotlin shim");
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

// kotlin.time.Duration
// ---------------------------------------------------------------------------

fn unit_millis(vm: &mut Vm, v: JValue) -> Result<i64, NatErr> {
    if v.is_null_ref() {
        return Ok(1000);
    }
    let desc = vm.class_desc_str(obj_class(vm, v.as_obj()));
    if desc.starts_with("Lkotlin/time/DurationUnit;") {
        Ok(1000)
    } else {
        Ok(1)
    }
}

pub(crate) fn duration_get_zero(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Long(0))
}

pub(crate) fn duration_to_duration_int(vm: &mut Vm, args: &[JValue]) -> R {
    let v = int_of(vm, args[0]);
    let ms = unit_millis(vm, args[1])?;
    Ok(JValue::Long(v as i64 * ms))
}

pub(crate) fn duration_to_duration_long(vm: &mut Vm, args: &[JValue]) -> R {
    let v = long_of(vm, args[0]);
    let ms = unit_millis(vm, args[1])?;
    Ok(JValue::Long(v * ms))
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

pub(crate) fn regex_replace(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let repl = jstr(vm, args[2])?;
    Ok(new_str(vm, &re.replace_all(&text, repl.as_str())))
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

pub(crate) fn collections_plus_iterable(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.extend(coll_elems(vm, args[1])?);
    list_alloc(vm, items)
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

// kotlin.jvm.internal.Intrinsics
// ---------------------------------------------------------------------------

pub(crate) fn intrinsics_are_equal(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(java_equals(vm, args[0], args[1])?)))
}

pub(crate) fn intrinsics_check_not_null_parameter(vm: &mut Vm, args: &[JValue]) -> R {
    if args[0].is_null_ref() {
        let name = jstr(vm, args[1]).unwrap_or_else(|_| "parameter".into());
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/NullPointerException;",
            format!("{name} must not be null"),
        )));
    }
    Ok(JValue::Null)
}

// kotlin.Pair / TuplesKt
// ---------------------------------------------------------------------------

pub(crate) fn tupled_to(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Lkotlin/Pair;", Native::Pair(args[0], args[1]))
}

pub(crate) fn pair_get_first(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Pair(a, _)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(*a)
}

pub(crate) fn pair_get_second(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Pair(_, b)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(*b)
}

pub(crate) fn pair_init(vm: &mut Vm, args: &[JValue]) -> R {
    let this = args[0].as_obj();
    vm.arena.objects[this as usize].native = Some(Native::Pair(args[1], args[2]));
    Ok(JValue::Null)
}

// kotlin.Result (inline class) and ResultKt
// ---------------------------------------------------------------------------

/// `Result.constructor-impl` — identity packaging of a non-failure value.
pub(crate) fn result_constructor_impl(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

/// `Result.isFailure-impl` — true only for `createFailure` markers.
pub(crate) fn result_is_failure_impl(vm: &mut Vm, args: &[JValue]) -> R {
    let failure = matches!(payload(vm, args[0]), Some(Native::ResultFailure(_)));
    Ok(JValue::Int(i32::from(failure)))
}

/// `ResultKt.createFailure(Throwable)` — wraps a throwable in a distinct
/// marker object (the real runtime uses an alias bit on the payload).
pub(crate) fn resultkt_create_failure(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlin/Result$Failure;",
        Native::ResultFailure(args[0]),
    )
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

// java.net.URI
// ---------------------------------------------------------------------------

fn uri_init(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[1])?;
    let obj = args[0].as_obj();
    vm.arena.objects[obj as usize].native = Some(Native::URI(s));
    Ok(JValue::Null)
}

fn uri_get_host(vm: &mut Vm, args: &[JValue]) -> R {
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

// kotlin.ranges.IntRange
// ---------------------------------------------------------------------------

pub(crate) fn int_range_init(vm: &mut Vm, args: &[JValue]) -> R {
    let first = int_of(vm, args[1]);
    let last = int_of(vm, args[2]);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::IntRange(dst_first, dst_last) => {
            *dst_first = first;
            *dst_last = last;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn int_range_get_first(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::IntRange(f, _)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(*f))
}

pub(crate) fn int_range_get_last(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::IntRange(_, l)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(*l))
}

// kotlin.collections.IntIterator
// ---------------------------------------------------------------------------

pub(crate) fn int_iterator_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::IntRange(f, l) => {
            *f = 0;
            *l = 0;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn int_iterator_next_int(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::IntRange(f, l)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    if f > l {
        return Err(no_such_elem(vm));
    }
    let v = *f;
    if let Some(Native::IntRange(f2, _)) = payload_mut(vm, args[0]) {
        *f2 += 1;
    }
    Ok(JValue::Int(v))
}

pub(crate) fn int_iterator_has_next(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::IntRange(f, l)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(f <= l)))
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

// kotlinx.coroutines compatibility: the VM is single-threaded, so launched
// work runs to completion synchronously while preserving the observable API.
fn coroutines_global_scope(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lkotlinx/coroutines/GlobalScope;", Native::Opaque)
}
fn coroutines_dispatchers_io(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlinx/coroutines/CoroutineDispatcher;",
        Native::Opaque,
    )
}
fn coroutines_launch_default(vm: &mut Vm, args: &[JValue]) -> R {
    let scope = args[0];
    let block = args[3];
    let _ = vm.invoke_virtual_args(
        block,
        "invoke",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        vec![scope, JValue::Null],
    );
    // A launched coroutine reports failure to its Job/exception handler; it
    // never throws synchronously into the caller of launch().
    alloc(vm, "Lkotlinx/coroutines/Job;", Native::Opaque)
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

// kotlin.UInt / UByte `constructor-impl`: identity (already raw ints).
// Static: the value arrives as the only argument.
fn uint_constructor_impl(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

fn ubyte_constructor_impl(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
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
    ne!("Lkotlin/Lazy;", "getValue", "()Ljava/lang/Object;", true, lazy_get_value),
    ne!("Lkotlin/LazyKt;", "lazy", "(Lkotlin/jvm/functions/Function0;)Lkotlin/Lazy;", false, lazy_kt_lazy),
    ne!("Lkotlin/LazyKt;", "lazy", "(Lkotlin/LazyThreadSafetyMode;Lkotlin/jvm/functions/Function0;)Lkotlin/Lazy;", false, lazy_kt_lazy_mode),
    ne!("Lkotlin/time/Duration$Companion;", "getZERO-UwyO8pc", "()J", true, duration_get_zero),
    ne!("Lkotlin/time/DurationKt;", "toDuration", "(ILkotlin/time/DurationUnit;)J", false, duration_to_duration_int),
    ne!("Lkotlin/time/DurationKt;", "toDuration", "(JLkotlin/time/DurationUnit;)J", false, duration_to_duration_long),
    ne!("Lkotlin/text/Regex;", "<init>", "(Ljava/lang/String;)V", true, regex_init),
    ne!("Lkotlin/text/Regex;", "replace", "(Ljava/lang/CharSequence;Ljava/lang/String;)Ljava/lang/String;", true, regex_replace),
    ne!("Lkotlin/text/Regex;", "matches", "(Ljava/lang/CharSequence;)Z", true, regex_matches),
    ne!("Lkotlin/text/Regex;", "containsMatchIn", "(Ljava/lang/CharSequence;)Z", true, regex_contains_match_in),
    ne!("Lkotlin/text/Regex;", "find$default", "(Lkotlin/text/Regex;Ljava/lang/CharSequence;IILjava/lang/Object;)Lkotlin/text/MatchResult;", false, regex_find_default),
    ne!("Lkotlin/text/Regex;", "split", "(Ljava/lang/CharSequence;I)Ljava/util/List;", true, regex_split),
    ne!("Lkotlin/text/Regex;", "toString", "()Ljava/lang/String;", true, regex_to_string),
    ne!("Lkotlin/text/StringsKt;", "append", "(Ljava/lang/StringBuilder;[Ljava/lang/String;)Ljava/lang/StringBuilder;", false, strings_append_array),
    ne!("Lkotlin/collections/CollectionsKt;", "listOf", "([Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_array),
    ne!("Lkotlin/collections/CollectionsKt;", "listOf", "(Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_single),
    ne!("Lkotlin/collections/CollectionsKt;", "mutableListOf", "([Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_array),
    ne!("Lkotlin/collections/CollectionsKt;", "emptyList", "()Ljava/util/List;", false, kotlin_empty_list),
    ne!("Lkotlin/collections/CollectionsKt;", "createListBuilder", "()Ljava/util/List;", false, kotlin_empty_list),
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
    ne!("Lkotlin/collections/SetsKt;", "setOf", "([Ljava/lang/Object;)Ljava/util/Set;", false, setskt_set_of),
    ne!("Lkotlin/collections/MapsKt;", "mapOf", "([Lkotlin/Pair;)Ljava/util/Map;", false, mapskt_map_of),
    ne!("Lkotlin/collections/MapsKt;", "toList", "(Ljava/util/Map;)Ljava/util/List;", false, mapskt_to_list),
    ne!("Lkotlin/collections/ArraysKt;", "plus", "([B[B)[B", false, arrayskt_plus_bytes),
    ne!("Lkotlin/collections/CollectionsKt;", "reversed", "(Ljava/lang/Iterable;)Ljava/util/List;", false, collections_reversed),
    ne!("Lkotlin/text/StringsKt;", "startsWith$default", "(Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Z", false, stringskt_starts_with_default),
    ne!("Lkotlin/collections/CollectionsKt;", "collectionSizeOrDefault", "(Ljava/lang/Iterable;I)I", false, collections_size_or_default),
    ne!("Lkotlin/collections/CollectionsKt;", "joinToString$default", "(Ljava/lang/Iterable;Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;ILjava/lang/CharSequence;Lkotlin/jvm/functions/Function1;ILjava/lang/Object;)Ljava/lang/String;", false, collections_join_to_string_default),
    ne!("Lkotlin/jvm/internal/Intrinsics;", "areEqual", "(Ljava/lang/Object;Ljava/lang/Object;)Z", false, intrinsics_are_equal),
    ne!("Lkotlin/jvm/internal/Intrinsics;", "checkNotNullParameter", "(Ljava/lang/Object;Ljava/lang/String;)V", false, intrinsics_check_not_null_parameter),
    ne!("Lkotlin/Pair;", "getFirst", "()Ljava/lang/Object;", true, pair_get_first),
    ne!("Lkotlin/Pair;", "getSecond", "()Ljava/lang/Object;", true, pair_get_second),
    ne!("Lkotlin/Pair;", "component1", "()Ljava/lang/Object;", true, pair_get_first),
    ne!("Lkotlin/Pair;", "component2", "()Ljava/lang/Object;", true, pair_get_second),
    ne!("Lkotlin/Pair;", "<init>", "(Ljava/lang/Object;Ljava/lang/Object;)V", true, pair_init),
    ne!("Lkotlin/TuplesKt;", "to", "(Ljava/lang/Object;Ljava/lang/Object;)Lkotlin/Pair;", false, tupled_to),
    ne!("Lkotlin/Result;", "constructor-impl", "(Ljava/lang/Object;)Ljava/lang/Object;", false, result_constructor_impl),
    ne!("Lkotlin/Result;", "isFailure-impl", "(Ljava/lang/Object;)Z", false, result_is_failure_impl),
    ne!("Lkotlin/ResultKt;", "createFailure", "(Ljava/lang/Throwable;)Ljava/lang/Object;", false, resultkt_create_failure),
    ne!("Lkotlin/text/StringsKt;", "isBlank", "(Ljava/lang/CharSequence;)Z", false, stringskt_is_blank),
    ne!("Lkotlin/text/StringsKt;", "toIntOrNull", "(Ljava/lang/String;)Ljava/lang/Integer;", false, stringskt_to_int_or_null),
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
    ne!("Lkotlin/text/CharsKt;", "isWhitespace", "(C)Z", false, charskt_is_whitespace),
    ne!("Lkotlin/text/CharsKt;", "titlecase", "(CLjava/util/Locale;)Ljava/lang/String;", false, charskt_titlecase),
    ne!("Lkotlin/ranges/RangesKt;", "coerceIn", "(III)I", false, rangeskt_coerce_in),
    ne!("Lkotlin/ranges/RangesKt;", "coerceAtLeast", "(II)I", false, rangeskt_coerce_at_least),
    ne!("Lkotlin/text/MatchResult;", "getValue", "()Ljava/lang/String;", true, match_result_get_value),
    ne!("Lkotlin/text/MatcherMatchResult;", "getValue", "()Ljava/lang/String;", true, match_result_get_value),
    ne!("Ljava/net/URI;", "<init>", "(Ljava/lang/String;)V", true, uri_init),
    ne!("Ljava/net/URI;", "getHost", "()Ljava/lang/String;", true, uri_get_host),
    ne!("Lkotlin/ranges/IntRange;", "<init>", "(II)V", true, int_range_init),
    ne!("Lkotlin/ranges/IntRange;", "getFirst", "()I", true, int_range_get_first),
    ne!("Lkotlin/ranges/IntRange;", "getLast", "()I", true, int_range_get_last),
    ne!("Lkotlin/collections/IntIterator;", "<init>", "()V", true, int_iterator_init),
    ne!("Lkotlin/collections/IntIterator;", "nextInt", "()I", true, int_iterator_next_int),
    ne!("Lkotlin/collections/IntIterator;", "hasNext", "()Z", true, int_iterator_has_next),
    ne!("Lkotlin/comparisons/ComparisonsKt;", "maxOf", "(Ljava/lang/Comparable;Ljava/lang/Comparable;)Ljava/lang/Comparable;", false, comparisons_max_of),
    ne!("Lkotlin/jvm/internal/DefaultConstructorMarker;", "<init>", "()V", true, object_noop),
    ne!("Lkotlin/jvm/internal/Lambda;", "<init>", "(I)V", true, object_noop),
    ne!("Lkotlin/text/StringsKt;", "contains$default", "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;ZILjava/lang/Object;)Z", false, stringskt_contains_default),
    ne!("Lkotlin/text/StringsKt;", "replace$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Ljava/lang/String;", false, stringskt_replace_default),
    ne!("Lkotlin/text/StringsKt;", "replace$default", "(Ljava/lang/String;CCZILjava/lang/Object;)Ljava/lang/String;", false, stringskt_replace_char_default),
    ne!("Lkotlin/text/StringsKt;", "trimStart", "(Ljava/lang/String;[C)Ljava/lang/String;", false, stringskt_trim_start),
    ne!("Lkotlin/collections/ArraysKt;", "copyOfRange", "([BII)[B", false, arrayskt_copy_of_range),
    ne!("Lkotlinx/coroutines/GlobalScope;", "getInstance", "()Lkotlinx/coroutines/GlobalScope;", false, coroutines_global_scope),
    ne!("Lkotlinx/coroutines/Dispatchers;", "getIO", "()Lkotlinx/coroutines/CoroutineDispatcher;", false, coroutines_dispatchers_io),
    ne!("Lkotlinx/coroutines/BuildersKt;", "launch$default", "(Lkotlinx/coroutines/CoroutineScope;Lkotlin/coroutines/CoroutineContext;Lkotlinx/coroutines/CoroutineStart;Lkotlin/jvm/functions/Function2;ILjava/lang/Object;)Lkotlinx/coroutines/Job;", false, coroutines_launch_default),
    ne!("Lkotlin/coroutines/jvm/internal/SuspendLambda;", "<init>", "(ILkotlin/coroutines/Continuation;)V", true, suspend_lambda_init),
    ne!("Lkotlin/coroutines/jvm/internal/ContinuationImpl;", "<init>", "(Lkotlin/coroutines/Continuation;)V", true, continuation_impl_init),
    ne!("Lkotlin/coroutines/jvm/internal/SpillingKt;", "nullOutSpilledVariable", "(Ljava/lang/Object;)Ljava/lang/Object;", false, null_out_spilled_variable),
    ne!("Lkotlin/coroutines/intrinsics/IntrinsicsKt;", "getCOROUTINE_SUSPENDED", "()Ljava/lang/Object;", false, coroutines_suspended),
    ne!("Lkotlin/ResultKt;", "throwOnFailure", "(Ljava/lang/Object;)V", false, resultkt_throw_on_failure),
    ne!("Lkotlin/UInt;", "constructor-impl", "(I)I", false, uint_constructor_impl),
    ne!("Lkotlin/UByte;", "constructor-impl", "(B)B", false, ubyte_constructor_impl),
    ne!("Lkotlin/io/CloseableKt;", "closeFinally", "(Ljava/io/Closeable;Ljava/lang/Throwable;)V", false, closeablekt_close_finally),
    ne!("Lkotlin/text/StringsKt;", "substringAfter$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_after_default),
    ne!("Lkotlin/text/StringsKt;", "trim", "(Ljava/lang/CharSequence;)Ljava/lang/CharSequence;", false, stringskt_trim),
    ne!("Lkotlin/time/Duration;", "minus-LRDsOJo", "(JJ)J", false, keiyoushi_duration_minus),
    ne!("Lkotlin/time/Duration;", "compareTo-LRDsOJo", "(JJ)I", false, keiyoushi_duration_compare),
    ne!("Lkotlin/time/Duration;", "equals-impl0", "(JJ)Z", false, keiyoushi_duration_equals),
    ne!("Lkotlin/time/Duration;", "box-impl", "(J)Lkotlin/time/Duration;", false, duration_box),
    ne!("Lkotlin/time/Duration;", "unbox-impl", "()J", true, duration_unbox),
    ne!("Lkotlin/time/Duration;", "getInWholeNanoseconds-impl", "(J)J", false, duration_nanos_impl),
    ne!("Lkotlin/time/Duration;", "getInWholeMilliseconds-impl", "(J)J", false, duration_millis_impl),
    ne!("Lkotlin/time/Duration;", "compareTo", "(Ljava/lang/Object;)I", true, duration_compare_to),
    ne!("Lkotlin/comparisons/ComparisonsKt;", "maxOf", "(Ljava/lang/Comparable;Ljava/lang/Comparable;Ljava/lang/Comparable;)Ljava/lang/Comparable;", false, comparisons_max_of3),
    ne!("Lkotlin/comparisons/ComparisonsKt;", "compareValues", "(Ljava/lang/Comparable;Ljava/lang/Comparable;)I", false, comparisons_compare_values),
];

#[cfg(test)]
mod tests;
