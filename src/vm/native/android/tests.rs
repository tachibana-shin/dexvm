//! Smoke tests for the android / androidx framework host shims.

use super::*;
use crate::context::Context;
use crate::permission::{FilesystemPermission, Permission};
use crate::SandboxOptions;

fn with_vm<T>(f: impl FnOnce(&mut Vm) -> T) -> T {
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
    f(ctx.vm())
}

fn with_denied_vm<T>(f: impl FnOnce(&mut Vm) -> T) -> T {
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let mut ctx = Context::new(&data).unwrap();
    f(ctx.vm())
}

#[test]
fn shared_preferences_roundtrip() {
    with_vm(|vm| {
        let ctx = opaque_inst(vm, "Landroid/content/Context;");
        let name = vm.alloc_string("prefs");
        let prefs = context_get_shared_prefs(vm, &[ctx, name, JValue::Int(0)]).unwrap();
        assert!(matches!(
            payload(vm, prefs),
            Some(Native::SharedPreferences(name)) if name == "prefs"
        ));

        let key = vm.alloc_string("pref_key");
        let def = JValue::Int(1);
        let got = shared_prefs_get_boolean(vm, &[prefs, key, def]).unwrap();
        assert_eq!(got, JValue::Int(1));

        let editor = shared_prefs_edit(vm, &[prefs]).unwrap();
        editor_put_boolean(vm, &[editor, key, JValue::Int(0)]).unwrap();
        editor_apply(vm, &[editor]).unwrap();
        let got = shared_prefs_get_boolean(vm, &[prefs, key, def]).unwrap();
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

#[test]
fn file_operations_enforce_scoped_permissions() {
    with_denied_vm(|vm| {
        let path = vm.cache_root_path().to_owned();
        let file = alloc(vm, "Ljava/io/File;", Native::File { path: path.clone() }).unwrap();

        assert!(matches!(file_exists(vm, &[file]), Err(NatErr::Throw(_))));
        assert!(matches!(file_mkdirs(vm, &[file]), Err(NatErr::Throw(_))));

        vm.perms
            .grant(Permission::Filesystem(FilesystemPermission::Path(
                path.clone(),
            )));
        assert_eq!(file_mkdirs(vm, &[file]).unwrap(), JValue::Int(1));
        assert_eq!(file_exists(vm, &[file]).unwrap(), JValue::Int(1));
        std::fs::remove_dir(path).unwrap();
    });
}

#[test]
fn file_table_marks_create_temp_file_static() {
    let entry = ANDROID_TABLE
        .iter()
        .find(|e| e.class == "Ljava/io/File;" && e.name == "createTempFile")
        .unwrap();
    assert!(!entry.instance);
}
