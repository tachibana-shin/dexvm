use super::*;

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

// ---------------------------------------------------------------------------
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

pub(crate) fn class_cop<'a>(vm: &'a Vm, v: JValue) -> Option<&'a ClassOrPrim> {
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
    let is_arr = matches!(cop, ClassOrPrim::Class(c) if vm.classes[*c as usize].array_elem.is_some());
    Ok(JValue::Int(i32::from(is_arr)))
}

pub(crate) fn class_is_primitive(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(cop) = class_cop(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(matches!(cop, ClassOrPrim::Primitive(_)))))
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
            let Some(elem) = vm.classes[*c as usize].array_elem else {
                return Ok(JValue::Null);
            };
            let e_desc = vm.dex.type_descriptor(elem).to_string();
            if e_desc.len() == 1 {
                let class_class = vm.ensure_class_by_desc("Ljava/lang/Class;").map_err(nat_fatal)?;
                return Ok(JValue::Obj(vm.arena.alloc(
                    class_class,
                    Vec::new(),
                    Some(Native::ClassObj(ClassOrPrim::Primitive(e_desc.as_bytes()[0]))),
                )));
            }
            let ec = vm.ensure_class_by_type(elem).map_err(nat_fatal)?;
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

pub(crate) fn class_get_class_loader(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn class_get_modifiers(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

pub(crate) fn class_for_name(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[0])?;
    let class_class = vm.ensure_class_by_desc("Ljava/lang/Class;").map_err(nat_fatal)?;
    if matches!(
        name.as_str(),
        "byte" | "char" | "double" | "float" | "int" | "long" | "short" | "boolean" | "void"
    ) {
        return Ok(JValue::Obj(vm.arena.alloc(
            class_class,
            Vec::new(),
            Some(Native::ClassObj(ClassOrPrim::Primitive(prim_code(&name).as_bytes()[0]))),
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
// java.lang.Throwable and subclasses
// ---------------------------------------------------------------------------

pub(crate) fn throwable_message_of(vm: &Vm, v: JValue) -> Option<String> {
    match payload(vm, v) {
        Some(Native::Throwable { message, .. }) => message.clone(),
        _ => None,
    }
}

pub(crate) fn tinit0(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Throwable { message, cause } => {
            *message = None;
            *cause = JValue::Null;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn tinit_str(vm: &mut Vm, args: &[JValue]) -> R {
    let msg = jstr(vm, args[1])?;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Throwable { message, cause } => {
            *message = Some(msg);
            *cause = JValue::Null;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn tinit_str_cause(vm: &mut Vm, args: &[JValue]) -> R {
    let msg = jstr(vm, args[1])?;
    let cause = args[2];
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Throwable { message, cause: c } => {
            *message = Some(msg);
            *c = cause;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn tinit_cause(vm: &mut Vm, args: &[JValue]) -> R {
    let cause = args[1];
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Throwable { message, cause: c } => {
            *message = None;
            *c = cause;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn throwable_get_message(vm: &mut Vm, args: &[JValue]) -> R {
    match throwable_message_of(vm, args[0]) {
        Some(m) => Ok(new_str(vm, &m)),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn throwable_get_localized_message(vm: &mut Vm, args: &[JValue]) -> R {
    throwable_get_message(vm, args)
}

pub(crate) fn throwable_get_cause(vm: &mut Vm, args: &[JValue]) -> R {
    let cause = match payload(vm, args[0]) {
        Some(Native::Throwable { cause, .. }) => *cause,
        _ => JValue::Null,
    };
    Ok(cause)
}

pub(crate) fn throwable_init_cause(vm: &mut Vm, args: &[JValue]) -> R {
    let cause = args[1];
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Throwable { cause: c, .. } => *c = cause,
        _ => return Err(npe(vm)),
    }
    Ok(args[0])
}

pub(crate) fn throwable_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let recv = args[0].as_obj();
    let class = obj_class(vm, recv);
    let desc = vm.str_of(vm.classes[class as usize].descriptor);
    let name = simple_name_of(desc);
    match throwable_message_of(vm, args[0]) {
        Some(m) if !m.is_empty() => Ok(new_str(vm, &format!("{name}: {m}"))),
        _ => Ok(new_str(vm, &name)),
    }
}

pub(crate) fn throwable_print_stack_trace(vm: &mut Vm, args: &[JValue]) -> R {
    let recv = args[0].as_obj();
    let class = obj_class(vm, recv);
    let desc = vm.str_of(vm.classes[class as usize].descriptor);
    let name = simple_name_of(desc);
    match throwable_message_of(vm, args[0]) {
        Some(m) if !m.is_empty() => vm.write_out(&format!("{name}: {m}\n")),
        _ => vm.write_out(&format!("{name}\n")),
    }
    Ok(JValue::Null)
}

pub(crate) fn throwable_fill_in_stack_trace(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn throwable_add_suppressed(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn throwable_get_suppressed(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc_empty_arr(vm, "Ljava/lang/Throwable;")
}

pub(crate) fn throwable_get_stack_trace(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc_empty_arr(vm, "Ljava/lang/StackTraceElement;")
}

// ---------------------------------------------------------------------------
// java.lang.System / java.io.PrintStream
// ---------------------------------------------------------------------------

pub(crate) fn sys_current_time_millis(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Long(now_millis()))
}

pub(crate) fn sys_nano_time(_vm: &mut Vm, _args: &[JValue]) -> R {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0);
    Ok(JValue::Long(n))
}

pub(crate) fn arrcopy_into(src: &ArrayData, sp: usize, dst: &mut ArrayData, dp: usize, len: usize) -> bool {
    for i in 0..len {
        let v = src.get(sp + i);
        let ok = match dst {
            ArrayData::Byte(d) => match v {
                JValue::Int(x) => {
                    d[dp + i] = x as i8;
                    true
                }
                _ => false,
            },
            ArrayData::Char(d) => match v {
                JValue::Int(x) => {
                    d[dp + i] = x as u16;
                    true
                }
                _ => false,
            },
            ArrayData::Short(d) => match v {
                JValue::Int(x) => {
                    d[dp + i] = x as i16;
                    true
                }
                _ => false,
            },
            ArrayData::Int(d) => match v {
                JValue::Int(x) => {
                    d[dp + i] = x;
                    true
                }
                JValue::Long(x) => {
                    d[dp + i] = x as i32;
                    true
                }
                _ => false,
            },
            ArrayData::Long(d) => match v {
                JValue::Int(x) => {
                    d[dp + i] = i64::from(x);
                    true
                }
                JValue::Long(x) => {
                    d[dp + i] = x;
                    true
                }
                _ => false,
            },
            ArrayData::Float(d) => match v {
                JValue::Int(x) => {
                    d[dp + i] = x as f32;
                    true
                }
                JValue::Long(x) => {
                    d[dp + i] = x as f32;
                    true
                }
                JValue::Float(x) => {
                    d[dp + i] = x;
                    true
                }
                _ => false,
            },
            ArrayData::Double(d) => match v {
                JValue::Int(x) => {
                    d[dp + i] = f64::from(x);
                    true
                }
                JValue::Long(x) => {
                    d[dp + i] = x as f64;
                    true
                }
                JValue::Float(x) => {
                    d[dp + i] = f64::from(x);
                    true
                }
                JValue::Double(x) => {
                    d[dp + i] = x;
                    true
                }
                _ => false,
            },
            ArrayData::Bool(d) => match v {
                JValue::Int(x) => {
                    d[dp + i] = x != 0;
                    true
                }
                _ => false,
            },
            ArrayData::Obj(d) => {
                d[dp + i] = v;
                true
            }
        };
        if !ok {
            return false;
        }
    }
    true
}

pub(crate) fn sys_arraycopy(vm: &mut Vm, args: &[JValue]) -> R {
    let src_id = match args[0] {
        JValue::Obj(id) => id,
        _ => return Err(npe(vm)),
    };
    let dst_id = match args[2] {
        JValue::Obj(id) => id,
        _ => return Err(npe(vm)),
    };
    let src_pos = int_of(vm, args[1]);
    let dst_pos = int_of(vm, args[3]);
    let len = int_of(vm, args[4]);
    if len < 0 {
        return Err(aioobe(vm, len, 0));
    }
    let src_len = match payload(vm, JValue::Obj(src_id)) {
        Some(Native::Array(d)) => d.len() as i64,
        _ => return Err(npe(vm)),
    };
    let dst_len = match payload(vm, JValue::Obj(dst_id)) {
        Some(Native::Array(d)) => d.len() as i64,
        _ => return Err(npe(vm)),
    };
    let sp = src_pos as i64;
    let dp = dst_pos as i64;
    let l = i64::from(len);
    if sp < 0 || dp < 0 || sp + l > src_len || dp + l > dst_len {
        return Err(aioobe(vm, len, src_len as i32));
    }
    let src_data = match payload(vm, JValue::Obj(src_id)) {
        Some(Native::Array(d)) => d.clone(),
        _ => return Err(npe(vm)),
    };
    let Some(n) = payload_mut(vm, JValue::Obj(dst_id)) else {
        return Err(npe(vm));
    };
    match n {
        Native::Array(dst_data) => {
            let ok = arrcopy_into(&src_data, src_pos as usize, dst_data, dst_pos as usize, len as usize);
            if !ok {
                return Err(iae(vm, "arraycopy: type mismatch"));
            }
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn sys_exit(_vm: &mut Vm, args: &[JValue]) -> R {
    Err(NatErr::Fatal(JvmError::Exit(int_of(_vm, args[0]))))
}

pub(crate) fn sys_gc(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn sys_identity_hash_code(_vm: &mut Vm, args: &[JValue]) -> R {
    match args[0] {
        JValue::Obj(id) => Ok(JValue::Int(id as i32)),
        _ => Ok(JValue::Int(0)),
    }
}

pub(crate) fn sys_get_property(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn sys_line_separator(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(new_str(vm, "\n"))
}

pub(crate) fn ps_println(vm: &mut Vm, args: &[JValue]) -> R {
    let s = if args.len() > 1 {
        to_string_of(vm, args[1])?
    } else {
        String::new()
    };
    vm.write_out(&format!("{s}\n"));
    Ok(JValue::Null)
}

pub(crate) fn ps_print(vm: &mut Vm, args: &[JValue]) -> R {
    let s = to_string_of(vm, args[1])?;
    vm.write_out(&s);
    Ok(JValue::Null)
}

pub(crate) fn ps_println_char(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[1]) as u16;
    vm.write_out(&format!("{}\n", u16str(&[c])));
    Ok(JValue::Null)
}

pub(crate) fn ps_print_char(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[1]) as u16;
    vm.write_out(&u16str(&[c]));
    Ok(JValue::Null)
}

pub(crate) fn ps_println_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Char(cs))) => u16str(cs),
        _ => return Err(npe(vm)),
    };
    vm.write_out(&format!("{s}\n"));
    Ok(JValue::Null)
}

pub(crate) fn ps_flush(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn ps_close(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

// ---------------------------------------------------------------------------
// boxed primitives: Integer / Long / Short / Byte / Character / Boolean /
// Float / Double
// ---------------------------------------------------------------------------

pub(crate) fn box_int_value(vm: &mut Vm, desc: &str, v: JValue) -> R {
    let n = int_of(vm, v);
    let native = match desc {
        "Ljava/lang/Integer;" => Native::IntBox(n),
        "Ljava/lang/Short;" => Native::ShortBox(n as i16),
        "Ljava/lang/Byte;" => Native::ByteBox(n as i8),
        "Ljava/lang/Character;" => Native::CharBox(n as u16),
        _ => return Err(iae(vm, "bad box class")),
    };
    boxed(vm, desc, native)
}

pub(crate) fn box_int(vm: &mut Vm, desc: &str, args: &[JValue], i: usize) -> R {
    box_int_value(vm, desc, args[i])
}

pub(crate) fn integer_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    box_int(vm, "Ljava/lang/Integer;", args, 0)
}

pub(crate) fn integer_value_of_str(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let n = parse_int_radix(vm, &s, 10)?;
    box_int_value(vm, "Ljava/lang/Integer;", JValue::Int(n))
}

pub(crate) fn integer_int_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0])))
}

pub(crate) fn integer_long_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(i64::from(int_of(vm, args[0]))))
}

pub(crate) fn integer_float_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(int_of(vm, args[0]) as f32))
}

pub(crate) fn integer_double_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(f64::from(int_of(vm, args[0]))))
}

pub(crate) fn integer_byte_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]) as i8 as i32))
}

