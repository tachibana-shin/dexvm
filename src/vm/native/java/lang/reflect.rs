//! Minimal java.lang.reflect: `Field` payloads from `Class.getDeclaredField`,
//! with `AccessibleObject.setAccessible` and `Field.set/get`. Host-owned
//! shim classes keep no instance fields, so field writes there are accepted
//! and exposed through the corresponding natives (e.g. the
//! `headers$delegate` install is reflected in `HttpSource.getHeaders()`
//! defaults); real dex classes get strict NoSuchFieldException semantics.

use crate::vm::native::*;

pub(crate) fn class_get_declared_field(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ClassObj(ClassOrPrim::Class(cid))) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let cid = *cid;
    let name = jstr(vm, args[1])?;
    if vm.class_has_instance_fields(cid) && vm.class_field_offset(cid, &name).is_none() {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/NoSuchFieldException;",
            format!("{} on {}.", name, vm.class_desc_str(cid)),
        )));
    }
    let name_id = vm.intern(&name);
    alloc(
        vm,
        "Ljava/lang/reflect/Field;",
        Native::Field {
            class: cid,
            name: name_id,
        },
    )
}

pub(crate) fn accessible_set_accessible(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

fn field_targets(vm: &Vm, field: JValue) -> Option<(u32, u32)> {
    match payload(vm, field) {
        Some(Native::Field { class, name }) => Some((*class, *name)),
        _ => None,
    }
}

pub(crate) fn field_set(vm: &mut Vm, args: &[JValue]) -> R {
    let Some((class, name)) = field_targets(vm, args[0]) else {
        return Err(npe(vm));
    };
    let name_str = vm.str_of(name).to_string();
    if !vm.class_has_instance_fields(class) {
        return Ok(JValue::Null);
    }
    let ok = vm.instance_field_set(args[1].as_obj(), &name_str, args[2]);
    if !ok {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/NoSuchFieldException;",
            format!("{} on {}.", name_str, vm.class_desc_str(class)),
        )));
    }
    Ok(JValue::Null)
}

pub(crate) fn field_get(vm: &mut Vm, args: &[JValue]) -> R {
    let Some((class, name)) = field_targets(vm, args[0]) else {
        return Err(npe(vm));
    };
    let name_str = vm.str_of(name).to_string();
    if !vm.class_has_instance_fields(class) {
        return Ok(JValue::Null);
    }
    match vm.instance_field(args[1].as_obj(), &name_str) {
        Some(v) => Ok(v),
        None => Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/NoSuchFieldException;",
            format!("{} on {}.", name_str, vm.class_desc_str(class)),
        ))),
    }
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/Class;",
        "getDeclaredField",
        "(Ljava/lang/String;)Ljava/lang/reflect/Field;",
        true,
        class_get_declared_field
    ),
    ne!(
        "Ljava/lang/reflect/AccessibleObject;",
        "setAccessible",
        "(Z)V",
        true,
        accessible_set_accessible
    ),
    ne!(
        "Ljava/lang/reflect/AccessibleObject;",
        "setAccessible",
        "(Z)Z",
        true,
        accessible_set_accessible
    ),
    ne!(
        "Ljava/lang/reflect/Field;",
        "set",
        "(Ljava/lang/Object;Ljava/lang/Object;)V",
        true,
        field_set
    ),
    ne!(
        "Ljava/lang/reflect/Field;",
        "get",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        field_get
    ),
];