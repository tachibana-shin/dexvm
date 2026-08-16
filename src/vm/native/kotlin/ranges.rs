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

fn progression_bounds(vm: &mut Vm, v: JValue) -> Result<(i32, i32, i32), NatErr> {
    match payload(vm, v) {
        Some(Native::IntProgression(first, last, step)) => Ok((*first, *last, *step)),
        Some(Native::IntRange(first, last)) => Ok((*first, *last, 1)),
        _ => Err(npe(vm)),
    }
}

pub(super) fn rangeskt_down_to(vm: &mut Vm, args: &[JValue]) -> R {
    let first = int_of(vm, args[0]);
    let last = int_of(vm, args[1]);
    alloc(
        vm,
        "Lkotlin/ranges/IntProgression;",
        Native::IntProgression(first, last, -1),
    )
}

pub(super) fn progression_step(vm: &mut Vm, args: &[JValue]) -> R {
    let (first, last, old_step) = progression_bounds(vm, args[0])?;
    let step = int_of(vm, args[1]);
    if step <= 0 {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/IllegalArgumentException;",
            &format!("Step must be positive, was: {step}"),
        )));
    }
    // Real Kotlin `IntProgression.step`: the direction follows the
    // original progression and the last element is recomputed so the
    // stepped progression covers only values reachable by the new step
    // (e.g. `(0 until 20).step(5)` has last 15, not 19).
    let step = if old_step > 0 { step } else { -step };
    let new_last = if step > 0 {
        if first >= last {
            last
        } else {
            last - (last - first).rem_euclid(step)
        }
    } else if first <= last {
        last
    } else {
        last + (first - last).rem_euclid(-step)
    };
    alloc(
        vm,
        "Lkotlin/ranges/IntProgression;",
        Native::IntProgression(first, new_last, step),
    )
}

pub(super) fn progression_reversed(vm: &mut Vm, args: &[JValue]) -> R {
    let (first, last, step) = progression_bounds(vm, args[0])?;
    if step == i32::MIN {
        return Err(NatErr::Throw(
            vm.throwable_of("Ljava/lang/ArithmeticException;", "step overflow"),
        ));
    }
    alloc(
        vm,
        "Lkotlin/ranges/IntProgression;",
        Native::IntProgression(last, first, -step),
    )
}

pub(super) fn progression_get_first(vm: &mut Vm, args: &[JValue]) -> R {
    let (first, _, _) = progression_bounds(vm, args[0])?;
    Ok(JValue::Int(first))
}

pub(super) fn progression_get_last(vm: &mut Vm, args: &[JValue]) -> R {
    let (_, last, _) = progression_bounds(vm, args[0])?;
    Ok(JValue::Int(last))
}

pub(super) fn progression_get_step(vm: &mut Vm, args: &[JValue]) -> R {
    let (_, _, step) = progression_bounds(vm, args[0])?;
    Ok(JValue::Int(step))
}

fn progression_bound_box(vm: &mut Vm, v: JValue, pick_last: bool) -> R {
    let (first, last, _) = progression_bounds(vm, v)?;
    boxed(
        vm,
        "Ljava/lang/Integer;",
        Native::IntBox(if pick_last { last } else { first }),
    )
}

pub(super) fn progression_get_start(vm: &mut Vm, args: &[JValue]) -> R {
    progression_bound_box(vm, args[0], false)
}

pub(super) fn progression_get_end_inclusive(vm: &mut Vm, args: &[JValue]) -> R {
    progression_bound_box(vm, args[0], true)
}

pub(super) fn char_range_init(vm: &mut Vm, args: &[JValue]) -> R {
    let first = int_of(vm, args[1]);
    let last = int_of(vm, args[2]);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::CharRange(dst_first, dst_last) => {
            *dst_first = first;
            *dst_last = last;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(super) fn char_range_get_first(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::CharRange(f, _)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(*f))
}

pub(super) fn char_range_get_last(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::CharRange(_, l)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(*l))
}

pub(super) fn long_range_init(vm: &mut Vm, args: &[JValue]) -> R {
    let first = long_of(vm, args[1]);
    let last = long_of(vm, args[2]);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::LongRange(dst_first, dst_last) => {
            *dst_first = first;
            *dst_last = last;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(super) fn long_range_get_first(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::LongRange(f, _)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Long(*f))
}

pub(super) fn long_range_get_last(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::LongRange(_, l)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Long(*l))
}

pub(super) fn rangeskt_until_long(vm: &mut Vm, args: &[JValue]) -> R {
    let first = long_of(vm, args[0]);
    let last = long_of(vm, args[1]).saturating_sub(1);
    alloc(
        vm,
        "Lkotlin/ranges/LongRange;",
        Native::LongRange(first, last),
    )
}

