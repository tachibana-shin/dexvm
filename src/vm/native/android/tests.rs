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
fn shared_preferences_persist_without_guest_filesystem_permission() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let root = std::env::temp_dir().join(format!(
        "dexvm-prefs-test-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let path = root.join("preferences.bin");

    {
        let mut ctx = Context::new(&data).unwrap();
        ctx.set_shared_preferences_path(&path);
        let vm = ctx.vm();
        let context = opaque_inst(vm, "Landroid/content/Context;");
        let name = vm.alloc_string("persistent");
        let prefs = context_get_shared_prefs(vm, &[context, name, JValue::Int(0)]).unwrap();
        let key = vm.alloc_string("answer");
        let editor = shared_prefs_edit(vm, &[prefs]).unwrap();
        editor_put_int(vm, &[editor, key, JValue::Int(42)]).unwrap();
        assert_eq!(editor_commit(vm, &[editor]).unwrap(), JValue::Int(1));
        let apply_key = vm.alloc_string("applied");
        let apply_value = vm.alloc_string("saved");
        let editor = shared_prefs_edit(vm, &[prefs]).unwrap();
        editor_put_string(vm, &[editor, apply_key, apply_value]).unwrap();
        editor_apply(vm, &[editor]).unwrap();
        assert!(path.is_file());
        assert!(!ctx.has_permission(&Permission::Filesystem(FilesystemPermission::Any)));
    }

    {
        let mut ctx = Context::new(&data).unwrap();
        ctx.set_shared_preferences_path(&path);
        let vm = ctx.vm();
        let context = opaque_inst(vm, "Landroid/content/Context;");
        let name = vm.alloc_string("persistent");
        let prefs = context_get_shared_prefs(vm, &[context, name, JValue::Int(0)]).unwrap();
        let key = vm.alloc_string("answer");
        assert_eq!(
            shared_prefs_get_int(vm, &[prefs, key, JValue::Int(-1)]).unwrap(),
            JValue::Int(42)
        );
        let apply_key = vm.alloc_string("applied");
        let default = vm.alloc_string("missing");
        let saved = shared_prefs_get_string(vm, &[prefs, apply_key, default]).unwrap();
        assert_eq!(jstr(vm, saved).unwrap(), "saved");
    }

    std::fs::remove_dir_all(root).unwrap();
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
