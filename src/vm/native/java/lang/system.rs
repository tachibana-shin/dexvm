//! java.lang.System host shims.

use crate::permission::Permission;
use crate::vm::native::*;

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

pub(crate) fn runtime_get_runtime(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/lang/Runtime;", Native::Opaque)
}

pub(crate) fn runtime_available_processors(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(std::thread::available_parallelism().map_or(1, |n| n.get() as i32)))
}

pub(crate) fn arrcopy_into(
    src: &ArrayData,
    sp: usize,
    dst: &mut ArrayData,
    dp: usize,
    len: usize,
) -> bool {
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
            let ok = arrcopy_into(
                &src_data,
                src_pos as usize,
                dst_data,
                dst_pos as usize,
                len as usize,
            );
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

pub(crate) fn sys_get_property(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[0])?;
    let value = match key.as_str() {
        "java.version" => Some("17"),
        "java.vm.name" => Some("dexvm"),
        "java.vendor" => Some("dexvm"),
        "line.separator" => Some("\n"),
        "file.separator" => Some(if cfg!(windows) { "\\" } else { "/" }),
        "path.separator" => Some(if cfg!(windows) { ";" } else { ":" }),
        "os.name" => Some(std::env::consts::OS),
        "os.arch" => Some(std::env::consts::ARCH),
        _ => None,
    };
    match value {
        Some(value) => Ok(new_str(vm, value)),
        None => Ok(args.get(1).copied().unwrap_or(JValue::Null)),
    }
}

pub(crate) fn sys_getenv(vm: &mut Vm, args: &[JValue]) -> R {
    check_native_permission(vm, &Permission::Env)?;
    let key = jstr(vm, args[0])?;
    match std::env::var(key) {
        Ok(value) => Ok(new_str(vm, &value)),
        Err(_) => Ok(JValue::Null),
    }
}

pub(crate) fn sys_getenv_all(vm: &mut Vm, _args: &[JValue]) -> R {
    check_native_permission(vm, &Permission::Env)?;
    let vars = std::env::vars().collect::<Vec<_>>();
    let mut entries = Vec::with_capacity(vars.len());
    for (key, value) in vars {
        entries.push((new_str(vm, &key), new_str(vm, &value)));
    }
    alloc(vm, "Ljava/util/HashMap;", Native::Map(entries))
}

pub(crate) fn sys_line_separator(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(new_str(vm, "\n"))
}

/// Native methods for Ljava/lang/System;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/System;",
        "currentTimeMillis",
        "()J",
        false,
        sys_current_time_millis
    ),
    ne!(
        "Ljava/lang/System;",
        "nanoTime",
        "()J",
        false,
        sys_nano_time
    ),
    ne!(
        "Ljava/lang/System;",
        "arraycopy",
        "(Ljava/lang/Object;ILjava/lang/Object;II)V",
        false,
        sys_arraycopy
    ),
    ne!("Ljava/lang/System;", "exit", "(I)V", false, sys_exit),
    ne!("Ljava/lang/System;", "gc", "()V", false, sys_gc),
    ne!(
        "Ljava/lang/System;",
        "identityHashCode",
        "(Ljava/lang/Object;)I",
        false,
        sys_identity_hash_code
    ),
    ne!(
        "Ljava/lang/System;",
        "getProperty",
        "(Ljava/lang/String;)Ljava/lang/String;",
        false,
        sys_get_property
    ),
    ne!(
        "Ljava/lang/System;",
        "getProperty",
        "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
        false,
        sys_get_property
    ),
    ne!(
        "Ljava/lang/System;",
        "getenv",
        "(Ljava/lang/String;)Ljava/lang/String;",
        false,
        sys_getenv
    ),
    ne!(
        "Ljava/lang/System;",
        "getenv",
        "()Ljava/util/Map;",
        false,
        sys_getenv_all
    ),
    ne!(
        "Ljava/lang/System;",
        "lineSeparator",
        "()Ljava/lang/String;",
        false,
        sys_line_separator
    ),
    ne!(
        "Ljava/lang/Runtime;",
        "getRuntime",
        "()Ljava/lang/Runtime;",
        false,
        runtime_get_runtime
    ),
    ne!(
        "Ljava/lang/Runtime;",
        "availableProcessors",
        "()I",
        true,
        runtime_available_processors
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, SandboxOptions};

    #[test]
    fn properties_are_virtual_and_environment_is_permissioned() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut denied = Context::new(&data).unwrap();
        let vm = denied.vm();
        let key = vm.alloc_string("java.vm.name");
        let value = sys_get_property(vm, &[key]).unwrap();
        assert_eq!(jstr(vm, value).unwrap(), "dexvm");

        let key = vm.alloc_string("PATH");
        assert!(matches!(sys_getenv(vm, &[key]), Err(NatErr::Throw(_))));

        let mut allowed = Context::new_with(
            &data,
            SandboxOptions {
                env: true,
                ..SandboxOptions::default()
            },
        )
        .unwrap();
        let vm = allowed.vm();
        let key = vm.alloc_string("DEXVM_VARIABLE_THAT_SHOULD_NOT_EXIST");
        assert!(sys_getenv(vm, &[key]).unwrap().is_null());
    }
}
