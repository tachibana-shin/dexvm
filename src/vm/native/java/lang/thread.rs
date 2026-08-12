//! java.lang.Thread host shims.

use crate::vm::native::*;

// java.lang.Thread / java.lang.Enum
// ---------------------------------------------------------------------------

pub(crate) fn thread_current(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Ljava/lang/Thread;",
        Native::Thread {
            name: "main".into(),
            daemon: false,
            alive: true,
            interrupted: false,
            started: true,
            runnable: JValue::Null,
        },
    )
}

pub(crate) fn thread_yield(_vm: &mut Vm, _args: &[JValue]) -> R {
    std::thread::yield_now();
    Ok(JValue::Null)
}

pub(crate) fn thread_get_name(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Thread { name, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let name = name.clone();
    Ok(new_str(vm, &name))
}

pub(crate) fn thread_get_id(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Long(1))
}

pub(crate) fn thread_is_alive(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Thread { alive, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(*alive)))
}

pub(crate) fn thread_is_daemon(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Thread { daemon, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(*daemon)))
}

pub(crate) fn thread_is_interrupted(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Thread { interrupted, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(*interrupted)))
}

pub(crate) fn thread_interrupted(_vm: &mut Vm, _args: &[JValue]) -> R {
    // Top-level execution always runs on the VM's implicit main thread.
    Ok(JValue::Int(0))
}

pub(crate) fn thread_init(vm: &mut Vm, args: &[JValue]) -> R {
    let (runnable, name) = match args.get(1).copied() {
        None => (JValue::Null, "Thread-0".to_owned()),
        Some(v) if matches!(payload(vm, v), Some(Native::Str(_))) => (JValue::Null, jstr(vm, v)?),
        Some(v) => {
            let name = match args.get(2).copied() {
                Some(n) => jstr(vm, n)?,
                None => "Thread-0".to_owned(),
            };
            (v, name)
        }
    };
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Thread {
            name: dst_name,
            runnable: dst_runnable,
            ..
        } => {
            *dst_name = name;
            *dst_runnable = runnable;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn thread_set_name(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    let Some(Native::Thread { name: dst, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = name;
    Ok(JValue::Null)
}

pub(crate) fn thread_set_daemon(vm: &mut Vm, args: &[JValue]) -> R {
    let daemon = int_of(vm, args[1]) != 0;
    let Some(Native::Thread {
        daemon: dst, alive, ..
    }) = payload_mut(vm, args[0])
    else {
        return Err(npe(vm));
    };
    if *alive {
        return Err(iae(vm, "cannot change daemon status of an active thread"));
    }
    *dst = daemon;
    Ok(JValue::Null)
}

pub(crate) fn thread_interrupt(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Thread { interrupted, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *interrupted = true;
    Ok(JValue::Null)
}

pub(crate) fn thread_run(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Thread { runnable, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let runnable = *runnable;
    if runnable.is_null() {
        return Ok(JValue::Null);
    }
    vm.invoke_virtual_args(runnable, "run", "()V", vec![])
        .map_err(nat_fatal)
}

pub(crate) fn thread_start(vm: &mut Vm, args: &[JValue]) -> R {
    let this = args[0];
    let Some(Native::Thread { started, alive, .. }) = payload_mut(vm, this) else {
        return Err(npe(vm));
    };
    if *started {
        return Err(iae(vm, "thread already started"));
    }
    *started = true;
    *alive = true;
    let result = vm
        .invoke_virtual_args(this, "run", "()V", vec![])
        .map_err(nat_fatal);
    if let Some(Native::Thread { alive, .. }) = payload_mut(vm, this) {
        *alive = false;
    }
    result
}

pub(crate) fn thread_sleep(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = match args[0] {
        JValue::Long(v) => v,
        JValue::Int(v) => i64::from(v),
        _ => return Err(iae(vm, "invalid sleep duration")),
    };
    let nanos = match args.get(1) {
        Some(JValue::Int(v)) => *v,
        Some(_) => return Err(iae(vm, "invalid nanosecond value")),
        None => 0,
    };
    if millis < 0 || !(0..=999_999).contains(&nanos) {
        return Err(iae(vm, "invalid sleep duration"));
    }
    std::thread::sleep(
        std::time::Duration::from_millis(millis as u64)
            + std::time::Duration::from_nanos(nanos as u64),
    );
    Ok(JValue::Null)
}

pub(crate) fn thread_join(_vm: &mut Vm, _args: &[JValue]) -> R {
    // `start` executes synchronously, so there is nothing left to wait for.
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
    ne!("Ljava/lang/Thread;", "start", "()V", true, thread_start),
    ne!("Ljava/lang/Thread;", "run", "()V", true, thread_run),
    ne!("Ljava/lang/Thread;", "yield", "()V", false, thread_yield),
    ne!(
        "Ljava/lang/Thread;",
        "interrupt",
        "()V",
        true,
        thread_interrupt
    ),
    ne!(
        "Ljava/lang/Thread;",
        "interrupted",
        "()Z",
        false,
        thread_interrupted
    ),
    ne!("Ljava/lang/Thread;", "sleep", "(J)V", false, thread_sleep),
    ne!("Ljava/lang/Thread;", "sleep", "(JI)V", false, thread_sleep),
    ne!(
        "Ljava/lang/Thread;",
        "setName",
        "(Ljava/lang/String;)V",
        true,
        thread_set_name
    ),
    ne!(
        "Ljava/lang/Thread;",
        "setDaemon",
        "(Z)V",
        true,
        thread_set_daemon
    ),
    ne!("Ljava/lang/Thread;", "join", "()V", true, thread_join),
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
    ne!(
        "Ljava/lang/Thread;",
        "getStackTrace",
        "()[Ljava/lang/StackTraceElement;",
        true,
        crate::vm::native::throwable_get_stack_trace
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, SandboxOptions};

    #[test]
    fn thread_properties_are_stateful_and_static_methods_are_static() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
        let vm = ctx.vm();
        let thread = alloc(
            vm,
            "Ljava/lang/Thread;",
            Native::Thread {
                name: "old".into(),
                daemon: false,
                alive: false,
                interrupted: false,
                started: false,
                runnable: JValue::Null,
            },
        )
        .unwrap();
        let name = vm.alloc_string("worker");
        thread_set_name(vm, &[thread, name]).unwrap();
        thread_set_daemon(vm, &[thread, JValue::Int(1)]).unwrap();
        thread_interrupt(vm, &[thread]).unwrap();
        let got_name = thread_get_name(vm, &[thread]).unwrap();
        assert_eq!(jstr(vm, got_name).unwrap(), "worker");
        assert_eq!(thread_is_daemon(vm, &[thread]).unwrap(), JValue::Int(1));
        assert_eq!(
            thread_is_interrupted(vm, &[thread]).unwrap(),
            JValue::Int(1)
        );

        for method in ["sleep", "yield"] {
            assert!(TABLE
                .iter()
                .filter(|e| e.name == method)
                .all(|e| !e.instance));
        }
    }
}