pub(crate) fn integer_short_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]) as i16 as i32))
}

pub(crate) fn integer_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(int_of(vm, args[0]) == int_of(vm, args[1]))))
}

pub(crate) fn integer_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0])))
}

pub(crate) fn integer_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[0]);
    Ok(new_str(vm, &n.to_string()))
}

pub(crate) fn integer_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).cmp(&int_of(vm, args[1])) as i32))
}

pub(crate) fn integer_to_string_static(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &int_to_string(int_of(vm, args[0]), 10)))
}

pub(crate) fn integer_to_string_radix(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &int_to_string(int_of(vm, args[0]), int_of(vm, args[1]) as u32)))
}

pub(crate) fn integer_parse_int(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let radix = if args.len() > 1 {
        int_of(vm, args[1]) as u32
    } else {
        10
    };
    parse_int_radix(vm, &s, radix).map(JValue::Int)
}

pub(crate) fn integer_to_hex(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &format!("{:x}", int_of(vm, args[0]) as u32)))
}

pub(crate) fn integer_to_binary(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &format!("{:b}", int_of(vm, args[0]) as u32)))
}

pub(crate) fn integer_to_octal(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &format!("{:o}", int_of(vm, args[0]) as u32)))
}

pub(crate) fn integer_compare(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).cmp(&int_of(vm, args[1])) as i32))
}

