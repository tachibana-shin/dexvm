use crate::vm::native::*;

pub(crate) fn prim_class_obj(vm: &mut Vm, code: u8) -> JValue {
    let class = vm
        .ensure_class_by_desc("Ljava/lang/Class;")
        .expect("Class shim");
    JValue::Obj(vm.arena.alloc(
        class,
        Vec::new(),
        Some(Native::ClassObj(ClassOrPrim::Primitive(code))),
    ))
}

pub fn lazy_int_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'I')
}
pub fn lazy_long_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'J')
}
pub fn lazy_short_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'S')
}
pub fn lazy_byte_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'B')
}
pub fn lazy_char_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'C')
}
pub fn lazy_bool_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'Z')
}
pub fn lazy_float_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'F')
}
pub fn lazy_double_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'D')
}

// java.lang.Class host shims.

// java.lang.Class
// ---------------------------------------------------------------------------

pub(crate) fn desc_to_java_name(desc: &str) -> String {
    let mut dims = 0;
    let mut rest = desc;
    while let Some(r) = rest.strip_prefix('[') {
        dims += 1;
        rest = r;
    }
    let base = match rest {
        "B" => "byte".to_string(),
        "C" => "char".to_string(),
        "D" => "double".to_string(),
        "F" => "float".to_string(),
        "I" => "int".to_string(),
        "J" => "long".to_string(),
        "S" => "short".to_string(),
        "Z" => "boolean".to_string(),
        "V" => "void".to_string(),
        r if r.starts_with('L') => r
            .strip_prefix('L')
            .and_then(|r| r.strip_suffix(';'))
            .map(|r| r.replace('/', "."))
            .unwrap_or_else(|| r.to_string()),
        r => r.to_string(),
    };
    let mut s = base;
    for _ in 0..dims {
        s.push_str("[]");
    }
    s
}

pub(crate) fn prim_name(b: u8) -> String {
    match b {
        b'B' => "byte".to_string(),
        b'C' => "char".to_string(),
        b'D' => "double".to_string(),
        b'F' => "float".to_string(),
        b'I' => "int".to_string(),
        b'J' => "long".to_string(),
        b'S' => "short".to_string(),
        b'Z' => "boolean".to_string(),
        b'V' => "void".to_string(),
        _ => "unknown".to_string(),
    }
}

pub(crate) fn prim_code(name: &str) -> String {
    match name {
        "byte" => "B".to_string(),
        "char" => "C".to_string(),
        "double" => "D".to_string(),
        "float" => "F".to_string(),
        "int" => "I".to_string(),
        "long" => "J".to_string(),
        "short" => "S".to_string(),
        "boolean" => "Z".to_string(),
        "void" => "V".to_string(),
        _ => name.to_string(),
    }
}

pub(crate) fn simple_name_of(desc: &str) -> String {
    let mut dims = 0;
    let mut rest = desc;
    while let Some(r) = rest.strip_prefix('[') {
        dims += 1;
        rest = r;
    }
    let base = if rest.len() == 1 {
        prim_name(rest.as_bytes()[0])
    } else if let Some(r) = rest.strip_prefix('L').and_then(|r| r.strip_suffix(';')) {
        let last = r.rsplit('/').next().unwrap_or(r);
        last.split('$').next().unwrap_or(last).to_string()
    } else {
        rest.to_string()
    };
    let mut s = base;
    for _ in 0..dims {
        s.push_str("[]");
    }
    s
}

pub(crate) fn class_cop(vm: &Vm, v: JValue) -> Option<&ClassOrPrim> {
    match payload(vm, v) {
        Some(Native::ClassObj(c)) => Some(c),
        _ => None,
    }
}

pub(crate) fn class_get_name(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(cop) = class_cop(vm, args[0]) else {
        return Err(npe(vm));
    };
    let name = match cop {
        ClassOrPrim::Class(c) => {
            let desc = vm.str_of(vm.classes[*c as usize].descriptor);
            desc_to_java_name(desc)
        }
        ClassOrPrim::Primitive(b) => prim_name(*b),
    };
    Ok(new_str(vm, &name))
}

pub(crate) fn class_get_simple_name(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(cop) = class_cop(vm, args[0]) else {
        return Err(npe(vm));
    };
    let name = match cop {
        ClassOrPrim::Class(c) => {
            let desc = vm.str_of(vm.classes[*c as usize].descriptor);
            simple_name_of(desc)
        }
        ClassOrPrim::Primitive(b) => prim_name(*b),
    };
    Ok(new_str(vm, &name))
}

