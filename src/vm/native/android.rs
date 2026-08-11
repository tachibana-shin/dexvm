//! android / androidx framework host shims (keiyoushi feature).

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine as _;

use super::*;
use crate::permission::{FilesystemPermission, Permission};

// ---------------------------------------------------------------------------
// android framework
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// android framework
// ---------------------------------------------------------------------------

pub(crate) fn context_get_shared_prefs(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1])?;
    load_shared_preferences(vm)?;
    vm.shared_preferences.entry(name.clone()).or_default();
    alloc(
        vm,
        "Landroid/content/SharedPreferences;",
        Native::SharedPreferences(name),
    )
}

const PREFS_MAGIC: &[u8] = b"DEXVM-PREFS\0\x01";

fn pref_u32(out: &mut Vec<u8>, value: usize) -> std::io::Result<()> {
    let value = u32::try_from(value).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "SharedPreferences value too large",
        )
    })?;
    out.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn pref_bytes(out: &mut Vec<u8>, value: &[u8]) -> std::io::Result<()> {
    pref_u32(out, value.len())?;
    out.extend_from_slice(value);
    Ok(())
}

fn encode_shared_preferences(
    prefs: &std::collections::HashMap<String, std::collections::HashMap<String, PreferenceValue>>,
) -> std::io::Result<Vec<u8>> {
    let mut out = PREFS_MAGIC.to_vec();
    let mut names: Vec<_> = prefs.iter().collect();
    names.sort_unstable_by(|a, b| a.0.cmp(b.0));
    pref_u32(&mut out, names.len())?;
    for (name, values) in names {
        pref_bytes(&mut out, name.as_bytes())?;
        let mut entries: Vec<_> = values.iter().collect();
        entries.sort_unstable_by(|a, b| a.0.cmp(b.0));
        pref_u32(&mut out, entries.len())?;
        for (key, value) in entries {
            pref_bytes(&mut out, key.as_bytes())?;
            match value {
                PreferenceValue::Bool(value) => {
                    out.push(0);
                    out.push(u8::from(*value));
                }
                PreferenceValue::String(value) => {
                    out.push(1);
                    pref_bytes(&mut out, value.as_bytes())?;
                }
                PreferenceValue::Int(value) => {
                    out.push(2);
                    out.extend_from_slice(&value.to_le_bytes());
                }
                PreferenceValue::Long(value) => {
                    out.push(3);
                    out.extend_from_slice(&value.to_le_bytes());
                }
                PreferenceValue::Float(value) => {
                    out.push(4);
                    out.extend_from_slice(&value.to_bits().to_le_bytes());
                }
            }
        }
    }
    Ok(out)
}

struct PrefReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> PrefReader<'a> {
    fn take(&mut self, len: usize) -> std::io::Result<&'a [u8]> {
        let end = self.pos.checked_add(len).ok_or_else(pref_invalid)?;
        let value = self.bytes.get(self.pos..end).ok_or_else(pref_invalid)?;
        self.pos = end;
        Ok(value)
    }

    fn u8(&mut self) -> std::io::Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> std::io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn string(&mut self) -> std::io::Result<String> {
        let len = self.u32()? as usize;
        String::from_utf8(self.take(len)?.to_vec()).map_err(|_| pref_invalid())
    }
}

fn pref_invalid() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "invalid dexvm SharedPreferences file",
    )
}

fn decode_shared_preferences(
    bytes: &[u8],
) -> std::io::Result<
    std::collections::HashMap<String, std::collections::HashMap<String, PreferenceValue>>,
