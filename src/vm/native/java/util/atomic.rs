//! java.util.concurrent.atomic.AtomicBoolean and AtomicInteger host shims
//! (mutable host state; memory ordering is irrelevant in the interpreter).

use crate::vm::native::*;

pub(crate) fn atomic_bool_init(vm: &mut Vm, args: &[JValue]) -> R {
    let v = if args.len() > 1 && args[1].as_int() != 0 {
        JValue::Int(1)
    } else {
        JValue::Int(0)
    };
    let Some(Native::AtomicBool(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot = v.as_int() != 0;
    Ok(JValue::Null)
}

pub(crate) fn atomic_bool_init_default(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::AtomicBool(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot = false;
    Ok(JValue::Null)
}

pub(crate) fn atomic_bool_get(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::AtomicBool(v)) => Ok(JValue::Int(u8::from(*v) as i32)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn atomic_bool_set(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::AtomicBool(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot = args[1].as_int() != 0;
    Ok(JValue::Null)
}

pub(crate) fn atomic_bool_cas(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::AtomicBool(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let exp = args[1].as_int() != 0;
    let upd = args[2].as_int() != 0;
    if *slot == exp {
        *slot = upd;
        return Ok(JValue::Int(1));
    }
    Ok(JValue::Int(0))
}

pub(crate) fn atomic_bool_tostring(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::AtomicBool(v)) => Ok(new_str(vm, if *v { "true" } else { "false" })),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn atomic_int_init(vm: &mut Vm, args: &[JValue]) -> R {
    let v = if args.len() > 1 { args[1].as_int() } else { 0 };
    let Some(Native::AtomicInt(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot = v;
    Ok(JValue::Null)
}

pub(crate) fn atomic_int_get(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::AtomicInt(v)) => Ok(JValue::Int(*v)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn atomic_int_set(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::AtomicInt(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot = args[1].as_int();
    Ok(JValue::Null)
}

pub(crate) fn atomic_int_add_and_get(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::AtomicInt(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot += args[1].as_int();
    Ok(JValue::Int(*slot))
}

pub(crate) fn atomic_int_get_and_add(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::AtomicInt(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let old = *slot;
    *slot += args[1].as_int();
    Ok(JValue::Int(old))
}

pub(crate) fn atomic_int_increment_and_get(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::AtomicInt(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot += 1;
    Ok(JValue::Int(*slot))
}

pub(crate) fn atomic_int_decrement_and_get(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::AtomicInt(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot -= 1;
    Ok(JValue::Int(*slot))
}

pub(crate) fn atomic_int_get_and_increment(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::AtomicInt(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let old = *slot;
    *slot += 1;
    Ok(JValue::Int(old))
}

pub(crate) fn atomic_int_cas(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::AtomicInt(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let exp = args[1].as_int();
    let upd = args[2].as_int();
    if *slot == exp {
        *slot = upd;
        return Ok(JValue::Int(1));
    }
    Ok(JValue::Int(0))
}

pub(crate) fn atomic_int_tostring(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::AtomicInt(v)) => Ok(new_str(vm, &v.to_string())),
        _ => Err(npe(vm)),
    }
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/concurrent/atomic/AtomicBoolean;",
        "<init>",
        "(Z)V",
        true,
        atomic_bool_init
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicBoolean;",
        "<init>",
        "()V",
        true,
        atomic_bool_init_default
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicBoolean;",
        "get",
        "()Z",
        true,
        atomic_bool_get
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicBoolean;",
        "set",
        "(Z)V",
        true,
        atomic_bool_set
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicBoolean;",
        "compareAndSet",
        "(ZZ)Z",
        true,
        atomic_bool_cas
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicBoolean;",
        "toString",
        "()Ljava/lang/String;",
        true,
        atomic_bool_tostring
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicInteger;",
        "<init>",
        "(I)V",
        true,
        atomic_int_init
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicInteger;",
        "<init>",
        "()V",
        true,
        atomic_int_init
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicInteger;",
        "get",
        "()I",
        true,
        atomic_int_get
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicInteger;",
        "set",
        "(I)V",
        true,
        atomic_int_set
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicInteger;",
        "addAndGet",
        "(I)I",
        true,
        atomic_int_add_and_get
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicInteger;",
        "getAndAdd",
        "(I)I",
        true,
        atomic_int_get_and_add
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicInteger;",
        "incrementAndGet",
        "()I",
        true,
        atomic_int_increment_and_get
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicInteger;",
        "decrementAndGet",
        "()I",
        true,
        atomic_int_decrement_and_get
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicInteger;",
        "getAndIncrement",
        "()I",
        true,
        atomic_int_get_and_increment
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicInteger;",
        "compareAndSet",
        "(II)Z",
        true,
        atomic_int_cas
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicInteger;",
        "toString",
        "()Ljava/lang/String;",
        true,
        atomic_int_tostring
    ),
];