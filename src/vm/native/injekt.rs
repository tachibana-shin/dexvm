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

/// `InjektFactory.getInstance(Type)` — allocates an instance of the concrete
/// type carried by the `java.lang.reflect.Type` argument (which
/// `FullTypeReference.getType()` fills with the receiver's generic
/// signature). Falls back to an Application opaque when the type is unknown.
pub(crate) fn injekt_get_instance(vm: &mut Vm, args: &[JValue]) -> R {
    let desc = match args.get(1).and_then(|t| payload(vm, *t)) {
        Some(Native::Type { desc }) => desc.clone(),
        _ => "Landroid/app/Application;".to_string(),
    };
    alloc(vm, &desc, Native::Opaque)
}

pub(crate) fn injekt_full_type_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// `FullTypeReference.getType()` — reflects the concrete generic type of the
/// receiver subclass. Two sources, in order:
/// 1. the bytecode-derived injekt registry (`getInstance` result check-casts),
///    which works even when minification stripped the dex `Signature`
///    annotation;
/// 2. the dex `Signature` annotation, e.g. a subclass
///    `class Lq extends FullTypeReference<Lkotlinx/serialization/json/Json;>`
///    yields `Lkotlinx/serialization/json/Json;`.
pub(crate) fn injekt_full_type_get(vm: &mut Vm, args: &[JValue]) -> R {
    let class = match args.first().copied() {
        Some(JValue::Obj(o)) => vm.arena.objects[o as usize].class,
        _ => return alloc(vm, "Ljava/lang/reflect/Type;", Native::Opaque),
    };
    if let Some(desc) = vm.injekt_type_of(vm.classes[class as usize].descriptor) {
        let desc = vm.str_of(desc).to_string();
        return alloc(vm, "Ljava/lang/reflect/Type;", Native::Type { desc });
    }
    let desc = vm.generic_signature(class).unwrap_or_default();
    if desc.is_empty() {
        return alloc(vm, "Ljava/lang/reflect/Type;", Native::Opaque);
    }
    alloc(vm, "Ljava/lang/reflect/Type;", Native::Type { desc })
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

#[cfg(test)]
mod tests;
