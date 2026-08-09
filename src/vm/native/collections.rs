use super::*;

// java.util collections
// ---------------------------------------------------------------------------

pub(crate) fn coll_elems(vm: &mut Vm, v: JValue) -> Result<Vec<JValue>, NatErr> {
    match v {
        JValue::Obj(_) => match payload(vm, v) {
            Some(Native::List(items)) => Ok(items.clone()),
            Some(Native::Set(items)) => Ok(items.clone()),
            Some(Native::Array(ArrayData::Obj(items))) => Ok(items.clone()),
            #[cfg(feature = "keiyoushi")]
            Some(Native::JsoupElements { doc, ids }) => {
                let doc = doc.clone();
                let ids = ids.clone();
                ids.into_iter()
                    .map(|id| {
                        alloc(
                            vm,
                            "Lorg/jsoup/nodes/Element;",
                            Native::JsoupElement { doc: doc.clone(), id },
                        )
                    })
                    .collect()
            }
            _ => Err(iae(vm, "not a collection")),
        },
        JValue::Null => Err(npe(vm)),
        _ => Err(iae(vm, "not a collection")),
    }
}

pub(crate) fn list_alloc(vm: &mut Vm, items: Vec<JValue>) -> Result<JValue, NatErr> {
    alloc(vm, "Ljava/util/ArrayList;", Native::List(items))
}

pub(crate) fn set_alloc(vm: &mut Vm, items: Vec<JValue>) -> Result<JValue, NatErr> {
    alloc(vm, "Ljava/util/HashSet;", Native::Set(items))
}

pub(crate) fn list_init(vm: &mut Vm, args: &[JValue]) -> R {
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
        Native::List(dst) => *dst = items,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn list_size(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::List(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(items.len() as i32))
}

pub(crate) fn list_is_empty(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::List(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(items.is_empty())))
}