pub(crate) fn integer_bit_count(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).count_ones() as i32))
}

pub(crate) fn integer_highest_one_bit(vm: &mut Vm, args: &[JValue]) -> R {
    let v = int_of(vm, args[0]);
    if v == 0 {
        Ok(JValue::Int(0))
    } else {
        Ok(JValue::Int(1i32 << (31 - v.leading_zeros())))
    }
}

pub(crate) fn long_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    let n = long_of(vm, args[0]);
    boxed(vm, "Ljava/lang/Long;", Native::LongBox(n))
}

pub(crate) fn long_value_of_str(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let n = parse_long_radix(vm, &s, 10)?;
    boxed(vm, "Ljava/lang/Long;", Native::LongBox(n))
}

pub(crate) fn long_int_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(long_of(vm, args[0]) as i32))
}

pub(crate) fn long_long_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0])))
}

pub(crate) fn long_float_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(long_of(vm, args[0]) as f32))
}

pub(crate) fn long_double_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(long_of(vm, args[0]) as f64))
}

pub(crate) fn long_byte_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(long_of(vm, args[0]) as i8 as i32))
}

pub(crate) fn long_short_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(long_of(vm, args[0]) as i16 as i32))
}

pub(crate) fn long_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(long_of(vm, args[0]) == long_of(vm, args[1]))))
}

