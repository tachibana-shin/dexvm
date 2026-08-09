//! Smoke tests for the android / androidx framework host shims.

use super::*;
use crate::context::Context;
use crate::SandboxOptions;

fn with_vm<T>(f: impl FnOnce(&mut Vm) -> T) -> T {
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
    f(ctx.vm())
}

#[test]
fn shared_preferences_stub() {
    with_vm(|vm| {
        // getSharedPreferences -> opaque SharedPreferences.
        let prefs = context_get_shared_prefs(vm, &[]).unwrap();
        assert!(matches!(payload(vm, prefs), Some(Native::Opaque)));

        // getBoolean echoes its default argument (no stored state).
        let key = vm.alloc_string("pref_key");
        let def = JValue::Int(1);
        let got = shared_prefs_get_boolean(vm, &[prefs, key, def]).unwrap();
        assert_eq!(got, JValue::Int(1));

        let key = vm.alloc_string("other");
        let def = JValue::Int(0);
        let k2 = vm.alloc_string("x");
        let got = shared_prefs_get_boolean(vm, &[k2, key, def]).unwrap();
        assert_eq!(got, JValue::Int(0));
    });
}

#[test]
fn androidx_preferences_stub() {
    with_vm(|vm| {
        // Preference.<init> / setKey are no-ops; prefs() returns a Context.
        assert!(prefs_obj(vm, &[]).unwrap().is_null());
        assert!(prefs_set(vm, &[]).unwrap().is_null());
        let ctx = prefs_ctx(vm, &[]).unwrap();
        assert!(matches!(payload(vm, ctx), Some(Native::Opaque)));
    });
}