pub(crate) fn list_get(vm: &mut Vm, args: &[JValue]) -> R {
    let i = int_of(vm, args[1]);
    let Some(Native::List(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    match items.get(i as usize) {
        Some(v) => Ok(*v),
        None => Err(ioobe(vm, i)),
    }
}

pub(crate) fn list_set(vm: &mut Vm, args: &[JValue]) -> R {
    let i = int_of(vm, args[1]);
    let new_v = args[2];
    let Some(Native::List(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some(_) = items.get(i as usize) else {
        return Err(ioobe(vm, i));
    };
    let old = items[i as usize];
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => dst[i as usize] = new_v,
        _ => return Err(npe(vm)),
    }
    Ok(old)
}

pub(crate) fn list_add(vm: &mut Vm, args: &[JValue]) -> R {
    let v = args[1];
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => dst.push(v),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Int(1))
}

pub(crate) fn list_add_at(vm: &mut Vm, args: &[JValue]) -> R {
    let i = int_of(vm, args[1]);
    let v = args[2];
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => {
            if i < 0 || i as usize > dst.len() {
                return Err(ioobe(vm, i));
            }
            dst.insert(i as usize, v);
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn list_remove_at(vm: &mut Vm, args: &[JValue]) -> R {
    let i = int_of(vm, args[1]);
    let Some(Native::List(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some(&old) = items.get(i as usize) else {
        return Err(ioobe(vm, i));
    };
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => {
            dst.remove(i as usize);
        }
        _ => return Err(npe(vm)),
    }
    Ok(old)
}

pub(crate) fn list_remove_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let target = args[1];
    let items = match payload(vm, args[0]) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut idx = None;
    for (i, it) in items.iter().enumerate() {
        if java_equals(vm, *it, target)? {
            idx = Some(i);
            break;
        }
    }
    match idx {
        Some(i) => {
            let Some(n) = payload_mut(vm, args[0]) else {
                return Err(npe(vm));
            };
            match n {
                Native::List(dst) => {
                    dst.remove(i);
                }
                _ => return Err(npe(vm)),
            }
            Ok(JValue::Int(1))
        }
        None => Ok(JValue::Int(0)),
    }
}

pub(crate) fn list_clear(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => dst.clear(),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn list_contains(vm: &mut Vm, args: &[JValue]) -> R {
    let target = args[1];
    let Some(Native::List(items)) = payload(vm, args[0]) else {
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

pub(crate) fn list_index_of(vm: &mut Vm, args: &[JValue]) -> R {
    let target = args[1];
    let items = match payload(vm, args[0]) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    for (i, it) in items.iter().enumerate() {
        if java_equals(vm, *it, target)? {
            return Ok(JValue::Int(i as i32));
        }
    }
    Ok(JValue::Int(-1))
}

pub(crate) fn list_last_index_of(vm: &mut Vm, args: &[JValue]) -> R {
    let target = args[1];
    let items = match payload(vm, args[0]) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    for (i, it) in items.iter().enumerate().rev() {
        if java_equals(vm, *it, target)? {
            return Ok(JValue::Int(i as i32));
        }
    }
    Ok(JValue::Int(-1))
}

pub(crate) fn list_iterator(vm: &mut Vm, args: &[JValue]) -> R {
    let list = args[0].as_obj();
    alloc(vm, "Ljava/util/Iterator;", Native::Iter(IterKind::List { list, idx: 0 }))
}

pub(crate) fn list_to_array(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let class = vm.ensure_class_by_desc("[Ljava/lang/Object;").map_err(nat_fatal)?;
    Ok(JValue::Obj(vm.arena.alloc(
        class,
        Vec::new(),
        Some(Native::Array(ArrayData::Obj(items))),
    )))
}

pub(crate) fn list_to_array_typed(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Obj(dst))) if dst.len() >= items.len() => {
            let mut out = dst.clone();
            for (i, it) in items.iter().enumerate() {
                out[i] = *it;
            }
            if out.len() > items.len() {
                out[items.len()] = JValue::Null;
            }
            let Some(n) = payload_mut(vm, args[1]) else {
                return Err(npe(vm));
            };
            match n {
                Native::Array(ArrayData::Obj(d)) => *d = out,
                _ => return Err(npe(vm)),
            }
            Ok(args[1])
        }
        _ => list_to_array(vm, args),
    }
}

pub(crate) fn list_add_all(vm: &mut Vm, args: &[JValue]) -> R {
    let extra = coll_elems(vm, args[1])?;
    let at = if args.len() > 2 {
        Some(int_of(vm, args[2]))
    } else {
        None
    };
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => match at {
            Some(i) => {
                if i < 0 || i as usize > dst.len() {
                    return Err(ioobe(vm, i));
                }
                for (k, v) in extra.into_iter().enumerate() {
                    dst.insert(i as usize + k, v);
                }
            }
            None => dst.extend(extra),
        },
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Int(1))
}

pub(crate) fn list_remove_all(vm: &mut Vm, args: &[JValue]) -> R {
    let coll = coll_elems(vm, args[1])?;
    let items = match payload(vm, args[0]) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut out = Vec::new();
    for it in items {
        let mut keep = true;
        for c in &coll {
            if java_equals(vm, it, *c)? {
                keep = false;
                break;
            }
        }
        if keep {
            out.push(it);
        }
    }
    let changed = out.len() != payload(vm, args[0]).map(|n| match n { Native::List(i) => i.len(), _ => 0 }).unwrap_or(0);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => *dst = out,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Int(i32::from(changed)))
}

pub(crate) fn list_retain_all(vm: &mut Vm, args: &[JValue]) -> R {
    let coll = coll_elems(vm, args[1])?;
    let items = match payload(vm, args[0]) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut out = Vec::new();
    for it in items {
        for c in &coll {
            if java_equals(vm, it, *c)? {
                out.push(it);
                break;
            }
        }
    }
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => *dst = out,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn list_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::List(items)) => items.clone(),
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

pub(crate) fn list_sort_cmp(vm: &mut Vm, args: &[JValue]) -> R {
    let list = args[0].as_obj();
    let cmp = args[1];
    let items = match payload(vm, JValue::Obj(list)) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut items = items;
    let mut err: Option<NatErr> = None;
    items.sort_by(|a, b| {
        match inv_virt(vm, cmp, "compare", "(Ljava/lang/Object;Ljava/lang/Object;)I", &[*a, *b]) {
            Ok(JValue::Int(i)) => i.cmp(&0),
            Ok(_) => Ordering::Equal,
            Err(e) => {
                err = Some(e);
                Ordering::Equal
            }
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    let Some(n) = payload_mut(vm, JValue::Obj(list)) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => *dst = items,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

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

pub(crate) fn map_find(vm: &mut Vm, entries: &[(JValue, JValue)], key: JValue) -> Result<Option<usize>, NatErr> {
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
    Ok(JValue::Int(i32::from(map_find(vm, &entries, args[1])?.is_some())))
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
        let entry = alloc(vm, "Ljava/util/Map$Entry;", Native::MapEntry { map: map_id, idx: i })?;
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

// ---- java.util.Map$Entry / java.util.Iterator ----

pub(crate) fn entry_get_key(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::MapEntry { map, idx }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some(Native::Map(entries)) = payload(vm, JValue::Obj(*map)) else {
        return Err(npe(vm));
    };
    match entries.get(*idx) {
        Some((k, _)) => Ok(*k),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn entry_get_value(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::MapEntry { map, idx }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let Some(Native::Map(entries)) = payload(vm, JValue::Obj(*map)) else {
        return Err(npe(vm));
    };
    match entries.get(*idx) {
        Some((_, v)) => Ok(*v),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn entry_set_value(vm: &mut Vm, args: &[JValue]) -> R {
    let (map, idx) = match payload(vm, args[0]) {
        Some(Native::MapEntry { map, idx }) => (*map, *idx),
        _ => return Err(npe(vm)),
    };
    let old = match payload(vm, JValue::Obj(map)) {
        Some(Native::Map(entries)) => entries.get(idx).map(|(_, v)| *v).unwrap_or(JValue::Null),
        _ => return Err(npe(vm)),
    };
    let val = args[1];
    let Some(n) = payload_mut(vm, JValue::Obj(map)) else {
        return Err(npe(vm));
    };
    match n {
        Native::Map(dst) => {
            if let Some(e) = dst.get_mut(idx) {
                e.1 = val;
            }
        }
        _ => return Err(npe(vm)),
    }
    Ok(old)
}

pub(crate) fn iter_has_next(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Iter(k)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let (coll, idx) = match k {
        IterKind::List { list, idx } => (JValue::Obj(*list), *idx),
        IterKind::Set { set, idx } => (JValue::Obj(*set), *idx),
        IterKind::MapEntries { map, idx } | IterKind::MapKeys { map, idx } | IterKind::MapValues { map, idx } => {
            (JValue::Obj(*map), *idx)
        }
    };
    let len = match payload(vm, coll) {
        Some(Native::List(items)) => items.len(),
        Some(Native::Set(items)) => items.len(),
        #[cfg(feature = "keiyoushi")]
        Some(Native::SFilterList(items)) => items.len(),
        Some(Native::Map(entries)) => entries.len(),
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(i32::from(idx < len)))
}

pub(crate) fn iter_next(vm: &mut Vm, args: &[JValue]) -> R {
    let k = match payload(vm, args[0]) {
        Some(Native::Iter(k)) => k.clone(),
        _ => return Err(npe(vm)),
    };
    let (coll, idx) = match &k {
        IterKind::List { list, idx } => (JValue::Obj(*list), *idx),
        IterKind::Set { set, idx } => (JValue::Obj(*set), *idx),
        IterKind::MapEntries { map, idx } | IterKind::MapKeys { map, idx } | IterKind::MapValues { map, idx } => {
            (JValue::Obj(*map), *idx)
        }
    };
    let item = match &k {
        IterKind::List { .. } => match payload(vm, coll) {
            Some(Native::List(items)) => items
                .get(idx)
                .copied()
                .ok_or_else(|| no_such_elem(vm)),
            #[cfg(feature = "keiyoushi")]
            Some(Native::SFilterList(items)) => items
                .get(idx)
                .copied()
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
        IterKind::Set { .. } => match payload(vm, coll) {
            Some(Native::Set(items)) => items
                .get(idx)
                .copied()
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
        IterKind::MapEntries { map, .. } => {
            alloc(vm, "Ljava/util/Map$Entry;", Native::MapEntry { map: *map, idx })
        }
        IterKind::MapKeys { .. } => match payload(vm, coll) {
            Some(Native::Map(entries)) => entries
                .get(idx)
                .map(|(k, _)| *k)
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
        IterKind::MapValues { .. } => match payload(vm, coll) {
            Some(Native::Map(entries)) => entries
                .get(idx)
                .map(|(_, v)| *v)
                .ok_or_else(|| no_such_elem(vm)),
            _ => Err(npe(vm)),
        },
    }?;
    let next_idx = idx + 1;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Iter(k2) => match k2 {
            IterKind::List { idx: i, .. }
            | IterKind::Set { idx: i, .. }
            | IterKind::MapEntries { idx: i, .. }
            | IterKind::MapKeys { idx: i, .. }
            | IterKind::MapValues { idx: i, .. } => *i = next_idx,
        },
        _ => return Err(npe(vm)),
    }
    Ok(item)
}

pub(crate) fn iter_remove(_vm: &mut Vm, _args: &[JValue]) -> R {
    Err(uoe(_vm, "Iterator.remove"))
}

// ---------------------------------------------------------------------------
// java.util.Collections (all static)
// ---------------------------------------------------------------------------

pub(crate) fn collections_empty_list(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/ArrayList;", Native::List(Vec::new()))
}

pub(crate) fn collections_empty_set(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/HashSet;", Native::Set(Vec::new()))
}

pub(crate) fn collections_empty_map(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/HashMap;", Native::Map(Vec::new()))
}

pub(crate) fn collections_singleton(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/HashSet;", Native::Set(vec![args[0]]))
}

pub(crate) fn collections_singleton_list(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/ArrayList;", Native::List(vec![args[0]]))
}

pub(crate) fn collections_singleton_map(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/HashMap;", Native::Map(vec![(args[0], args[1])]))
}

pub(crate) fn collections_identity(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn collections_sort(vm: &mut Vm, args: &[JValue]) -> R {
    let list = args[0].as_obj();
    let items = match payload(vm, JValue::Obj(list)) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut items = items;
    let mut err: Option<NatErr> = None;
    items.sort_by(|a, b| match java_cmp(vm, *a, *b) {
        Ok(o) => o,
        Err(e) => {
            err = Some(e);
            Ordering::Equal
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    let Some(n) = payload_mut(vm, JValue::Obj(list)) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => *dst = items,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn collections_reverse(vm: &mut Vm, args: &[JValue]) -> R {
    let list = args[0].as_obj();
    let items = match payload(vm, JValue::Obj(list)) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut items = items;
    items.reverse();
    let Some(n) = payload_mut(vm, JValue::Obj(list)) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => *dst = items,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn collections_add_all(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Obj(items))) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::List(dst) => dst.extend_from_slice(&items),
        Native::Set(dst) => dst.extend_from_slice(&items),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Int(i32::from(!items.is_empty())))
}

// ---------------------------------------------------------------------------
// java.util.Arrays (all static)
// ---------------------------------------------------------------------------

pub(crate) fn arrays_as_list(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Obj(items))) => items.clone(),
        _ => return Err(npe(vm)),
    };
    alloc(vm, "Ljava/util/ArrayList;", Native::List(items))
}

pub(crate) fn arrays_copy_of(vm: &mut Vm, args: &[JValue]) -> R {
    let (elem, src) = match payload(vm, args[0]) {
        Some(Native::Array(data)) => (data.elem_desc().to_string(), data.len()),
        _ => return Err(npe(vm)),
    };
    let n = int_of(vm, args[1]);
    if n < 0 {
        return Err(NatErr::Throw(vm.err_neg_arr_size()));
    }
    let mut dst = ArrayData::new(&elem, n as usize);
    let copy = src.min(n as usize);
    for i in 0..copy {
        let v = match payload(vm, args[0]) {
            Some(Native::Array(data)) => data.get(i),
            _ => return Err(npe(vm)),
        };
        dst.set(i, v);
    }
    let e = elem.clone();
    alloc_arr(vm, &e, n as usize, move || dst)
}

pub(crate) fn arrays_copy_of_range(vm: &mut Vm, args: &[JValue]) -> R {
    let (elem, src_len) = match payload(vm, args[0]) {
        Some(Native::Array(data)) => (data.elem_desc().to_string(), data.len()),
        _ => return Err(npe(vm)),
    };
    let from = int_of(vm, args[1]);
    let to = int_of(vm, args[2]);
    if from < 0 || to < from || to as usize > src_len {
        return Err(aioobe(vm, to, src_len as i32));
    }
    let n = (to - from) as usize;
    let mut dst = ArrayData::new(&elem, n);
    for i in 0..n {
        let v = match payload(vm, args[0]) {
            Some(Native::Array(data)) => data.get(from as usize + i),
            _ => return Err(npe(vm)),
        };
        dst.set(i, v);
    }
    let e = elem.clone();
    alloc_arr(vm, &e, n, move || dst)
}

pub(crate) fn prim_ordering(a: JValue, b: JValue) -> Ordering {
    match (a, b) {
        (JValue::Int(x), JValue::Int(y)) => x.cmp(&y),
        (JValue::Long(x), JValue::Long(y)) => x.cmp(&y),
        (JValue::Int(x), JValue::Long(y)) => (x as i64).cmp(&y),
        (JValue::Long(x), JValue::Int(y)) => x.cmp(&(y as i64)),
        (JValue::Float(x), JValue::Float(y)) => match (x.is_nan(), y.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        },
        (JValue::Double(x), JValue::Double(y)) => match (x.is_nan(), y.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        },
        _ => Ordering::Equal,
    }
}

pub(crate) fn arrays_sort_prim(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::Array(data)) => (0..data.len()).map(|i| data.get(i)).collect::<Vec<_>>(),
        _ => return Err(npe(vm)),
    };
    let mut items = items;
    items.sort_by(|a, b| prim_ordering(*a, *b));
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Array(dst) => {
            for (i, v) in items.iter().enumerate() {
                dst.set(i, *v);
            }
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn arrays_sort_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::Array(data)) => (0..data.len()).map(|i| data.get(i)).collect::<Vec<_>>(),
        _ => return Err(npe(vm)),
    };
    let mut items = items;
    let mut err: Option<NatErr> = None;
    items.sort_by(|a, b| match java_cmp(vm, *a, *b) {
        Ok(o) => o,
        Err(e) => {
            err = Some(e);
            Ordering::Equal
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Array(dst) => {
            for (i, v) in items.iter().enumerate() {
                dst.set(i, *v);
            }
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn arrays_sort_obj_cmp(vm: &mut Vm, args: &[JValue]) -> R {
    let cmp = args[1];
    let items = match payload(vm, args[0]) {
        Some(Native::Array(data)) => (0..data.len()).map(|i| data.get(i)).collect::<Vec<_>>(),
        _ => return Err(npe(vm)),
    };
    let mut items = items;
    let mut err: Option<NatErr> = None;
    items.sort_by(|a, b| {
        match inv_virt(vm, cmp, "compare", "(Ljava/lang/Object;Ljava/lang/Object;)I", &[*a, *b]) {
            Ok(JValue::Int(i)) => i.cmp(&0),
            Ok(_) => Ordering::Equal,
            Err(e) => {
                err = Some(e);
                Ordering::Equal
            }
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Array(dst) => {
            for (i, v) in items.iter().enumerate() {
                dst.set(i, *v);
            }
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn elem_fmt(v: JValue) -> String {
    match v {
        JValue::Int(i) => i.to_string(),
        JValue::Long(l) => l.to_string(),
        JValue::Float(f) => fmt_f32(f),
        JValue::Double(d) => fmt_f64(d),
        _ => format!("{v:?}"),
    }
}

pub(crate) fn arrays_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let (elem, items) = match payload(vm, args[0]) {
        Some(Native::Array(data)) => {
            let elem = data.elem_desc().to_string();
            let items: Vec<JValue> = (0..data.len()).map(|i| data.get(i)).collect();
            (elem, items)
        }
        _ => return Err(npe(vm)),
    };
    let parts: Vec<String> = items
        .iter()
        .map(|v| match *v {
            JValue::Null => "null".to_string(),
            JValue::Obj(_) => to_string_of(vm, *v).unwrap_or_else(|_| "null".to_string()),
            v => {
                if elem == "C" {
                    char::from_u32(int_of(vm, v) as u32)
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "\u{0}".to_string())
                } else if elem == "Z" {
                    if int_of(vm, v) != 0 {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    }
                } else {
                    elem_fmt(v)
                }
            }
        })
        .collect();
    Ok(new_str(vm, &format!("[{}]", parts.join(", "))))
}

pub(crate) fn arrays_fill(vm: &mut Vm, args: &[JValue]) -> R {
    let v = args[1];
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Array(data) => {
            for i in 0..data.len() {
                data.set(i, v);
            }
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn arrays_equals(vm: &mut Vm, args: &[JValue]) -> R {
    let (la, av) = match payload(vm, args[0]) {
        Some(Native::Array(data)) => (data.len(), (0..data.len()).map(|i| data.get(i)).collect::<Vec<_>>()),
        _ => return Err(npe(vm)),
    };
    let (lb, bv) = match payload(vm, args[1]) {
        Some(Native::Array(data)) => (data.len(), (0..data.len()).map(|i| data.get(i)).collect::<Vec<_>>()),
        _ => return Err(npe(vm)),
    };
    if la != lb {
        return Ok(JValue::Int(0));
    }
    for i in 0..la {
        if !java_equals(vm, av[i], bv[i])? {
            return Ok(JValue::Int(0));
        }
    }
    Ok(JValue::Int(1))
}

// ---------------------------------------------------------------------------
