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
    if let Some(Native::KotlinLazy {
        evaluating: true, ..
    }) = payload(vm, args[0])
    {
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
            if let Some(Native::KotlinLazy {
                value, evaluating, ..
            }) = payload_mut(vm, args[0])
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

fn regex_option_enum(vm: &mut Vm, name: &str, ordinal: i32) -> JValue {
    alloc(
        vm,
        "Lkotlin/text/RegexOption;",
        Native::Enum {
            name: name.into(),
            ordinal,
        },
    )
    .expect("alloc RegexOption")
}

pub(crate) fn lazy_regex_option_unix_lines(vm: &mut Vm) -> JValue {
    regex_option_enum(vm, "UNIX_LINES", 0)
}

pub(crate) fn lazy_regex_option_comments(vm: &mut Vm) -> JValue {
    regex_option_enum(vm, "COMMENTS", 1)
}

pub(crate) fn lazy_regex_option_ignore_case(vm: &mut Vm) -> JValue {
    regex_option_enum(vm, "IGNORE_CASE", 2)
}

pub(crate) fn lazy_regex_option_multiline(vm: &mut Vm) -> JValue {
    regex_option_enum(vm, "MULTILINE", 3)
}

pub(crate) fn lazy_regex_option_dot_matches_all(vm: &mut Vm) -> JValue {
    regex_option_enum(vm, "DOT_MATCHES_ALL", 4)
}

pub(crate) fn lazy_regex_option_literal(vm: &mut Vm) -> JValue {
    regex_option_enum(vm, "LITERAL", 5)
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
