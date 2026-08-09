//! java.util.HashSet host shims.

use crate::vm::native::*;

// ---- java.util.HashSet ----

pub(crate) fn set_init(vm: &mut Vm, args: &[JValue]) -> R {
    let items = if args.len() > 1 {
        match args[1] {
            JValue::Null => Vec::new(),
            JValue::Obj(_) => coll_elems(vm, args[1])?,
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Set(dst) => *dst = items,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn set_size(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Set(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(items.len() as i32))
}

pub(crate) fn set_is_empty(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Set(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(items.is_empty())))
}

pub(crate) fn set_contains(vm: &mut Vm, args: &[JValue]) -> R {
    let target = args[1];
    let Some(Native::Set(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let items = items.clone();
    for it in &items {
        if java_equals(vm, *it, target)? {
            return Ok(JValue::Int(1));
        }
    }
    Ok(JValue::Int(0))
}

pub(crate) fn set_add(vm: &mut Vm, args: &[JValue]) -> R {
    let v = args[1];
    let Some(Native::Set(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let items = items.clone();
    let kh = java_hash(vm, v);
    for it in &items {
        if java_hash(vm, *it) == kh && java_equals(vm, *it, v)? {
            return Ok(JValue::Int(0));
        }
    }
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Set(dst) => dst.push(v),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Int(1))
}

pub(crate) fn set_remove(vm: &mut Vm, args: &[JValue]) -> R {
    let target = args[1];
    let items = match payload(vm, args[0]) {
        Some(Native::Set(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    for (i, it) in items.iter().enumerate() {
        if java_equals(vm, *it, target)? {
            let Some(n) = payload_mut(vm, args[0]) else {
                return Err(npe(vm));
            };
            match n {
                Native::Set(dst) => {
                    dst.remove(i);
                }
                _ => return Err(npe(vm)),
            }
            return Ok(JValue::Int(1));
        }
    }
    Ok(JValue::Int(0))
}

pub(crate) fn set_clear(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Set(dst) => dst.clear(),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn set_iterator(vm: &mut Vm, args: &[JValue]) -> R {
    let set = args[0].as_obj();
    alloc(vm, "Ljava/util/Iterator;", Native::Iter(IterKind::Set { set, idx: 0 }))
}

pub(crate) fn set_add_all(vm: &mut Vm, args: &[JValue]) -> R {
    let extra = coll_elems(vm, args[1])?;
    let items = match payload(vm, args[0]) {
        Some(Native::Set(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut out = items.clone();
    let mut changed = false;
    for v in extra {
        let kh = java_hash(vm, v);
        let mut present = false;
        for it in &out {
            if java_hash(vm, *it) == kh && java_equals(vm, *it, v)? {
                present = true;
                break;
            }
        }
        if !present {
            out.push(v);
            changed = true;
        }
    }
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Set(dst) => *dst = out,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Int(i32::from(changed)))
}

pub(crate) fn set_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::Set(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut s = String::from("[");
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&to_string_of(vm, *it)?);
    }
    s.push(']');
    Ok(new_str(vm, &s))
}


/// Native methods for Ljava/util/HashSet;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/util/HashSet;", "<init>", "()V", true, set_init),
    ne!("Ljava/util/HashSet;", "<init>", "(I)V", true, set_init),
    ne!("Ljava/util/HashSet;", "<init>", "(Ljava/util/Collection;)V", true, set_init),
    ne!("Ljava/util/HashSet;", "size", "()I", true, set_size),
    ne!("Ljava/util/HashSet;", "isEmpty", "()Z", true, set_is_empty),
    ne!("Ljava/util/HashSet;", "contains", "(Ljava/lang/Object;)Z", true, set_contains),
    ne!("Ljava/util/HashSet;", "add", "(Ljava/lang/Object;)Z", true, set_add),
    ne!("Ljava/util/HashSet;", "remove", "(Ljava/lang/Object;)Z", true, set_remove),
    ne!("Ljava/util/HashSet;", "clear", "()V", true, set_clear),
    ne!("Ljava/util/HashSet;", "iterator", "()Ljava/util/Iterator;", true, set_iterator),
    ne!("Ljava/util/HashSet;", "addAll", "(Ljava/util/Collection;)Z", true, set_add_all),
    ne!("Ljava/util/HashSet;", "toString", "()Ljava/lang/String;", true, set_to_string),
];