pub(crate) fn long_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    let l = long_of(vm, args[0]);
    Ok(JValue::Int((l ^ (l >> 32)) as i32))
}

pub(crate) fn long_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &long_of(vm, args[0]).to_string()))
}

pub(crate) fn long_to_string_static(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &long_to_string_help(long_of(vm, args[0]), 10)))
}

pub(crate) fn long_to_string_radix(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &long_to_string_help(long_of(vm, args[0]), int_of(vm, args[1]) as u32)))
}

pub(crate) fn long_parse_long(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let radix = if args.len() > 1 {
        int_of(vm, args[1]) as u32
    } else {
        10
    };
    parse_long_radix(vm, &s, radix).map(JValue::Long)
}

pub(crate) fn long_to_hex(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &format!("{:x}", long_of(vm, args[0]) as u64)))
}

pub(crate) fn long_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(long_of(vm, args[0]).cmp(&long_of(vm, args[1])) as i32))
}

pub(crate) fn long_compare(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(long_of(vm, args[0]).cmp(&long_of(vm, args[1])) as i32))
}

pub(crate) fn long_bit_count(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(long_of(vm, args[0]).count_ones() as i32))
}

pub(crate) fn short_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    box_int_value(vm, "Ljava/lang/Short;", args[0])
}

pub(crate) fn short_parse_short(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let radix = if args.len() > 1 {
        int_of(vm, args[1]) as u32
    } else {
        10
    };
    let n = parse_int_radix(vm, &s, radix)?;
    if n < i32::from(i16::MIN) || n > i32::from(i16::MAX) {
        return Err(nfe(vm, format!("Value out of range: \"{s}\"")));
    }
    Ok(JValue::Int(n))
}

