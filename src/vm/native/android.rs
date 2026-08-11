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

/// `Context.getCacheDir() -> File`. The multiapk (L s) path only checks it for
/// null; an opaque File satisfies that.
pub(crate) fn context_get_cache_dir(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/io/File;", Native::Opaque)
}

/// `File.mkdirs() -> boolean`; the multiapk path ignores the result.
pub(crate) fn file_mkdirs(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(1))
}

/// `File.exists() -> boolean`; report true so cache dirs are accepted.
pub(crate) fn file_exists(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(1))
}

/// `File.lastModified() -> long`; 0 makes filter caches permanently stale,
/// forcing recomputation from the network each time.
pub(crate) fn file_last_modified(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Long(0))
}

/// `kotlin.io.FilesKt.resolve(File, String) -> File`; returns the same
/// opaque file (path ignored, only the return value is used).
pub(crate) fn fileskt_resolve(_vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(_vm, "Ljava/io/File;", Native::Opaque)
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
        "Landroid/content/Context;",
        "getCacheDir",
        "()Ljava/io/File;",
        true,
        context_get_cache_dir
    ),
    ne!(
        "Landroid/content/ContextWrapper;",
        "getCacheDir",
        "()Ljava/io/File;",
        true,
        context_get_cache_dir
    ),
    ne!(
        "Ljava/io/File;",
        "mkdirs",
        "()Z",
        true,
        file_mkdirs
    ),
    ne!(
        "Ljava/io/File;",
        "exists",
        "()Z",
        true,
        file_exists
    ),
    ne!(
        "Ljava/io/File;",
        "lastModified",
        "()J",
        true,
        file_last_modified
    ),
    ne!(
        "Lkotlin/io/FilesKt;",
        "resolve",
        "(Ljava/io/File;Ljava/lang/String;)Ljava/io/File;",
        true,
        fileskt_resolve
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
    ne!(
        "Landroid/os/SystemClock;",
        "elapsedRealtime",
        "()J",
        true,
        elpased_realtime
    ),
];

#[cfg(feature = "okhttp")]
pub(crate) fn elpased_realtime(_vm: &mut Vm, _args: &[JValue]) -> R {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(0))
        .unwrap_or(0);
    Ok(JValue::Long(millis))
}

#[cfg(test)]
mod tests;
