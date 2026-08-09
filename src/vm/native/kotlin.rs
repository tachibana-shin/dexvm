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

pub(crate) fn lazy_duration_unit_millis(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/time/DurationUnit;")
}

pub(crate) fn lazy_unit_instance(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/Unit;")
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
    if f.is_null() {
        return Err(npe(vm));
    }
    inv_virt(vm, f, "invoke", "()Ljava/lang/Object;", &[])
}

// kotlin.time.Duration
// ---------------------------------------------------------------------------

fn unit_millis(vm: &mut Vm, v: JValue) -> Result<i64, NatErr> {
    if v.is_null() {
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
    let re = ::regex::Regex::new(&src).map_err(|e| iae(vm, format!("bad regex {src}: {e}")))?;
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
    Ok(JValue::Int(i32::from(re.is_match(&text))))
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
    collections::list_alloc(vm, items)
}

pub(crate) fn collections_list_of_single(vm: &mut Vm, args: &[JValue]) -> R {
    let items = if args[0].is_null() { Vec::new() } else { vec![args[0]] };
    collections::list_alloc(vm, items)
}

pub(crate) fn kotlin_empty_list(vm: &mut Vm, _args: &[JValue]) -> R {
    collections::list_alloc(vm, Vec::new())
}

pub(crate) fn collections_plus_iterable(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.extend(coll_elems(vm, args[1])?);
    collections::list_alloc(vm, items)
}

pub(crate) fn collections_plus_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let mut items = coll_elems(vm, args[0])?;
    items.push(args[1]);
    collections::list_alloc(vm, items)
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
    let mask = if args.len() > 7 { int_of(vm, args[7]) } else { 0 };
    let has = |bit: i32| (mask >> bit) & 1 == 0;
    let separator = if has(0) { charseq_of(vm, args[1])? } else { ", ".to_string() };
    let prefix = if has(1) { charseq_of(vm, args[2])? } else { String::new() };
    let postfix = if has(2) { charseq_of(vm, args[3])? } else { String::new() };
    let limit = if has(3) { int_of(vm, args[4]) } else { -1 };
    let truncated = if has(4) { charseq_of(vm, args[5])? } else { "...".to_string() };
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
        let s = if transform.is_null() {
            charseq_of(vm, *v)?
        } else {
            let r = inv_virt(vm, transform, "invoke", "(Ljava/lang/Object;)Ljava/lang/Object;", &[*v])?;
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
