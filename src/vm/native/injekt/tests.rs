//! Smoke tests for the injekt (kohesive) DI host shims.

use super::*;
use crate::context::Context;
use crate::SandboxOptions;

fn with_vm<T>(f: impl FnOnce(&mut Vm) -> T) -> T {
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
    f(ctx.vm())
}

#[test]
fn injekt_scope_and_instance_opaque() {
    with_vm(|vm| {
        // Injekt.get -> opaque InjektScope.
        let scope = injekt_get_injekt(vm, &[]).unwrap();
        assert!(matches!(payload(vm, scope), Some(Native::Opaque)));
    });
}

#[test]
#[cfg(feature = "android")]
fn injekt_instance_is_application() {
    with_vm(|vm| {
        // factory.getInstance -> Application (needs the android shim).
        let inst = injekt_get_instance(vm, &[]).unwrap();
        assert_eq!(
            vm.class_desc_str(obj_class(vm, inst.as_obj())),
            "android.app.Application"
        );
    });
}

#[test]
fn full_type_reference() {
    with_vm(|vm| {
        // <init> is a no-op; getType returns an opaque reflect Type.
        assert!(injekt_full_type_init(vm, &[]).unwrap().is_null());
        let t = injekt_full_type_get(vm, &[]).unwrap();
        assert!(matches!(payload(vm, t), Some(Native::Opaque)));
    });
}
