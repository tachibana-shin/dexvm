//! Kotlin Pair/Triple/Tuples registrations.
use crate::vm::native::*;

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

pub(crate) fn tripled_to(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlin/Triple;",
        Native::Triple(args[0], args[1], args[2]),
    )
}

fn triple_get(vm: &mut Vm, args: &[JValue], which: u8) -> R {
    match payload(vm, args[0]) {
        Some(Native::Triple(a, b, c)) => Ok(match which {
            0 => *a,
            1 => *b,
            _ => *c,
        }),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn triple_get_first(vm: &mut Vm, args: &[JValue]) -> R {
    triple_get(vm, args, 0)
}
pub(crate) fn triple_get_second(vm: &mut Vm, args: &[JValue]) -> R {
    triple_get(vm, args, 1)
}
pub(crate) fn triple_get_third(vm: &mut Vm, args: &[JValue]) -> R {
    triple_get(vm, args, 2)
}
pub(crate) fn triple_component1(vm: &mut Vm, args: &[JValue]) -> R {
    triple_get_first(vm, args)
}
pub(crate) fn triple_component2(vm: &mut Vm, args: &[JValue]) -> R {
    triple_get_second(vm, args)
}
pub(crate) fn triple_component3(vm: &mut Vm, args: &[JValue]) -> R {
    triple_get_third(vm, args)
}
pub(crate) fn triple_init(vm: &mut Vm, args: &[JValue]) -> R {
    vm.arena.objects[args[0].as_obj() as usize].native =
        Some(Native::Triple(args[1], args[2], args[3]));
    Ok(JValue::Null)
}
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Lkotlin/Pair;",
        "getFirst",
        "()Ljava/lang/Object;",
        true,
        pair_get_first
    ),
    ne!(
        "Lkotlin/Pair;",
        "getSecond",
        "()Ljava/lang/Object;",
        true,
        pair_get_second
    ),
    ne!(
        "Lkotlin/Pair;",
        "component1",
        "()Ljava/lang/Object;",
        true,
        pair_get_first
    ),
    ne!(
        "Lkotlin/Pair;",
        "component2",
        "()Ljava/lang/Object;",
        true,
        pair_get_second
    ),
    ne!(
        "Lkotlin/Pair;",
        "<init>",
        "(Ljava/lang/Object;Ljava/lang/Object;)V",
        true,
        pair_init
    ),
    ne!(
        "Lkotlin/TuplesKt;",
        "to",
        "(Ljava/lang/Object;Ljava/lang/Object;)Lkotlin/Pair;",
        false,
        tupled_to
    ),
    ne!(
        "Lkotlin/TuplesKt;",
        "to",
        "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)Lkotlin/Triple;",
        false,
        tripled_to
    ),
    ne!(
        "Lkotlin/Triple;",
        "getFirst",
        "()Ljava/lang/Object;",
        true,
        triple_get_first
    ),
    ne!(
        "Lkotlin/Triple;",
        "getSecond",
        "()Ljava/lang/Object;",
        true,
        triple_get_second
    ),
    ne!(
        "Lkotlin/Triple;",
        "getThird",
        "()Ljava/lang/Object;",
        true,
        triple_get_third
    ),
    ne!(
        "Lkotlin/Triple;",
        "component1",
        "()Ljava/lang/Object;",
        true,
        triple_component1
    ),
    ne!(
        "Lkotlin/Triple;",
        "component2",
        "()Ljava/lang/Object;",
        true,
        triple_component2
    ),
    ne!(
        "Lkotlin/Triple;",
        "component3",
        "()Ljava/lang/Object;",
        true,
        triple_component3
    ),
    ne!(
        "Lkotlin/Triple;",
        "<init>",
        "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)V",
        true,
        triple_init
    ),
];
