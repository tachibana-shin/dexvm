//! Kotlin ranges and primitive iterator registrations.
use crate::vm::native::*;

pub(super) fn rangeskt_until(vm: &mut Vm, args: &[JValue]) -> R {
    let first = int_of(vm, args[0]);
    let last = int_of(vm, args[1]).saturating_sub(1);
    alloc(
        vm,
        "Lkotlin/ranges/IntRange;",
        Native::IntRange(first, last),
    )
}

pub(super) fn int_range_init(vm: &mut Vm, args: &[JValue]) -> R {
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

pub(super) fn int_range_get_first(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::IntRange(f, _)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(*f))
}

pub(super) fn int_range_get_last(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::IntRange(_, l)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(*l))
}

pub(super) fn int_iterator_init(vm: &mut Vm, args: &[JValue]) -> R {
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

pub(super) fn int_iterator_next_int(vm: &mut Vm, args: &[JValue]) -> R {
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

pub(super) fn int_iterator_has_next(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::IntRange(f, l)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(f <= l)))
}

fn coerce_at_least(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(args[0].as_int().max(args[1].as_int())))
}
fn coerce_in(vm: &mut Vm, args: &[JValue]) -> R {
    let value = int_of(vm, args[0]);
    Ok(JValue::Int(
        value.max(int_of(vm, args[1])).min(int_of(vm, args[2])),
    ))
}
pub(super) fn progression_last_element(_vm: &mut Vm, args: &[JValue]) -> R {
    let (first, last, step) = (args[0].as_int(), args[1].as_int(), args[2].as_int());
    if step == 0 {
        return Ok(JValue::Int(last));
    }
    Ok(JValue::Int(if step > 0 {
        last - (last - first).rem_euclid(step)
    } else {
        last + (first - last).rem_euclid(-step)
    }))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Lkotlin/ranges/RangesKt;",
        "coerceIn",
        "(III)I",
        false,
        coerce_in
    ),
    ne!(
        "Lkotlin/ranges/RangesKt;",
        "coerceAtLeast",
        "(II)I",
        false,
        coerce_at_least
    ),
    ne!(
        "Lkotlin/internal/ProgressionUtilKt;",
        "getProgressionLastElement",
        "(III)I",
        false,
        progression_last_element
    ),
    ne!(
        "Lkotlin/ranges/IntRange;",
        "<init>",
        "(II)V",
        true,
        int_range_init
    ),
    ne!(
        "Lkotlin/ranges/IntRange;",
        "getFirst",
        "()I",
        true,
        int_range_get_first
    ),
    ne!(
        "Lkotlin/ranges/IntRange;",
        "getLast",
        "()I",
        true,
        int_range_get_last
    ),
    ne!(
        "Lkotlin/ranges/RangesKt;",
        "until",
        "(II)Lkotlin/ranges/IntRange;",
        false,
        rangeskt_until
    ),
    ne!(
        "Lkotlin/collections/IntIterator;",
        "<init>",
        "()V",
        true,
        int_iterator_init
    ),
    ne!(
        "Lkotlin/collections/IntIterator;",
        "nextInt",
        "()I",
        true,
        int_iterator_next_int
    ),
    ne!(
        "Lkotlin/collections/IntIterator;",
        "hasNext",
        "()Z",
        true,
        int_iterator_has_next
    ),
];
