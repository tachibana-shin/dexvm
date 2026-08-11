use crate::vm::native::*;

pub fn lazy_bool_true(vm: &mut Vm) -> JValue {
    let class = vm
        .ensure_class_by_desc("Ljava/lang/Boolean;")
        .expect("Boolean shim");
    JValue::Obj(
        vm.arena
            .alloc(class, Vec::new(), Some(Native::BoolBox(true))),
    )
}
pub fn lazy_bool_false(vm: &mut Vm) -> JValue {
    let class = vm
        .ensure_class_by_desc("Ljava/lang/Boolean;")
        .expect("Boolean shim");
    JValue::Obj(
        vm.arena
            .alloc(class, Vec::new(), Some(Native::BoolBox(false))),
    )
}
// java.lang.Boolean host shims.

pub(crate) fn bool_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    let b = bool_of(vm, args[0]);
    boxed(vm, "Ljava/lang/Boolean;", Native::BoolBox(b))
}

pub(crate) fn bool_boolean_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(bool_of(vm, args[0]))))
}

pub(crate) fn bool_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(
        bool_of(vm, args[0]) == bool_of(vm, args[1]),
    )))
}

pub(crate) fn bool_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(if bool_of(vm, args[0]) { 1231 } else { 1237 }))
}

pub(crate) fn bool_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(
        vm,
        if bool_of(vm, args[0]) {
            "true"
        } else {
            "false"
        },
    ))
}

pub(crate) fn bool_to_string_static(vm: &mut Vm, args: &[JValue]) -> R {
    bool_to_string(vm, args)
}

pub(crate) fn bool_parse_boolean(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    Ok(JValue::Int(i32::from(s.eq_ignore_ascii_case("true"))))
}

pub(crate) fn bool_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    let a = bool_of(vm, args[0]);
    let b = bool_of(vm, args[1]);
    Ok(JValue::Int(a.cmp(&b) as i32))
}

/// Native methods for Ljava/lang/Boolean;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/Boolean;",
        "valueOf",
        "(Z)Ljava/lang/Boolean;",
        false,
        bool_value_of
    ),
    ne!(
        "Ljava/lang/Boolean;",
        "booleanValue",
        "()Z",
        true,
        bool_boolean_value
    ),
    ne!(
        "Ljava/lang/Boolean;",
        "equals",
        "(Ljava/lang/Object;)Z",
        true,
        bool_equals
    ),
    ne!(
        "Ljava/lang/Boolean;",
        "hashCode",
        "()I",
        true,
        bool_hash_code
    ),
    ne!(
        "Ljava/lang/Boolean;",
        "hashCode",
        "(Z)I",
        false,
        bool_hash_code
    ),
    ne!(
        "Ljava/lang/Boolean;",
        "toString",
        "()Ljava/lang/String;",
        true,
        bool_to_string
    ),
    ne!(
        "Ljava/lang/Boolean;",
        "toString",
        "(Z)Ljava/lang/String;",
        false,
        bool_to_string_static
    ),
    ne!(
        "Ljava/lang/Boolean;",
        "parseBoolean",
        "(Ljava/lang/String;)Z",
        false,
        bool_parse_boolean
    ),
    ne!(
        "Ljava/lang/Boolean;",
        "compareTo",
        "(Ljava/lang/Boolean;)I",
        true,
        bool_compare_to
    ),
    ne!(
        "Ljava/lang/Boolean;",
        "compareTo",
        "(Ljava/lang/Object;)I",
        true,
        bool_compare_to
    ),
];
