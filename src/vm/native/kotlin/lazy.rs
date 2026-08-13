//! kotlin.Lazy and LazyKt bridges.
use crate::vm::native::*;

fn lazy(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlin/SynchronizedLazyImpl;",
        Native::KotlinLazy {
            f: args[0],
            value: None,
            evaluating: false,
        },
    )
}
fn lazy_mode(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlin/SynchronizedLazyImpl;",
        Native::KotlinLazy {
            f: args[1],
            value: None,
            evaluating: false,
        },
    )
}
fn get_value(vm: &mut Vm, args: &[JValue]) -> R {
    if let Some(Native::KotlinLazy { value: Some(v), .. }) = payload(vm, args[0]) {
        return Ok(*v);
    }
    if let Some(Native::KotlinLazy { evaluating: true, .. }) = payload(vm, args[0]) {
        return Err(iae(vm, "Lazy value cannot be computed recursively"));
    }
    let f = match payload(vm, args[0]) {
        Some(Native::KotlinLazy { f, .. }) => *f,
        _ => return Err(npe(vm)),
    };
    if f.is_null_ref() {
        return Err(npe(vm));
    }
    if let Some(Native::KotlinLazy { evaluating, .. }) = payload_mut(vm, args[0]) {
        *evaluating = true;
    }
    match inv_virt(vm, f, "invoke", "()Ljava/lang/Object;", &[]) {
        Ok(v) => {
            if let Some(Native::KotlinLazy { value, evaluating, .. }) =
                payload_mut(vm, args[0])
            {
                *value = Some(v);
                *evaluating = false;
            }
            Ok(v)
        }
        Err(e) => {
            if let Some(Native::KotlinLazy { evaluating, .. }) = payload_mut(vm, args[0]) {
                *evaluating = false;
            }
            Err(e)
        }
    }
}

fn lazy_mode_enum(vm: &mut Vm, name: &str, ordinal: i32) -> JValue {
    alloc(
        vm,
        "Lkotlin/LazyThreadSafetyMode;",
        Native::Enum {
            name: name.into(),
            ordinal,
        },
    )
    .expect("alloc LazyThreadSafetyMode")
}

pub(crate) fn lazy_lazy_mode_synchronized(vm: &mut Vm) -> JValue {
    lazy_mode_enum(vm, "SYNCHRONIZED", 0)
}

pub(crate) fn lazy_lazy_mode_publication(vm: &mut Vm) -> JValue {
    lazy_mode_enum(vm, "PUBLICATION", 1)
}

pub(crate) fn lazy_lazy_mode_none(vm: &mut Vm) -> JValue {
    lazy_mode_enum(vm, "NONE", 2)
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
