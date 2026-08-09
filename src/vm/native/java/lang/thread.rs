//! java.lang.Thread host shims.

use crate::vm::native::*;

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

/// Native methods for Ljava/lang/Thread;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/lang/Thread;", "<init>", "()V", true, thread_init),
    ne!(
        "Ljava/lang/Thread;",
        "<init>",
        "(Ljava/lang/Runnable;)V",
        true,
        thread_init
    ),
    ne!(
        "Ljava/lang/Thread;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        thread_init
    ),
    ne!(
        "Ljava/lang/Thread;",
        "<init>",
        "(Ljava/lang/Runnable;Ljava/lang/String;)V",
        true,
        thread_init
    ),
    ne!(
        "Ljava/lang/Thread;",
        "currentThread",
        "()Ljava/lang/Thread;",
        false,
        thread_current
    ),
    ne!("Ljava/lang/Thread;", "start", "()V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "run", "()V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "yield", "()V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "interrupt", "()V", true, thread_noop),
    ne!(
        "Ljava/lang/Thread;",
        "interrupted",
        "()Z",
        false,
        thread_is_interrupted
    ),
    ne!("Ljava/lang/Thread;", "sleep", "(J)V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "sleep", "(JI)V", true, thread_noop),
    ne!(
        "Ljava/lang/Thread;",
        "setName",
        "(Ljava/lang/String;)V",
        true,
        thread_noop
    ),
    ne!("Ljava/lang/Thread;", "setDaemon", "(Z)V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "join", "()V", true, thread_noop),
    ne!(
        "Ljava/lang/Thread;",
        "getName",
        "()Ljava/lang/String;",
        true,
        thread_get_name
    ),
    ne!("Ljava/lang/Thread;", "getId", "()J", true, thread_get_id),
    ne!(
        "Ljava/lang/Thread;",
        "isAlive",
        "()Z",
        true,
        thread_is_alive
    ),
    ne!(
        "Ljava/lang/Thread;",
        "isDaemon",
        "()Z",
        true,
        thread_is_daemon
    ),
    ne!(
        "Ljava/lang/Thread;",
        "isInterrupted",
        "()Z",
        true,
        thread_is_interrupted
    ),
];
