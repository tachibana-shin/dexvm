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

pub(crate) fn shared_prefs_get_string(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[2])
}

pub(crate) fn prefs_obj(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn prefs_ctx(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/content/Context;", Native::Opaque)
}

/// `Log.e(tag, msg, throwable)` — android log; swallowed on the host.
pub(crate) fn log_error(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}
pub(crate) fn prefs_set(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// `Context.getCacheDir() -> File` backed by a real per-VM host directory
/// (created lazily; the extension mkdirs its own subdirectories on top).
pub(crate) fn context_get_cache_dir(vm: &mut Vm, _args: &[JValue]) -> R {
    let path = vm.cache_root_path().to_string();
    alloc(vm, "Ljava/io/File;", Native::File { path })
}

fn file_path(vm: &mut Vm, arg: JValue) -> Result<String, NatErr> {
    match payload(vm, arg) {
        Some(Native::File { path }) => Ok(path.clone()),
        _ => Err(npe(vm)),
    }
}

/// `File.mkdirs() -> boolean`: really creates the directory tree.
pub(crate) fn file_mkdirs(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    Ok(JValue::Int(i32::from(std::fs::create_dir_all(&path).is_ok())))
}

/// `File.exists() -> boolean`: real filesystem check.
pub(crate) fn file_exists(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    Ok(JValue::Int(i32::from(
        std::fs::metadata(&path).is_ok(),
    )))
}

/// `File.lastModified() -> long`: real mtime in epoch millis (0 when the
/// file is missing, exactly like the JVM).
pub(crate) fn file_last_modified(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    let millis = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| i64::try_from(d.as_millis()).unwrap_or(0))
        })
        .unwrap_or(0);
    Ok(JValue::Long(millis))
}

/// `File.length() -> long`: real size in bytes (0 when missing).
pub(crate) fn file_length(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    let len = std::fs::metadata(&path)
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(JValue::Long(len as i64))
}

/// `File.isDirectory() -> boolean`: real check.
pub(crate) fn file_is_directory(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    Ok(JValue::Int(i32::from(
        std::fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false),
    )))
}

/// `File.delete() -> boolean`: really removes the file or empty directory.
pub(crate) fn file_delete(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    Ok(JValue::Int(i32::from(
        std::fs::remove_file(&path)
            .or_else(|_| std::fs::remove_dir(&path))
            .is_ok(),
    )))
}

/// `File.getAbsolutePath() -> String` / `File.getPath()`: the real path.
pub(crate) fn file_get_path(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    Ok(new_str(vm, &path))
}

/// `File.createTempFile(prefix, suffix, directory) -> File`: creates a
/// unique real file next to the given directory.
pub(crate) fn file_create_temp_file(vm: &mut Vm, args: &[JValue]) -> R {
    let prefix = jstr(vm, args[0]).unwrap_or_default();
    let suffix = jstr(vm, args[1]).unwrap_or_default();
    let dir = file_path(vm, args[2])?;
    let path = match tempfile_in(&dir, &prefix, &suffix) {
        Ok(p) => p,
        Err(_) => return Err(fnf(vm, "createTempFile failed")),
    };
    alloc(vm, "Ljava/io/File;", Native::File { path })
}

/// `File.renameTo(File) -> boolean`: real rename across the same filesystem.
pub(crate) fn file_rename_to(vm: &mut Vm, args: &[JValue]) -> R {
    let from = file_path(vm, args[0])?;
    let to = file_path(vm, args[1])?;
    Ok(JValue::Int(i32::from(
        std::fs::rename(&from, &to).is_ok(),
    )))
}

fn tempfile_in(dir: &str, prefix: &str, suffix: &str) -> std::io::Result<String> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    std::fs::create_dir_all(dir)?;
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let base = dir.trim_end_matches('/');
    let path = format!(
        "{base}/{prefix}dexvm{}-{}{}",
        std::process::id(),
        n,
        suffix
    );
    std::fs::write(&path, [])?;
    Ok(path)
}

/// `kotlin.io.FilesKt.resolve(File, String) -> File`: real path join.
pub(crate) fn fileskt_resolve(vm: &mut Vm, args: &[JValue]) -> R {
    let base = file_path(vm, args[0])?;
    let child = jstr(vm, args[1]).unwrap_or_default();
    let joined = {
        let mut p = std::path::PathBuf::from(&base);
        p.push(child);
        p.to_string_lossy().into_owned()
    };
    alloc(vm, "Ljava/io/File;", Native::File { path: joined })
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
        "Landroid/util/Log;",
        "e",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/Throwable;)I",
        false,
        log_error
    ),
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
        "Ljava/io/File;",
        "length",
        "()J",
        true,
        file_length
    ),
    ne!(
        "Ljava/io/File;",
        "isDirectory",
        "()Z",
        true,
        file_is_directory
    ),
    ne!(
        "Ljava/io/File;",
        "delete",
        "()Z",
        true,
        file_delete
    ),
    ne!(
        "Ljava/io/File;",
        "getAbsolutePath",
        "()Ljava/lang/String;",
        true,
        file_get_path
    ),
    ne!(
        "Ljava/io/File;",
        "getPath",
        "()Ljava/lang/String;",
        true,
        file_get_path
    ),
    ne!(
        "Ljava/io/File;",
        "createTempFile",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/io/File;)Ljava/io/File;",
        true,
        file_create_temp_file
    ),
    ne!(
        "Ljava/io/File;",
        "renameTo",
        "(Ljava/io/File;)Z",
        true,
        file_rename_to
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
        "Landroid/content/SharedPreferences;",
        "getString",
        "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
        true,
        shared_prefs_get_string
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
