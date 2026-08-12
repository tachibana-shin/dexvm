//! java.util.concurrent.atomic.AtomicBoolean and AtomicInteger host shims
//! (mutable host state; memory ordering is irrelevant in the interpreter).

use crate::vm::native::*;

pub(crate) fn atomic_ref_init(vm: &mut Vm, args: &[JValue]) -> R {
    let v = args.get(1).copied().unwrap_or(JValue::Null);
    let Some(Native::Lazy(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot = v;
    Ok(JValue::Null)
}
pub(crate) fn atomic_ref_get(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Lazy(v)) => Ok(*v),
        _ => Err(npe(vm)),
    }
}
pub(crate) fn atomic_ref_set(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Lazy(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot = args[1];
    Ok(JValue::Null)
}

pub(crate) fn atomic_ref_array_init(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[1]).max(0) as usize;
    let JValue::Obj(id) = args[0] else {
        return Err(npe(vm));
    };
    vm.arena.objects[id as usize].native =
        Some(Native::Array(ArrayData::Obj(vec![JValue::Null; n])));
    Ok(JValue::Null)
}
pub(crate) fn atomic_ref_array_get(vm: &mut Vm, args: &[JValue]) -> R {
    let i = int_of(vm, args[1]);
    let Some(Native::Array(ArrayData::Obj(v))) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    match v.get(i as usize) {
        Some(&x) => Ok(x),
        None => Err(ioobe(vm, i)),
    }
}
pub(crate) fn atomic_ref_array_set(vm: &mut Vm, args: &[JValue]) -> R {
    let i = int_of(vm, args[1]).max(0) as usize;
    let value = args[2];
    let Some(Native::Array(ArrayData::Obj(v))) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if i >= v.len() {
        return Err(ioobe(vm, i as i32));
    }
    v[i] = value;
    Ok(JValue::Null)
}
pub(crate) fn atomic_ref_array_get_and_set(vm: &mut Vm, args: &[JValue]) -> R {
    let i = int_of(vm, args[1]).max(0) as usize;
    let value = args[2];
    let Some(Native::Array(ArrayData::Obj(v))) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if i >= v.len() {
        return Err(ioobe(vm, i as i32));
    }
    let old = v[i];
    v[i] = value;
    Ok(old)
}
pub(crate) fn atomic_ref_array_cas(vm: &mut Vm, args: &[JValue]) -> R {
    let i = int_of(vm, args[1]).max(0) as usize;
    let expect = args[2];
    let update = args[3];
    let Some(Native::Array(ArrayData::Obj(v))) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match v.get_mut(i) {
        Some(slot) if *slot == expect => {
            *slot = update;
            Ok(JValue::Int(1))
        }
        Some(_) => Ok(JValue::Int(0)),
        None => Err(ioobe(vm, i as i32)),
    }
}

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
        "Ljava/util/concurrent/atomic/AtomicReference;",
        "<init>",
        "(Ljava/lang/Object;)V",
        true,
        atomic_ref_init
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicReference;",
        "<init>",
        "()V",
        true,
        atomic_ref_init
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicReference;",
        "get",
        "()Ljava/lang/Object;",
        true,
        atomic_ref_get
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicReference;",
        "set",
        "(Ljava/lang/Object;)V",
        true,
        atomic_ref_set
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicReferenceArray;",
        "<init>",
        "(I)V",
        true,
        atomic_ref_array_init
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicReferenceArray;",
        "get",
        "(I)Ljava/lang/Object;",
        true,
        atomic_ref_array_get
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicReferenceArray;",
        "set",
        "(ILjava/lang/Object;)V",
        true,
        atomic_ref_array_set
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicReferenceArray;",
        "getAndSet",
        "(ILjava/lang/Object;)Ljava/lang/Object;",
        true,
        atomic_ref_array_get_and_set
    ),
    ne!(
        "Ljava/util/concurrent/atomic/AtomicReferenceArray;",
        "compareAndSet",
        "(ILjava/lang/Object;Ljava/lang/Object;)Z",
        true,
        atomic_ref_array_cas
    ),
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
