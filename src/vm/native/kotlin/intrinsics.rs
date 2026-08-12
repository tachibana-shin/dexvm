//! Kotlin JVM intrinsic helpers.
use crate::vm::native::*;

pub(crate) fn are_equal(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(java_equals(vm, args[0], args[1])?)))
}
pub(crate) fn check_not_null_parameter(vm: &mut Vm, args: &[JValue]) -> R {
    if args[0].is_null_ref() {
        let name = jstr(vm, args[1]).unwrap_or_else(|_| "parameter".into());
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/NullPointerException;",
            format!("{name} must not be null"),
        )));
    }
    Ok(JValue::Null)
}
pub(crate) fn throw_uninitialized(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[0]).unwrap_or_else(|_| "property".into());
    Err(NatErr::Throw(vm.throwable_of(
        "Lkotlin/UninitializedPropertyAccessException;",
        format!("lateinit property {name} has not been initialized"),
    )))
}
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Lkotlin/jvm/internal/Intrinsics;",
        "areEqual",
        "(Ljava/lang/Object;Ljava/lang/Object;)Z",
        false,
        are_equal
    ),
    ne!(
        "Lkotlin/jvm/internal/Intrinsics;",
        "checkNotNullParameter",
        "(Ljava/lang/Object;Ljava/lang/String;)V",
        false,
        check_not_null_parameter
    ),
    ne!(
        "Lkotlin/jvm/internal/Intrinsics;",
        "throwUninitializedPropertyAccessException",
        "(Ljava/lang/String;)V",
        false,
        throw_uninitialized
    ),
];