pub(crate) fn class_get_canonical_name(vm: &mut Vm, args: &[JValue]) -> R {
    class_get_name(vm, args)
}

pub(crate) fn class_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(cop) = class_cop(vm, args[0]) else {
        return Err(npe(vm));
    };
    let name = match cop {
        ClassOrPrim::Class(c) => {
            let desc = vm.str_of(vm.classes[*c as usize].descriptor);
            desc_to_java_name(desc)
        }
        ClassOrPrim::Primitive(b) => prim_name(*b),
    };
    Ok(new_str(vm, &format!("class {name}")))
}

pub(crate) fn class_is_instance(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(ClassOrPrim::Class(target)) = class_cop(vm, args[0]) else {
        return Err(npe(vm));
    };
    let target = *target;
    let obj = args[1];
    if obj.is_null() {
        return Ok(JValue::Int(0));
    }
    let obj_class = obj_class(vm, obj.as_obj());
    let ok = vm.is_assignable(obj_class, target).map_err(nat_fatal)?;
    Ok(JValue::Int(i32::from(ok)))
}

pub(crate) fn class_is_array(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(cop) = class_cop(vm, args[0]) else {
        return Err(npe(vm));
    };
    let is_arr =
        matches!(cop, ClassOrPrim::Class(c) if vm.classes[*c as usize].array_elem.is_some());
    Ok(JValue::Int(i32::from(is_arr)))
}

pub(crate) fn class_is_primitive(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(cop) = class_cop(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(matches!(
        cop,
        ClassOrPrim::Primitive(_)
    ))))
}

pub(crate) fn class_is_interface(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(cop) = class_cop(vm, args[0]) else {
        return Err(npe(vm));
    };
    let is_if = matches!(cop, ClassOrPrim::Class(c) if vm.classes[*c as usize].is_interface);
    Ok(JValue::Int(i32::from(is_if)))
}

pub(crate) fn class_get_component_type(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(cop) = class_cop(vm, args[0]) else {
        return Err(npe(vm));
    };
    match cop {
        ClassOrPrim::Class(c) => {
            let Some((edx, elem)) = vm.classes[*c as usize].array_elem else {
                return Ok(JValue::Null);
            };
            let e_desc = vm.dex_at(edx).type_descriptor(elem).to_string();
            if e_desc.len() == 1 {
                let class_class = vm
                    .ensure_class_by_desc("Ljava/lang/Class;")
                    .map_err(nat_fatal)?;
                return Ok(JValue::Obj(vm.arena.alloc(
                    class_class,
                    Vec::new(),
                    Some(Native::ClassObj(ClassOrPrim::Primitive(
                        e_desc.as_bytes()[0],
                    ))),
                )));
            }
            let ec = vm.ensure_class_by_type(edx, elem).map_err(nat_fatal)?;
            vm.class_obj(ec).map_err(nat_fatal)
        }
        ClassOrPrim::Primitive(_) => Ok(JValue::Null),
    }
}

