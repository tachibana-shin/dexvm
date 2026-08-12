//! android / androidx framework host shims (keiyoushi feature).

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine as _;

use super::*;
use crate::permission::{FilesystemPermission, Permission};

pub(crate) mod graphics;

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
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| pref_invalid())?,
        ))
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
                2 => PreferenceValue::Int(i32::from_le_bytes(
                    reader.take(4)?.try_into().map_err(|_| pref_invalid())?,
                )),
                3 => PreferenceValue::Long(i64::from_le_bytes(
                    reader.take(8)?.try_into().map_err(|_| pref_invalid())?,
                )),
                4 => PreferenceValue::Float(f32::from_bits(u32::from_le_bytes(
                    reader.take(4)?.try_into().map_err(|_| pref_invalid())?,
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

pub(crate) fn prefs_obj(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    let desc = vm.str_of(vm.classes[vm.arena.objects[this as usize].class as usize].descriptor);
    vm.arena.objects[this as usize].native =
        Some(if desc == "Landroidx/preference/PreferenceScreen;" {
            Native::PreferenceScreen {
                children: Vec::new(),
                title: None,
            }
        } else {
            Native::Preference {
                key: None,
                title: None,
                summary: None,
                default_value: JValue::Null,
                enabled: true,
                visible: true,
            }
        });
    Ok(JValue::Null)
}

pub(crate) fn prefs_ctx(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/content/Context;", Native::Opaque)
}

/// `Log.e(tag, msg, throwable)` — android log; swallowed on the host.
pub(crate) fn log_error(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}
pub(crate) fn prefs_set(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Preference { default_value, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if let Some(value) = args.get(1) {
        *default_value = *value;
    }
    Ok(JValue::Null)
}

pub(crate) fn prefs_add_preference(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::PreferenceScreen { children, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    children.push(args[1]);
    Ok(JValue::Int(1))
}

/// `Context.getCacheDir() -> File` backed by a real per-VM host directory
/// (created lazily; the extension mkdirs its own subdirectories on top).
pub(crate) fn context_get_cache_dir(vm: &mut Vm, _args: &[JValue]) -> R {
    let path = vm.cache_root_path().to_string();
    alloc(vm, "Ljava/io/File;", Native::File { path })
}

fn intent_native(vm: &mut Vm, receiver: JValue) -> Result<&mut Native, NatErr> {
    if payload(vm, receiver).is_none() {
        return Err(npe(vm));
    }
    Ok(payload_mut(vm, receiver).expect("payload checked"))
}

pub(crate) fn intent_init(vm: &mut Vm, args: &[JValue]) -> R {
    let action = args.get(1).copied().filter(|v| !matches!(v, JValue::Null));
    let action = action.map(|v| jstr(vm, v)).transpose()?;
    let native = intent_native(vm, args[0])?;
    *native = Native::Intent {
        action,
        data: None,
        extras: Vec::new(),
    };
    Ok(JValue::Null)
}

pub(crate) fn intent_put_extra_string(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let value = args[2];
    let native = intent_native(vm, args[0])?;
    let Native::Intent { extras, .. } = native else {
        return Err(npe(vm));
    };
    if let Some(entry) = extras.iter_mut().find(|(k, _)| *k == key) {
        entry.1 = value;
    } else {
        extras.push((key, value));
    }
    Ok(args[0])
}

pub(crate) fn intent_put_extra_long(vm: &mut Vm, args: &[JValue]) -> R {
    intent_put_extra_string(vm, args)
}

pub(crate) fn intent_get_data(vm: &mut Vm, args: &[JValue]) -> R {
    let data = match intent_native(vm, args[0])? {
        Native::Intent { data, .. } => data.clone(),
        _ => return Err(npe(vm)),
    };
    match data {
        Some(value) => alloc(vm, "Landroid/net/Uri;", Native::URI(value)),
        None => Ok(JValue::Null),
    }
}

pub(crate) fn intent_set_action(vm: &mut Vm, args: &[JValue]) -> R {
    let action = jstr(vm, args[1])?;
    let native = intent_native(vm, args[0])?;
    let Native::Intent { action: slot, .. } = native else {
        return Err(npe(vm));
    };
    *slot = Some(action);
    Ok(args[0])
}

pub(crate) fn context_wrapper_start_activity(_vm: &mut Vm, _args: &[JValue]) -> R {
    // The VM has no UI host. Keep this as an intentional, side-effect-free bridge.
    Ok(JValue::Null)
}

pub(crate) fn context_get_package_name(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(new_str(vm, "dexvm.extension"))
}

pub(crate) fn activity_init(vm: &mut Vm, args: &[JValue]) -> R {
    let intent = alloc(
        vm,
        "Landroid/content/Intent;",
        Native::Intent {
            action: None,
            data: None,
            extras: Vec::new(),
        },
    )?;
    let native = intent_native(vm, args[0])?;
    *native = Native::Activity {
        intent,
        finished: false,
    };
    Ok(JValue::Null)
}

pub(crate) fn activity_finish(vm: &mut Vm, args: &[JValue]) -> R {
    let native = intent_native(vm, args[0])?;
    let Native::Activity { finished, .. } = native else {
        return Err(npe(vm));
    };
    *finished = true;
    Ok(JValue::Null)
}

pub(crate) fn activity_get_intent(vm: &mut Vm, args: &[JValue]) -> R {
    let existing = match payload(vm, args[0]) {
        Some(Native::Activity { intent, .. }) => *intent,
        _ => return Err(npe(vm)),
    };
    let intent = if matches!(existing, JValue::Null) {
        let created = alloc(
            vm,
            "Landroid/content/Intent;",
            Native::Intent {
                action: None,
                data: None,
                extras: Vec::new(),
            },
        )?;
        let native = intent_native(vm, args[0])?;
        let Native::Activity { intent, .. } = native else {
            return Err(npe(vm));
        };
        *intent = created;
        created
    } else {
        existing
    };
    Ok(intent)
}

pub(crate) fn activity_on_create(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
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

/// `Base64.decode(byte[], int) -> [B`: byte-input sibling of `base64_decode`.
fn base64_decode_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let flags = int_of(vm, args[1]);
    let trimmed: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let unpadded = trimmed.len() - trimmed.iter().rev().take_while(|&&b| b == b'=').count();
    let decoded = if flags & 8 != 0 {
        URL_SAFE
            .decode(&trimmed)
            .or_else(|_| URL_SAFE_NO_PAD.decode(&trimmed[..unpadded]))
    } else {
        STANDARD
            .decode(&trimmed)
            .or_else(|_| STANDARD_NO_PAD.decode(&trimmed[..unpadded]))
    }
    .map_err(|_| iae(vm, "Base64 decode failed"))?;
    let data = decoded.into_iter().map(|b| b as i8).collect::<Vec<_>>();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

/// `Base64.encode(byte[], int) -> [B`: byte-output sibling of
/// `base64_encode_to_string` with the same flag semantics.
fn base64_encode_bytes(vm: &mut Vm, args: &[JValue]) -> R {
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
    let wrapped = if flags & 2 != 0 || raw.is_empty() {
        raw.into_bytes()
    } else {
        let newline: &[u8] = if flags & 4 != 0 { b"\r\n" } else { b"\n" };
        let mut out: Vec<u8> = raw.as_bytes().chunks(76).collect::<Vec<_>>().join(newline);
        out.extend_from_slice(newline);
        out
    };
    let data = wrapped.into_iter().map(|b| b as i8).collect::<Vec<_>>();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

// ---------------------------------------------------------------------------
// android.widget / android.view stubs (headless host: UI never renders)
// ---------------------------------------------------------------------------

/// Generic void / constructor no-op for UI and framework objects.
fn ui_noop(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// Generic `0`-returning stub for getters without host state.
fn ui_zero(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

/// Generic `false`-returning stub for boolean getters.
fn ui_false(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

/// Generic null-returning stub for object getters.
fn ui_null(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// Generic empty-string stub for string getters.
fn ui_empty_string(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(new_str(vm, ""))
}

/// Identity stub: a dead view's root view is itself.
fn view_self(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

/// `View.post(Runnable)` / `Handler.post(Runnable)` / `Handler.postDelayed`:
/// the host runs the runnable synchronously (there is no UI thread).
fn runnable_post(vm: &mut Vm, args: &[JValue]) -> R {
    if args[1].is_null() {
        return Err(npe(vm));
    }
    inv_virt(vm, args[1], "run", "()V", &[])?;
    Ok(JValue::Int(1))
}

/// `View$MeasureSpec.makeMeasureSpec(size, mode)`: real packed value.
fn measure_spec_make(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]) + int_of(vm, args[1])))
}

/// `Toast.makeText(Context, CharSequence, int) -> Toast`.
fn toast_make_text(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/widget/Toast;", Native::Opaque)
}

fn toast_show(_vm: &mut Vm, _args: &[JValue]) -> R {
    log::warn!("Toast.show is a no-op on the headless host");
    Ok(JValue::Null)
}

// ---------------------------------------------------------------------------
// android.os / Looper / Handler
// ---------------------------------------------------------------------------

/// `Looper.getMainLooper()` / `Looper.myLooper()`.
fn looper_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/os/Looper;", Native::Opaque)
}

/// `ParcelFileDescriptor.open(File, int)`: opaque handle (pdf renderers are
/// stubbed anyway).
fn parcel_fd_open(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/os/ParcelFileDescriptor;", Native::Opaque)
}

// ---------------------------------------------------------------------------
// android.webkit stubs (no real webview on the host)
// ---------------------------------------------------------------------------

const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36 DexVM/0.1";

fn webview_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    log::warn!("WebView is not implemented on the headless host");
    Ok(JValue::Null)
}

fn webview_load_url(vm: &mut Vm, args: &[JValue]) -> R {
    let url = jstr(vm, args[1]).unwrap_or_default();
    log::warn!("WebView.loadUrl is a no-op on the headless host (url={url})");
    Ok(JValue::Null)
}

fn webview_load_data(vm: &mut Vm, args: &[JValue]) -> R {
    let data = jstr(vm, args[1]).unwrap_or_default();
    log::warn!(
        "WebView.loadDataWithBaseURL is a no-op on the headless host (len={})",
        data.len()
    );
    Ok(JValue::Null)
}

fn webview_evaluate_js(_vm: &mut Vm, _args: &[JValue]) -> R {
    log::warn!("WebView.evaluateJavascript is a no-op on the headless host");
    Ok(JValue::Null)
}

fn webview_add_js_interface(_vm: &mut Vm, _args: &[JValue]) -> R {
    log::warn!("WebView.addJavascriptInterface is a no-op on the headless host");
    Ok(JValue::Null)
}

/// `WebView.getSettings() -> WebSettings`.
fn web_settings_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/webkit/WebSettings;", Native::Opaque)
}

/// `WebSettings.getUserAgentString()` / `getDefaultUserAgent(Context)`.
fn default_user_agent(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(new_str(vm, DEFAULT_USER_AGENT))
}

/// `WebResourceRequest.getRequestHeaders() -> Map`.
fn web_request_headers(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/HashMap;", Native::Map(Vec::new()))
}

/// `ConsoleMessage$MessageLevel.values()`: the real five levels.
fn console_message_level_values(vm: &mut Vm, _args: &[JValue]) -> R {
    let levels = [
        ("DEBUG", 0),
        ("ERROR", 1),
        ("WARNING", 2),
        ("LOG", 3),
        ("TIP", 4),
    ];
    let values = levels
        .iter()
        .map(|(name, ordinal)| {
            alloc(
                vm,
                "Landroid/webkit/ConsoleMessage$MessageLevel;",
                Native::Enum {
                    name: (*name).to_string(),
                    ordinal: *ordinal,
                },
            )
        })
        .collect::<Result<Vec<_>, NatErr>>()?;
    alloc_arr(
        vm,
        "Landroid/webkit/ConsoleMessage$MessageLevel;",
        values.len(),
        move || ArrayData::Obj(values),
    )
}

/// `URLUtil.isValidUrl(String)`: scheme prefix check.
fn url_util_valid_url(vm: &mut Vm, args: &[JValue]) -> R {
    let url = jstr(vm, args[0]).unwrap_or_default().to_ascii_lowercase();
    let valid = ["http://", "https://", "file://", "content://", "ftp://"]
        .iter()
        .any(|prefix| url.starts_with(prefix));
    Ok(JValue::Int(i32::from(valid)))
}

/// `CookieManager.getInstance() -> CookieManager`.
fn cookie_manager_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/webkit/CookieManager;", Native::Opaque)
}

fn cookie_manager_set(_vm: &mut Vm, _args: &[JValue]) -> R {
    log::warn!("CookieManager has no cookie store on the headless host");
    Ok(JValue::Null)
}

fn cookie_manager_get(_vm: &mut Vm, _args: &[JValue]) -> R {
    log::warn!("CookieManager.getCookie returns null on the headless host");
    Ok(JValue::Null)
}

// ---------------------------------------------------------------------------
// android.net.Uri real bridge
// ---------------------------------------------------------------------------

/// Percent-encode like `Uri.encode`: everything but letters, digits and
/// `-_.!~*'()` becomes %XX.
fn uri_encode_impl(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        };
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(hi * 16 + lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn uri_string_of(vm: &mut Vm, v: JValue) -> Result<String, NatErr> {
    match payload(vm, v) {
        Some(Native::URI(s)) => Ok(s.clone()),
        _ => Err(npe(vm)),
    }
}

/// Mutable access to a `Uri$Builder`'s accumulated string; fresh objects
/// (plain `new-instance`) get an empty payload on first use.
fn uri_builder_mut(vm: &mut Vm, v: JValue) -> Result<&mut Native, NatErr> {
    let JValue::Obj(id) = v else {
        return Err(npe(vm));
    };
    let has = vm
        .arena
        .get(id)
        .is_some_and(|o| matches!(o.native.as_ref(), Some(Native::URI(_))));
    if !has {
        let Some(o) = vm.arena.get_mut(id) else {
            return Err(npe(vm));
        };
        o.native = Some(Native::URI(String::new()));
    }
    if vm.arena.get(id).is_none() {
        return Err(npe(vm));
    }
    let err = npe(vm);
    let Some(o) = vm.arena.get_mut(id) else {
        return Err(err);
    };
    match o.native.as_mut() {
        Some(payload) => Ok(payload),
        None => Err(err),
    }
}

fn uri_builder_append_query(vm: &mut Vm, args: &[JValue]) -> R {
    let key = uri_encode_impl(&jstr(vm, args[1])?);
    let value = uri_encode_impl(&jstr(vm, args[2])?);
    let Native::URI(s) = uri_builder_mut(vm, args[0])? else {
        unreachable!("payload installed by uri_builder_mut")
    };
    if s.contains('?') {
        s.push('&');
    } else {
        s.push('?');
    }
    s.push_str(&key);
    s.push('=');
    s.push_str(&value);
    Ok(args[0])
}

fn uri_builder_append_segment(vm: &mut Vm, args: &[JValue], encoded: bool) -> R {
    let segment = jstr(vm, args[1])?;
    let segment = if encoded {
        segment
    } else {
        uri_encode_impl(&segment)
    };
    let Native::URI(s) = uri_builder_mut(vm, args[0])? else {
        unreachable!("payload installed by uri_builder_mut")
    };
    if !s.is_empty() && !s.ends_with('/') {
        s.push('/');
    }
    s.push_str(&segment);
    Ok(args[0])
}

fn uri_builder_append_path(vm: &mut Vm, args: &[JValue]) -> R {
    uri_builder_append_segment(vm, args, false)
}

fn uri_builder_append_encoded_path(vm: &mut Vm, args: &[JValue]) -> R {
    uri_builder_append_segment(vm, args, true)
}

fn uri_builder_fragment(vm: &mut Vm, args: &[JValue]) -> R {
    let fragment = uri_encode_impl(&jstr(vm, args[1])?);
    let Native::URI(s) = uri_builder_mut(vm, args[0])? else {
        unreachable!("payload installed by uri_builder_mut")
    };
    if let Some(idx) = s.find('#') {
        s.truncate(idx);
    }
    s.push('#');
    s.push_str(&fragment);
    Ok(args[0])
}

fn uri_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let s = uri_string_of(vm, args[0])?;
    alloc(vm, "Landroid/net/Uri;", Native::URI(s))
}

fn uri_builder_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let s = uri_string_of(vm, args[0])?;
    Ok(new_str(vm, &s))
}

/// `Uri.parse(String) -> Uri`.
fn uri_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    alloc(vm, "Landroid/net/Uri;", Native::URI(s))
}

fn uri_build_upon(vm: &mut Vm, args: &[JValue]) -> R {
    let s = uri_string_of(vm, args[0])?;
    alloc(vm, "Landroid/net/Uri$Builder;", Native::URI(s))
}

/// `Uri.encode(String) -> String`.
fn uri_encode(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    Ok(new_str(vm, &uri_encode_impl(&s)))
}

struct UriParts {
    host: Option<String>,
    path: String,
    query: Option<String>,
}

fn uri_parts_of(vm: &mut Vm, v: JValue) -> Result<UriParts, NatErr> {
    let s = uri_string_of(vm, v)?;
    let before_fragment = s.split_once('#').map_or(&*s, |(a, _)| a);
    let (before_query, query) = match before_fragment.split_once('?') {
        Some((a, b)) => (a, Some(b.to_string())),
        None => (before_fragment, None),
    };
    let rest = before_query
        .split_once(':')
        .map_or(before_query, |(_, b)| b);
    let (host, path) = match rest.strip_prefix("//") {
        Some(r) => match r.split_once('/') {
            Some((h, p)) => (Some(h.to_string()), format!("/{p}")),
            None => (Some(r.to_string()), String::new()),
        },
        None => (None, rest.to_string()),
    };
    Ok(UriParts { host, path, query })
}

fn uri_get_host(vm: &mut Vm, args: &[JValue]) -> R {
    match uri_parts_of(vm, args[0])?.host {
        Some(host) => Ok(new_str(vm, &host)),
        None => Ok(JValue::Null),
    }
}

fn uri_get_path(vm: &mut Vm, args: &[JValue]) -> R {
    let path = uri_parts_of(vm, args[0])?.path;
    Ok(new_str(vm, &percent_decode(&path)))
}

fn uri_get_encoded_path(vm: &mut Vm, args: &[JValue]) -> R {
    let path = uri_parts_of(vm, args[0])?.path;
    Ok(new_str(vm, &path))
}

fn uri_get_encoded_query(vm: &mut Vm, args: &[JValue]) -> R {
    match uri_parts_of(vm, args[0])?.query {
        Some(query) => Ok(new_str(vm, &query)),
        None => Ok(JValue::Null),
    }
}

// ---------------------------------------------------------------------------
// android.content leftovers
// ---------------------------------------------------------------------------

/// `SharedPreferences$Editor.putStringSet`: stored as a '\n'-joined string
/// (the host preference store has no set type).
fn editor_put_string_set(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[2])?
        .into_iter()
        .map(|v| jstr(vm, v))
        .collect::<Result<Vec<_>, NatErr>>()?
        .join("\n");
    editor_put(vm, args, PreferenceValue::String(values))
}

/// `ContextWrapper.getExternalCacheDir() -> File`: a real per-VM host dir.
fn context_wrapper_external_cache_dir(vm: &mut Vm, _args: &[JValue]) -> R {
    let path = std::path::Path::new(vm.cache_root_path())
        .join("external")
        .to_string_lossy()
        .into_owned();
    alloc(vm, "Ljava/io/File;", Native::File { path })
}

fn context_wrapper_application_info(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/content/pm/ApplicationInfo;", Native::Opaque)
}

fn context_wrapper_system_service(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

fn intent_add_flags(vm: &mut Vm, args: &[JValue]) -> R {
    intent_native(vm, args[0])?;
    Ok(args[0])
}

fn intent_set_component(vm: &mut Vm, args: &[JValue]) -> R {
    intent_native(vm, args[0])?;
    Ok(args[0])
}

// ---------------------------------------------------------------------------
// android.util extras
// ---------------------------------------------------------------------------

/// `Base64InputStream.<init>`: the decode stream is not implemented.
fn base64_input_stream_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    log::warn!("Base64InputStream is not implemented on the host; it reads nothing");
    Ok(JValue::Null)
}

/// `JsonReader` is a streaming parser over a `Reader`; the host has no real
/// reader bridge, so the reader starts empty (hasNext() == false).
fn json_reader_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    log::warn!("android.util.JsonReader is not implemented on the host; the reader starts empty");
    Ok(JValue::Null)
}

