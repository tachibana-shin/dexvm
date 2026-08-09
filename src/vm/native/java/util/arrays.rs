//! java.util.Arrays host shims.

use crate::vm::native::*;

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
        Some(Native::Array(data)) => (
            data.len(),
            (0..data.len()).map(|i| data.get(i)).collect::<Vec<_>>(),
        ),
        _ => return Err(npe(vm)),
    };
    let (lb, bv) = match payload(vm, args[1]) {
        Some(Native::Array(data)) => (
            data.len(),
            (0..data.len()).map(|i| data.get(i)).collect::<Vec<_>>(),
        ),
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

/// Native methods for Ljava/util/Arrays;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/Arrays;",
        "asList",
        "([Ljava/lang/Object;)Ljava/util/List;",
        false,
        arrays_as_list
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOf",
        "([BI)[B",
        false,
        arrays_copy_of
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOf",
        "([CI)[C",
        false,
        arrays_copy_of
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOf",
        "([SI)[S",
        false,
        arrays_copy_of
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOf",
        "([II)[I",
        false,
        arrays_copy_of
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOf",
        "([JI)[J",
        false,
        arrays_copy_of
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOf",
        "([FI)[F",
        false,
        arrays_copy_of
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOf",
        "([DI)[D",
        false,
        arrays_copy_of
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOf",
        "([ZI)[Z",
        false,
        arrays_copy_of
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOf",
        "([Ljava/lang/Object;I)[Ljava/lang/Object;",
        false,
        arrays_copy_of
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOfRange",
        "([BII)[B",
        false,
        arrays_copy_of_range
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOfRange",
        "([CII)[C",
        false,
        arrays_copy_of_range
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOfRange",
        "([SII)[S",
        false,
        arrays_copy_of_range
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOfRange",
        "([III)[I",
        false,
        arrays_copy_of_range
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOfRange",
        "([JII)[J",
        false,
        arrays_copy_of_range
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOfRange",
        "([FII)[F",
        false,
        arrays_copy_of_range
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOfRange",
        "([DII)[D",
        false,
        arrays_copy_of_range
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOfRange",
        "([ZII)[Z",
        false,
        arrays_copy_of_range
    ),
    ne!(
        "Ljava/util/Arrays;",
        "copyOfRange",
        "([Ljava/lang/Object;II)[Ljava/lang/Object;",
        false,
        arrays_copy_of_range
    ),
    ne!(
        "Ljava/util/Arrays;",
        "sort",
        "([I)V",
        false,
        arrays_sort_prim
    ),
    ne!(
        "Ljava/util/Arrays;",
        "sort",
        "([J)V",
        false,
        arrays_sort_prim
    ),
    ne!(
        "Ljava/util/Arrays;",
        "sort",
        "([B)V",
        false,
        arrays_sort_prim
    ),
    ne!(
        "Ljava/util/Arrays;",
        "sort",
        "([C)V",
        false,
        arrays_sort_prim
    ),
    ne!(
        "Ljava/util/Arrays;",
        "sort",
        "([S)V",
        false,
        arrays_sort_prim
    ),
    ne!(
        "Ljava/util/Arrays;",
        "sort",
        "([F)V",
        false,
        arrays_sort_prim
    ),
    ne!(
        "Ljava/util/Arrays;",
        "sort",
        "([D)V",
        false,
        arrays_sort_prim
    ),
    ne!(
        "Ljava/util/Arrays;",
        "sort",
        "([Z)V",
        false,
        arrays_sort_prim
    ),
    ne!(
        "Ljava/util/Arrays;",
        "sort",
        "([Ljava/lang/Object;)V",
        false,
        arrays_sort_obj
    ),
    ne!(
        "Ljava/util/Arrays;",
        "sort",
        "([Ljava/lang/Object;Ljava/util/Comparator;)V",
        false,
        arrays_sort_obj_cmp
    ),
    ne!(
        "Ljava/util/Arrays;",
        "toString",
        "([B)Ljava/lang/String;",
        false,
        arrays_to_string
    ),
    ne!(
        "Ljava/util/Arrays;",
        "toString",
        "([C)Ljava/lang/String;",
        false,
        arrays_to_string
    ),
    ne!(
        "Ljava/util/Arrays;",
        "toString",
        "([S)Ljava/lang/String;",
        false,
        arrays_to_string
    ),
    ne!(
        "Ljava/util/Arrays;",
        "toString",
        "([I)Ljava/lang/String;",
        false,
        arrays_to_string
    ),
    ne!(
        "Ljava/util/Arrays;",
        "toString",
        "([J)Ljava/lang/String;",
        false,
        arrays_to_string
    ),
    ne!(
        "Ljava/util/Arrays;",
        "toString",
        "([F)Ljava/lang/String;",
        false,
        arrays_to_string
    ),
    ne!(
        "Ljava/util/Arrays;",
        "toString",
        "([D)Ljava/lang/String;",
        false,
        arrays_to_string
    ),
    ne!(
        "Ljava/util/Arrays;",
        "toString",
        "([Z)Ljava/lang/String;",
        false,
        arrays_to_string
    ),
    ne!(
        "Ljava/util/Arrays;",
        "toString",
        "([Ljava/lang/Object;)Ljava/lang/String;",
        false,
        arrays_to_string
    ),
    ne!("Ljava/util/Arrays;", "fill", "([II)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([JI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([BI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([CI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([SI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([FI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([DI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([ZI)V", false, arrays_fill),
    ne!(
        "Ljava/util/Arrays;",
        "fill",
        "([Ljava/lang/Object;Ljava/lang/Object;)V",
        false,
        arrays_fill
    ),
    ne!(
        "Ljava/util/Arrays;",
        "equals",
        "([B[B)Z",
        false,
        arrays_equals
    ),
    ne!(
        "Ljava/util/Arrays;",
        "equals",
        "([C[C)Z",
        false,
        arrays_equals
    ),
    ne!(
        "Ljava/util/Arrays;",
        "equals",
        "([S[S)Z",
        false,
        arrays_equals
    ),
    ne!(
        "Ljava/util/Arrays;",
        "equals",
        "([I[I)Z",
        false,
        arrays_equals
    ),
    ne!(
        "Ljava/util/Arrays;",
        "equals",
        "([J[J)Z",
        false,
        arrays_equals
    ),
    ne!(
        "Ljava/util/Arrays;",
        "equals",
        "([F[F)Z",
        false,
        arrays_equals
    ),
    ne!(
        "Ljava/util/Arrays;",
        "equals",
        "([D[D)Z",
        false,
        arrays_equals
    ),
    ne!(
        "Ljava/util/Arrays;",
        "equals",
        "([Z[Z)Z",
        false,
        arrays_equals
    ),
    ne!(
        "Ljava/util/Arrays;",
        "equals",
        "([Ljava/lang/Object;[Ljava/lang/Object;)Z",
        false,
        arrays_equals
    ),
];
