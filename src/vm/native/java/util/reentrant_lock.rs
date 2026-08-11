//! java.util.concurrent.locks.ReentrantLock / Condition host shims.

use crate::vm::native::*;

pub(crate) fn reentrant_lock_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ReentrantLock { locked }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    // Fairness controls waiter ordering; it must not make a new lock held.
    // This VM executes synchronously, so both constructors start unlocked.
    *locked = false;
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

/// Native methods for Ljava/util/concurrent/locks/ReentrantLock;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/concurrent/locks/ReentrantLock;",
        "<init>",
        "()V",
        true,
        reentrant_lock_init
    ),
    ne!(
        "Ljava/util/concurrent/locks/ReentrantLock;",
        "<init>",
        "(Z)V",
        true,
        reentrant_lock_init
    ),
    ne!(
        "Ljava/util/concurrent/locks/ReentrantLock;",
        "lock",
        "()V",
        true,
        reentrant_lock_lock
    ),
    ne!(
        "Ljava/util/concurrent/locks/ReentrantLock;",
        "unlock",
        "()V",
        true,
        reentrant_lock_unlock
    ),
    ne!(
        "Ljava/util/concurrent/locks/ReentrantLock;",
        "newCondition",
        "()Ljava/util/concurrent/locks/Condition;",
        true,
        reentrant_lock_new_condition
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, SandboxOptions};

    #[test]
    fn both_constructors_start_unlocked() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
        let vm = ctx.vm();
        let lock = alloc(
            vm,
            "Ljava/util/concurrent/locks/ReentrantLock;",
            Native::ReentrantLock { locked: true },
        )
        .unwrap();
        reentrant_lock_init(vm, &[lock]).unwrap();
        assert!(matches!(
            payload(vm, lock),
            Some(Native::ReentrantLock { locked: false })
        ));
        reentrant_lock_init(vm, &[lock, JValue::Int(1)]).unwrap();
        assert!(matches!(
            payload(vm, lock),
            Some(Native::ReentrantLock { locked: false })
        ));
    }
}
