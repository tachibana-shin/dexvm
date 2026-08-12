//! Kotlin sequence bridge registrations.
use crate::vm::native::*;

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
        if inv_virt(
            vm,
            f,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[value],
        )?
        .as_int()
            != 0
        {
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
];
