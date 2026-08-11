//! kotlinx.coroutines host shims.

use crate::vm::native::*;

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Lkotlinx/coroutines/CoroutineScopeKt;", "CoroutineScope", "(Lkotlin/coroutines/CoroutineContext;)Lkotlinx/coroutines/CoroutineScope;", false, crate::vm::native::kotlin::coroutine_scope_create),
    ne!("Lkotlinx/coroutines/GlobalScope;", "getInstance", "()Lkotlinx/coroutines/GlobalScope;", false, crate::vm::native::kotlin::coroutines_global_scope),
    ne!("Lkotlinx/coroutines/Dispatchers;", "getIO", "()Lkotlinx/coroutines/CoroutineDispatcher;", false, crate::vm::native::kotlin::coroutines_dispatchers_io),
    ne!("Lkotlinx/coroutines/BuildersKt;", "launch$default", "(Lkotlinx/coroutines/CoroutineScope;Lkotlin/coroutines/CoroutineContext;Lkotlinx/coroutines/CoroutineStart;Lkotlin/jvm/functions/Function2;ILjava/lang/Object;)Lkotlinx/coroutines/Job;", false, crate::vm::native::kotlin::coroutines_launch_default),
    ne!("Lkotlinx/coroutines/BuildersKt;", "async$default", "(Lkotlinx/coroutines/CoroutineScope;Lkotlin/coroutines/CoroutineContext;Lkotlinx/coroutines/CoroutineStart;Lkotlin/jvm/functions/Function2;ILjava/lang/Object;)Lkotlinx/coroutines/Deferred;", false, crate::vm::native::kotlin::coroutines_async_default),
    ne!("Lkotlinx/coroutines/CoroutineScopeKt;", "coroutineScope", "(Lkotlin/jvm/functions/Function2;Lkotlin/coroutines/Continuation;)Ljava/lang/Object;", false, crate::vm::native::kotlin::coroutines_scope),
    ne!("Lkotlinx/coroutines/BuildersKt;", "withContext", "(Lkotlin/coroutines/CoroutineContext;Lkotlin/jvm/functions/Function2;Lkotlin/coroutines/Continuation;)Ljava/lang/Object;", false, crate::vm::native::kotlin::coroutines_with_context),
    ne!("Lkotlinx/coroutines/Deferred;", "await", "(Lkotlin/coroutines/Continuation;)Ljava/lang/Object;", true, crate::vm::native::kotlin::deferred_await),
    ne!("Lkotlinx/coroutines/AwaitKt;", "awaitAll", "(Ljava/util/Collection;Lkotlin/coroutines/Continuation;)Ljava/lang/Object;", false, crate::vm::native::kotlin::deferred_await_all),
];
