//! java.nio.charset.Charset host shims.

use crate::vm::native::*;

pub(crate) fn lazy_charset(vm: &mut Vm, name: &str) -> JValue {
    let class = vm
        .ensure_class_by_desc("Ljava/nio/charset/Charset;")
        .expect("Charset shim");
    JValue::Obj(
        vm.arena
            .alloc(class, Vec::new(), Some(Native::Str(name.to_string()))),
    )
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

/// Native methods for Ljava/nio/charset/Charset;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/nio/charset/Charset;",
        "forName",
        "(Ljava/lang/String;)Ljava/nio/charset/Charset;",
        false,
        charset_for_name
    ),
    ne!(
        "Ljava/nio/charset/Charset;",
        "name",
        "()Ljava/lang/String;",
        true,
        charset_name
    ),
    ne!(
        "Ljava/nio/charset/Charset;",
        "toString",
        "()Ljava/lang/String;",
        true,
        charset_name
    ),
    ne!(
        "Ljava/nio/charset/Charset;",
        "displayName",
        "()Ljava/lang/String;",
        true,
        charset_name
    ),
    ne!(
        "Ljava/nio/charset/Charset;",
        "displayName",
        "(Ljava/util/Locale;)Ljava/lang/String;",
        true,
        charset_name
    ),
    ne!(
        "Ljava/nio/charset/Charset;",
        "canEncode",
        "()Z",
        true,
        charset_can_encode
    ),
    ne!(
        "Ljava/nio/charset/Charset;",
        "defaultCharset",
        "()Ljava/nio/charset/Charset;",
        false,
        charset_default_charset
    ),
    ne!(
        "Ljava/nio/charset/Charset;",
        "isSupported",
        "(Ljava/lang/String;)Z",
        false,
        charset_is_supported
    ),
];
