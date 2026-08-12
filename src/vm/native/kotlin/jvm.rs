//! Kotlin JVM implementation helpers.
use crate::vm::native::*;

fn spread_init(vm: &mut Vm, args: &[JValue]) -> R {
    vm.arena.objects[args[0].as_obj() as usize].native = Some(Native::List(Vec::new()));
    Ok(JValue::Null)
}
fn spread_add(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::List(values)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    values.push(args[1]);
    Ok(JValue::Null)
}
fn spread_add_all(vm: &mut Vm, args: &[JValue]) -> R {
    let extra = coll_elems(vm, args[1])?;
    let Some(Native::List(values)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    values.extend(extra);
    Ok(JValue::Null)
}
fn spread_size(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::List(values)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(values.len() as i32))
}
fn spread_to_array(vm: &mut Vm, args: &[JValue]) -> R {
    let values = match payload(vm, args[0]) {
        Some(Native::List(values)) => values.clone(),
        _ => return Err(npe(vm)),
    };
    if let Some(Native::Array(ArrayData::Obj(dst))) = payload_mut(vm, args[1]) {
        for (slot, value) in dst.iter_mut().zip(values) {
            *slot = value;
        }
    }
    Ok(args[1])
}
fn ctor_noop(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}
fn null_out(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Lkotlin/jvm/internal/SpreadBuilder;",
        "<init>",
        "(I)V",
        true,
        spread_init
    ),
    ne!(
        "Lkotlin/jvm/internal/SpreadBuilder;",
        "add",
        "(Ljava/lang/Object;)V",
        true,
        spread_add
    ),
    ne!(
        "Lkotlin/jvm/internal/SpreadBuilder;",
        "addSpread",
        "(Ljava/lang/Object;)V",
        true,
        spread_add_all
    ),
    ne!(
        "Lkotlin/jvm/internal/SpreadBuilder;",
        "size",
        "()I",
        true,
        spread_size
    ),
    ne!(
        "Lkotlin/jvm/internal/SpreadBuilder;",
        "toArray",
        "([Ljava/lang/Object;)[Ljava/lang/Object;",
        true,
        spread_to_array
    ),
    ne!(
        "Lkotlin/coroutines/jvm/internal/SuspendLambda;",
        "<init>",
        "(ILkotlin/coroutines/Continuation;)V",
        true,
        ctor_noop
    ),
    ne!(
        "Lkotlin/coroutines/jvm/internal/ContinuationImpl;",
        "<init>",
        "(Lkotlin/coroutines/Continuation;)V",
        true,
        ctor_noop
    ),
    ne!(
        "Lkotlin/coroutines/jvm/internal/SpillingKt;",
        "nullOutSpilledVariable",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        false,
        null_out
    ),
];
