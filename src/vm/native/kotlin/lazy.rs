//! kotlin.Lazy and LazyKt bridges.
use crate::vm::native::*;

fn lazy(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Lkotlin/SynchronizedLazyImpl;", Native::Lazy(args[0]))
}
fn lazy_mode(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Lkotlin/SynchronizedLazyImpl;", Native::Lazy(args[1]))
}
fn get_value(vm: &mut Vm, args: &[JValue]) -> R {
    let f = match payload(vm, args[0]) {
        Some(Native::Lazy(f)) => *f,
        _ => return Err(npe(vm)),
    };
    if f.is_null_ref() {
        return Err(npe(vm));
    }
    inv_virt(vm, f, "invoke", "()Ljava/lang/Object;", &[])
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Lkotlin/Lazy;",
        "getValue",
        "()Ljava/lang/Object;",
        true,
        get_value
    ),
    ne!(
        "Lkotlin/LazyKt;",
        "lazy",
        "(Lkotlin/jvm/functions/Function0;)Lkotlin/Lazy;",
        false,
        lazy
    ),
    ne!(
        "Lkotlin/LazyKt;",
        "lazy",
        "(Lkotlin/LazyThreadSafetyMode;Lkotlin/jvm/functions/Function0;)Lkotlin/Lazy;",
        false,
        lazy_mode
    ),
];
