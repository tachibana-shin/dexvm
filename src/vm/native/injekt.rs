//! injekt (kohesive) DI host shims.

use super::*;

// ---------------------------------------------------------------------------
// injekt DI (kohesive)
// ---------------------------------------------------------------------------

// injekt DI (kohesive)
// ---------------------------------------------------------------------------

pub(crate) fn injekt_get_injekt(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Luy/kohesive/injekt/api/InjektScope;", Native::Opaque)
}

pub(crate) fn injekt_get_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/app/Application;", Native::Opaque)
}

pub(crate) fn injekt_full_type_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn injekt_full_type_get(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/lang/reflect/Type;", Native::Opaque)
}

// ---------------------------------------------------------------------------
// injekt native table
// ---------------------------------------------------------------------------

pub(crate) const INJEKT_TABLE: &[NativeEntry] = &[
    ne!(
        "Luy/kohesive/injekt/InjektKt;",
        "getInjekt",
        "()Luy/kohesive/injekt/api/InjektScope;",
        false,
        injekt_get_injekt
    ),
    ne!(
        "Luy/kohesive/injekt/api/InjektFactory;",
        "getInstance",
        "(Ljava/lang/reflect/Type;)Ljava/lang/Object;",
        true,
        injekt_get_instance
    ),
    ne!(
        "Luy/kohesive/injekt/api/FullTypeReference;",
        "<init>",
        "()V",
        true,
        injekt_full_type_init
    ),
    ne!(
        "Luy/kohesive/injekt/api/FullTypeReference;",
        "getType",
        "()Ljava/lang/reflect/Type;",
        true,
        injekt_full_type_get
    ),
];
