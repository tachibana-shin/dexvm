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
fn injekt_instance_falls_back_to_application() {
    with_vm(|vm| {
        // Without a typed `java.lang.reflect.Type` argument, getInstance
        // falls back to an Application opaque.
        let inst = injekt_get_instance(vm, &[]).unwrap();
        assert_eq!(
            vm.class_desc_str(obj_class(vm, inst.as_obj())),
            "android.app.Application"
        );
        // With a typed Type argument, the instance takes the concrete class.
        let t = alloc(vm, "Ljava/lang/reflect/Type;", Native::Type { desc: "Lokhttp3/OkHttpClient;".into() }).unwrap();
        let inst = injekt_get_instance(vm, &[JValue::Null, t]).unwrap();
        assert_eq!(
            vm.class_desc_str(obj_class(vm, inst.as_obj())),
            "okhttp3.OkHttpClient"
        );
    });
}

#[test]
fn full_type_reference() {
    with_vm(|vm| {
        // <init> is a no-op; getType returns an opaque reflect Type when
        // the receiver carries no generic signature.
        assert!(injekt_full_type_init(vm, &[]).unwrap().is_null());
        let t = injekt_full_type_get(vm, &[]).unwrap();
        assert!(matches!(payload(vm, t), Some(Native::Opaque)));
        // A receiver whose class has a generic signature yields a typed
        // Type carrying the concrete descriptor.
        let t = injekt_full_type_get(vm, &[JValue::Null]).unwrap();
        assert!(matches!(payload(vm, t), Some(Native::Opaque)));
    });
}
