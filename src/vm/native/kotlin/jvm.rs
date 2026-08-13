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
fn collection_to_array(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    alloc_arr(vm, "Ljava/lang/Object;", items.len(), move || {
        ArrayData::Obj(items)
    })
}
fn collection_to_array_typed(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[0])?;
    if let Some(Native::Array(ArrayData::Obj(slots))) = payload(vm, args[1]) {
        if slots.len() >= items.len() {
            let mut out = slots.clone();
            for (i, v) in items.iter().enumerate() {
                out[i] = *v;
            }
            if out.len() > items.len() {
                out[items.len()] = JValue::Null;
            }
            if let Some(Native::Array(ArrayData::Obj(d))) = payload_mut(vm, args[1]) {
                *d = out;
            }
            return Ok(args[1]);
        }
        let cls = vm.arena.objects[args[1].as_obj() as usize].class;
        return Ok(JValue::Obj(vm.arena.alloc(
            cls,
            Vec::new(),
            Some(Native::Array(ArrayData::Obj(items))),
        )));
    }
    Err(npe(vm))
}
fn close_finally(vm: &mut Vm, args: &[JValue]) -> R {
    if args[0].is_null() {
        return Ok(JValue::Null);
    }
    let result = vm
        .invoke_virtual_args(args[0], "close", "()V", vec![])
        .map_err(nat_fatal);
    if args[1].is_null() {
        result
    } else {
        Ok(JValue::Null)
    }
}
fn intrinsics_compare(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(args[0].as_int().cmp(&args[1].as_int()) as i32))
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
    ne!(
        "Lkotlin/jvm/internal/MutablePropertyReference1Impl;",
        "<init>",
        "(Ljava/lang/Class;Ljava/lang/String;Ljava/lang/String;I)V",
        true,
        ctor_noop
    ),
    ne!(
        "Lkotlin/jvm/internal/PropertyReference1Impl;",
        "<init>",
        "(Ljava/lang/Class;Ljava/lang/String;Ljava/lang/String;I)V",
        true,
        ctor_noop
    ),
    ne!(
        "Lkotlin/jvm/internal/PropertyReference0Impl;",
        "<init>",
        "(Ljava/lang/Class;Ljava/lang/String;Ljava/lang/String;I)V",
        true,
        ctor_noop
    ),
    ne!(
        "Lkotlin/jvm/internal/FunctionReferenceImpl;",
        "<init>",
        "(ILjava/lang/Class;Ljava/lang/String;Ljava/lang/String;I)V",
        true,
        ctor_noop
    ),
    ne!(
        "Lkotlin/jvm/internal/CollectionToArray;",
        "toArray",
        "(Ljava/util/Collection;)[Ljava/lang/Object;",
        false,
        collection_to_array
    ),
    ne!(
        "Lkotlin/jvm/internal/CollectionToArray;",
        "toArray",
        "(Ljava/util/Collection;[Ljava/lang/Object;)[Ljava/lang/Object;",
        false,
        collection_to_array_typed
    ),
    ne!(
        "Lkotlin/coroutines/jvm/internal/RestrictedSuspendLambda;",
        "<init>",
        "(ILkotlin/coroutines/Continuation;)V",
        true,
        ctor_noop
    ),
    ne!(
        "Lkotlin/jvm/internal/Intrinsics;",
        "compare",
        "(II)I",
        false,
        intrinsics_compare
    ),
    ne!(
        "Lkotlin/jdk7/AutoCloseableKt;",
        "closeFinally",
        "(Ljava/lang/AutoCloseable;Ljava/lang/Throwable;)V",
        false,
        close_finally
    ),
];
