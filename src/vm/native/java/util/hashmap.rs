//! java.util.HashMap host shims.

use crate::vm::native::*;

// ---- java.util.HashMap ----

pub(crate) fn map_init(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = if args.len() > 1 && !args[1].is_null() {
        match payload(vm, args[1]) {
            Some(Native::Map(src)) => src.clone(),
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Map(dst) => *dst = entries,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn map_size(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Map(entries)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(entries.len() as i32))
}

pub(crate) fn map_is_empty(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Map(entries)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(entries.is_empty())))
}

pub(crate) fn map_find(
    vm: &mut Vm,
    entries: &[(JValue, JValue)],
    key: JValue,
) -> Result<Option<usize>, NatErr> {
    let kh = java_hash(vm, key);
    for (i, (ek, _)) in entries.iter().enumerate() {
        if java_hash(vm, *ek) == kh && java_equals(vm, *ek, key)? {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

pub(crate) fn map_get(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    match map_find(vm, &entries, args[1])? {
        Some(i) => Ok(entries[i].1),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn map_get_default(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    match map_find(vm, &entries, args[1])? {
        Some(i) => Ok(entries[i].1),
        None => Ok(args[2]),
    }
}

pub(crate) fn map_put(vm: &mut Vm, args: &[JValue]) -> R {
    let key = args[1];
    let val = args[2];
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let kh = java_hash(vm, key);
    let mut out = Vec::with_capacity(entries.len() + 1);
    let mut replaced = JValue::Null;
    let mut found = false;
    for (ek, ev) in entries {
        if java_hash(vm, ek) == kh && java_equals(vm, ek, key)? {
            replaced = ev;
            out.push((ek, val));
            found = true;
        } else {
            out.push((ek, ev));
        }
    }
    if !found {
        out.push((key, val));
    }
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Map(dst) => *dst = out,
        _ => return Err(npe(vm)),
    }
    Ok(replaced)
}

pub(crate) fn map_put_all(vm: &mut Vm, args: &[JValue]) -> R {
    let src = match payload(vm, args[1]) {
        Some(Native::Map(src)) => src.clone(),
        _ => return Err(iae(vm, "not a map")),
    };
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let mut out = entries.clone();
    for (k, v) in src {
        if let Some(i) = map_find(vm, &out, k)? {
            out[i] = (k, v);
        } else {
            out.push((k, v));
        }
    }
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Map(dst) => *dst = out,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn map_put_if_absent(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    match map_find(vm, &entries, args[1])? {
        Some(i) => Ok(entries[i].1),
        None => {
            let val = args[2];
            let mut out = entries;
            out.push((args[1], val));
            let Some(n) = payload_mut(vm, args[0]) else {
                return Err(npe(vm));
            };
            match n {
                Native::Map(dst) => *dst = out,
                _ => return Err(npe(vm)),
            }
            Ok(JValue::Null)
        }
    }
}

pub(crate) fn map_contains_key(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(i32::from(
        map_find(vm, &entries, args[1])?.is_some(),
    )))
}

pub(crate) fn map_contains_value(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let target = args[1];
    for (_, v) in &entries {
        if java_equals(vm, *v, target)? {
            return Ok(JValue::Int(1));
        }
    }
    Ok(JValue::Int(0))
}

pub(crate) fn map_remove(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let key = args[1];
    let must_value = if args.len() > 2 { Some(args[2]) } else { None };
    let mut out = Vec::new();
    let mut removed = JValue::Null;
    let mut did_remove = false;
    for (ek, ev) in entries {
        if !did_remove && java_hash(vm, ek) == java_hash(vm, key) && java_equals(vm, ek, key)? {
            if let Some(mv) = must_value {
                if !java_equals(vm, ev, mv)? {
                    out.push((ek, ev));
                    continue;
                }
            }
            removed = ev;
            did_remove = true;
        } else {
            out.push((ek, ev));
        }
    }
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Map(dst) => *dst = out,
        _ => return Err(npe(vm)),
    }
    if must_value.is_some() {
        Ok(JValue::Int(i32::from(did_remove)))
    } else {
        Ok(removed)
    }
}

pub(crate) fn map_clear(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Map(dst) => dst.clear(),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn map_keys(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let keys: Vec<JValue> = entries.iter().map(|(k, _)| *k).collect();
    set_alloc(vm, keys)
}

pub(crate) fn map_values(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let vals: Vec<JValue> = entries.iter().map(|(_, v)| *v).collect();
    set_alloc(vm, vals)
}

pub(crate) fn map_entries(vm: &mut Vm, args: &[JValue]) -> R {
    let map_id = args[0].as_obj();
    let entries = match payload(vm, JValue::Obj(map_id)) {
        Some(Native::Map(entries)) => entries.len(),
        _ => return Err(npe(vm)),
    };
    let mut out = Vec::with_capacity(entries);
    for i in 0..entries {
        let entry = alloc(
            vm,
            "Ljava/util/Map$Entry;",
            Native::MapEntry {
                map: map_id,
                idx: i,
            },
        )?;
        out.push(entry);
    }
    set_alloc(vm, out)
}

pub(crate) fn map_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let mut s = String::from("{");
    for (i, (k, v)) in entries.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&to_string_of(vm, *k)?);
        s.push('=');
        s.push_str(&to_string_of(vm, *v)?);
    }
    s.push('}');
    Ok(new_str(vm, &s))
}

/// Native methods for Ljava/util/HashMap;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/util/HashMap;", "<init>", "()V", true, map_init),
    ne!("Ljava/util/HashMap;", "<init>", "(I)V", true, map_init),
    ne!("Ljava/util/HashMap;", "<init>", "(IF)V", true, map_init),
    ne!(
        "Ljava/util/HashMap;",
        "<init>",
        "(Ljava/util/Map;)V",
        true,
        map_init
    ),
    ne!("Ljava/util/HashMap;", "size", "()I", true, map_size),
    ne!("Ljava/util/HashMap;", "isEmpty", "()Z", true, map_is_empty),
    ne!(
        "Ljava/util/HashMap;",
        "get",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        map_get
    ),
    ne!(
        "Ljava/util/HashMap;",
        "getOrDefault",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        map_get_default
    ),
    ne!(
        "Ljava/util/HashMap;",
        "put",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        map_put
    ),
    ne!(
        "Ljava/util/HashMap;",
        "putAll",
        "(Ljava/util/Map;)V",
        true,
        map_put_all
    ),
    ne!(
        "Ljava/util/HashMap;",
        "putIfAbsent",
        "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        map_put_if_absent
    ),
    ne!(
        "Ljava/util/HashMap;",
        "containsKey",
        "(Ljava/lang/Object;)Z",
        true,
        map_contains_key
    ),
    ne!(
        "Ljava/util/HashMap;",
        "containsValue",
        "(Ljava/lang/Object;)Z",
        true,
        map_contains_value
    ),
    ne!(
        "Ljava/util/HashMap;",
        "remove",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        map_remove
    ),
    ne!(
        "Ljava/util/HashMap;",
        "remove",
        "(Ljava/lang/Object;Ljava/lang/Object;)Z",
        true,
        map_remove
    ),
    ne!("Ljava/util/HashMap;", "clear", "()V", true, map_clear),
    ne!(
        "Ljava/util/HashMap;",
        "keySet",
        "()Ljava/util/Set;",
        true,
        map_keys
    ),
    ne!(
        "Ljava/util/HashMap;",
        "values",
        "()Ljava/util/Collection;",
        true,
        map_values
    ),
    ne!(
        "Ljava/util/HashMap;",
        "entrySet",
        "()Ljava/util/Set;",
        true,
        map_entries
    ),
    ne!(
        "Ljava/util/HashMap;",
        "toString",
        "()Ljava/lang/String;",
        true,
        map_to_string
    ),
];