> {
    let Some(rest) = bytes.strip_prefix(PREFS_MAGIC) else {
        return Err(pref_invalid());
    };
    let mut reader = PrefReader {
        bytes: rest,
        pos: 0,
    };
    let mut prefs = std::collections::HashMap::new();
    for _ in 0..reader.u32()? {
        let name = reader.string()?;
        let mut values = std::collections::HashMap::new();
        for _ in 0..reader.u32()? {
            let key = reader.string()?;
            let value = match reader.u8()? {
                0 => PreferenceValue::Bool(match reader.u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err(pref_invalid()),
                }),
                1 => PreferenceValue::String(reader.string()?),
                2 => PreferenceValue::Int(i32::from_le_bytes(reader.take(4)?.try_into().unwrap())),
                3 => PreferenceValue::Long(i64::from_le_bytes(reader.take(8)?.try_into().unwrap())),
                4 => PreferenceValue::Float(f32::from_bits(u32::from_le_bytes(
                    reader.take(4)?.try_into().unwrap(),
                ))),
                _ => return Err(pref_invalid()),
            };
            values.insert(key, value);
        }
        prefs.insert(name, values);
    }
    if reader.pos != reader.bytes.len() {
        return Err(pref_invalid());
    }
    Ok(prefs)
}

fn load_shared_preferences(vm: &mut Vm) -> Result<(), NatErr> {
    if vm.shared_preferences_loaded {
        return Ok(());
    }
    let Some(path) = vm.shared_preferences_path.clone() else {
        vm.shared_preferences_loaded = true;
        return Ok(());
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            vm.shared_preferences_loaded = true;
            return Ok(());
        }
        Err(error) => return Err(ioe(vm, format!("read {}: {error}", path.display()))),
    };
    vm.shared_preferences = decode_shared_preferences(&bytes)
        .map_err(|error| ioe(vm, format!("read {}: {error}", path.display())))?;
    vm.shared_preferences_loaded = true;
    Ok(())
}

