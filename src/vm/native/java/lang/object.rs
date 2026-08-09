//! java.lang.Object host shims.

use crate::vm::native::*;

// java.lang.Object
// ---------------------------------------------------------------------------

pub(crate) fn object_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn object_get_class(vm: &mut Vm, args: &[JValue]) -> R {
    let recv = args[0].as_obj();
    let class = obj_class(vm, recv);
    vm.class_obj(class).map_err(nat_fatal)
}

pub(crate) fn object_hash_code(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(args[0].as_obj() as i32))
}

pub(crate) fn object_equals(_vm: &mut Vm, args: &[JValue]) -> R {
    let eq = match (args[0], args[1]) {
        (JValue::Obj(x), JValue::Obj(y)) => x == y,
        (JValue::Null, JValue::Null) => true,
        _ => false,
    };
    Ok(JValue::Int(i32::from(eq)))
}

pub(crate) fn object_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let recv = args[0].as_obj();
    let class = obj_class(vm, recv);
    let name = vm.class_desc_str(class);
    Ok(new_str(vm, &format!("{name}@{:x}", recv as u32)))
}

pub(crate) fn object_clone(vm: &mut Vm, args: &[JValue]) -> R {
    let recv = args[0].as_obj();
    let class = obj_class(vm, recv);
    let Some(n) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Array(d) => Ok(JValue::Obj(vm.arena.alloc(
            class,
            Vec::new(),
            Some(Native::Array(d.clone())),
        ))),
        _ => Err(uoe(vm, "clone not supported")),
    }
}

pub(crate) fn object_noop(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// Native methods for Ljava/lang/Object;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/lang/Object;", "<init>", "()V", true, object_init),
    ne!(
        "Ljava/lang/Object;",
        "getClass",
        "()Ljava/lang/Class;",
        true,
        object_get_class
    ),
    ne!(
        "Ljava/lang/Object;",
        "hashCode",
        "()I",
        true,
        object_hash_code
    ),
    ne!(
        "Ljava/lang/Object;",
        "equals",
        "(Ljava/lang/Object;)Z",
        true,
        object_equals
    ),
    ne!(
        "Ljava/lang/Object;",
        "toString",
        "()Ljava/lang/String;",
        true,
        object_to_string
    ),
    ne!(
        "Ljava/lang/Object;",
        "clone",
        "()Ljava/lang/Object;",
        true,
        object_clone
    ),
];
