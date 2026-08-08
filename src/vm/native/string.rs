use super::*;

// java.lang.String
// ---------------------------------------------------------------------------

pub(crate) fn string_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn string_init_copy(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[1])?;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Str(dst) => *dst = s,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn string_init_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Array(ArrayData::Char(cs))) = payload(vm, args[1]) else {
        return Err(npe(vm));
    };
    let s = u16str(cs);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Str(dst) => *dst = s,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn string_init_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let b = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Byte(bs))) => bs.clone(),
        _ => return Err(npe(vm)),
    };
    let charset = if args.len() > 2 {
        jstr(vm, args[2]).ok().unwrap_or_default()
    } else {
        String::new()
    };
    let s = decode_bytes(&b, &charset);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Str(dst) => *dst = s,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn decode_bytes(bs: &[i8], charset: &str) -> String {
    let up = charset.to_uppercase();
    if up.contains("8859") || up.contains("LATIN1") {
        bs.iter()
            .map(|&b| char::from_u32(u32::from(b as u8)).unwrap_or('\u{fffd}'))
            .collect()
    } else {
        let bytes: Vec<u8> = bs.iter().map(|&b| b as u8).collect();
        String::from_utf8_lossy(&bytes).to_string()
    }
}

pub(crate) fn encode_bytes(s: &str, charset: &str) -> Vec<i8> {
    let up = charset.to_uppercase();
    if up.contains("8859") || up.contains("LATIN1") {
        s.chars()
            .map(|c| ((c as u32).min(0xFF) as u8) as i8)
            .collect()
    } else {
        s.as_bytes().iter().map(|&b| b as i8).collect()
    }
}

pub(crate) fn string_length(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(s) = peek_str(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(u16len(s) as i32))
}

pub(crate) fn string_is_empty(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(s) = peek_str(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(s.is_empty())))
}

pub(crate) fn string_char_at(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let i = int_of(vm, args[1]);
    if i < 0 {
        return Err(sioobe(vm, format!("String index out of range: {i}")));
    }
    match char_at(&s, i as usize) {
        Some(c) => Ok(JValue::Int(i32::from(c))),
        None => Err(sioobe(vm, format!("String index out of range: {i}"))),
    }
}

pub(crate) fn string_equals(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(s) = peek_str(vm, args[0]) else {
        return Err(npe(vm));
    };
    let t = peek_str(vm, args[1]);
    Ok(JValue::Int(i32::from(t.is_some_and(|t| t == s))))
}

pub(crate) fn string_equals_ignore_case(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let t = jstr(vm, args[1])?;
    Ok(JValue::Int(i32::from(
        s.to_lowercase() == t.to_lowercase(),
    )))
}

pub(crate) fn string_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(s) = peek_str(vm, args[0]) else {
        return Err(npe(vm));
    };
    let mut h: i64 = 0;
    for u in s.encode_utf16() {
        h = (h * 31 + i64::from(u)) & 0xFFFF_FFFF;
    }
    Ok(JValue::Int(h as i32))
}

pub(crate) fn string_to_string(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn string_substring(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let begin = int_of(vm, args[1]);
    let end = if args.len() > 2 {
        int_of(vm, args[2])
    } else {
        u16len(&s) as i32
    };
    let v = u16(&s);
    if begin < 0 || end < begin || end as usize > v.len() {
        return Err(sioobe(
            vm,
            format!("begin {begin}, end {end}, length {}", v.len()),
        ));
    }
    Ok(new_str(vm, &u16str(&v[begin as usize..end as usize])))
}

pub(crate) fn string_sub_sequence(vm: &mut Vm, args: &[JValue]) -> R {
    string_substring(vm, args)
}

pub(crate) fn string_concat(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let t = jstr(vm, args[1])?;
    Ok(new_str(vm, &format!("{s}{t}")))
}

pub(crate) fn string_contains(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let t = jstr(vm, args[1])?;
    Ok(JValue::Int(i32::from(s.contains(&t))))
}

pub(crate) fn string_starts_with(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let p = jstr(vm, args[1])?;
    let off = if args.len() > 2 {
        int_of(vm, args[2]).max(0) as usize
    } else {
        0
    };
    let sv = u16(&s);
    let pv = u16(&p);
    Ok(JValue::Int(i32::from(
        off <= sv.len() && sv[off..].starts_with(&pv),
    )))
}

pub(crate) fn string_ends_with(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let p = jstr(vm, args[1])?;
    Ok(JValue::Int(i32::from(s.ends_with(&p))))
}

pub(crate) fn string_index_of_char(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let c = int_of(vm, args[1]) as u16;
    let from = if args.len() > 2 {
        int_of(vm, args[2]).max(0) as usize
    } else {
        0
    };
    let v = u16(&s);
    let idx = v
        .iter()
        .enumerate()
        .skip(from)
        .find(|&(_, &u)| u == c)
        .map(|(i, _)| i);
    Ok(JValue::Int(idx.map_or(-1, |i| i as i32)))
}

pub(crate) fn string_index_of_str(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
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

pub(crate) fn string_last_index_of_char(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let c = int_of(vm, args[1]) as u16;
    let from = if args.len() > 2 {
        int_of(vm, args[2])
    } else {
        u16len(&s) as i32 - 1
    };
    let v = u16(&s);
    if v.is_empty() {
        return Ok(JValue::Int(-1));
    }
    let mut i = (from.max(0) as usize).min(v.len() - 1);
    let idx = loop {
        if v[i] == c {
            break i;
        }
        if i == 0 {
            break usize::MAX;
        }
        i -= 1;
    };
    Ok(JValue::Int(if idx == usize::MAX { -1 } else { idx as i32 }))
}

pub(crate) fn string_last_index_of_str(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let n = jstr(vm, args[1])?;
    let from = if args.len() > 2 {
        i64::from(int_of(vm, args[2]))
    } else {
        u16len(&s) as i64 - u16len(&n) as i64 + 1
    };
    Ok(JValue::Int(
        u16_last_index_of(&s, &n, from).map_or(-1, |i| i as i32),
    ))
}

pub(crate) fn string_to_lower(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    Ok(new_str(vm, &s.to_lowercase()))
}

pub(crate) fn string_to_upper(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    Ok(new_str(vm, &s.to_uppercase()))
}

pub(crate) fn string_trim(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    Ok(new_str(vm, s.trim_matches(|c: char| c <= '\u{20}')))
}

pub(crate) fn string_value_of_int(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &int_of(vm, args[0]).to_string()))
}

