//! java.lang.StringBuilder host shims.

use crate::vm::native::*;

// java.lang.StringBuilder
// ---------------------------------------------------------------------------

pub(crate) fn sb_init(vm: &mut Vm, args: &[JValue]) -> R {
    let init = if args.len() > 1 && matches!(args[1], JValue::Obj(_)) {
        jstr(vm, args[1]).ok()
    } else {
        None
    };
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::StringBuilder(s) => {
            s.clear();
            if let Some(init) = init {
                s.push_str(&init);
            }
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn sb_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::StringBuilder(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &s))
}

pub(crate) fn sb_append_str(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[1])?;
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&s);
    Ok(args[0])
}

pub(crate) fn sb_append_charseq(vm: &mut Vm, args: &[JValue]) -> R {
    if args[1].is_null() {
        let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
            return Err(npe(vm));
        };
        dst.push_str("null");
        return Ok(args[0]);
    }
    let s = charseq_of(vm, args[1])?;
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&s);
    Ok(args[0])
}

pub(crate) fn sb_append_charseq_range(vm: &mut Vm, args: &[JValue]) -> R {
    let s = if args[1].is_null() {
        "null".to_string()
    } else {
        charseq_of(vm, args[1])?
    };
    let start = int_of(vm, args[2]).max(0) as usize;
    let end = (int_of(vm, args[3]).max(0) as usize).min(s.chars().count());
    let slice: String = s.chars().skip(start).take(end.saturating_sub(start)).collect();
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&slice);
    Ok(args[0])
}

pub(crate) fn sb_append_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let s = to_string_of(vm, args[1])?;
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&s);
    Ok(args[0])
}

pub(crate) fn sb_append_int(vm: &mut Vm, args: &[JValue]) -> R {
    let s = int_of(vm, args[1]).to_string();
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&s);
    Ok(args[0])
}

pub(crate) fn sb_append_long(vm: &mut Vm, args: &[JValue]) -> R {
    let s = long_of(vm, args[1]).to_string();
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&s);
    Ok(args[0])
}

pub(crate) fn sb_append_bool(vm: &mut Vm, args: &[JValue]) -> R {
    let s = if bool_of(vm, args[1]) {
        "true"
    } else {
        "false"
    };
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(s);
    Ok(args[0])
}

pub(crate) fn sb_append_char(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[1]) as u16;
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&u16str(&[c]));
    Ok(args[0])
}

pub(crate) fn sb_append_float(vm: &mut Vm, args: &[JValue]) -> R {
    let s = fmt_f32(float_of(vm, args[1]));
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&s);
    Ok(args[0])
}

pub(crate) fn sb_append_double(vm: &mut Vm, args: &[JValue]) -> R {
    let s = fmt_f64(double_of(vm, args[1]));
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&s);
    Ok(args[0])
}

pub(crate) fn sb_append_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Array(ArrayData::Char(cs))) = payload(vm, args[1]) else {
        return Err(npe(vm));
    };
    let s = u16str(cs);
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&s);
    Ok(args[0])
}

pub(crate) fn sb_length(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::StringBuilder(s)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(u16len(s) as i32))
}

pub(crate) fn sb_char_at(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::StringBuilder(s)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let i = int_of(vm, args[1]);
    match char_at(s, i.max(0) as usize) {
        Some(c) => Ok(JValue::Int(i32::from(c))),
        None => Err(ioobe(vm, i)),
    }
}

pub(crate) fn sb_substring(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::StringBuilder(s)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let begin = int_of(vm, args[1]);
    let end = if args.len() > 2 {
        int_of(vm, args[2])
    } else {
        u16len(s) as i32
    };
    let v = u16(s);
    if begin < 0 || end < begin || end as usize > v.len() {
        return Err(sioobe(vm, "StringBuilder.substring out of range"));
    }
    Ok(new_str(vm, &u16str(&v[begin as usize..end as usize])))
}

pub(crate) fn sb_delete(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::StringBuilder(s)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let begin = int_of(vm, args[1]);
    let end = int_of(vm, args[2]);
    let v = u16(s);
    if begin < 0 || begin > v.len() as i32 {
        return Err(sioobe(vm, "StringBuilder.delete out of range"));
    }
    let end = end.clamp(begin, v.len() as i32);
    let mut kept: Vec<u16> = Vec::with_capacity(v.len() - (end - begin) as usize);
    kept.extend_from_slice(&v[..begin as usize]);
    kept.extend_from_slice(&v[end as usize..]);
    let out = u16str(&kept);
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = out;
    Ok(args[0])
}

pub(crate) fn sb_set_length(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::StringBuilder(s)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let new_len = int_of(vm, args[1]);
    if new_len < 0 {
        return Err(sioobe(vm, "StringBuilder.setLength negative"));
    }
    let v = u16(s);
    let out = if (new_len as usize) <= v.len() {
        u16str(&v[..new_len as usize])
    } else {
        let mut w = v.clone();
        w.resize(new_len as usize, 0);
        u16str(&w)
    };
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = out;
    Ok(JValue::Null)
}

