//! java.util.ArrayList host shims.

use crate::vm::native::*;

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
    if std::env::var("DEXVM_TRACE").is_ok()
        && matches!(v, crate::vm::value::JValue::Int(_))
    {
        let len = match payload(vm, args[0]) {
            Some(Native::List(items)) => items.len(),
            _ => 0,
        };
        eprintln!("DEXTRACE list_add INT: list={:?} len={len}", args[0]);
    }
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
    alloc(
        vm,
        "Ljava/util/Iterator;",
        Native::Iter(IterKind::List { list, idx: 0 }),
    )
}

pub(crate) fn list_iterator_at(vm: &mut Vm, args: &[JValue]) -> R {
    let list = args[0].as_obj();
    let idx = int_of(vm, args[1]).max(0) as usize;
    alloc(
        vm,
        "Ljava/util/ListIterator;",
        Native::Iter(IterKind::List { list, idx }),
    )
}

pub(crate) fn list_sublist(vm: &mut Vm, args: &[JValue]) -> R {
    let from = int_of(vm, args[1]);
    let to = int_of(vm, args[2]);
    let Some(Native::List(items)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    if from < 0 || to < from || to as usize > items.len() {
        return Err(ioobe(vm, to));
    }
    let slice = items[from as usize..to as usize].to_vec();
    list_alloc(vm, slice)
}

pub(crate) fn list_ensure_capacity(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn list_to_array(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::List(items)) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let class = vm
        .ensure_class_by_desc("[Ljava/lang/Object;")
        .map_err(nat_fatal)?;
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
    if let Some(Native::Array(ArrayData::Obj(dst))) = payload(vm, args[1]) {
        if dst.len() >= items.len() {
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
            return Ok(args[1]);
        }
        let cls = vm.arena.objects[args[1].as_obj() as usize].class;
        return Ok(JValue::Obj(vm.arena.alloc(
            cls,
            Vec::new(),
            Some(Native::Array(ArrayData::Obj(items))),
        )));
    }
    list_to_array(vm, args)
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
    let changed = out.len()
        != payload(vm, args[0])
            .map(|n| match n {
                Native::List(i) => i.len(),
                _ => 0,
            })
            .unwrap_or(0);
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
        match inv_virt(
            vm,
            cmp,
            "compare",
            "(Ljava/lang/Object;Ljava/lang/Object;)I",
            &[*a, *b],
        ) {
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

/// Native methods for Ljava/util/ArrayList;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/AbstractCollection;",
        "size",
        "()I",
        true,
        list_size
    ),
    ne!(
        "Ljava/util/AbstractCollection;",
        "isEmpty",
        "()Z",
        true,
        list_is_empty
    ),
    ne!(
        "Ljava/util/AbstractList;",
        "get",
        "(I)Ljava/lang/Object;",
        true,
        list_get
    ),
    ne!(
        "Ljava/util/AbstractList;",
        "remove",
        "(I)Ljava/lang/Object;",
        true,
        list_remove_at
    ),
    ne!(
        "Ljava/util/AbstractList;",
        "subList",
        "(II)Ljava/util/List;",
        true,
        list_sublist
    ),
    ne!(
        "Ljava/util/AbstractCollection;",
        "add",
        "(Ljava/lang/Object;)Z",
        true,
        coll_generic_add
    ),
    ne!(
        "Ljava/util/AbstractCollection;",
        "remove",
        "(Ljava/lang/Object;)Z",
        true,
        coll_generic_remove
    ),
    ne!(
        "Ljava/util/AbstractCollection;",
        "clear",
        "()V",
        true,
        coll_generic_clear
    ),
    ne!(
        "Ljava/util/AbstractCollection;",
        "contains",
        "(Ljava/lang/Object;)Z",
        true,
        coll_generic_contains
    ),
    ne!(
        "Ljava/util/AbstractCollection;",
        "containsAll",
        "(Ljava/util/Collection;)Z",
        true,
        coll_generic_contains_all
    ),
    ne!(
        "Ljava/util/AbstractCollection;",
        "addAll",
        "(Ljava/util/Collection;)Z",
        true,
        coll_generic_add_all
    ),
    ne!("Ljava/util/ArrayList;", "<init>", "()V", true, list_init),
    ne!("Ljava/util/ArrayList;", "<init>", "(I)V", true, list_init),
    ne!(
        "Ljava/util/ArrayList;",
        "<init>",
        "(Ljava/util/Collection;)V",
        true,
        list_init
    ),
    ne!("Ljava/util/ArrayList;", "size", "()I", true, list_size),
    ne!(
        "Ljava/util/ArrayList;",
        "isEmpty",
        "()Z",
        true,
        list_is_empty
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "get",
        "(I)Ljava/lang/Object;",
        true,
        list_get
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "set",
        "(ILjava/lang/Object;)Ljava/lang/Object;",
        true,
        list_set
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "add",
        "(Ljava/lang/Object;)Z",
        true,
        list_add
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "add",
        "(ILjava/lang/Object;)V",
        true,
        list_add_at
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "remove",
        "(I)Ljava/lang/Object;",
        true,
        list_remove_at
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "remove",
        "(Ljava/lang/Object;)Z",
        true,
        list_remove_obj
    ),
    ne!("Ljava/util/ArrayList;", "clear", "()V", true, list_clear),
    ne!(
        "Ljava/util/ArrayList;",
        "contains",
        "(Ljava/lang/Object;)Z",
        true,
        list_contains
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "indexOf",
        "(Ljava/lang/Object;)I",
        true,
        list_index_of
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "lastIndexOf",
        "(Ljava/lang/Object;)I",
        true,
        list_last_index_of
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "iterator",
        "()Ljava/util/Iterator;",
        true,
        list_iterator
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "listIterator",
        "()Ljava/util/ListIterator;",
        true,
        list_iterator
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "listIterator",
        "(I)Ljava/util/ListIterator;",
        true,
        list_iterator_at
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "subList",
        "(II)Ljava/util/List;",
        true,
        list_sublist
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "ensureCapacity",
        "(I)V",
        true,
        list_ensure_capacity
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "toArray",
        "()[Ljava/lang/Object;",
        true,
        list_to_array
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "toArray",
        "([Ljava/lang/Object;)[Ljava/lang/Object;",
        true,
        list_to_array_typed
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "addAll",
        "(Ljava/util/Collection;)Z",
        true,
        list_add_all
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "addAll",
        "(ILjava/util/Collection;)Z",
        true,
        list_add_all
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "removeAll",
        "(Ljava/util/Collection;)Z",
        true,
        list_remove_all
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "retainAll",
        "(Ljava/util/Collection;)Z",
        true,
        list_retain_all
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "toString",
        "()Ljava/lang/String;",
        true,
        list_to_string
    ),
    ne!(
        "Ljava/util/ArrayList;",
        "sort",
        "(Ljava/util/Comparator;)V",
        true,
        list_sort_cmp
    ),
];

/// java.util.concurrent.CopyOnWriteArrayList: extends Object directly (not
/// ArrayList), so it needs its own entries even though they share the same
/// `Native::List`-backed implementations.
pub(crate) const COW_TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "<init>",
        "()V",
        true,
        list_init
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "<init>",
        "(Ljava/util/Collection;)V",
        true,
        list_init
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "size",
        "()I",
        true,
        list_size
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "isEmpty",
        "()Z",
        true,
        list_is_empty
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "get",
        "(I)Ljava/lang/Object;",
        true,
        list_get
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "set",
        "(ILjava/lang/Object;)Ljava/lang/Object;",
        true,
        list_set
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "add",
        "(Ljava/lang/Object;)Z",
        true,
        list_add
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "add",
        "(ILjava/lang/Object;)V",
        true,
        list_add_at
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "remove",
        "(I)Ljava/lang/Object;",
        true,
        list_remove_at
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "remove",
        "(Ljava/lang/Object;)Z",
        true,
        list_remove_obj
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "clear",
        "()V",
        true,
        list_clear
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "contains",
        "(Ljava/lang/Object;)Z",
        true,
        list_contains
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "indexOf",
        "(Ljava/lang/Object;)I",
        true,
        list_index_of
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "iterator",
        "()Ljava/util/Iterator;",
        true,
        list_iterator
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "toArray",
        "()[Ljava/lang/Object;",
        true,
        list_to_array
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "addAll",
        "(Ljava/util/Collection;)Z",
        true,
        list_add_all
    ),
    ne!(
        "Ljava/util/concurrent/CopyOnWriteArrayList;",
        "toString",
        "()Ljava/lang/String;",
        true,
        list_to_string
    ),
];