pub(crate) fn string_value_of_long(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &long_of(vm, args[0]).to_string()))
}

pub(crate) fn string_value_of_bool(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, if bool_of(vm, args[0]) { "true" } else { "false" }))
}

pub(crate) fn string_value_of_char(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[0]) as u16;
    Ok(new_str(vm, &u16str(&[c])))
}

pub(crate) fn string_value_of_float(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &fmt_f32(float_of(vm, args[0]))))
}

pub(crate) fn string_value_of_double(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(new_str(vm, &fmt_f64(double_of(vm, args[0]))))
}

pub(crate) fn string_value_of_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let s = to_string_of(vm, args[0])?;
    Ok(new_str(vm, &s))
}

pub(crate) fn string_get_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let charset = if args.len() > 1 {
        match payload(vm, args[1]) {
            Some(Native::Str(name)) => name.clone(),
            _ => jstr(vm, args[1])?,
        }
    } else {
        String::new()
    };
    let bytes = encode_bytes(&s, &charset);
    let class = vm
        .ensure_class_by_desc("[B")
        .map_err(nat_fatal)
        .or_else(|_| vm.ensure_class_by_desc("[Ljava/lang/Object;").map_err(nat_fatal))?;
    Ok(JValue::Obj(vm.arena.alloc(
        class,
        Vec::new(),
        Some(Native::Array(ArrayData::Byte(bytes))),
    )))
}

pub(crate) fn string_to_char_array(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let chars = u16(&s);
    let class = vm.ensure_class_by_desc("[C").map_err(nat_fatal)?;
    Ok(JValue::Obj(vm.arena.alloc(
        class,
        Vec::new(),
        Some(Native::Array(ArrayData::Char(chars))),
    )))
}

pub(crate) fn string_get_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let src_begin = int_of(vm, args[1]);
    let src_end = int_of(vm, args[2]);
    let dst_begin = int_of(vm, args[4]);
    let Some(Native::Array(ArrayData::Char(cs))) = payload(vm, args[3]) else {
        return Err(npe(vm));
    };
    let v = u16(&s);
    if src_begin < 0 || src_end < src_begin || src_end as usize > v.len() {
        return Err(sioobe(vm, "String.getChars out of range"));
    }
    let n_copy = (src_end - src_begin) as usize;
    if dst_begin < 0 || (dst_begin as usize) + n_copy > cs.len() {
        return Err(aioobe(vm, dst_begin, cs.len() as i32));
    }
    let Some(n) = payload_mut(vm, args[3]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Array(ArrayData::Char(dst)) => {
            let db = dst_begin as usize;
            dst[db..db + n_copy].copy_from_slice(&v[src_begin as usize..src_end as usize]);
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn string_split(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let re_str = jstr(vm, args[1])?;
    let limit = if args.len() > 2 {
        int_of(vm, args[2])
    } else {
        0
    };
    let re = Regex::new(&re_str).map_err(|e| iae(vm, format!("PatternSyntaxException: {e}")))?;
    let parts = split_java(&re, &s, limit);
    str_array(vm, parts)
}

pub(crate) fn str_array(vm: &mut Vm, parts: Vec<String>) -> Result<JValue, NatErr> {
    let class = vm.ensure_class_by_desc("[Ljava/lang/String;").map_err(nat_fatal)?;
    let items: Vec<JValue> = parts.iter().map(|p| vm.alloc_string(p)).collect();
    Ok(JValue::Obj(vm.arena.alloc(
        class,
        Vec::new(),
        Some(Native::Array(ArrayData::Obj(items))),
    )))
}

pub(crate) fn string_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let t = jstr(vm, args[1])?;
    Ok(JValue::Int(utf16_cmp(&s, &t) as i32))
}

pub(crate) fn string_compare_to_ignore_case(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?.to_lowercase();
    let t = jstr(vm, args[1])?.to_lowercase();
    Ok(JValue::Int(utf16_cmp(&s, &t) as i32))
}

pub(crate) fn string_matches(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let re_str = jstr(vm, args[1])?;
    let re = Regex::new(&re_str).map_err(|e| iae(vm, format!("PatternSyntaxException: {e}")))?;
    Ok(JValue::Int(i32::from(re.is_match(&s))))
}

pub(crate) fn string_replace_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let a = char::from_u32(int_of(vm, args[1]) as u32).unwrap_or('\u{0}');
    let b = char::from_u32(int_of(vm, args[2]) as u32).unwrap_or('\u{0}');
    Ok(new_str(vm, &s.replace(&a.to_string(), &b.to_string())))
}

