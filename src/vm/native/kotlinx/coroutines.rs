//! kotlinx.coroutines host shims.

use crate::vm::native::*;

pub(crate) fn coroutine_scope_create(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lkotlinx/coroutines/CoroutineScope;", Native::Opaque)
}
pub(crate) fn coroutines_global_scope(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Lkotlinx/coroutines/GlobalScope;", Native::Opaque)
}
pub(crate) fn coroutines_dispatchers_io(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlinx/coroutines/CoroutineDispatcher;",
        Native::Opaque,
    )
}
pub(crate) fn mutex_default(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlinx/coroutines/sync/Mutex;",
        Native::Mutex { locked: false },
    )
}
pub(crate) fn mutex_lock(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Mutex { locked }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *locked = true;
    Ok(JValue::Null)
}
pub(crate) fn mutex_try_lock(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Mutex { locked }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if *locked {
        return Ok(JValue::Int(0));
    }
    *locked = true;
    Ok(JValue::Int(1))
}
pub(crate) fn mutex_unlock(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Mutex { locked }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if !*locked {
        return Err(iae(vm, "Mutex is not locked"));
    }
    *locked = false;
    Ok(JValue::Null)
}
pub(crate) fn mutex_is_locked(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Mutex { locked }) => Ok(JValue::Int(i32::from(*locked))),
        _ => Err(npe(vm)),
    }
}
pub(crate) fn coroutines_launch_default(vm: &mut Vm, args: &[JValue]) -> R {
    let _ = vm.invoke_virtual_args(
        args[3],
        "invoke",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        vec![args[0], JValue::Null],
    );
    alloc(vm, "Lkotlinx/coroutines/Job;", Native::Opaque)
}
pub(crate) fn coroutines_scope(vm: &mut Vm, args: &[JValue]) -> R {
    let scope = alloc(vm, "Lkotlinx/coroutines/CoroutineScope;", Native::Opaque)?;
    inv_virt(
        vm,
        args[0],
        "invoke",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        &[scope, args[1]],
    )
}
pub(crate) fn coroutines_with_context(vm: &mut Vm, args: &[JValue]) -> R {
    let scope = alloc(vm, "Lkotlinx/coroutines/CoroutineScope;", Native::Opaque)?;
    inv_virt(
        vm,
        args[1],
        "invoke",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        &[scope, args[2]],
    )
}
pub(crate) fn coroutines_async_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = inv_virt(
        vm,
        args[3],
        "invoke",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        &[args[0], JValue::Null],
    )
    .unwrap_or(JValue::Null);
    alloc(
        vm,
        "Lkotlinx/coroutines/Deferred;",
        Native::Deferred {
            value,
            error: JValue::Null,
        },
    )
}
pub(crate) fn deferred_await(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Deferred { value, .. }) => Ok(*value),
        _ => Err(npe(vm)),
    }
}
pub(crate) fn deferred_await_all(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[0])?
        .into_iter()
        .map(|v| deferred_await(vm, &[v]))
        .collect::<Result<Vec<_>, _>>()?;
    list_alloc(vm, values)
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Lkotlinx/coroutines/CoroutineScopeKt;", "CoroutineScope", "(Lkotlin/coroutines/CoroutineContext;)Lkotlinx/coroutines/CoroutineScope;", false, coroutine_scope_create),
    ne!("Lkotlinx/coroutines/GlobalScope;", "getInstance", "()Lkotlinx/coroutines/GlobalScope;", false, coroutines_global_scope),
    ne!("Lkotlinx/coroutines/Dispatchers;", "getIO", "()Lkotlinx/coroutines/CoroutineDispatcher;", false, coroutines_dispatchers_io),
    ne!("Lkotlinx/coroutines/BuildersKt;", "launch$default", "(Lkotlinx/coroutines/CoroutineScope;Lkotlin/coroutines/CoroutineContext;Lkotlinx/coroutines/CoroutineStart;Lkotlin/jvm/functions/Function2;ILjava/lang/Object;)Lkotlinx/coroutines/Job;", false, coroutines_launch_default),
    ne!("Lkotlinx/coroutines/BuildersKt;", "async$default", "(Lkotlinx/coroutines/CoroutineScope;Lkotlin/coroutines/CoroutineContext;Lkotlinx/coroutines/CoroutineStart;Lkotlin/jvm/functions/Function2;ILjava/lang/Object;)Lkotlinx/coroutines/Deferred;", false, coroutines_async_default),
    ne!("Lkotlinx/coroutines/CoroutineScopeKt;", "coroutineScope", "(Lkotlin/jvm/functions/Function2;Lkotlin/coroutines/Continuation;)Ljava/lang/Object;", false, coroutines_scope),
    ne!("Lkotlinx/coroutines/BuildersKt;", "withContext", "(Lkotlin/coroutines/CoroutineContext;Lkotlin/jvm/functions/Function2;Lkotlin/coroutines/Continuation;)Ljava/lang/Object;", false, coroutines_with_context),
    ne!("Lkotlinx/coroutines/Deferred;", "await", "(Lkotlin/coroutines/Continuation;)Ljava/lang/Object;", true, deferred_await),
    ne!("Lkotlinx/coroutines/AwaitKt;", "awaitAll", "(Ljava/util/Collection;Lkotlin/coroutines/Continuation;)Ljava/lang/Object;", false, deferred_await_all),
    ne!("Lkotlinx/coroutines/sync/MutexKt;", "Mutex$default", "(ZILjava/lang/Object;)Lkotlinx/coroutines/sync/Mutex;", false, mutex_default),
    ne!("Lkotlinx/coroutines/sync/Mutex;", "lock", "(Lkotlin/coroutines/Continuation;)Ljava/lang/Object;", true, mutex_lock),
    ne!("Lkotlinx/coroutines/sync/Mutex;", "tryLock", "()Z", true, mutex_try_lock),
    ne!("Lkotlinx/coroutines/sync/Mutex;", "unlock", "()V", true, mutex_unlock),
    ne!("Lkotlinx/coroutines/sync/Mutex;", "isLocked", "()Z", true, mutex_is_locked),
];