fn json_reader_has_next(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

fn json_reader_noop(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// `JsonReader.nextString()` / `nextName()`: no tokens on an empty reader.
fn json_reader_null(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

fn json_reader_next_double(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Double(f64::NAN))
}

// ---------------------------------------------------------------------------
// android.content.res / android.app stubs
// ---------------------------------------------------------------------------

fn resources_system(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/content/res/Resources;", Native::Opaque)
}

fn resources_display_metrics(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/util/DisplayMetrics;", Native::Opaque)
}

// ---------------------------------------------------------------------------
// android.text stubs
// ---------------------------------------------------------------------------

/// `Html.fromHtml(String, int)`: returns the tag-stripped text as a String.
fn html_from_html(vm: &mut Vm, args: &[JValue]) -> R {
    let html = jstr(vm, args[0])?;
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let text = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    Ok(new_str(vm, &text))
}

/// Chained `StaticLayout$Builder` setters: keep the builder payload as-is.
fn static_layout_builder_set(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

/// `StaticLayout$Builder.obtain(CharSequence, int, int, TextPaint, int)`:
/// a fresh opaque builder (the host renders no text).
fn static_layout_builder_obtain_cs(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/text/StaticLayout$Builder;", Native::Opaque)
}

fn link_movement_method_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Landroid/text/method/LinkMovementMethod;",
        Native::Opaque,
    )
}

// ---------------------------------------------------------------------------
// android.graphics leftovers (not in graphics.rs)
// ---------------------------------------------------------------------------

fn typeface_create_from_file(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/graphics/Typeface;", Native::Opaque)
}

fn image_decoder_create_source(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/graphics/ImageDecoder$Source;", Native::Opaque)
}

fn pdf_renderer_open_page(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Landroid/graphics/pdf/PdfRenderer$Page;",
        Native::Opaque,
    )
}