pub(crate) fn class_get_superclass(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(ClassOrPrim::Class(c)) = class_cop(vm, args[0]) else {
        return Ok(JValue::Null);
    };
    match vm.classes[*c as usize].superclass {
        Some(s) => vm.class_obj(s).map_err(nat_fatal),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn class_cast(vm: &mut Vm, args: &[JValue]) -> R {
    let obj = args[1];
    if obj.is_null() {
        return Ok(JValue::Null);
    }
    let Some(ClassOrPrim::Class(target)) = class_cop(vm, args[0]) else {
        return Ok(obj);
    };
    let obj_class = obj_class(vm, obj.as_obj());
    let target = *target;
    let ok = vm.is_assignable(obj_class, target).map_err(nat_fatal)?;
    if ok {
        Ok(obj)
    } else {
        let target_name = {
            let desc = vm.str_of(vm.classes[target as usize].descriptor);
            desc_to_java_name(desc)
        };
        Err(cce(vm, format!("cannot cast to {target_name}")))
    }
}

pub(crate) fn class_desired_assertion_status(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

pub(crate) fn class_get_class_loader(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/lang/ClassLoader;", Native::Opaque)
}

pub(crate) fn class_get_modifiers(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

pub(crate) fn class_for_name(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[0])?;
    let class_class = vm
        .ensure_class_by_desc("Ljava/lang/Class;")
        .map_err(nat_fatal)?;
    if matches!(
        name.as_str(),
        "byte" | "char" | "double" | "float" | "int" | "long" | "short" | "boolean" | "void"
    ) {
        return Ok(JValue::Obj(vm.arena.alloc(
            class_class,
            Vec::new(),
            Some(Native::ClassObj(ClassOrPrim::Primitive(
                prim_code(&name).as_bytes()[0],
            ))),
        )));
    }
    let desc = if name.starts_with('[') {
        name
    } else if name.ends_with("[]") {
        let dims = name.matches("[]").count();
        let base = name.trim_end_matches("[]");
        let elem = if matches!(
            base,
            "byte" | "char" | "double" | "float" | "int" | "long" | "short" | "boolean" | "void"
        ) {
            prim_code(base)
        } else {
            format!("L{};", base.replace('.', "/"))
        };
        format!("{}{}", "[".repeat(dims), elem)
    } else {
        format!("L{};", name.replace('.', "/"))
    };
    let class = vm.ensure_class_by_desc(&desc).map_err(nat_fatal)?;
    vm.class_obj(class).map_err(nat_fatal)
}
pub(crate) fn class_is_assignable_from(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(ClassOrPrim::Class(target)) = class_cop(vm, args[0]) else {
        return Ok(JValue::Int(0));
    };
    let Some(ClassOrPrim::Class(src)) = class_cop(vm, args[1]) else {
        return Ok(JValue::Int(0));
    };
    let ok = vm.is_assignable(*src, *target).map_err(nat_fatal)?;
    Ok(JValue::Int(i32::from(ok)))
}

pub(crate) fn class_get_interfaces(_vm: &mut Vm, _args: &[JValue]) -> R {
    alloc_empty_arr(_vm, "Ljava/lang/Class;")
}

// ---------------------------------------------------------------------------

/// Native methods for Ljava/lang/Class;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/Class;",
        "getName",
        "()Ljava/lang/String;",
        true,
        class_get_name
    ),
    ne!(
        "Ljava/lang/Class;",
        "getSimpleName",
        "()Ljava/lang/String;",
        true,
        class_get_simple_name
    ),
    ne!(
        "Ljava/lang/Class;",
        "getCanonicalName",
        "()Ljava/lang/String;",
        true,
        class_get_canonical_name
    ),
    ne!(
        "Ljava/lang/Class;",
        "toString",
        "()Ljava/lang/String;",
        true,
        class_to_string
    ),
    ne!(
        "Ljava/lang/Class;",
        "isInstance",
        "(Ljava/lang/Object;)Z",
        true,
        class_is_instance
    ),
    ne!("Ljava/lang/Class;", "isArray", "()Z", true, class_is_array),
    ne!(
        "Ljava/lang/Class;",
        "isPrimitive",
        "()Z",
        true,
        class_is_primitive
    ),
    ne!(
        "Ljava/lang/Class;",
        "isInterface",
        "()Z",
        true,
        class_is_interface
    ),
    ne!(
        "Ljava/lang/Class;",
        "getComponentType",
        "()Ljava/lang/Class;",
        true,
        class_get_component_type
    ),
    ne!(
        "Ljava/lang/Class;",
        "getSuperclass",
        "()Ljava/lang/Class;",
        true,
        class_get_superclass
    ),
    ne!(
        "Ljava/lang/Class;",
        "cast",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        class_cast
    ),
    ne!(
        "Ljava/lang/Class;",
        "desiredAssertionStatus",
        "()Z",
        true,
        class_desired_assertion_status
    ),
    ne!(
        "Ljava/lang/Class;",
        "getClassLoader",
        "()Ljava/lang/ClassLoader;",
        true,
        class_get_class_loader
    ),
    ne!(
        "Ljava/lang/Class;",
        "getModifiers",
        "()I",
        true,
        class_get_modifiers
    ),
    ne!(
        "Ljava/lang/Class;",
        "isAssignableFrom",
        "(Ljava/lang/Class;)Z",
        true,
        class_is_assignable_from
    ),
    ne!(
        "Ljava/lang/Class;",
        "getInterfaces",
        "()[Ljava/lang/Class;",
        true,
        class_get_interfaces
    ),
    ne!(
        "Ljava/lang/Class;",
        "forName",
        "(Ljava/lang/String;)Ljava/lang/Class;",
        false,
        class_for_name
    ),
    ne!(
        "Ljava/lang/Class;",
        "forName",
        "(Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;",
        false,
        class_for_name
    ),
];