pub(crate) fn short_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &int_of(vm, args[0]).to_string()))
}

pub(crate) fn short_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).cmp(&int_of(vm, args[1])) as i32))
}

pub(crate) fn byte_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    box_int_value(vm, "Ljava/lang/Byte;", args[0])
}

pub(crate) fn byte_parse_byte(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let radix = if args.len() > 1 {
        int_of(vm, args[1]) as u32
    } else {
        10
    };
    let n = parse_int_radix(vm, &s, radix)?;
    if n < i32::from(i8::MIN) || n > i32::from(i8::MAX) {
        return Err(nfe(vm, format!("Value out of range: \"{s}\"")));
    }
    Ok(JValue::Int(n))
}

pub(crate) fn byte_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &int_of(vm, args[0]).to_string()))
}

pub(crate) fn byte_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).cmp(&int_of(vm, args[1])) as i32))
}

pub(crate) fn char_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    box_int_value(vm, "Ljava/lang/Character;", args[0])
}

pub(crate) fn char_char_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0])))
}

pub(crate) fn char_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(int_of(vm, args[0]) == int_of(vm, args[1]))))
}

pub(crate) fn char_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0])))
}

pub(crate) fn char_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u16;
    Ok(new_str(vm, &u16str(&[c])))
}

pub(crate) fn char_to_string_static(vm: &mut Vm, args: &[JValue]) -> R {
    char_to_string(vm, args)
}

pub(crate) fn char_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).cmp(&int_of(vm, args[1])) as i32))
}

pub(crate) fn char_is_digit(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(char::from_u32(c).is_some_and(|c| c.is_ascii_digit()))))
}

pub(crate) fn char_is_letter(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(char::from_u32(c).is_some_and(|c| c.is_alphabetic()))))
}

pub(crate) fn char_is_letter_or_digit(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(
        char::from_u32(c).is_some_and(|c| c.is_alphanumeric()),
    )))
}

pub(crate) fn char_is_whitespace(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(char::from_u32(c).is_some_and(|c| c.is_whitespace()))))
}

pub(crate) fn char_is_upper(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(char::from_u32(c).is_some_and(|c| c.is_uppercase()))))
}

pub(crate) fn char_is_lower(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(char::from_u32(c).is_some_and(|c| c.is_lowercase()))))
}

pub(crate) fn char_to_upper(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(
        char::from_u32(c).and_then(|c| c.to_uppercase().next()).map(|c| c as u32).unwrap_or(c) as u16,
    )))
}

pub(crate) fn char_to_lower(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    Ok(JValue::Int(i32::from(
        char::from_u32(c).and_then(|c| c.to_lowercase().next()).map(|c| c as u32).unwrap_or(c) as u16,
    )))
}

