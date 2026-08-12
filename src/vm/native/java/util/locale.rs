use crate::vm::native::*;

pub fn lazy_opaque_locale(vm: &mut Vm) -> JValue {
    let Ok(class) = vm.ensure_class_by_desc("Ljava/util/Locale;") else {
        return JValue::Null;
    };
    JValue::Obj(vm.arena.alloc(class, Vec::new(), Some(Native::Opaque)))
}

// Locale constants (e.g. Locale.US) used by `String.lowercase(Locale)` etc.
macro_rules! locale_const {
    ($name:ident, $tag:expr) => {
        pub fn $name(vm: &mut Vm) -> JValue {
            let Ok(class) = vm.ensure_class_by_desc("Ljava/util/Locale;") else {
                return JValue::Null;
            };
            JValue::Obj(
                vm.arena
                    .alloc(class, Vec::new(), Some(Native::Str($tag.into()))),
            )
        }
    };
}
locale_const!(lazy_locale_us, "en-US");
locale_const!(lazy_locale_uk, "en-GB");
locale_const!(lazy_locale_canada, "en-CA");
locale_const!(lazy_locale_japan, "ja-JP");
locale_const!(lazy_locale_korea, "ko-KR");
locale_const!(lazy_locale_china, "zh-CN");
locale_const!(lazy_locale_france, "fr-FR");
locale_const!(lazy_locale_germany, "de-DE");
locale_const!(lazy_locale_italy, "it-IT");

// java.util.Locale host shims.

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

/// Native methods for Ljava/util/Locale;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/Locale;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        locale_init
    ),
    ne!(
        "Ljava/util/Locale;",
        "<init>",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        true,
        locale_init
    ),
    ne!(
        "Ljava/util/Locale;",
        "<init>",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        true,
        locale_init
    ),
    ne!(
        "Ljava/util/Locale;",
        "getDefault",
        "()Ljava/util/Locale;",
        false,
        locale_get_default
    ),
    ne!(
        "Ljava/util/Locale;",
        "toString",
        "()Ljava/lang/String;",
        true,
        locale_to_string
    ),
    ne!(
        "Ljava/util/Locale;",
        "getLanguage",
        "()Ljava/lang/String;",
        true,
        locale_get_language
    ),
    ne!(
        "Ljava/util/Locale;",
        "getCountry",
        "()Ljava/lang/String;",
        true,
        locale_get_country
    ),
    ne!(
        "Ljava/util/Locale;",
        "forLanguageTag",
        "(Ljava/lang/String;)Ljava/util/Locale;",
        false,
        locale_for_language_tag
    ),
];
