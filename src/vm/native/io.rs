use super::*;

// lazy static materializers (ShimStaticDef::Lazy)
// ---------------------------------------------------------------------------

pub fn lazy_print_stream(vm: &mut Vm) -> JValue {
    let class = vm
        .ensure_class_by_desc("Ljava/io/PrintStream;")
        .expect("PrintStream shim");
    JValue::Obj(vm.arena.alloc(class, Vec::new(), Some(Native::PrintStream)))
}

pub fn lazy_opaque_locale(vm: &mut Vm) -> JValue {
    let class = vm
        .ensure_class_by_desc("Ljava/util/Locale;")
        .expect("Locale shim");
    JValue::Obj(vm.arena.alloc(class, Vec::new(), Some(Native::Opaque)))
}

pub(crate) fn lazy_charset(vm: &mut Vm, name: &str) -> JValue {
    let class = vm
        .ensure_class_by_desc("Ljava/nio/charset/Charset;")
        .expect("Charset shim");
    JValue::Obj(vm.arena.alloc(class, Vec::new(), Some(Native::Str(name.to_string()))))
}

pub fn lazy_charset_utf8(vm: &mut Vm) -> JValue {
    lazy_charset(vm, "UTF-8")
}
pub fn lazy_charset_iso(vm: &mut Vm) -> JValue {
    lazy_charset(vm, "ISO-8859-1")
}
pub fn lazy_charset_ascii(vm: &mut Vm) -> JValue {
    lazy_charset(vm, "US-ASCII")
}

pub(crate) fn prim_class_obj(vm: &mut Vm, code: u8) -> JValue {
    let class = vm.ensure_class_by_desc("Ljava/lang/Class;").expect("Class shim");
    JValue::Obj(vm.arena.alloc(
        class,
        Vec::new(),
        Some(Native::ClassObj(ClassOrPrim::Primitive(code))),
    ))
}

pub fn lazy_int_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'I')
}
pub fn lazy_long_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'J')
}
pub fn lazy_short_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'S')
}
pub fn lazy_byte_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'B')
}
pub fn lazy_char_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'C')
}
pub fn lazy_bool_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'Z')
}
pub fn lazy_bool_true(vm: &mut Vm) -> JValue {
    let class = vm
        .ensure_class_by_desc("Ljava/lang/Boolean;")
        .expect("Boolean shim");
    JValue::Obj(vm.arena.alloc(class, Vec::new(), Some(Native::BoolBox(true))))
}
pub fn lazy_bool_false(vm: &mut Vm) -> JValue {
    let class = vm
        .ensure_class_by_desc("Ljava/lang/Boolean;")
        .expect("Boolean shim");
    JValue::Obj(vm.arena.alloc(class, Vec::new(), Some(Native::BoolBox(false))))
}
pub fn lazy_float_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'F')
}
pub fn lazy_double_type(vm: &mut Vm) -> JValue {
    prim_class_obj(vm, b'D')
}

// ---------------------------------------------------------------------------
// java.util.Random
// ---------------------------------------------------------------------------

