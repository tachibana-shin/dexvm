//! android / androidx framework host shims (keiyoushi feature).

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;

use super::*;

// ---------------------------------------------------------------------------
// android framework
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// android framework
// ---------------------------------------------------------------------------

pub(crate) fn context_get_shared_prefs(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/content/SharedPreferences;", Native::Opaque)
}

pub(crate) fn shared_prefs_get_boolean(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[2])
}

pub(crate) fn prefs_obj(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn prefs_ctx(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/content/Context;", Native::Opaque)
}
pub(crate) fn prefs_set(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// `android.util.Base64.decode(String, int) -> [B`. Standard alphabet (flag 0)
/// or URL-safe (flag 4); padding-tolerant like the Android implementation.
fn base64_decode(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let flags = int_of(vm, args[1]);
    let t = s.trim();
    let bytes = if flags & 4 != 0 {
        URL_SAFE_NO_PAD
            .decode(t.trim_end_matches('='))
            .map_err(|_| iae(vm, "Base64 decode failed"))?
    } else {
        STANDARD
            .decode(t)
            .map_err(|_| iae(vm, "Base64 decode failed"))?
    };
    let data = bytes.into_iter().map(|b| b as i8).collect::<Vec<_>>();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

// ---------------------------------------------------------------------------
// android native table
// ---------------------------------------------------------------------------

pub(crate) const ANDROID_TABLE: &[NativeEntry] = &[
    ne!(
        "Landroid/content/Context;",
        "getSharedPreferences",
        "(Ljava/lang/String;I)Landroid/content/SharedPreferences;",
        true,
        context_get_shared_prefs
    ),
    ne!(
        "Landroid/content/SharedPreferences;",
        "getBoolean",
        "(Ljava/lang/String;Z)Z",
        true,
        shared_prefs_get_boolean
    ),
    ne!(
        "Landroidx/preference/Preference;",
        "<init>",
        "(Landroid/content/Context;)V",
        true,
        prefs_obj
    ),
    ne!(
        "Landroidx/preference/Preference;",
        "setKey",
        "(Ljava/lang/String;)V",
        true,
        prefs_set
    ),
    ne!(
        "Landroidx/preference/Preference;",
        "setTitle",
        "(Ljava/lang/CharSequence;)V",
        true,
        prefs_set
    ),
    ne!(
        "Landroidx/preference/Preference;",
        "setSummary",
        "(Ljava/lang/CharSequence;)V",
        true,
        prefs_set
    ),
    ne!(
        "Landroidx/preference/Preference;",
        "setDefaultValue",
        "(Ljava/lang/Object;)V",
        true,
        prefs_set
    ),
    ne!(
        "Landroidx/preference/PreferenceScreen;",
        "<init>",
        "(Landroid/content/Context;)V",
        true,
        prefs_obj
    ),
    ne!(
        "Landroidx/preference/PreferenceScreen;",
        "getContext",
        "()Landroid/content/Context;",
        true,
        prefs_ctx
    ),
    ne!(
        "Landroidx/preference/PreferenceScreen;",
        "setTitle",
        "(Ljava/lang/CharSequence;)V",
        true,
        prefs_set
    ),
    ne!(
        "Landroidx/preference/SwitchPreferenceCompat;",
        "<init>",
        "(Landroid/content/Context;)V",
        true,
        prefs_obj
    ),
    ne!(
        "Landroidx/preference/SwitchPreferenceCompat;",
        "setKey",
        "(Ljava/lang/String;)V",
        true,
        prefs_set
    ),
    ne!(
        "Landroidx/preference/SwitchPreferenceCompat;",
        "setTitle",
        "(Ljava/lang/CharSequence;)V",
        true,
        prefs_set
    ),
    ne!(
        "Landroidx/preference/SwitchPreferenceCompat;",
        "setSummary",
        "(Ljava/lang/CharSequence;)V",
        true,
        prefs_set
    ),
    ne!(
        "Landroidx/preference/SwitchPreferenceCompat;",
        "setDefaultValue",
        "(Ljava/lang/Object;)V",
        true,
        prefs_set
    ),
    ne!(
        "Landroid/util/Base64;",
        "decode",
        "(Ljava/lang/String;I)[B",
        true,
        base64_decode
    ),
];

#[cfg(test)]
mod tests;
