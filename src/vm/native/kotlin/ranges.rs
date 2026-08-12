//! Kotlin ranges and primitive iterator registrations.
use crate::vm::native::*;

pub(crate) fn rangeskt_until(vm: &mut Vm, args: &[JValue]) -> R {
    let first = int_of(vm, args[0]);
    let last = int_of(vm, args[1]).saturating_sub(1);
    alloc(
        vm,
        "Lkotlin/ranges/IntRange;",
        Native::IntRange(first, last),
    )
}

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

pub(crate) const TABLE: &[NativeEntry] = &[
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
