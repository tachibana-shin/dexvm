use crate::vm::native::*;

pub(crate) fn latch_init(vm: &mut Vm, args: &[JValue]) -> R {
    let count = int_of(vm, args[1]).max(0);
    let Some(Native::CountDownLatch(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot = count;
    Ok(JValue::Null)
}
pub(crate) fn latch_count_down(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::CountDownLatch(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot = slot.saturating_sub(1);
    Ok(JValue::Null)
}
pub(crate) fn latch_get_count(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::CountDownLatch(n)) => Ok(JValue::Long(i64::from(*n))),
        _ => Err(npe(vm)),
    }
}
pub(crate) fn latch_await(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::CountDownLatch(n)) => Ok(JValue::Int(i32::from(*n == 0))),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn semaphore_init(vm: &mut Vm, args: &[JValue]) -> R {
    let permits = int_of(vm, args[1]);
    let Some(Native::Semaphore(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot = permits;
    Ok(JValue::Null)
}
pub(crate) fn semaphore_release(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Semaphore(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *slot += 1;
    Ok(JValue::Null)
}
pub(crate) fn semaphore_try_acquire(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Semaphore(slot)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if *slot > 0 {
        *slot -= 1;
        Ok(JValue::Int(1))
    } else {
        Ok(JValue::Int(0))
    }
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/concurrent/CountDownLatch;",
        "<init>",
        "(I)V",
        true,
        latch_init
    ),
    ne!(
        "Ljava/util/concurrent/CountDownLatch;",
        "countDown",
        "()V",
        true,
        latch_count_down
    ),
    ne!(
        "Ljava/util/concurrent/CountDownLatch;",
        "getCount",
        "()J",
        true,
        latch_get_count
    ),
    ne!(
        "Ljava/util/concurrent/CountDownLatch;",
        "await",
        "(JLjava/util/concurrent/TimeUnit;)Z",
        true,
        latch_await
    ),
    ne!(
        "Ljava/util/concurrent/Semaphore;",
        "<init>",
        "(I)V",
        true,
        semaphore_init
    ),
    ne!(
        "Ljava/util/concurrent/Semaphore;",
        "release",
        "()V",
        true,
        semaphore_release
    ),
    ne!(
        "Ljava/util/concurrent/Semaphore;",
        "tryAcquire",
        "(JLjava/util/concurrent/TimeUnit;)Z",
        true,
        semaphore_try_acquire
    ),
    ne!(
        "Ljava/util/concurrent/Semaphore;",
        "tryAcquire",
        "()Z",
        true,
        semaphore_try_acquire
    ),
];
