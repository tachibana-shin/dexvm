//! Kotlin Result inline-class helpers.
use crate::vm::native::*;

fn constructor_impl(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}
fn is_failure_impl(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(matches!(
        payload(vm, args[0]),
        Some(Native::ResultFailure(_))
    ))))
}
fn create_failure(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlin/Result$Failure;",
        Native::ResultFailure(args[0]),
    )
}
fn exception_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::ResultFailure(error)) => Ok(*error),
        _ => Ok(JValue::Null),
    }
}
fn throw_on_failure(vm: &mut Vm, args: &[JValue]) -> R {
    if let Some(Native::ResultFailure(value)) = payload(vm, args[0]) {
        return Err(NatErr::Throw(value.as_obj()));
    }
    Ok(JValue::Null)
}
fn is_success_impl(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(!matches!(
        payload(vm, args[0]),
        Some(Native::ResultFailure(_))
    ))))
}
fn identity(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Lkotlin/Result;",
        "constructor-impl",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        false,
        constructor_impl
    ),
    ne!(
        "Lkotlin/Result;",
        "isFailure-impl",
        "(Ljava/lang/Object;)Z",
        false,
        is_failure_impl
    ),
    ne!(
        "Lkotlin/ResultKt;",
        "createFailure",
        "(Ljava/lang/Throwable;)Ljava/lang/Object;",
        false,
        create_failure
    ),
    ne!(
        "Lkotlin/Result;",
        "exceptionOrNull-impl",
        "(Ljava/lang/Object;)Ljava/lang/Throwable;",
        false,
        exception_or_null
    ),
    ne!(
        "Lkotlin/ResultKt;",
        "throwOnFailure",
        "(Ljava/lang/Object;)V",
        false,
        throw_on_failure
    ),
    ne!(
        "Lkotlin/Result;",
        "isSuccess-impl",
        "(Ljava/lang/Object;)Z",
        false,
        is_success_impl
    ),
    ne!(
        "Lkotlin/Result;",
        "box-impl",
        "(Ljava/lang/Object;)Lkotlin/Result;",
        false,
        identity
    ),
    ne!(
        "Lkotlin/Result;",
        "unbox-impl",
        "()Ljava/lang/Object;",
        true,
        identity
    ),
];