pub(crate) fn char_is_high_surrogate(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u16;
    Ok(JValue::Int(i32::from((0xD800..=0xDBFF).contains(&c))))
}

pub(crate) fn char_is_low_surrogate(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u16;
    Ok(JValue::Int(i32::from((0xDC00..=0xDFFF).contains(&c))))
}

pub(crate) fn char_get_numeric_value(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u32;
    let v = char::from_u32(c)
        .map(|c| c.to_digit(10).map(|d| d as i32).unwrap_or(-1))
        .unwrap_or(-1);
    Ok(JValue::Int(v))
}

pub(crate) fn bool_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    let b = bool_of(vm, args[0]);
    boxed(vm, "Ljava/lang/Boolean;", Native::BoolBox(b))
}

pub(crate) fn bool_boolean_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(bool_of(vm, args[0]))))
}

pub(crate) fn bool_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(bool_of(vm, args[0]) == bool_of(vm, args[1]))))
}

pub(crate) fn bool_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(if bool_of(vm, args[0]) { 1231 } else { 1237 }))
}

pub(crate) fn bool_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, if bool_of(vm, args[0]) { "true" } else { "false" }))
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
    Ok(JValue::Int(i32::from(a.cmp(&b) as i32)))
}

pub(crate) fn float_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    let f = float_of(vm, args[0]);
    boxed(vm, "Ljava/lang/Float;", Native::FloatBox(f))
}

pub(crate) fn float_value_of_str(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let f = parse_float(vm, &s)?;
    boxed(vm, "Ljava/lang/Float;", Native::FloatBox(f))
}

pub(crate) fn float_parse_float(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    parse_float(vm, &s).map(JValue::Float)
}

pub(crate) fn float_int_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(float_of(vm, args[0]) as i32))
}

pub(crate) fn float_long_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(float_of(vm, args[0]) as i64))
}

pub(crate) fn float_float_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(float_of(vm, args[0])))
}

pub(crate) fn float_double_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(f64::from(float_of(vm, args[0]))))
}

pub(crate) fn float_byte_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(float_of(vm, args[0]) as i8 as i32))
}

pub(crate) fn float_short_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(float_of(vm, args[0]) as i16 as i32))
}

pub(crate) fn float_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(
        float_of(vm, args[0]).to_bits() == float_of(vm, args[1]).to_bits(),
    )))
}

pub(crate) fn float_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(float_of(vm, args[0]).to_bits() as i32))
}

pub(crate) fn float_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &fmt_f32(float_of(vm, args[0]))))
}

pub(crate) fn float_to_string_static(vm: &mut Vm, args: &[JValue]) -> R {
    float_to_string(vm, args)
}

pub(crate) fn float_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    float_compare(vm, args)
}

pub(crate) fn float_compare(vm: &mut Vm, args: &[JValue]) -> R {
    let a = float_of(vm, args[0]);
    let b = float_of(vm, args[1]);
    let r = if a.is_nan() || b.is_nan() {
        if a.is_nan() && b.is_nan() {
            0
        } else if a.is_nan() {
            1
        } else {
            -1
        }
    } else if a < b {
        -1
    } else if a > b {
        1
    } else {
        a.partial_cmp(&b).unwrap_or(Ordering::Equal) as i32
    };
    Ok(JValue::Int(r))
}

pub(crate) fn float_is_nan(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(float_of(vm, args[0]).is_nan())))
}

pub(crate) fn float_is_infinite(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(float_of(vm, args[0]).is_infinite())))
}

pub(crate) fn float_to_int_bits(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(float_of(vm, args[0]).to_bits() as i32))
}

pub(crate) fn float_int_bits_to_float(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(f32::from_bits(int_of(vm, args[0]) as u32)))
}

pub(crate) fn double_value_of(vm: &mut Vm, args: &[JValue]) -> R {
    let d = double_of(vm, args[0]);
    boxed(vm, "Ljava/lang/Double;", Native::DoubleBox(d))
}