fn persist_shared_preferences(
    path: &std::path::Path,
    prefs: &std::collections::HashMap<String, std::collections::HashMap<String, PreferenceValue>>,
) -> std::io::Result<()> {
    let bytes = encode_shared_preferences(prefs)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut temp = path.as_os_str().to_os_string();
    temp.push(".tmp");
    let temp = std::path::PathBuf::from(temp);
    let result = (|| {
        use std::io::Write as _;
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)?;
        #[cfg(unix)]
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}

fn shared_prefs_name(vm: &mut Vm, value: JValue) -> Result<String, NatErr> {
    match payload(vm, value) {
        Some(Native::SharedPreferences(name)) => Ok(name.clone()),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn shared_prefs_get_boolean(vm: &mut Vm, args: &[JValue]) -> R {
    let name = shared_prefs_name(vm, args[0])?;
    let key = jstr(vm, args[1])?;
    let value = vm.shared_preferences.get(&name).and_then(|p| p.get(&key));
    Ok(match value {
        Some(PreferenceValue::Bool(v)) => JValue::Int(i32::from(*v)),
        _ => args[2],
    })
}

pub(crate) fn shared_prefs_get_string(vm: &mut Vm, args: &[JValue]) -> R {
    let name = shared_prefs_name(vm, args[0])?;
    let key = jstr(vm, args[1])?;
    let value = vm
        .shared_preferences
        .get(&name)
        .and_then(|p| p.get(&key))
        .and_then(|v| match v {
            PreferenceValue::String(s) => Some(s.clone()),
            _ => None,
        });
    match value {
        Some(v) => Ok(new_str(vm, &v)),
        None => Ok(args[2]),
    }
}

pub(crate) fn shared_prefs_get_int(vm: &mut Vm, args: &[JValue]) -> R {
    let name = shared_prefs_name(vm, args[0])?;
    let key = jstr(vm, args[1])?;
    Ok(
        match vm.shared_preferences.get(&name).and_then(|p| p.get(&key)) {
            Some(PreferenceValue::Int(v)) => JValue::Int(*v),
            _ => args[2],
        },
    )
}

pub(crate) fn shared_prefs_get_long(vm: &mut Vm, args: &[JValue]) -> R {
    let name = shared_prefs_name(vm, args[0])?;
    let key = jstr(vm, args[1])?;
    Ok(
        match vm.shared_preferences.get(&name).and_then(|p| p.get(&key)) {
            Some(PreferenceValue::Long(v)) => JValue::Long(*v),
            _ => args[2],
        },
    )
}

pub(crate) fn shared_prefs_get_float(vm: &mut Vm, args: &[JValue]) -> R {
    let name = shared_prefs_name(vm, args[0])?;
    let key = jstr(vm, args[1])?;
    Ok(
        match vm.shared_preferences.get(&name).and_then(|p| p.get(&key)) {
            Some(PreferenceValue::Float(v)) => JValue::Float(*v),
            _ => args[2],
        },
    )
}

pub(crate) fn shared_prefs_contains(vm: &mut Vm, args: &[JValue]) -> R {
    let name = shared_prefs_name(vm, args[0])?;
    let key = jstr(vm, args[1])?;
    Ok(JValue::Int(i32::from(
        vm.shared_preferences
            .get(&name)
            .is_some_and(|p| p.contains_key(&key)),
    )))
}

pub(crate) fn shared_prefs_edit(vm: &mut Vm, args: &[JValue]) -> R {
    let name = shared_prefs_name(vm, args[0])?;
    alloc(
        vm,
        "Landroid/content/SharedPreferences$Editor;",
        Native::SharedPreferencesEditor {
            name,
            edits: Vec::new(),
            clear: false,
        },
    )
}

fn editor_put(vm: &mut Vm, args: &[JValue], value: PreferenceValue) -> R {
    let key = jstr(vm, args[1])?;
    let Some(Native::SharedPreferencesEditor { edits, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    edits.push(PreferenceEdit::Put(key, value));
    Ok(args[0])
}

pub(crate) fn editor_put_boolean(vm: &mut Vm, args: &[JValue]) -> R {
    let value = int_of(vm, args[2]) != 0;
    editor_put(vm, args, PreferenceValue::Bool(value))
}

pub(crate) fn editor_put_string(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[2])?;
    editor_put(vm, args, PreferenceValue::String(value))
}

pub(crate) fn editor_put_int(vm: &mut Vm, args: &[JValue]) -> R {
    let value = int_of(vm, args[2]);
    editor_put(vm, args, PreferenceValue::Int(value))
}

pub(crate) fn editor_put_long(vm: &mut Vm, args: &[JValue]) -> R {
    let value = long_of(vm, args[2]);
    editor_put(vm, args, PreferenceValue::Long(value))
}

pub(crate) fn editor_put_float(vm: &mut Vm, args: &[JValue]) -> R {
    let value = match args[2] {
        JValue::Float(value) => value,
        _ => return Err(iae(vm, "expected float")),
    };
    editor_put(vm, args, PreferenceValue::Float(value))
}

pub(crate) fn editor_remove(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let Some(Native::SharedPreferencesEditor { edits, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    edits.push(PreferenceEdit::Remove(key));
    Ok(args[0])
}

pub(crate) fn editor_clear(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::SharedPreferencesEditor { clear, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *clear = true;
    Ok(args[0])
}

fn edited_preferences(
    vm: &mut Vm,
    editor: JValue,
) -> Result<
    std::collections::HashMap<String, std::collections::HashMap<String, PreferenceValue>>,
    NatErr,
> {
    let Some(Native::SharedPreferencesEditor { name, edits, clear }) = payload(vm, editor) else {
        return Err(npe(vm));
    };
    let (name, edits, clear) = (name.clone(), edits.clone(), *clear);
    let mut preferences = vm.shared_preferences.clone();
    let values = preferences.entry(name).or_default();
    if clear {
        values.clear();
    }
    for edit in edits {
        match edit {
            PreferenceEdit::Put(key, value) => {
                values.insert(key, value);
            }
            PreferenceEdit::Remove(key) => {
                values.remove(&key);
            }
        }
    }
    Ok(preferences)
}

pub(crate) fn editor_apply(vm: &mut Vm, args: &[JValue]) -> R {
    let preferences = edited_preferences(vm, args[0])?;
    vm.shared_preferences = preferences;
    if let Some(path) = vm.shared_preferences_path.clone() {
        // Android's apply() has no failure result: the in-memory update is
        // immediate and persistence is best-effort.
        let _ = persist_shared_preferences(&path, &vm.shared_preferences);
    }
    Ok(JValue::Null)
}

pub(crate) fn editor_commit(vm: &mut Vm, args: &[JValue]) -> R {
    let preferences = edited_preferences(vm, args[0])?;
    vm.shared_preferences = preferences;
    if let Some(path) = vm.shared_preferences_path.clone() {
        if persist_shared_preferences(&path, &vm.shared_preferences).is_err() {
            return Ok(JValue::Int(0));
        }
    }
    Ok(JValue::Int(1))
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

pub(crate) fn file_init_string(vm: &mut Vm, args: &[JValue]) -> R {
    let path = jstr(vm, args[1])?;
    let Some(Native::File { path: dst }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = path;
    Ok(JValue::Null)
}

pub(crate) fn file_init_parent_string(vm: &mut Vm, args: &[JValue]) -> R {
    let parent = file_path(vm, args[1])?;
    let child = jstr(vm, args[2])?;
    let path = std::path::PathBuf::from(parent)
        .join(child)
        .to_string_lossy()
        .into_owned();
    let Some(Native::File { path: dst }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = path;
    Ok(JValue::Null)
}

pub(crate) fn file_init_parent_strings(vm: &mut Vm, args: &[JValue]) -> R {
    let parent = jstr(vm, args[1])?;
    let child = jstr(vm, args[2])?;
    let path = std::path::PathBuf::from(parent)
        .join(child)
        .to_string_lossy()
        .into_owned();
    let Some(Native::File { path: dst }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = path;
    Ok(JValue::Null)
}

fn check_file_read(vm: &mut Vm, path: &str) -> Result<(), NatErr> {
    check_native_permission(
        vm,
        &Permission::Filesystem(FilesystemPermission::ReadPath(path.to_owned())),
    )
}

fn check_file_write(vm: &mut Vm, path: &str) -> Result<(), NatErr> {
    check_native_permission(
        vm,
        &Permission::Filesystem(FilesystemPermission::WritePath(path.to_owned())),
    )
}

/// `File.mkdirs() -> boolean`: really creates the directory tree.
pub(crate) fn file_mkdirs(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    check_file_write(vm, &path)?;
    Ok(JValue::Int(i32::from(
        std::fs::create_dir_all(&path).is_ok(),
    )))
}

/// `File.exists() -> boolean`: real filesystem check.
pub(crate) fn file_exists(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    check_file_read(vm, &path)?;
    Ok(JValue::Int(i32::from(std::fs::metadata(&path).is_ok())))
}

/// `File.lastModified() -> long`: real mtime in epoch millis (0 when the
/// file is missing, exactly like the JVM).
pub(crate) fn file_last_modified(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    check_file_read(vm, &path)?;
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
    check_file_read(vm, &path)?;
    let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    Ok(JValue::Long(len as i64))
}

/// `File.isDirectory() -> boolean`: real check.
pub(crate) fn file_is_directory(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    check_file_read(vm, &path)?;
    Ok(JValue::Int(i32::from(
        std::fs::metadata(&path)
            .map(|m| m.is_dir())
            .unwrap_or(false),
    )))
}

pub(crate) fn file_is_file(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    check_file_read(vm, &path)?;
    Ok(JValue::Int(i32::from(
        std::fs::metadata(path).is_ok_and(|m| m.is_file()),
    )))
}

pub(crate) fn file_create_new_file(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    check_file_write(vm, &path)?;
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(_) => Ok(JValue::Int(1)),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(JValue::Int(0)),
        Err(e) => Err(fnf(vm, e.to_string())),
    }
}

/// `File.delete() -> boolean`: really removes the file or empty directory.
pub(crate) fn file_delete(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    check_file_write(vm, &path)?;
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

pub(crate) fn file_get_absolute_path(vm: &mut Vm, args: &[JValue]) -> R {
    let path = std::path::PathBuf::from(file_path(vm, args[0])?);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/"))
            .join(path)
    };
    Ok(new_str(vm, &absolute.to_string_lossy()))
}

pub(crate) fn file_get_canonical_path(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    check_file_read(vm, &path)?;
    let canonical = std::fs::canonicalize(&path).map_err(|e| ioe(vm, e.to_string()))?;
    Ok(new_str(vm, &canonical.to_string_lossy()))
}

pub(crate) fn file_get_name(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(new_str(vm, &name))
}

pub(crate) fn file_get_parent(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    let Some(parent) = std::path::Path::new(&path).parent() else {
        return Ok(JValue::Null);
    };
    Ok(new_str(vm, &parent.to_string_lossy()))
}

pub(crate) fn file_get_parent_file(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_path(vm, args[0])?;
    let Some(parent) = std::path::Path::new(&path).parent() else {
        return Ok(JValue::Null);
    };
    alloc(
        vm,
        "Ljava/io/File;",
        Native::File {
            path: parent.to_string_lossy().into_owned(),
        },
    )
}

/// `File.createTempFile(prefix, suffix, directory) -> File`: creates a
/// unique real file next to the given directory.
pub(crate) fn file_create_temp_file(vm: &mut Vm, args: &[JValue]) -> R {
    let prefix = jstr(vm, args[0]).unwrap_or_default();
    let suffix = jstr(vm, args[1]).unwrap_or_default();
    let dir = file_path(vm, args[2])?;
    check_file_write(vm, &dir)?;
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
    check_file_write(vm, &from)?;
    check_file_write(vm, &to)?;
    Ok(JValue::Int(i32::from(std::fs::rename(&from, &to).is_ok())))
}

fn tempfile_in(dir: &str, prefix: &str, suffix: &str) -> std::io::Result<String> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    std::fs::create_dir_all(dir)?;
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let base = dir.trim_end_matches('/');
    let path = format!("{base}/{prefix}dexvm{}-{}{}", std::process::id(), n, suffix);
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
    let t: String = s.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let bytes = if flags & 8 != 0 {
        URL_SAFE
            .decode(&t)
            .or_else(|_| URL_SAFE_NO_PAD.decode(t.trim_end_matches('=')))
    } else {
        STANDARD
            .decode(&t)
            .or_else(|_| STANDARD_NO_PAD.decode(t.trim_end_matches('=')))
    }
    .map_err(|_| iae(vm, "Base64 decode failed"))?;
    let data = bytes.into_iter().map(|b| b as i8).collect::<Vec<_>>();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

/// `android.util.Base64.encodeToString(byte[], int)`.
fn base64_encode_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let flags = int_of(vm, args[1]);
    let url_safe = flags & 8 != 0;
    let no_padding = flags & 1 != 0;
    let raw = match (url_safe, no_padding) {
        (false, false) => STANDARD.encode(bytes),
        (false, true) => STANDARD_NO_PAD.encode(bytes),
        (true, false) => URL_SAFE.encode(bytes),
        (true, true) => URL_SAFE_NO_PAD.encode(bytes),
    };
    if flags & 2 != 0 || raw.is_empty() {
        return Ok(new_str(vm, &raw));
    }
    let newline = if flags & 4 != 0 { "\r\n" } else { "\n" };
    let mut wrapped = raw
        .as_bytes()
        .chunks(76)
        .map(|line| std::str::from_utf8(line).expect("base64 is ASCII"))
        .collect::<Vec<_>>()
        .join(newline);
    wrapped.push_str(newline);
    Ok(new_str(vm, &wrapped))
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
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        file_init_string
    ),
    ne!(
        "Ljava/io/File;",
        "<init>",
        "(Ljava/io/File;Ljava/lang/String;)V",
        true,
        file_init_parent_string
    ),
    ne!(
        "Ljava/io/File;",
        "<init>",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        true,
        file_init_parent_strings
    ),
    ne!("Ljava/io/File;", "mkdirs", "()Z", true, file_mkdirs),
    ne!("Ljava/io/File;", "exists", "()Z", true, file_exists),
    ne!(
        "Ljava/io/File;",
        "lastModified",
        "()J",
        true,
        file_last_modified
    ),
    ne!("Ljava/io/File;", "length", "()J", true, file_length),
    ne!(
        "Ljava/io/File;",
        "isDirectory",
        "()Z",
        true,
        file_is_directory
    ),
    ne!("Ljava/io/File;", "isFile", "()Z", true, file_is_file),
    ne!(
        "Ljava/io/File;",
        "createNewFile",
        "()Z",
        true,
        file_create_new_file
    ),
    ne!("Ljava/io/File;", "delete", "()Z", true, file_delete),
    ne!(
        "Ljava/io/File;",
        "getAbsolutePath",
        "()Ljava/lang/String;",
        true,
        file_get_absolute_path
    ),
    ne!(
        "Ljava/io/File;",
        "getCanonicalPath",
        "()Ljava/lang/String;",
        true,
        file_get_canonical_path
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
        "getName",
        "()Ljava/lang/String;",
        true,
        file_get_name
    ),
    ne!(
        "Ljava/io/File;",
        "getParent",
        "()Ljava/lang/String;",
        true,
        file_get_parent
    ),
    ne!(
        "Ljava/io/File;",
        "getParentFile",
        "()Ljava/io/File;",
        true,
        file_get_parent_file
    ),
    ne!(
        "Ljava/io/File;",
        "createTempFile",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/io/File;)Ljava/io/File;",
        false,
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
        false,
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
        "Landroid/content/SharedPreferences;",
        "contains",
        "(Ljava/lang/String;)Z",
        true,
        shared_prefs_contains
    ),
    ne!(
        "Landroid/content/SharedPreferences;",
        "getInt",
        "(Ljava/lang/String;I)I",
        true,
        shared_prefs_get_int
    ),
    ne!(
        "Landroid/content/SharedPreferences;",
        "getLong",
        "(Ljava/lang/String;J)J",
        true,
        shared_prefs_get_long
    ),
    ne!(
        "Landroid/content/SharedPreferences;",
        "getFloat",
        "(Ljava/lang/String;F)F",
        true,
        shared_prefs_get_float
    ),
    ne!(
        "Landroid/content/SharedPreferences;",
        "edit",
        "()Landroid/content/SharedPreferences$Editor;",
        true,
        shared_prefs_edit
    ),
    ne!(
        "Landroid/content/SharedPreferences$Editor;",
        "putBoolean",
        "(Ljava/lang/String;Z)Landroid/content/SharedPreferences$Editor;",
        true,
        editor_put_boolean
    ),
    ne!(
        "Landroid/content/SharedPreferences$Editor;",
        "putString",
        "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/SharedPreferences$Editor;",
        true,
        editor_put_string
    ),
    ne!(
        "Landroid/content/SharedPreferences$Editor;",
        "remove",
        "(Ljava/lang/String;)Landroid/content/SharedPreferences$Editor;",
        true,
        editor_remove
    ),
    ne!(
        "Landroid/content/SharedPreferences$Editor;",
        "putInt",
        "(Ljava/lang/String;I)Landroid/content/SharedPreferences$Editor;",
        true,
        editor_put_int
    ),
    ne!(
        "Landroid/content/SharedPreferences$Editor;",
        "putLong",
        "(Ljava/lang/String;J)Landroid/content/SharedPreferences$Editor;",
        true,
        editor_put_long
    ),
    ne!(
        "Landroid/content/SharedPreferences$Editor;",
        "putFloat",
        "(Ljava/lang/String;F)Landroid/content/SharedPreferences$Editor;",
        true,
        editor_put_float
    ),
    ne!(
        "Landroid/content/SharedPreferences$Editor;",
        "clear",
        "()Landroid/content/SharedPreferences$Editor;",
        true,
        editor_clear
    ),
    ne!(
        "Landroid/content/SharedPreferences$Editor;",
        "apply",
        "()V",
        true,
        editor_apply
    ),
    ne!(
        "Landroid/content/SharedPreferences$Editor;",
        "commit",
        "()Z",
        true,
        editor_commit
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
        false,
        base64_decode
    ),
    ne!(
        "Landroid/util/Base64;",
        "encodeToString",
        "([BI)Ljava/lang/String;",
        false,
        base64_encode_to_string
    ),
    ne!(
        "Landroid/os/SystemClock;",
        "elapsedRealtime",
        "()J",
        false,
        elpased_realtime
    ),
];

pub(crate) fn elpased_realtime(_vm: &mut Vm, _args: &[JValue]) -> R {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let millis = i64::try_from(
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_millis(),
    )
    .unwrap_or(i64::MAX);
    Ok(JValue::Long(millis))
}

#[cfg(test)]
mod tests;