pub(crate) fn sb_append_code_point(vm: &mut Vm, args: &[JValue]) -> R {
    let cp = int_of(vm, args[1]) as u32;
    let c = char::from_u32(cp).unwrap_or('\u{fffd}');
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push(c);
    Ok(args[0])
}

pub(crate) fn sb_insert_string(vm: &mut Vm, args: &[JValue]) -> R {
    let idx = int_of(vm, args[1]).max(0) as usize;
    let s = jstr(vm, args[2])?;
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let byte_idx = dst
        .char_indices()
        .nth(idx)
        .map(|(i, _)| i)
        .unwrap_or(dst.len());
    dst.insert_str(byte_idx, &s);
    Ok(args[0])
}

pub(crate) fn sb_get_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let start = int_of(vm, args[1]).max(0) as usize;
    let end = int_of(vm, args[2]).max(0) as usize;
    let dst_begin = int_of(vm, args[4]).max(0) as usize;
    let src: Vec<char> = match payload(vm, args[0]) {
        Some(Native::StringBuilder(s)) => s.chars().collect(),
        _ => return Err(npe(vm)),
    };
    if end > src.len() || start > end {
        return Err(sioobe(vm, "getChars index out of range"));
    }
    let slice = &src[start..end];
    if let Some(Native::Array(ArrayData::Char(dst))) = payload_mut(vm, args[3]) {
        for (i, c) in slice.iter().enumerate() {
            if dst_begin + i < dst.len() {
                dst[dst_begin + i] = *c as u16;
            }
        }
    }
    Ok(JValue::Null)
}

pub(crate) fn sb_capacity(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

pub(crate) fn sb_index_of(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::StringBuilder(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    let n = jstr(vm, args[1])?;
    let from = if args.len() > 2 {
        int_of(vm, args[2]).max(0) as usize
    } else {
        0
    };
    Ok(JValue::Int(
        u16_index_of(&s, &n, from).map_or(-1, |i| i as i32),
    ))
}

/// Native methods for Ljava/lang/StringBuilder;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/lang/StringBuilder;", "<init>", "()V", true, sb_init),
    ne!("Ljava/lang/StringBuilder;", "<init>", "(I)V", true, sb_init),
    ne!(
        "Ljava/lang/StringBuilder;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        sb_init
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "toString",
        "()Ljava/lang/String;",
        true,
        sb_to_string
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(Ljava/lang/String;)Ljava/lang/StringBuilder;",
        true,
        sb_append_str
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(Ljava/lang/CharSequence;)Ljava/lang/StringBuilder;",
        true,
        sb_append_charseq
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(Ljava/lang/CharSequence;II)Ljava/lang/StringBuilder;",
        true,
        sb_append_charseq_range
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(Ljava/lang/Object;)Ljava/lang/StringBuilder;",
        true,
        sb_append_obj
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(I)Ljava/lang/StringBuilder;",
        true,
        sb_append_int
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(J)Ljava/lang/StringBuilder;",
        true,
        sb_append_long
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(Z)Ljava/lang/StringBuilder;",
        true,
        sb_append_bool
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(C)Ljava/lang/StringBuilder;",
        true,
        sb_append_char
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(C)Ljava/lang/Appendable;",
        true,
        sb_append_char
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(F)Ljava/lang/StringBuilder;",
        true,
        sb_append_float
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(D)Ljava/lang/StringBuilder;",
        true,
        sb_append_double
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "([C)Ljava/lang/StringBuilder;",
        true,
        sb_append_chars
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "length",
        "()I",
        true,
        sb_length
    ),
    ne!("Ljava/lang/StringBuffer;", "length", "()I", true, sb_length),
    ne!(
        "Ljava/lang/StringBuilder;",
        "charAt",
        "(I)C",
        true,
        sb_char_at
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "substring",
        "(II)Ljava/lang/String;",
        true,
        sb_substring
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "delete",
        "(II)Ljava/lang/StringBuilder;",
        true,
        sb_delete
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "setLength",
        "(I)V",
        true,
        sb_set_length
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "capacity",
        "()I",
        true,
        sb_capacity
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "indexOf",
        "(Ljava/lang/String;)I",
        true,
        sb_index_of
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "indexOf",
        "(Ljava/lang/String;I)I",
        true,
        sb_index_of
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "appendCodePoint",
        "(I)Ljava/lang/StringBuilder;",
        true,
        sb_append_code_point
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "insert",
        "(ILjava/lang/String;)Ljava/lang/StringBuilder;",
        true,
        sb_insert_string
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "getChars",
        "(II[CI)V",
        true,
        sb_get_chars
    ),
    ne!(
        "Ljava/lang/StringBuffer;",
        "getChars",
        "(II[CI)V",
        true,
        sb_get_chars
    ),
    ne!(
        "Ljava/lang/StringBuilder;",
        "append",
        "(Ljava/lang/CharSequence;)Ljava/lang/Appendable;",
        true,
        sb_append_charseq
    ),
];
