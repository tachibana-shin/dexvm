//! java.util.ArrayDeque host shims.

use crate::vm::native::*;

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

/// Native methods for Ljava/util/ArrayDeque;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/util/ArrayDeque;", "<init>", "()V", true, deque_init),
    ne!("Ljava/util/ArrayDeque;", "<init>", "(I)V", true, deque_init),
    ne!("Ljava/util/ArrayDeque;", "addLast", "(Ljava/lang/Object;)V", true, deque_add_last),
    ne!("Ljava/util/ArrayDeque;", "addFirst", "(Ljava/lang/Object;)V", true, deque_add_first),
    ne!("Ljava/util/ArrayDeque;", "removeFirst", "()Ljava/lang/Object;", true, deque_remove_first),
    ne!("Ljava/util/ArrayDeque;", "removeLast", "()Ljava/lang/Object;", true, deque_remove_last),
    ne!("Ljava/util/ArrayDeque;", "size", "()I", true, deque_size),
    ne!("Ljava/util/ArrayDeque;", "isEmpty", "()Z", true, deque_is_empty),
    ne!("Ljava/util/ArrayDeque;", "peekFirst", "()Ljava/lang/Object;", true, deque_peek_first),
];
