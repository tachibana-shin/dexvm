//! java.util.ArrayDeque + java.util.concurrent.locks host shims.
//! Locks are non-blocking no-ops; the queue stores plain Java values.

use super::*;

pub(crate) fn deque_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.clear();
    Ok(JValue::Null)
}

pub(crate) fn deque_add_last(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push(args[1]);
    Ok(JValue::Null)
}

pub(crate) fn deque_add_first(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.insert(0, args[1]);
    Ok(JValue::Null)
}

pub(crate) fn deque_remove_first(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if dst.is_empty() {
        return Err(no_such_elem(vm));
    }
    Ok(dst.remove(0))
}

pub(crate) fn deque_remove_last(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.pop().ok_or_else(|| no_such_elem(vm))
}

pub(crate) fn deque_size(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(dst)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(dst.len() as i32))
}

pub(crate) fn deque_is_empty(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(dst)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(dst.is_empty())))
}

pub(crate) fn deque_peek_first(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ArrayDeque(dst)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    match dst.first() {
        Some(v) => Ok(*v),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn reentrant_lock_init(vm: &mut Vm, args: &[JValue]) -> R {
    let fair = int_of(vm, args[1]);
    let Some(Native::ReentrantLock { locked }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *locked = fair != 0;
    Ok(JValue::Null)
}

pub(crate) fn reentrant_lock_lock(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ReentrantLock { locked }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *locked = true;
    Ok(JValue::Null)
}

pub(crate) fn reentrant_lock_unlock(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ReentrantLock { locked }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *locked = false;
    Ok(JValue::Null)
}

pub(crate) fn reentrant_lock_new_condition(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/concurrent/locks/Condition;", Native::Opaque)
}

pub(crate) fn condition_await_nanos(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Long(0))
}

pub(crate) fn condition_await(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn condition_signal(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn condition_signal_all(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}