pub(crate) fn double_value_of_str(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let d = parse_double(vm, &s)?;
    boxed(vm, "Ljava/lang/Double;", Native::DoubleBox(d))
}

pub(crate) fn double_parse_double(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    parse_double(vm, &s).map(JValue::Double)
}

pub(crate) fn double_int_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(double_of(vm, args[0]) as i32))
}

pub(crate) fn double_long_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(double_of(vm, args[0]) as i64))
}

pub(crate) fn double_float_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(double_of(vm, args[0]) as f32))
}

pub(crate) fn double_double_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(double_of(vm, args[0])))
}

pub(crate) fn double_byte_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(double_of(vm, args[0]) as i8 as i32))
}

pub(crate) fn double_short_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(double_of(vm, args[0]) as i16 as i32))
}

pub(crate) fn double_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(
        double_of(vm, args[0]).to_bits() == double_of(vm, args[1]).to_bits(),
    )))
}

pub(crate) fn double_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    let b = double_of(vm, args[0]).to_bits();
    Ok(JValue::Int((b ^ (b >> 32)) as i32))
}

pub(crate) fn double_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &fmt_f64(double_of(vm, args[0]))))
}

pub(crate) fn double_to_string_static(vm: &mut Vm, args: &[JValue]) -> R {
    double_to_string(vm, args)
}

pub(crate) fn double_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    double_compare(vm, args)
}

pub(crate) fn double_compare(vm: &mut Vm, args: &[JValue]) -> R {
    let a = double_of(vm, args[0]);
    let b = double_of(vm, args[1]);
    let r = if a.is_nan() || b.is_nan() {
        if a.is_nan() && b.is_nan() {
            0
        } else if a.is_nan() {
            1
        } else {
            -1
        }
    } else if a < b {
        -1
    } else if a > b {
        1
    } else {
        a.partial_cmp(&b).unwrap_or(Ordering::Equal) as i32
    };
    Ok(JValue::Int(r))
}

pub(crate) fn double_is_nan(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(double_of(vm, args[0]).is_nan())))
}

pub(crate) fn double_is_infinite(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(double_of(vm, args[0]).is_infinite())))
}

pub(crate) fn double_to_long_bits(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(double_of(vm, args[0]).to_bits() as i64))
}

pub(crate) fn double_long_bits_to_double(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(f64::from_bits(long_of(vm, args[0]) as u64)))
}

// ---------------------------------------------------------------------------
// java.lang.Thread / java.lang.Enum
// ---------------------------------------------------------------------------

pub(crate) fn thread_current(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/lang/Thread;", Native::Opaque)
}

pub(crate) fn thread_noop(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn thread_get_name(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(new_str(vm, "main"))
}

pub(crate) fn thread_get_id(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Long(1))
}

pub(crate) fn thread_is_alive(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(1))
}

pub(crate) fn thread_is_daemon(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

pub(crate) fn thread_is_interrupted(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

pub(crate) fn thread_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Opaque => {}
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn enum_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    let ordinal = int_of(vm, args[2]);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Enum { name: dst, ordinal: o } => {
            *dst = name;
            *o = ordinal;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn enum_name(vm: &mut Vm, args: &[JValue]) -> R {
    let name = match payload(vm, args[0]) {
        Some(Native::Enum { name, .. }) => name.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &name))
}

pub(crate) fn enum_ordinal(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Enum { ordinal, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(*ordinal))
}

pub(crate) fn enum_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    enum_name(vm, args)
}

pub(crate) fn enum_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    let a = match payload(vm, args[0]) {
        Some(Native::Enum { ordinal, .. }) => *ordinal,
        _ => return Err(npe(vm)),
    };
    let b = match payload(vm, args[1]) {
        Some(Native::Enum { ordinal, .. }) => *ordinal,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(a.cmp(&b) as i32))
}

// ---------------------------------------------------------------------------

pub(crate) fn integer_signum(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).signum()))
}

pub(crate) fn long_signum(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0]).signum()))
}

// ---------------------------------------------------------------------------