pub(crate) fn rand_next(seed: &mut u64) -> u64 {
    let mut x = *seed;
    if x == 0 {
        x = 0x9E37_79B9_7F4A_7C15;
    }
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *seed = x;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

pub(crate) fn random_init(vm: &mut Vm, args: &[JValue]) -> R {
    let seed = (now_millis() as u64) ^ next_random_u64();
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(dst) => *dst = seed,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn random_init_seed(vm: &mut Vm, args: &[JValue]) -> R {
    let seed = long_of(vm, args[1]) as u64;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(dst) => *dst = seed,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn random_next_int(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => Ok(JValue::Int((rand_next(seed) >> 32) as i32)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_int_bound(vm: &mut Vm, args: &[JValue]) -> R {
    let bound = int_of(vm, args[1]);
    if bound <= 0 {
        return Err(iae(vm, "bound must be positive"));
    }
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => {
            let v = (rand_next(seed) >> 32) as u32;
            Ok(JValue::Int((v % bound as u32) as i32))
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_long(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => Ok(JValue::Long(rand_next(seed) as i64)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_double(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => {
            let v = (rand_next(seed) >> 11) as f64 * (1.0 / ((1u64 << 53) as f64));
            Ok(JValue::Double(v))
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_float(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => {
            let v = ((rand_next(seed) >> 40) as f32) * (1.0 / ((1u32 << 24) as f32));
            Ok(JValue::Float(v))
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_boolean(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(seed) => Ok(JValue::Int(i32::from((rand_next(seed) >> 63) != 0))),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn random_next_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let mut seed = match payload_mut(vm, args[0]) {
        Some(Native::Random(seed)) => *seed,
        _ => return Err(npe(vm)),
    };
    let Some(Native::Array(ArrayData::Byte(bs))) = payload_mut(vm, args[1]) else {
        return Err(npe(vm));
    };
    let mut w = rand_next(&mut seed);
    let n_bytes = bs.len();
    for (i, b) in bs.iter_mut().enumerate() {
        *b = (w & 0xff) as i8;
        w >>= 8;
        if w == 0 && i + 1 < n_bytes {
            w = rand_next(&mut seed);
        }
    }
    Ok(JValue::Null)
}

pub(crate) fn random_set_seed(vm: &mut Vm, args: &[JValue]) -> R {
    let seed = long_of(vm, args[1]) as u64;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Random(dst) => *dst = seed,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

// ---------------------------------------------------------------------------
// java.util.Date
// ---------------------------------------------------------------------------

pub(crate) fn date_init(vm: &mut Vm, args: &[JValue]) -> R {
    let t = now_millis();
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Date(dst) => *dst = t,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn date_init_ms(vm: &mut Vm, args: &[JValue]) -> R {
    let t = long_of(vm, args[1]);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Date(dst) => *dst = t,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn date_get_time(vm: &mut Vm, args: &[JValue]) -> R {
    let t = match payload(vm, args[0]) {
        Some(Native::Date(t)) => *t,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Long(t))
}

pub(crate) fn date_set_time(vm: &mut Vm, args: &[JValue]) -> R {
    let t = long_of(vm, args[1]);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Date(dst) => *dst = t,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn date_millis(vm: &mut Vm, v: JValue) -> Result<i64, NatErr> {
    match payload(vm, v) {
        Some(Native::Date(t)) => Ok(*t),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn date_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let t = date_millis(vm, args[0])?;
    Ok(new_str(vm, &format!("java.util.Date({t})")))
}

pub(crate) fn date_after(vm: &mut Vm, args: &[JValue]) -> R {
    let a = date_millis(vm, args[0])?;
    let b = date_millis(vm, args[1])?;
    Ok(JValue::Int(i32::from(a > b)))
}

pub(crate) fn date_before(vm: &mut Vm, args: &[JValue]) -> R {
    let a = date_millis(vm, args[0])?;
    let b = date_millis(vm, args[1])?;
    Ok(JValue::Int(i32::from(a < b)))
}

pub(crate) fn date_equals(vm: &mut Vm, args: &[JValue]) -> R {
    let a = date_millis(vm, args[0])?;
    match payload(vm, args[1]) {
        Some(Native::Date(b)) => Ok(JValue::Int(i32::from(a == *b))),
        _ => Ok(JValue::Int(0)),
    }
}

pub(crate) fn date_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    let a = date_millis(vm, args[0])?;
    let b = date_millis(vm, args[1])?;
    Ok(JValue::Int(a.cmp(&b) as i32))
}

// ---------------------------------------------------------------------------
// java.util.Locale
// ---------------------------------------------------------------------------

pub(crate) fn locale_get_default(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(lazy_opaque_locale(vm))
}

pub(crate) fn locale_init(vm: &mut Vm, args: &[JValue]) -> R {
    let tag = if args.len() > 1 {
        jstr(vm, args[1])?
    } else {
        String::new()
    };
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Str(dst) => *dst = tag,
        _ => *n = Native::Str(tag),
    }
    Ok(JValue::Null)
}

pub(crate) fn locale_tag(vm: &mut Vm, v: JValue) -> String {
    match payload(vm, v) {
        Some(Native::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

pub(crate) fn locale_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let tag = locale_tag(vm, args[0]);
    Ok(new_str(vm, &tag))
}

pub(crate) fn locale_get_language(vm: &mut Vm, args: &[JValue]) -> R {
    let lang = locale_tag(vm, args[0])
        .split(['_', '-'])
        .next()
        .unwrap_or("")
        .to_string();
    Ok(new_str(vm, &lang))
}

pub(crate) fn locale_get_country(vm: &mut Vm, args: &[JValue]) -> R {
    let tag = locale_tag(vm, args[0]);
    let mut parts = tag.split(['_', '-']);
    let _lang = parts.next();
    Ok(new_str(vm, parts.next().unwrap_or("")))
}

pub(crate) fn locale_for_language_tag(vm: &mut Vm, args: &[JValue]) -> R {
    let tag = jstr(vm, args[0])?;
    alloc(vm, "Ljava/util/Locale;", Native::Str(tag))
}

// ---------------------------------------------------------------------------
// java.nio.charset.Charset
// ---------------------------------------------------------------------------

pub(crate) fn normalize_charset(name: &str) -> Option<String> {
    let up = name.trim().to_uppercase();
    let n = match up.as_str() {
        "UTF8" | "UTF_8" => "UTF-8",
        "US-ASCII" | "ASCII" | "US_ASCII" | "646" => "US-ASCII",
        "UTF-16" | "UTF16" | "UTF_16" => "UTF-16",
        "UTF-16LE" | "UTF16LE" | "UTF_16LE" => "UTF-16LE",
        "UTF-16BE" | "UTF16BE" | "UTF_16BE" => "UTF-16BE",
        "UTF-32" | "UTF32" | "UTF_32" => "UTF-32",
        "ISO-8859-1" | "ISO8859-1" | "ISO_8859-1" | "ISO8859_1" | "LATIN1" | "L1" | "8859-1"
        | "CP819" => "ISO-8859-1",
        _ => {
            if up.contains("8859") || up.starts_with("LATIN") {
                "ISO-8859-1"
            } else {
                return None;
            }
        }
    };
    Some(n.to_string())
}

pub(crate) fn charset_for_name(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[0])?;
    match normalize_charset(&name) {
        Some(n) => alloc(vm, "Ljava/nio/charset/Charset;", Native::Str(n)),
        None => Err(iae(vm, format!("Unsupported charset: {name}"))),
    }
}

pub(crate) fn charset_name(vm: &mut Vm, args: &[JValue]) -> R {
    let name = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &name))
}

pub(crate) fn charset_can_encode(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(1))
}

pub(crate) fn charset_default_charset(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(lazy_charset_utf8(vm))
}

pub(crate) fn charset_is_supported(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[0])?;
    Ok(JValue::Int(i32::from(normalize_charset(&name).is_some())))
}

// ---------------------------------------------------------------------------
// java.io.PrintStream.<init> (objects constructed by dex)
// ---------------------------------------------------------------------------

pub(crate) fn ps_init(vm: &mut Vm, args: &[JValue]) -> R {
    if payload_mut(vm, args[0]).is_none() {
        return Err(npe(vm));
    }
    Ok(JValue::Null)
}

// ---------------------------------------------------------------------------
// java.util.Objects (all static)
// ---------------------------------------------------------------------------

pub(crate) fn objects_equals(vm: &mut Vm, args: &[JValue]) -> R {
    java_equals(vm, args[0], args[1]).map(|b| JValue::Int(i32::from(b)))
}

pub(crate) fn objects_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(java_hash(vm, args[0])))
}

pub(crate) fn objects_hash(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Obj(items))) => items.clone(),
        _ => return Err(npe(vm)),
    };
    let mut h: i32 = 1;
    for v in items {
        h = h.wrapping_mul(31).wrapping_add(java_hash(vm, v));
    }
    Ok(JValue::Int(h))
}

pub(crate) fn objects_require_non_null(vm: &mut Vm, args: &[JValue]) -> R {
    if args[0].is_null() {
        let msg = if args.len() > 1 && !args[1].is_null() {
            jstr(vm, args[1])?
        } else {
            "null".to_string()
        };
        Err(NatErr::Throw(vm.throwable_of("Ljava/lang/NullPointerException;", msg)))
    } else {
        Ok(args[0])
    }
}

pub(crate) fn objects_require_non_null_else(_vm: &mut Vm, args: &[JValue]) -> R {
    if args[0].is_null() {
        Ok(args[1])
    } else {
        Ok(args[0])
    }
}

pub(crate) fn objects_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = to_string_of(vm, args[0])?;
    Ok(new_str(vm, &s))
}

pub(crate) fn objects_to_string_def(vm: &mut Vm, args: &[JValue]) -> R {
    if args[0].is_null() {
        let s = jstr(vm, args[1])?;
        Ok(new_str(vm, &s))
    } else {
        let s = to_string_of(vm, args[0])?;
        Ok(new_str(vm, &s))
    }
}

pub(crate) fn objects_is_null(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(args[0].is_null())))
}

pub(crate) fn objects_non_null(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(!args[0].is_null())))
}

// ---------------------------------------------------------------------------