// ---------------------------------------------------------------------------
// android.icu.text stubs
// ---------------------------------------------------------------------------

fn break_iterator_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/icu/text/BreakIterator;", Native::Opaque)
}

/// `BreakIterator.first()`: text boundaries are not computed; 0 = start.
fn break_iterator_first(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

/// `BreakIterator.next()`: no boundary past the start (DONE = -1).
fn break_iterator_next(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(-1))
}

fn collator_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/icu/text/Collator;", Native::Opaque)
}

fn normalizer2_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/icu/text/Normalizer2;", Native::Opaque)
}

/// `Normalizer2.normalize(CharSequence)`: without normalization, identity.
fn normalizer2_normalize(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[1])?;
    Ok(new_str(vm, &s))
}

/// `SearchIterator.first()` / `next()`: no matches ever (DONE = -1).
fn search_iterator_done(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(-1))
}

// ---------------------------------------------------------------------------
// android native table
// ---------------------------------------------------------------------------

pub(crate) const ANDROID_TABLE: &[NativeEntry] = &[
    ne!(
        "Landroid/content/Intent;",
        "<init>",
        "()V",
        true,
        intent_init
    ),
    ne!(
        "Landroid/content/Intent;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        intent_init
    ),
    ne!(
        "Landroid/content/Intent;",
        "putExtra",
        "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;",
        true,
        intent_put_extra_string
    ),
    ne!(
        "Landroid/content/Intent;",
        "putExtra",
        "(Ljava/lang/String;J)Landroid/content/Intent;",
        true,
        intent_put_extra_long
    ),
    ne!(
        "Landroid/content/Intent;",
        "getData",
        "()Landroid/net/Uri;",
        true,
        intent_get_data
    ),
    ne!(
        "Landroid/content/Intent;",
        "setAction",
        "(Ljava/lang/String;)Landroid/content/Intent;",
        true,
        intent_set_action
    ),
    ne!(
        "Landroid/content/ContextWrapper;",
        "startActivity",
        "(Landroid/content/Intent;)V",
        true,
        context_wrapper_start_activity
    ),
    ne!(
        "Landroid/content/Context;",
        "getPackageName",
        "()Ljava/lang/String;",
        true,
        context_get_package_name
    ),
    ne!(
        "Landroid/content/ContextWrapper;",
        "getPackageName",
        "()Ljava/lang/String;",
        true,
        context_get_package_name
    ),
    ne!(
        "Landroid/app/Activity;",
        "<init>",
        "()V",
        true,
        activity_init
    ),
    ne!(
        "Landroid/app/Activity;",
        "finish",
        "()V",
        true,
        activity_finish
    ),
    ne!(
        "Landroid/app/Activity;",
        "getIntent",
        "()Landroid/content/Intent;",
        true,
        activity_get_intent
    ),
    ne!(
        "Landroid/app/Activity;",
        "onCreate",
        "(Landroid/os/Bundle;)V",
        true,
        activity_on_create
    ),
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
    // ---- android.widget / android.view ----
    ne!(
        "Landroid/widget/TextView;",
        "addTextChangedListener",
        "(Landroid/text/TextWatcher;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/widget/TextView;",
        "setHint",
        "(Ljava/lang/CharSequence;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/widget/TextView;",
        "setHorizontallyScrolling",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/widget/TextView;",
        "setInputType",
        "(I)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/widget/TextView;",
        "getError",
        "()Ljava/lang/CharSequence;",
        true,
        ui_null
    ),
    ne!(
        "Landroid/widget/TextView;",
        "setError",
        "(Ljava/lang/CharSequence;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/widget/TextView;",
        "setMovementMethod",
        "(Landroid/text/method/MovementMethod;)V",
        true,
        ui_noop
    ),
    ne!("Landroid/widget/EditText;", "selectAll", "()V", true, ui_noop),
    ne!(
        "Landroid/view/View;",
        "findViewById",
        "(I)Landroid/view/View;",
        true,
        ui_null
    ),
    ne!(
        "Landroid/view/View;",
        "getRootView",
        "()Landroid/view/View;",
        true,
        view_self
    ),
    ne!(
        "Landroid/view/View;",
        "setEnabled",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/view/View;",
        "post",
        "(Ljava/lang/Runnable;)Z",
        true,
        runnable_post
    ),
    ne!(
        "Landroid/view/View;",
        "layout",
        "(IIII)V",
        true,
        ui_noop
    ),
    ne!("Landroid/view/View;", "measure", "(II)V", true, ui_noop),
    ne!(
        "Landroid/view/View;",
        "getParent",
        "()Landroid/view/ViewParent;",
        true,
        ui_null
    ),
    ne!(
        "Landroid/view/ViewGroup;",
        "getChildCount",
        "()I",
        true,
        ui_zero
    ),
    ne!(
        "Landroid/view/ViewGroup;",
        "getChildAt",
        "(I)Landroid/view/View;",
        true,
        ui_null
    ),
    ne!(
        "Landroid/view/ViewGroup$LayoutParams;",
        "<init>",
        "(II)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/view/View$MeasureSpec;",
        "makeMeasureSpec",
        "(II)I",
        false,
        measure_spec_make
    ),
    ne!(
        "Landroid/widget/Toast;",
        "makeText",
        "(Landroid/content/Context;Ljava/lang/CharSequence;I)Landroid/widget/Toast;",
        false,
        toast_make_text
    ),
    ne!("Landroid/widget/Toast;", "show", "()V", true, toast_show),
    // ---- android.os ----
    ne!(
        "Landroid/os/Looper;",
        "getMainLooper",
        "()Landroid/os/Looper;",
        false,
        looper_instance
    ),
    ne!(
        "Landroid/os/Looper;",
        "myLooper",
        "()Landroid/os/Looper;",
        false,
        looper_instance
    ),
    ne!(
        "Landroid/os/Handler;",
        "<init>",
        "(Landroid/os/Looper;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/os/Handler;",
        "post",
        "(Ljava/lang/Runnable;)Z",
        true,
        runnable_post
    ),
    ne!(
        "Landroid/os/Handler;",
        "postDelayed",
        "(Ljava/lang/Runnable;J)Z",
        true,
        runnable_post
    ),
    ne!(
        "Landroid/os/Handler;",
        "removeCallbacks",
        "(Ljava/lang/Runnable;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/os/ParcelFileDescriptor;",
        "open",
        "(Ljava/io/File;I)Landroid/os/ParcelFileDescriptor;",
        false,
        parcel_fd_open
    ),
    // ---- android.webkit ----
    ne!(
        "Landroid/webkit/WebView;",
        "<init>",
        "(Landroid/content/Context;)V",
        true,
        webview_init
    ),
    ne!(
        "Landroid/webkit/WebView;",
        "getSettings",
        "()Landroid/webkit/WebSettings;",
        true,
        web_settings_instance
    ),
    ne!("Landroid/webkit/WebView;", "destroy", "()V", true, ui_noop),
    ne!(
        "Landroid/webkit/WebView;",
        "stopLoading",
        "()V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebView;",
        "evaluateJavascript",
        "(Ljava/lang/String;Landroid/webkit/ValueCallback;)V",
        true,
        webview_evaluate_js
    ),
    ne!(
        "Landroid/webkit/WebView;",
        "loadDataWithBaseURL",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        true,
        webview_load_data
    ),
    ne!(
        "Landroid/webkit/WebView;",
        "loadUrl",
        "(Ljava/lang/String;)V",
        true,
        webview_load_url
    ),
    ne!(
        "Landroid/webkit/WebView;",
        "loadUrl",
        "(Ljava/lang/String;Ljava/util/Map;)V",
        true,
        webview_load_url
    ),
    ne!(
        "Landroid/webkit/WebView;",
        "addJavascriptInterface",
        "(Ljava/lang/Object;Ljava/lang/String;)V",
        true,
        webview_add_js_interface
    ),
    ne!(
        "Landroid/webkit/WebView;",
        "removeJavascriptInterface",
        "(Ljava/lang/String;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebView;",
        "setWebViewClient",
        "(Landroid/webkit/WebViewClient;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebView;",
        "setWebChromeClient",
        "(Landroid/webkit/WebChromeClient;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebView;",
        "setLayoutParams",
        "(Landroid/view/ViewGroup$LayoutParams;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebView;",
        "setLayerType",
        "(ILandroid/graphics/Paint;)V",
        true,
        ui_noop
    ),
    ne!("Landroid/webkit/WebView;", "onResume", "()V", true, ui_noop),
    ne!(
        "Landroid/webkit/WebView;",
        "resumeTimers",
        "()V",
        false,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebViewClient;",
        "<init>",
        "()V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebViewClient;",
        "shouldInterceptRequest",
        "(Landroid/webkit/WebView;Landroid/webkit/WebResourceRequest;)Landroid/webkit/WebResourceResponse;",
        true,
        ui_null
    ),
    ne!(
        "Landroid/webkit/WebViewClient;",
        "onPageFinished",
        "(Landroid/webkit/WebView;Ljava/lang/String;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebViewClient;",
        "onPageStarted",
        "(Landroid/webkit/WebView;Ljava/lang/String;Landroid/graphics/Bitmap;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebChromeClient;",
        "<init>",
        "()V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "setJavaScriptEnabled",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "setDomStorageEnabled",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "setBlockNetworkImage",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "setUserAgentString",
        "(Ljava/lang/String;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "setLoadWithOverviewMode",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "setUseWideViewPort",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "setDatabaseEnabled",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "setBlockNetworkLoads",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "setLoadsImagesAutomatically",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "getUserAgentString",
        "()Ljava/lang/String;",
        true,
        default_user_agent
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "getBlockNetworkImage",
        "()Z",
        true,
        ui_false
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "getDomStorageEnabled",
        "()Z",
        true,
        ui_false
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "getJavaScriptEnabled",
        "()Z",
        true,
        ui_false
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "getLoadWithOverviewMode",
        "()Z",
        true,
        ui_false
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "getUseWideViewPort",
        "()Z",
        true,
        ui_false
    ),
    ne!(
        "Landroid/webkit/WebSettings;",
        "getDefaultUserAgent",
        "(Landroid/content/Context;)Ljava/lang/String;",
        false,
        default_user_agent
    ),
    ne!(
        "Landroid/webkit/ConsoleMessage$MessageLevel;",
        "values",
        "()[Landroid/webkit/ConsoleMessage$MessageLevel;",
        false,
        console_message_level_values
    ),
    ne!(
        "Landroid/webkit/ConsoleMessage;",
        "lineNumber",
        "()I",
        true,
        ui_zero
    ),
    ne!(
        "Landroid/webkit/ConsoleMessage;",
        "message",
        "()Ljava/lang/String;",
        true,
        ui_empty_string
    ),
    ne!(
        "Landroid/webkit/ConsoleMessage;",
        "messageLevel",
        "()Landroid/webkit/ConsoleMessage$MessageLevel;",
        true,
        ui_null
    ),
    ne!(
        "Landroid/webkit/ConsoleMessage;",
        "sourceId",
        "()Ljava/lang/String;",
        true,
        ui_empty_string
    ),
    ne!(
        "Landroid/webkit/RenderProcessGoneDetail;",
        "didCrash",
        "()Z",
        true,
        ui_false
    ),
    ne!(
        "Landroid/webkit/WebResourceRequest;",
        "getUrl",
        "()Landroid/net/Uri;",
        true,
        ui_null
    ),
    ne!(
        "Landroid/webkit/WebResourceRequest;",
        "getRequestHeaders",
        "()Ljava/util/Map;",
        true,
        web_request_headers
    ),
    ne!(
        "Landroid/webkit/WebResourceRequest;",
        "getMethod",
        "()Ljava/lang/String;",
        true,
        ui_empty_string
    ),
    ne!(
        "Landroid/webkit/WebResourceRequest;",
        "isForMainFrame",
        "()Z",
        true,
        ui_false
    ),
    ne!(
        "Landroid/webkit/WebResourceResponse;",
        "<init>",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/io/InputStream;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebResourceResponse;",
        "<init>",
        "(Ljava/lang/String;Ljava/lang/String;ILjava/lang/String;Ljava/util/Map;Ljava/io/InputStream;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/WebResourceResponse;",
        "getStatusCode",
        "()I",
        true,
        ui_zero
    ),
    ne!(
        "Landroid/webkit/SslErrorHandler;",
        "proceed",
        "()V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/webkit/URLUtil;",
        "isValidUrl",
        "(Ljava/lang/String;)Z",
        false,
        url_util_valid_url
    ),
    ne!(
        "Landroid/webkit/CookieManager;",
        "getInstance",
        "()Landroid/webkit/CookieManager;",
        false,
        cookie_manager_instance
    ),
    ne!(
        "Landroid/webkit/CookieManager;",
        "setCookie",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        true,
        cookie_manager_set
    ),
    ne!(
        "Landroid/webkit/CookieManager;",
        "getCookie",
        "(Ljava/lang/String;)Ljava/lang/String;",
        true,
        cookie_manager_get
    ),
    ne!(
        "Landroid/webkit/CookieManager;",
        "setAcceptCookie",
        "(Z)V",
        true,
        cookie_manager_set
    ),
    ne!(
        "Landroid/webkit/CookieManager;",
        "setAcceptThirdPartyCookies",
        "(Landroid/webkit/WebView;Z)V",
        true,
        cookie_manager_set
    ),
    // ---- android.text ----
    ne!(
        "Landroid/text/Html;",
        "fromHtml",
        "(Ljava/lang/String;I)Landroid/text/Spanned;",
        false,
        html_from_html
    ),
    ne!(
        "Landroid/text/StaticLayout;",
        "getLineCount",
        "()I",
        true,
        ui_zero
    ),
    ne!("Landroid/text/Layout;", "getHeight", "()I", true, ui_zero),
    ne!(
        "Landroid/text/StaticLayout$Builder;",
        "setBreakStrategy",
        "(I)Landroid/text/StaticLayout$Builder;",
        true,
        static_layout_builder_set
    ),
    ne!(
        "Landroid/text/StaticLayout$Builder;",
        "setHyphenationFrequency",
        "(I)Landroid/text/StaticLayout$Builder;",
        true,
        static_layout_builder_set
    ),
    ne!(
        "Landroid/text/StaticLayout$Builder;",
        "setAlignment",
        "(Landroid/text/Layout$Alignment;)Landroid/text/StaticLayout$Builder;",
        true,
        static_layout_builder_set
    ),
    ne!(
        "Landroid/text/StaticLayout$Builder;",
        "setIncludePad",
        "(Z)Landroid/text/StaticLayout$Builder;",
        true,
        static_layout_builder_set
    ),
    ne!(
        "Landroid/text/SpannableString;",
        "<init>",
        "(Ljava/lang/CharSequence;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/text/method/LinkMovementMethod;",
        "getInstance",
        "()Landroid/text/method/MovementMethod;",
        false,
        link_movement_method_instance
    ),
    ne!(
        "Landroid/text/util/Linkify;",
        "addLinks",
        "(Landroid/text/Spannable;I)Z",
        false,
        ui_false
    ),
    // ---- android.net.Uri ----
    ne!(
        "Landroid/net/Uri;",
        "encode",
        "(Ljava/lang/String;)Ljava/lang/String;",
        false,
        uri_encode
    ),
    ne!(
        "Landroid/net/Uri;",
        "parse",
        "(Ljava/lang/String;)Landroid/net/Uri;",
        false,
        uri_parse
    ),
    ne!(
        "Landroid/net/Uri;",
        "buildUpon",
        "()Landroid/net/Uri$Builder;",
        true,
        uri_build_upon
    ),
    ne!(
        "Landroid/net/Uri;",
        "getHost",
        "()Ljava/lang/String;",
        true,
        uri_get_host
    ),
    ne!(
        "Landroid/net/Uri;",
        "getPath",
        "()Ljava/lang/String;",
        true,
        uri_get_path
    ),
    ne!(
        "Landroid/net/Uri;",
        "getEncodedPath",
        "()Ljava/lang/String;",
        true,
        uri_get_encoded_path
    ),
    ne!(
        "Landroid/net/Uri;",
        "getEncodedQuery",
        "()Ljava/lang/String;",
        true,
        uri_get_encoded_query
    ),
    ne!(
        "Landroid/net/Uri$Builder;",
        "appendQueryParameter",
        "(Ljava/lang/String;Ljava/lang/String;)Landroid/net/Uri$Builder;",
        true,
        uri_builder_append_query
    ),
    ne!(
        "Landroid/net/Uri$Builder;",
        "appendPath",
        "(Ljava/lang/String;)Landroid/net/Uri$Builder;",
        true,
        uri_builder_append_path
    ),
    ne!(
        "Landroid/net/Uri$Builder;",
        "appendEncodedPath",
        "(Ljava/lang/String;)Landroid/net/Uri$Builder;",
        true,
        uri_builder_append_encoded_path
    ),
    ne!(
        "Landroid/net/Uri$Builder;",
        "fragment",
        "(Ljava/lang/String;)Landroid/net/Uri$Builder;",
        true,
        uri_builder_fragment
    ),
    ne!(
        "Landroid/net/Uri$Builder;",
        "build",
        "()Landroid/net/Uri;",
        true,
        uri_builder_build
    ),
    ne!(
        "Landroid/net/Uri$Builder;",
        "toString",
        "()Ljava/lang/String;",
        true,
        uri_builder_to_string
    ),
    // ---- android.content ----
    ne!(
        "Landroid/content/SharedPreferences$Editor;",
        "putStringSet",
        "(Ljava/lang/String;Ljava/util/Set;)Landroid/content/SharedPreferences$Editor;",
        true,
        editor_put_string_set
    ),
    ne!(
        "Landroid/content/ComponentName;",
        "<init>",
        "(Landroid/content/Context;Ljava/lang/String;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/content/ContextWrapper;",
        "getExternalCacheDir",
        "()Ljava/io/File;",
        true,
        context_wrapper_external_cache_dir
    ),
    ne!(
        "Landroid/content/ContextWrapper;",
        "getApplicationInfo",
        "()Landroid/content/pm/ApplicationInfo;",
        true,
        context_wrapper_application_info
    ),
    ne!(
        "Landroid/content/ContextWrapper;",
        "getSystemService",
        "(Ljava/lang/String;)Ljava/lang/Object;",
        true,
        context_wrapper_system_service
    ),
    ne!(
        "Landroid/content/Intent;",
        "addFlags",
        "(I)Landroid/content/Intent;",
        true,
        intent_add_flags
    ),
    ne!(
        "Landroid/content/Intent;",
        "setComponent",
        "(Landroid/content/ComponentName;)Landroid/content/Intent;",
        true,
        intent_set_component
    ),
    // ---- android.util ----
    ne!(
        "Landroid/util/Log;",
        "e",
        "(Ljava/lang/String;Ljava/lang/String;)I",
        false,
        log_error
    ),
    ne!(
        "Landroid/util/Log;",
        "d",
        "(Ljava/lang/String;Ljava/lang/String;)I",
        false,
        log_error
    ),
    ne!(
        "Landroid/util/Log;",
        "d",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/Throwable;)I",
        false,
        log_error
    ),
    ne!(
        "Landroid/util/Log;",
        "wtf",
        "(Ljava/lang/String;Ljava/lang/String;)I",
        false,
        log_error
    ),
    ne!(
        "Landroid/util/Log;",
        "println",
        "(ILjava/lang/String;Ljava/lang/String;)I",
        false,
        log_error
    ),
    ne!(
        "Landroid/util/Base64;",
        "decode",
        "([BI)[B",
        false,
        base64_decode_bytes
    ),
    ne!(
        "Landroid/util/Base64;",
        "encode",
        "([BI)[B",
        false,
        base64_encode_bytes
    ),
    ne!(
        "Landroid/util/Base64InputStream;",
        "<init>",
        "(Ljava/io/InputStream;I)V",
        true,
        base64_input_stream_init
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "<init>",
        "(Ljava/io/Reader;)V",
        true,
        json_reader_init
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "hasNext",
        "()Z",
        true,
        json_reader_has_next
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "nextString",
        "()Ljava/lang/String;",
        true,
        json_reader_null
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "nextName",
        "()Ljava/lang/String;",
        true,
        json_reader_null
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "peek",
        "()Landroid/util/JsonToken;",
        true,
        json_reader_null
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "nextDouble",
        "()D",
        true,
        json_reader_next_double
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "nextNull",
        "()V",
        true,
        json_reader_noop
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "beginArray",
        "()V",
        true,
        json_reader_noop
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "beginObject",
        "()V",
        true,
        json_reader_noop
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "endArray",
        "()V",
        true,
        json_reader_noop
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "endObject",
        "()V",
        true,
        json_reader_noop
    ),
    ne!(
        "Landroid/util/JsonReader;",
        "skipValue",
        "()V",
        true,
        json_reader_noop
    ),
    ne!(
        "Landroid/util/LruCache;",
        "<init>",
        "(I)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/util/LruCache;",
        "get",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        ui_null
    ),
    // ---- android.content.res / android.app ----
    ne!(
        "Landroid/content/res/Resources;",
        "getSystem",
        "()Landroid/content/res/Resources;",
        false,
        resources_system
    ),
    ne!(
        "Landroid/content/res/Resources;",
        "getDisplayMetrics",
        "()Landroid/util/DisplayMetrics;",
        true,
        resources_display_metrics
    ),
    ne!(
        "Landroid/app/ActivityManager$MemoryInfo;",
        "<init>",
        "()V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/app/ActivityManager;",
        "getMemoryInfo",
        "(Landroid/app/ActivityManager$MemoryInfo;)V",
        true,
        ui_noop
    ),
    // ---- android.graphics leftovers ----
    ne!(
        "Landroid/graphics/Typeface;",
        "createFromFile",
        "(Ljava/io/File;)Landroid/graphics/Typeface;",
        false,
        typeface_create_from_file
    ),
    ne!(
        "Landroid/graphics/ImageDecoder;",
        "createSource",
        "([B)Landroid/graphics/ImageDecoder$Source;",
        false,
        image_decoder_create_source
    ),
    ne!(
        "Landroid/graphics/ImageDecoder;",
        "setAllocator",
        "(I)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/graphics/pdf/PdfRenderer;",
        "<init>",
        "(Landroid/os/ParcelFileDescriptor;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/graphics/pdf/PdfRenderer;",
        "getPageCount",
        "()I",
        true,
        ui_zero
    ),
    ne!(
        "Landroid/graphics/pdf/PdfRenderer;",
        "openPage",
        "(I)Landroid/graphics/pdf/PdfRenderer$Page;",
        true,
        pdf_renderer_open_page
    ),
    ne!(
        "Landroid/graphics/pdf/PdfRenderer$Page;",
        "getWidth",
        "()I",
        true,
        ui_zero
    ),
    ne!(
        "Landroid/graphics/pdf/PdfRenderer$Page;",
        "getHeight",
        "()I",
        true,
        ui_zero
    ),
    // ---- android.icu.text ----
    ne!(
        "Landroid/icu/text/BreakIterator;",
        "getCharacterInstance",
        "()Landroid/icu/text/BreakIterator;",
        false,
        break_iterator_instance
    ),
    ne!(
        "Landroid/icu/text/BreakIterator;",
        "getWordInstance",
        "()Landroid/icu/text/BreakIterator;",
        false,
        break_iterator_instance
    ),
    ne!(
        "Landroid/icu/text/BreakIterator;",
        "first",
        "()I",
        true,
        break_iterator_first
    ),
    ne!(
        "Landroid/icu/text/BreakIterator;",
        "next",
        "()I",
        true,
        break_iterator_next
    ),
    ne!(
        "Landroid/icu/text/BreakIterator;",
        "getRuleStatus",
        "()I",
        true,
        ui_zero
    ),
    ne!(
        "Landroid/icu/text/BreakIterator;",
        "setText",
        "(Ljava/text/CharacterIterator;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/icu/text/Collator;",
        "getInstance",
        "()Landroid/icu/text/Collator;",
        false,
        collator_instance
    ),
    ne!(
        "Landroid/icu/text/RuleBasedCollator;",
        "setCaseLevel",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/icu/text/RuleBasedCollator;",
        "setDecomposition",
        "(I)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/icu/text/RuleBasedCollator;",
        "setStrength",
        "(I)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/icu/text/Normalizer2;",
        "getNFKCCasefoldInstance",
        "()Landroid/icu/text/Normalizer2;",
        false,
        normalizer2_instance
    ),
    ne!(
        "Landroid/icu/text/Normalizer2;",
        "normalize",
        "(Ljava/lang/CharSequence;)Ljava/lang/String;",
        true,
        normalizer2_normalize
    ),
    ne!(
        "Landroid/icu/text/SearchIterator;",
        "first",
        "()I",
        true,
        search_iterator_done
    ),
    ne!(
        "Landroid/icu/text/SearchIterator;",
        "next",
        "()I",
        true,
        search_iterator_done
    ),
    ne!(
        "Landroid/icu/text/SearchIterator;",
        "getMatchedText",
        "()Ljava/lang/String;",
        true,
        ui_null
    ),
    ne!(
        "Landroid/icu/text/SearchIterator;",
        "setOverlapping",
        "(Z)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/icu/text/StringSearch;",
        "<init>",
        "(Ljava/lang/String;Ljava/text/CharacterIterator;Landroid/icu/text/RuleBasedCollator;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/icu/text/StringSearch;",
        "setPattern",
        "(Ljava/lang/String;)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/icu/text/StringSearch;",
        "setTarget",
        "(Ljava/text/CharacterIterator;)V",
        true,
        ui_noop
    ),
    // ---- android.text java.lang.CharSequence variants ----
    ne!(
        "Landroid/text/StaticLayout;",
        "<init>",
        "(Ljava/lang/CharSequence;Landroid/text/TextPaint;ILandroid/text/Layout$Alignment;FFZ)V",
        true,
        ui_noop
    ),
    ne!(
        "Landroid/text/StaticLayout$Builder;",
        "obtain",
        "(Ljava/lang/CharSequence;IILandroid/text/TextPaint;I)Landroid/text/StaticLayout$Builder;",
        false,
        static_layout_builder_obtain_cs
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