pub(super) fn rangeskt_random(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::IntRange(first, last)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    if first > last {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/IllegalArgumentException;",
            "empty random range",
        )));
    }
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    Ok(JValue::Int(
        first + (seed % (last - first + 1) as u32) as i32,
    ))
}

fn coerce_at_most(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(args[0].as_int().min(args[1].as_int())))
}
fn coerce_at_least_long(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0]).max(long_of(vm, args[1]))))
}
fn coerce_in_closed_range(vm: &mut Vm, args: &[JValue]) -> R {
    let value = int_of(vm, args[0]);
    let start = vm
        .invoke_virtual(args[1], "getStart", "()Ljava/lang/Comparable;")
        .map_err(nat_fatal)
        .and_then(|v| Ok::<i32, NatErr>(int_of(vm, v)))?;
    let end = vm
        .invoke_virtual(args[1], "getEndInclusive", "()Ljava/lang/Comparable;")
        .map_err(nat_fatal)
        .and_then(|v| Ok::<i32, NatErr>(int_of(vm, v)))?;
    Ok(JValue::Int(value.max(start).min(end)))
}

/// `CharIterator.nextChar()` — cursor is a CharRange payload (mirrors the
/// IntIterator handling).
fn char_iterator_next_char(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::CharRange(f, l)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    if f > l {
        return Err(no_such_elem(vm));
    }
    let v = *f;
    if let Some(Native::CharRange(f2, _)) = payload_mut(vm, args[0]) {
        *f2 += 1;
    }
    Ok(JValue::Int(v))
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
        "coerceIn",
        "(ILkotlin/ranges/ClosedRange;)I",
        false,
        coerce_in_closed_range
    ),
    ne!(
        "Lkotlin/ranges/RangesKt;",
        "coerceAtLeast",
        "(II)I",
        false,
        coerce_at_least
    ),
    ne!(
        "Lkotlin/ranges/RangesKt;",
        "coerceAtLeast",
        "(JJ)J",
        false,
        coerce_at_least_long
    ),
    ne!(
        "Lkotlin/ranges/RangesKt;",
        "coerceAtMost",
        "(II)I",
        false,
        coerce_at_most
    ),
    ne!(
        "Lkotlin/ranges/RangesKt;",
        "downTo",
        "(II)Lkotlin/ranges/IntProgression;",
        false,
        rangeskt_down_to
    ),
    ne!(
        "Lkotlin/ranges/RangesKt;",
        "step",
        "(Lkotlin/ranges/IntProgression;I)Lkotlin/ranges/IntProgression;",
        false,
        progression_step
    ),
    ne!(
        "Lkotlin/ranges/RangesKt;",
        "reversed",
        "(Lkotlin/ranges/IntProgression;)Lkotlin/ranges/IntProgression;",
        false,
        progression_reversed
    ),
    ne!(
        "Lkotlin/ranges/RangesKt;",
        "until",
        "(JJ)Lkotlin/ranges/LongRange;",
        false,
        rangeskt_until_long
    ),
    ne!(
        "Lkotlin/ranges/RangesKt;",
        "random",
        "(Lkotlin/ranges/IntRange;Lkotlin/random/Random;)I",
        false,
        rangeskt_random
    ),
    ne!(
        "Lkotlin/ranges/IntProgression;",
        "getFirst",
        "()I",
        true,
        progression_get_first
    ),
    ne!(
        "Lkotlin/ranges/IntProgression;",
        "getLast",
        "()I",
        true,
        progression_get_last
    ),
    ne!(
        "Lkotlin/ranges/IntProgression;",
        "getStep",
        "()I",
        true,
        progression_get_step
    ),
    ne!(
        "Lkotlin/ranges/IntProgression;",
        "getStart",
        "()Ljava/lang/Comparable;",
        true,
        progression_get_start
    ),
    ne!(
        "Lkotlin/ranges/IntProgression;",
        "getEndInclusive",
        "()Ljava/lang/Comparable;",
        true,
        progression_get_end_inclusive
    ),
    ne!(
        "Lkotlin/ranges/CharRange;",
        "<init>",
        "(CC)V",
        true,
        char_range_init
    ),
    ne!(
        "Lkotlin/ranges/CharRange;",
        "getFirst",
        "()C",
        true,
        char_range_get_first
    ),
    ne!(
        "Lkotlin/ranges/CharRange;",
        "getLast",
        "()C",
        true,
        char_range_get_last
    ),
    ne!(
        "Lkotlin/ranges/LongRange;",
        "<init>",
        "(JJ)V",
        true,
        long_range_init
    ),
    ne!(
        "Lkotlin/ranges/LongRange;",
        "getFirst",
        "()J",
        true,
        long_range_get_first
    ),
    ne!(
        "Lkotlin/ranges/LongRange;",
        "getLast",
        "()J",
        true,
        long_range_get_last
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
    ne!(
        "Lkotlin/collections/CharIterator;",
        "nextChar",
        "()C",
        true,
        char_iterator_next_char
    ),
];
