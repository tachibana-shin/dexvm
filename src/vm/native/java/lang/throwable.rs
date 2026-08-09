//! java.lang.Throwable host shims.

use crate::vm::native::*;

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

/// Native methods for Ljava/lang/Throwable;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/lang/Throwable;", "getMessage", "()Ljava/lang/String;", true, throwable_get_message),
    ne!("Ljava/lang/Throwable;", "getLocalizedMessage", "()Ljava/lang/String;", true, throwable_get_localized_message),
    ne!("Ljava/lang/Throwable;", "getCause", "()Ljava/lang/Throwable;", true, throwable_get_cause),
    ne!("Ljava/lang/Throwable;", "initCause", "(Ljava/lang/Throwable;)Ljava/lang/Throwable;", true, throwable_init_cause),
    ne!("Ljava/lang/Throwable;", "toString", "()Ljava/lang/String;", true, throwable_to_string),
    ne!("Ljava/lang/Throwable;", "printStackTrace", "()V", true, throwable_print_stack_trace),
    ne!("Ljava/lang/Throwable;", "fillInStackTrace", "()Ljava/lang/Throwable;", true, throwable_fill_in_stack_trace),
    ne!("Ljava/lang/Throwable;", "addSuppressed", "(Ljava/lang/Throwable;)V", true, throwable_add_suppressed),
    ne!("Ljava/lang/Throwable;", "getSuppressed", "()[Ljava/lang/Throwable;", true, throwable_get_suppressed),
    ne!("Ljava/lang/Throwable;", "getStackTrace", "()[Ljava/lang/StackTraceElement;", true, throwable_get_stack_trace),
];