pub(crate) fn string_replace_seq(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let a = charseq_of(vm, args[1])?;
    let b = charseq_of(vm, args[2])?;
    Ok(new_str(vm, &s.replace(&a, &b)))
}

pub(crate) fn string_intern(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn string_format(vm: &mut Vm, args: &[JValue]) -> R {
    let fmt_off = if args.len() >= 3 { 1 } else { 0 };
    let fmt = jstr(vm, args[fmt_off])?;
    let varargs = args.get(fmt_off + 1).copied();
    let items: Vec<JValue> = match varargs {
        Some(JValue::Obj(id)) => match payload(vm, JValue::Obj(id)) {
            Some(Native::Array(ArrayData::Obj(items))) => items.clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    let mut argi = 0usize;
    let mut out = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c != '%' {
            out.push(c);
            i += 1;
            continue;
        }
        i += 1;
        if i >= chars.len() {
            break;
        }
        match chars[i] {
            '%' => {
                out.push('%');
                i += 1;
            }
            'n' => {
                out.push('\n');
                i += 1;
            }
            _ => {
                let mut j = i;
                let mut flags = String::new();
                while j < chars.len() && matches!(chars[j], '-' | '0' | '+' | ' ' | ',' | '(') {
                    flags.push(chars[j]);
                    j += 1;
                }
                let mut width_s = String::new();
                while j < chars.len() && chars[j].is_ascii_digit() {
                    width_s.push(chars[j]);
                    j += 1;
                }
                let mut prec: Option<usize> = None;
                if j < chars.len() && chars[j] == '.' {
                    j += 1;
                    let mut ps = String::new();
                    while j < chars.len() && chars[j].is_ascii_digit() {
                        ps.push(chars[j]);
                        j += 1;
                    }
                    prec = ps.parse().ok();
                }
                if j >= chars.len() {
                    break;
                }
                let conv = chars[j];
                let width: usize = width_s.parse().unwrap_or(0);
                let left = flags.contains('-');
                let zero = flags.contains('0');
                let plus = flags.contains('+');
                let comma = flags.contains(',');
                let arg = items.get(argi).copied().unwrap_or(JValue::Null);
                argi += 1;
                let s = match conv.to_ascii_lowercase() {
                    's' => to_string_of(vm, arg)?,
                    'd' => {
                        let n = long_of(vm, arg);
                        let mut t = if comma {
                            comma_group(n.to_string())
                        } else {
                            n.to_string()
                        };
                        if plus && n >= 0 {
                            t.insert(0, '+');
                        }
                        t
                    }
                    'f' => match prec {
                        Some(p) => format!("{:.p$}", double_of(vm, arg), p = p),
                        None => format!("{:.6}", double_of(vm, arg)),
                    },
                    'x' => format!("{:x}", int_of(vm, arg) as u32),
                    'b' => bool_of(vm, arg).to_string(),
                    'c' => char::from_u32(int_of(vm, arg) as u32)
                        .map(String::from)
                        .unwrap_or_default(),
                    'e' => format!("{:e}", double_of(vm, arg)),
                    'o' => format!("{:o}", int_of(vm, arg) as u32),
                    _ => "?".to_string(),
                };
                if s.len() < width {
                    let pad = width - s.len();
                    let fill = if zero && !left { '0' } else { ' ' };
                    if left {
                        out.push_str(&s);
                        out.extend(std::iter::repeat_n(fill, pad));
                    } else {
                        out.extend(std::iter::repeat_n(fill, pad));
                        out.push_str(&s);
                    }
                } else {
                    out.push_str(&s);
                }
                i = j + 1;
            }
        }
    }
    Ok(new_str(vm, &out))
}

// ---------------------------------------------------------------------------
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
    let s = if bool_of(vm, args[1]) { "true" } else { "false" };
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
    Ok(JValue::Int(u16_index_of(&s, &n, from).map_or(-1, |i| i as i32)))
}

// ---------------------------------------------------------------------------
// misc: String.valueOf(char[]), Integer/Long.signum
// ---------------------------------------------------------------------------

pub(crate) fn string_value_of_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Array(ArrayData::Char(cs))) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(new_str(vm, &u16str(cs)))
}
