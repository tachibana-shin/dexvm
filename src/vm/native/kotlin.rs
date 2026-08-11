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

// Lazy / LazyKt
// ---------------------------------------------------------------------------

pub(crate) fn lazy_kt_lazy(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Lkotlin/SynchronizedLazyImpl;", Native::Lazy(args[0]))
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

// kotlinx.coroutines stubs: the extension fires an async cache-writer via
// GlobalScope.launch(Dispatchers.IO) and ignores the returned Job; none of
// it must run in the VM.
fn coroutines_global_scope(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lkotlinx/coroutines/GlobalScope;", Native::Opaque)
}
fn coroutines_dispatchers_io(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}
fn coroutines_launch_default(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

fn suspend_lambda_init(_vm: &mut Vm, _args: &[JValue]) -> R {
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

// kotlin.io.CloseableKt.closeFinally(source, cause) — no-op; the VM closes
// nothing on the host side.
fn closeablekt_close_finally(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
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
    alloc(
        vm,
        "Lkotlin/time/Duration;",
        Native::Duration(raw),
    )
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
    ne!("Lkotlin/time/Duration$Companion;", "getZERO-UwyO8pc", "()J", true, duration_get_zero),
    ne!("Lkotlin/time/DurationKt;", "toDuration", "(ILkotlin/time/DurationUnit;)J", false, duration_to_duration_int),
    ne!("Lkotlin/time/DurationKt;", "toDuration", "(JLkotlin/time/DurationUnit;)J", false, duration_to_duration_long),
    ne!("Lkotlin/text/Regex;", "<init>", "(Ljava/lang/String;)V", true, regex_init),
    ne!("Lkotlin/text/Regex;", "replace", "(Ljava/lang/CharSequence;Ljava/lang/String;)Ljava/lang/String;", true, regex_replace),
    ne!("Lkotlin/text/Regex;", "matches", "(Ljava/lang/CharSequence;)Z", true, regex_matches),
    ne!("Lkotlin/text/Regex;", "toString", "()Ljava/lang/String;", true, regex_to_string),
    ne!("Lkotlin/text/StringsKt;", "append", "(Ljava/lang/StringBuilder;[Ljava/lang/String;)Ljava/lang/StringBuilder;", false, strings_append_array),
    ne!("Lkotlin/collections/CollectionsKt;", "listOf", "([Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_array),
    ne!("Lkotlin/collections/CollectionsKt;", "listOf", "(Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_single),
    ne!("Lkotlin/collections/CollectionsKt;", "mutableListOf", "([Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_array),
    ne!("Lkotlin/collections/CollectionsKt;", "emptyList", "()Ljava/util/List;", false, kotlin_empty_list),
    ne!("Lkotlin/collections/CollectionsKt;", "createListBuilder", "()Ljava/util/List;", false, kotlin_empty_list),
    ne!("Lkotlin/collections/CollectionsKt;", "build", "(Ljava/util/List;)Ljava/util/List;", true, kotlin_list_identity),
    ne!("Lkotlin/collections/CollectionsKt;", "plus", "(Ljava/util/Collection;Ljava/lang/Iterable;)Ljava/util/List;", false, collections_plus_iterable),
    ne!("Lkotlin/collections/CollectionsKt;", "plus", "(Ljava/util/Collection;Ljava/lang/Object;)Ljava/util/List;", false, collections_plus_obj),
    ne!("Lkotlin/collections/CollectionsKt;", "contains", "(Ljava/lang/Iterable;Ljava/lang/Object;)Z", false, collections_contains),
    ne!("Lkotlin/collections/CollectionsKt;", "first", "(Ljava/lang/Iterable;)Ljava/lang/Object;", false, collections_first),
    ne!("Lkotlin/collections/CollectionsKt;", "reversed", "(Ljava/lang/Iterable;)Ljava/util/List;", false, collections_reversed),
    ne!("Lkotlin/text/StringsKt;", "startsWith$default", "(Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Z", false, stringskt_starts_with_default),
    ne!("Lkotlin/collections/CollectionsKt;", "collectionSizeOrDefault", "(Ljava/lang/Iterable;I)I", false, collections_size_or_default),
    ne!("Lkotlin/collections/CollectionsKt;", "joinToString$default", "(Ljava/lang/Iterable;Ljava/lang/CharSequence;Ljava/lang/CharSequence;Ljava/lang/CharSequence;ILjava/lang/CharSequence;Lkotlin/jvm/functions/Function1;ILjava/lang/Object;)Ljava/lang/String;", false, collections_join_to_string_default),
    ne!("Lkotlin/jvm/internal/Intrinsics;", "areEqual", "(Ljava/lang/Object;Ljava/lang/Object;)Z", false, intrinsics_are_equal),
    ne!("Lkotlin/Pair;", "getFirst", "()Ljava/lang/Object;", true, pair_get_first),
    ne!("Lkotlin/Pair;", "getSecond", "()Ljava/lang/Object;", true, pair_get_second),
    ne!("Lkotlin/Pair;", "<init>", "(Ljava/lang/Object;Ljava/lang/Object;)V", true, pair_init),
    ne!("Lkotlin/TuplesKt;", "to", "(Ljava/lang/Object;Ljava/lang/Object;)Lkotlin/Pair;", false, tupled_to),
    ne!("Lkotlin/ranges/IntRange;", "<init>", "(II)V", true, int_range_init),
    ne!("Lkotlin/ranges/IntRange;", "getFirst", "()I", true, int_range_get_first),
    ne!("Lkotlin/ranges/IntRange;", "getLast", "()I", true, int_range_get_last),
    ne!("Lkotlin/collections/IntIterator;", "<init>", "()V", true, int_iterator_init),
    ne!("Lkotlin/collections/IntIterator;", "nextInt", "()I", true, int_iterator_next_int),
    ne!("Lkotlin/collections/IntIterator;", "hasNext", "()Z", true, int_iterator_has_next),
    ne!("Lkotlin/comparisons/ComparisonsKt;", "maxOf", "(Ljava/lang/Comparable;Ljava/lang/Comparable;)Ljava/lang/Comparable;", false, comparisons_max_of),
    ne!("Lkotlin/jvm/internal/DefaultConstructorMarker;", "<init>", "()V", true, object_noop),
    ne!("Lkotlin/text/StringsKt;", "contains$default", "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;ZILjava/lang/Object;)Z", true, stringskt_contains_default),
    ne!("Lkotlin/text/StringsKt;", "replace$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Ljava/lang/String;", true, stringskt_replace_default),
    ne!("Lkotlin/text/StringsKt;", "replace$default", "(Ljava/lang/String;CCZILjava/lang/Object;)Ljava/lang/String;", true, stringskt_replace_char_default),
    ne!("Lkotlin/text/StringsKt;", "trimStart", "(Ljava/lang/String;[C)Ljava/lang/String;", true, stringskt_trim_start),
    ne!("Lkotlin/collections/ArraysKt;", "copyOfRange", "([BII)[B", true, arrayskt_copy_of_range),
    ne!("Lkotlinx/coroutines/GlobalScope;", "getInstance", "()Lkotlinx/coroutines/GlobalScope;", true, coroutines_global_scope),
    ne!("Lkotlinx/coroutines/Dispatchers;", "getIO", "()Lkotlinx/coroutines/CoroutineDispatcher;", true, coroutines_dispatchers_io),
    ne!("Lkotlinx/coroutines/BuildersKt;", "launch$default", "(Lkotlinx/coroutines/CoroutineScope;Lkotlin/coroutines/CoroutineContext;Lkotlinx/coroutines/CoroutineStart;Lkotlin/jvm/functions/Function2;ILjava/lang/Object;)Lkotlinx/coroutines/Job;", false, coroutines_launch_default),
    ne!("Lkotlin/coroutines/jvm/internal/SuspendLambda;", "<init>", "(ILkotlin/coroutines/Continuation;)V", true, suspend_lambda_init),
    ne!("Lkotlin/UInt;", "constructor-impl", "(I)I", false, uint_constructor_impl),
    ne!("Lkotlin/UByte;", "constructor-impl", "(B)B", false, ubyte_constructor_impl),
    ne!("Lkotlin/io/CloseableKt;", "closeFinally", "(Ljava/io/Closeable;Ljava/lang/Throwable;)V", false, closeablekt_close_finally),
    ne!("Lkotlin/text/StringsKt;", "substringAfter$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", true, stringskt_substring_after_default),
    ne!("Lkotlin/text/StringsKt;", "trim", "(Ljava/lang/CharSequence;)Ljava/lang/CharSequence;", true, stringskt_trim),
    ne!("Lkotlin/time/Duration;", "minus-LRDsOJo", "(JJ)J", false, keiyoushi_duration_minus),
    ne!("Lkotlin/time/Duration;", "compareTo-LRDsOJo", "(JJ)I", false, keiyoushi_duration_compare),
    ne!("Lkotlin/time/Duration;", "equals-impl0", "(JJ)Z", false, keiyoushi_duration_equals),
    ne!("Lkotlin/time/Duration;", "box-impl", "(J)Lkotlin/time/Duration;", false, duration_box),
    ne!("Lkotlin/time/Duration;", "unbox-impl", "()J", true, duration_unbox),
    ne!("Lkotlin/time/Duration;", "getInWholeNanoseconds-impl", "(J)J", false, duration_nanos_impl),
    ne!("Lkotlin/time/Duration;", "getInWholeMilliseconds-impl", "(J)J", false, duration_millis_impl),
    ne!("Lkotlin/time/Duration;", "compareTo", "(Ljava/lang/Object;)I", true, duration_compare_to),
    ne!("Lkotlin/comparisons/ComparisonsKt;", "maxOf", "(Ljava/lang/Comparable;Ljava/lang/Comparable;Ljava/lang/Comparable;)Ljava/lang/Comparable;", false, comparisons_max_of3),
];

#[cfg(test)]
mod tests;
