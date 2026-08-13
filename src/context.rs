//! Embeddable runtime API: a QuickJS/d4rt-style [`Context`] that owns a
//! loaded extension dex (from a raw `.dex` or an `.apk`/`.zip` container)
//! plus its sandbox permissions and host API registrations.
//!
//! (This example needs the `keiyoushi` feature for the Lk shim classes, so it
//! is marked `ignore`.)
//!
//! ```ignore
//! use dexvm::Context;
//! use dexvm::permission::{NetworkPermission, Permission};
//!
//! let apk = std::fs::read("fixtures/tachiyomi-all.akuma-v1.4.10.apk").unwrap();
//! let mut ctx = Context::new(&apk).unwrap();
//! ctx.grant(Permission::Network(NetworkPermission::Any));
//! let lang = ctx.call("Lk", "getLang", &[]).unwrap();
//! assert!(matches!(lang, dexvm::JValue::Obj(_))); // a String object in the vm arena
//! ```
//!
//! By default a context denies every host capability ([`SandboxOptions`]
//! is deny-first, like `d4rt_rs::SandboxOptions` and Deno's `--allow-*`).

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use crate::dex::DexFile;
use crate::manifest::{AppManifest, ManifestError, ResourceTable};
use crate::permission::{FilesystemPermission, NetworkPermission, Permission, ProcessPermission};
use crate::vm::error::JvmError;
pub use crate::vm::object::PreferenceValue as SettingValue;
use crate::vm::object::{Native, PreferenceValue};
use crate::vm::value::JValue;
use crate::vm::{interpret, NativeEntry};
use crate::Vm;

/// Host-visible definition of an AndroidX preference declared by an extension.
#[derive(Clone, Debug)]
pub struct SettingDefinition {
    pub key: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub default_value: JValue,
    pub enabled: bool,
    pub visible: bool,
    pub children: Vec<SettingDefinition>,
}

/// Errors produced while constructing a [`Context`] from bytes or a file.
#[derive(Debug)]
pub enum ContextError {
    /// The container is not a valid zip/apk (or no `classes*.dex` entry).
    BadArchive(String),
    /// The dex container failed to parse.
    Dex(String),
    /// The virtual machine failed to boot.
    Jvm(JvmError),
    /// Reading the input file failed.
    Io(std::io::Error),
}

impl std::fmt::Display for ContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContextError::BadArchive(m) => write!(f, "bad archive: {m}"),
            ContextError::Dex(m) => write!(f, "dex error: {m}"),
            ContextError::Jvm(e) => write!(f, "vm error: {e}"),
            ContextError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for ContextError {}

impl From<JvmError> for ContextError {
    fn from(e: JvmError) -> Self {
        ContextError::Jvm(e)
    }
}

/// One-shot sandbox configuration, modeled after `d4rt_rs::SandboxOptions`.
/// `Default` grants nothing.
#[derive(Default, Clone, Debug)]
pub struct SandboxOptions {
    /// Network capability; `None` denies all network access.
    pub network: Option<NetworkPermission>,
    /// Filesystem capability; `None` denies all filesystem access.
    pub filesystem: Option<FilesystemPermission>,
    /// Process capability; `None` denies running any command.
    pub process: Option<ProcessPermission>,
    /// Host environment variable reads.
    pub env: bool,
}

impl SandboxOptions {
    /// Grants every capability (dev-tool behavior).
    pub fn allow_all() -> Self {
        SandboxOptions {
            network: Some(NetworkPermission::Any),
            filesystem: Some(FilesystemPermission::Any),
            process: Some(ProcessPermission::Any),
            env: true,
        }
    }
}

/// An independent DEX runtime: the loaded extension, its sandbox grants and
/// the host API surface. Roughly the equivalent of a QuickJS `Context` or a
/// `d4rt_rs::Context`. Each context owns its own VM state, so nothing leaks
/// between contexts.
pub struct Context {
    vm: Vm,
    /// The instance created by the most recent instance-method call, so
    /// `invoke` can dispatch further methods on it (like
    /// `d4rt_rs::Context::invoke` after `execute`).
    last_instance: Option<u32>,
    settings_update: Option<std::rc::Rc<dyn Fn(&str, &PreferenceValue)>>,
}

type ApkContents = (Vec<Vec<u8>>, HashMap<String, Vec<u8>>);

/// Extract every `classes*.dex` entry from a zip/apk container, in dex order
/// (`classes.dex`, `classes2.dex`, ...), so multidex programs load completely.
fn contents_from_apk(data: &[u8]) -> Result<ApkContents, ContextError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))
        .map_err(|e| ContextError::BadArchive(e.to_string()))?;
    let mut entries = Vec::new();
    let mut resources = HashMap::new();
    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| ContextError::BadArchive(e.to_string()))?;
        let name = f.name().to_string();
        if name.starts_with("classes") && name.ends_with(".dex") {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)
                .map_err(|e| ContextError::BadArchive(format!("read {}: {e}", f.name())))?;
            entries.push((name, buf));
        } else if !f.is_dir() {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)
                .map_err(|e| ContextError::BadArchive(format!("read {}: {e}", f.name())))?;
            resources.insert(name, buf);
        }
    }
    if entries.is_empty() {
        return Err(ContextError::BadArchive("no classes*.dex entry".into()));
    }
    entries.sort_by(|(a, _), (b, _)| {
        let num = |name: &str| -> u32 {
            name.strip_prefix("classes")
                .and_then(|s| s.strip_suffix(".dex"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0)
        };
        num(a).cmp(&num(b))
    });
    Ok((entries.into_iter().map(|(_, b)| b).collect(), resources))
}

impl Context {
    /// Loads a dex from raw bytes: either a plain `.dex` file or an
    /// `.apk`/`.zip` container (every `classes*.dex` entry, i.e. multidex).
    /// All capabilities are denied by default.
    pub fn new(data: &[u8]) -> Result<Context, ContextError> {
        Context::new_with(data, SandboxOptions::default())
    }

    /// Loads a dex with the given sandbox grants pre-applied.
    pub fn new_with(data: &[u8], options: SandboxOptions) -> Result<Context, ContextError> {
        Context::new_with_libraries(data, &[], options)
    }

    /// Loads an application DEX/APK plus additional DEX/APK libraries.
    ///
    /// Application classes have precedence, followed by libraries in the
    /// supplied order. This acts as a boot classpath for pure Java/Kotlin
    /// dependencies such as jsoup or kotlinx.serialization: their real DEX
    /// bytecode can run in the VM while host shims remain responsible only
    /// for platform operations.
    pub fn new_with_libraries(
        data: &[u8],
        libraries: &[&[u8]],
        options: SandboxOptions,
    ) -> Result<Context, ContextError> {
        fn container_contents(data: &[u8]) -> Result<ApkContents, ContextError> {
            if data.starts_with(b"PK\x03\x04") || data.starts_with(b"PK\x05\x06") {
                contents_from_apk(data)
            } else {
                Ok((vec![data.to_vec()], HashMap::new()))
            }
        }

        let (mut dexes_data, mut resources) = container_contents(data)?;
        for library in libraries {
            let (library_dexes, library_resources) = container_contents(library)?;
            dexes_data.extend(library_dexes);
            for (name, bytes) in library_resources {
                resources.entry(name).or_insert(bytes);
            }
        }
        let mut dexes = Vec::with_capacity(dexes_data.len());
        for d in dexes_data {
            dexes.push(DexFile::parse(&d).map_err(|e| ContextError::Dex(e.to_string()))?);
        }
        let mut vm = Vm::new(dexes, Box::new(std::io::sink()))?;
        vm.resources = resources;
        if let Some(p) = options.network {
            vm.perms.grant(Permission::Network(p));
        }
        if let Some(p) = options.filesystem {
            vm.perms.grant(Permission::Filesystem(p));
        }
        if let Some(p) = options.process {
            vm.perms.grant(Permission::Process(p));
        }
        if options.env {
            vm.perms.grant(Permission::Env);
        }
        Ok(Context {
            vm,
            last_instance: None,
            settings_update: None,
        })
    }

    /// Opens a `.dex` or `.apk` file from disk.
    pub fn open(path: &str) -> Result<Context, ContextError> {
        let data = std::fs::read(path).map_err(ContextError::Io)?;
        Context::new(&data)
    }

    /// Grants a capability to this context. Matches `d4rt_rs`'s
    /// `Context::grant`. By default nothing is granted: host APIs that
    /// check permissions throw until explicitly allowed.
    pub fn grant(&mut self, p: Permission) {
        self.vm.perms.grant(p);
    }

    /// Withdraws every grant that covers this permission. This includes a
    /// broader grant such as `Any`, so the revoked capability is actually
    /// denied afterwards.
    pub fn revoke(&mut self, p: &Permission) {
        self.vm.perms.revoke(p);
    }

    /// Returns all AndroidX preference definitions currently materialized by
    /// the extension, including nested preference screens.
    pub fn get_all_setting_definitions(&self) -> Vec<SettingDefinition> {
        fn text(vm: &Vm, v: Option<JValue>) -> Option<String> {
            let JValue::Obj(id) = v? else { return None };
            match vm
                .arena
                .objects
                .get(id as usize)
                .and_then(|o| o.native.as_ref())
            {
                Some(Native::Str(s)) => Some(s.clone()),
                _ => None,
            }
        }
        fn walk(vm: &Vm, v: JValue) -> Option<SettingDefinition> {
            match vm
                .arena
                .objects
                .get(v.as_obj() as usize)
                .and_then(|o| o.native.as_ref())
            {
                Some(Native::Preference {
                    key,
                    title,
                    summary,
                    default_value,
                    enabled,
                    visible,
                    ..
                }) => Some(SettingDefinition {
                    key: text(vm, *key),
                    title: text(vm, *title),
                    summary: text(vm, *summary),
                    default_value: *default_value,
                    enabled: *enabled,
                    visible: *visible,
                    children: Vec::new(),
                }),
                Some(Native::PreferenceScreen { children, title }) => Some(SettingDefinition {
                    key: None,
                    title: text(vm, *title),
                    summary: None,
                    default_value: JValue::Null,
                    enabled: true,
                    visible: true,
                    children: children.iter().filter_map(|c| walk(vm, *c)).collect(),
                }),
                _ => None,
            }
        }
        let mut child_ids = std::collections::HashSet::new();
        for object in &self.vm.arena.objects {
            if let Some(Native::PreferenceScreen { children, .. }) = object.native.as_ref() {
                child_ids.extend(children.iter().filter_map(|v| match v {
                    JValue::Obj(id) => Some(*id),
                    _ => None,
                }));
            }
        }
        self.vm
            .arena
            .objects
            .iter()
            .enumerate()
            .filter(|(id, object)| {
                !child_ids.contains(&(*id as u32))
                    && matches!(
                        object.native,
                        Some(Native::PreferenceScreen { .. }) | Some(Native::Preference { .. })
                    )
            })
            .filter_map(|(id, _)| walk(&self.vm, JValue::Obj(id as u32)))
            .collect()
    }

    /// Resolves an arena object id to its string payload, if the object is a
    /// java String (used e.g. to read a preference's `default_value`).
    pub fn string_of(&mut self, id: u32) -> Option<String> {
        match self
            .vm()
            .arena
            .objects
            .get(id as usize)
            .and_then(|o| o.native.as_ref())
        {
            Some(Native::Str(s)) => Some(s.clone()),
            _ => None,
        }
    }

    /// Parsed `AndroidManifest.xml` metadata: package id, app name, icon
    /// resource id and sdk levels of the loaded APK. Fails with
    /// [`ManifestError::Missing`] for plain dex input.
    pub fn manifest(&mut self) -> Result<AppManifest, ManifestError> {
        let bytes = self
            .vm()
            .resources
            .get("AndroidManifest.xml")
            .cloned()
            .ok_or_else(|| ManifestError::Missing("AndroidManifest.xml".into()))?;
        let table = self
            .vm()
            .resources
            .get("resources.arsc")
            .map(|d| ResourceTable::parse(d))
            .transpose()?;
        crate::manifest::parse_manifest(&bytes, table.as_ref())
    }

    /// Maps a resource id to its APK entry path via `resources.arsc`, e.g.
    /// the icon `0x7f010000` → `res/9w.png` in obfuscated builds.
    pub fn resource_path(&mut self, resource_id: u32) -> Option<String> {
        let table = ResourceTable::parse(self.vm().resources.get("resources.arsc")?).ok()?;
        table.path(resource_id)
    }

    /// Maps a resource id to its string value via `resources.arsc`, e.g. a
    /// `@string/app_name` label.
    pub fn resource_string(&mut self, resource_id: u32) -> Option<String> {
        let table = ResourceTable::parse(self.vm().resources.get("resources.arsc")?).ok()?;
        table.string(resource_id)
    }

    /// The raw bytes of an APK entry (e.g. an icon path from
    /// [`Context::resource_path`]).
    pub fn resource_bytes(&mut self, path: &str) -> Option<Vec<u8>> {
        self.vm().resources.get(path).cloned()
    }

    /// The decoded icon of the loaded APK: resolves the manifest
    /// `android:icon` through the resource table and returns the PNG bytes.
    pub fn icon_bytes(&mut self) -> Option<Vec<u8>> {
        let id = self.manifest().ok()?.icon_resource_id?;
        let path = self.resource_path(id)?;
        self.resource_bytes(&path)
    }

    /// Returns persisted/in-memory values for named settings. Mirrors the
    /// Android lazy load: values on disk are picked up on the first read.
    pub fn get_settings(
        &mut self,
        preference_file: &str,
    ) -> std::collections::HashMap<String, PreferenceValue> {
        let _ = crate::vm::native::android::load_shared_preferences(self.vm());
        self.vm
            .shared_preferences
            .get(preference_file)
            .cloned()
            .unwrap_or_default()
    }

    /// Registers the host callback invoked by [`Context::update_setting`].
    pub fn on_update_settings<F>(&mut self, callback: F)
    where
        F: Fn(&str, &PreferenceValue) + 'static,
    {
        self.settings_update = Some(std::rc::Rc::new(callback));
    }

    /// Updates one setting through the same persistence path as Android
    /// SharedPreferences and notifies the host callback.
    pub fn update_setting(
        &mut self,
        preference_file: &str,
        key: &str,
        value: PreferenceValue,
    ) -> std::io::Result<()> {
        self.vm
            .shared_preferences
            .entry(preference_file.to_string())
            .or_default()
            .insert(key.to_string(), value);
        if let Some(path) = self.vm.shared_preferences_path.clone() {
            crate::vm::native::android::persist_shared_preferences(
                &path,
                &self.vm.shared_preferences,
            )?;
        }
        if let Some(cb) = &self.settings_update {
            if let Some(value) = self
                .vm
                .shared_preferences
                .get(preference_file)
                .and_then(|m| m.get(key))
            {
                cb(key, value);
            }
        }
        Ok(())
    }

    /// Selects a host-owned persistence file for Android `SharedPreferences`.
    ///
    /// The file is loaded lazily on the next `getSharedPreferences` call. This
    /// storage is deliberately separate from guest [`FilesystemPermission`]
    /// grants: setting the path is the host's explicit authorization to use
    /// this one internal file.
    pub fn set_shared_preferences_path(&mut self, path: impl AsRef<Path>) {
        self.vm.shared_preferences_path = Some(path.as_ref().to_path_buf());
        self.vm.shared_preferences.clear();
        self.vm.shared_preferences_loaded = false;
    }

    /// Disables on-disk `SharedPreferences`, retaining the current values in
    /// memory for the remainder of this context's lifetime.
    pub fn disable_shared_preferences_persistence(&mut self) {
        self.vm.shared_preferences_path = None;
        self.vm.shared_preferences_loaded = true;
    }

    /// Does the current grant set cover `p`?
    pub fn has_permission(&self, p: &Permission) -> bool {
        self.vm.perms.has(p)
    }

    /// Registers a host API: a native method for the shim class `e.class`,
    /// callable from dex code. The class is loaded on demand with this
    /// native (plus any statically registered ones). Natives may consult
    /// [`Vm::has_permission`]/[`Vm::check_permission`] through
    /// [`Context::vm`].
    pub fn register_native(&mut self, e: NativeEntry) -> Result<(), JvmError> {
        self.vm.register_native(e)
    }

    /// Convenience for [`Context::register_native`] over a whole table.
    pub fn register_natives(&mut self, tables: &[&'static [NativeEntry]]) -> Result<(), JvmError> {
        for t in tables {
            for e in t.iter() {
                self.register_native(*e)?;
            }
        }
        Ok(())
    }

    /// Collects the heap: reclaims every object unreachable from the roots
    /// (interned/runtime strings, class statics, monitor keys, and the
    /// instance retained by the last call). Object handles never move; the
    /// reclaimed slots are reused by later allocations. Returns the number
    /// of objects freed. Also called automatically before each top-level
    /// [`Context::call`]/[`Context::invoke`].
    pub fn gc(&mut self) -> usize {
        let last = self.last_instance.unwrap_or(u32::MAX);
        self.vm.gc(&[last])
    }

    /// Calls a method (by dex name) on `class` with the given arguments.
    /// Instance methods get a fresh instance unless one was already created
    /// by a previous call (see [`Context::invoke`]).
    ///
    /// This is a *top-level* call: when the VM is idle the heap is collected
    /// first (see [`Context::gc`]), so objects created by earlier calls are
    /// reclaimed unless reachable from class statics or the last retained
    /// instance. JValues returned by a call must not be kept across later
    /// top-level calls unless they were stored where the collector can see
    /// them (a static field, or returned as part of [`Context::invoke`]).
    pub fn call(&mut self, class: &str, method: &str, args: &[JValue]) -> Result<JValue, JvmError> {
        let class = if class.ends_with(';') {
            class.to_string()
        } else {
            format!("{class};")
        };
        let cid = self.vm.ensure_class_by_desc(&class)?;
        let slot = self.vm.classes[cid as usize]
            .methods
            .iter()
            .position(|m| self.vm.str_of(m.name) == method)
            .ok_or_else(|| JvmError::Resolution(format!("no method {method} in {class}")))?;
        let is_static = self.vm.classes[cid as usize].methods[slot].static_method;
        if self.vm.frames.is_empty() {
            self.gc();
        }
        let mut call_args = Vec::with_capacity(args.len() + 1);
        if !is_static {
            let obj = match self.last_instance {
                Some(o)
                    if self.vm.classes[self.vm.arena.objects[o as usize].class as usize]
                        .descriptor
                        == self.vm.intern(&class) =>
                {
                    o
                }
                _ => self.vm.alloc_instance(cid)?,
            };
            call_args.push(JValue::Obj(obj));
            self.last_instance = Some(obj);
        }
        call_args.extend_from_slice(args);
        let v = interpret::run(&mut self.vm, cid, slot as u32, call_args)?;
        Ok(v)
    }

    /// Dispatches `method` on the object returned by the most recent
    /// [`Context::call`], mirroring `d4rt_rs::Context::invoke`.
    pub fn invoke(&mut self, method: &str, args: &[JValue]) -> Result<JValue, JvmError> {
        let obj = self.last_instance.ok_or_else(|| {
            JvmError::Resolution(
                "no instance from a previous call; call an instance method first".into(),
            )
        })?;
        let cid = self.vm.arena.objects[obj as usize].class;
        let slot = self.vm.classes[cid as usize]
            .methods
            .iter()
            .position(|m| self.vm.str_of(m.name) == method)
            .ok_or_else(|| JvmError::Resolution(format!("no method {method}")))?;
        let mut call_args = vec![JValue::Obj(obj)];
        call_args.extend_from_slice(args);
        let v = interpret::run(&mut self.vm, cid, slot as u32, call_args)?;
        if let JValue::Obj(o) = v {
            if self.vm.arena.objects[o as usize].class != self.vm.hot.string {
                self.last_instance = Some(o);
            }
        }
        Ok(v)
    }

    /// Read-only access to the primary loaded dex file.
    pub fn dex(&self) -> &DexFile {
        &self.vm.dexes[0]
    }

    /// The object retained by the most recent instance call ([`Context::call`]
    /// or [`Context::invoke`]), if any.
    pub fn last_instance(&self) -> Option<u32> {
        self.last_instance
    }

    /// Dispatches `method` (by dex name and signature) on a specific object,
    /// resolving through the receiver's vtable including inherited natives.
    ///
    /// Unlike [`Context::call`]/[`Context::invoke`] this does *not* run a GC
    /// first: intermediate objects handed to later calls inside one bridge
    /// transaction stay alive. The receiver becomes the `last_instance`, so
    /// a following [`Context::invoke`] keeps working on it.
    pub fn invoke_on(
        &mut self,
        obj: u32,
        name: &str,
        sig: &str,
        args: &[JValue],
    ) -> Result<JValue, JvmError> {
        use crate::dex::insn::InvokeKind;
        use crate::vm::MethodRef;
        let mref = MethodRef {
            name: self.vm.intern(name),
            sig: self.vm.intern(sig),
            ret: 0,
            args: Vec::new(),
            class_desc: 0,
        };
        let recv = JValue::Obj(obj);
        let target = self
            .vm
            .resolve_target(InvokeKind::Virtual, &mref, Some(obj), 0)
            .map_err(|e| JvmError::Resolution(format!("invoke_on {name}{sig}: {e}")))?;
        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(recv);
        call_args.extend_from_slice(args);
        let v = self.vm.call_target(target, call_args)?;
        self.last_instance = Some(obj);
        Ok(v)
    }

    /// Registers the HTTP client the keiyoushi bridge executes requests
    /// through (`RequestsKt.__host_execute`). Without one, request
    /// execution throws an IllegalStateException. Executing a request also
    /// requires a matching [`NetworkPermission::Connect`] grant.
    #[cfg(feature = "tachiyomi")]
    pub fn set_http<F>(&mut self, f: F)
    where
        F: Fn(&crate::vm::native::keiyoushi::HttpData) -> crate::vm::native::keiyoushi::HttpResp
            + 'static,
    {
        self.vm.http = Some(std::rc::Rc::new(f));
    }

    /// Registers a host-owned per-host header resolver, e.g. one backed by
    /// a global cookie/User-Agent store:
    ///
    /// ```
    /// use dexvm::Context;
    /// # fn register(mut ctx: Context) {
    /// ctx.set_host_headers(|host| {
    ///     let cookie = format!("session=abc"); // store.get_cookies_for_domain(host)
    ///     (Some("dexvm/0.1".into()), Some(cookie))
    /// });
    /// # }
    /// ```
    ///
    /// The callback receives the lowercase host of the request URL (the
    /// same value `reqwest::Url::host_str()` yields) and returns an optional
    /// User-Agent and Cookie header value. Headers are injected into the
    /// outgoing request only when it does not already set them itself.
    #[cfg(feature = "tachiyomi")]
    pub fn set_host_headers<F>(&mut self, f: F)
    where
        F: Fn(&str) -> (Option<String>, Option<String>) + 'static,
    {
        self.vm.host_headers = Some(std::rc::Rc::new(f));
    }

    /// Access to the underlying VM for advanced use (registering natives
    /// that need to call back into dex code, checking permissions, etc.).
    pub fn vm(&mut self) -> &mut Vm {
        &mut self.vm
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::error::JvmError;
    use crate::vm::object::Native;
    use crate::vm::value::JValue;
    use crate::vm::NatErr;

    const APK: &str = "fixtures/tachiyomi-all.akuma-v1.4.10.apk";

    fn fixture() -> Vec<u8> {
        std::fs::read(APK).unwrap()
    }

    #[test]
    #[cfg(all(feature = "tachiyomi", feature = "okhttp"))]
    fn load_apk_and_call() {
        let data = fixture();
        let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
        // Lk's <init> takes an int language code (1 = "es"); getLang maps it
        // back to the code string via a packed switch.
        let v = ctx.call("Lk", "<init>", &[JValue::Int(1)]).unwrap();
        assert_eq!(v, JValue::Null);
        let lang = ctx.call("Lk", "getLang", &[]).unwrap();
        let JValue::Obj(o) = lang else {
            panic!("not an object")
        };
        let s = match &ctx.vm().arena.objects[o as usize].native {
            Some(Native::Str(s)) => s.clone(),
            _ => panic!("not a string"),
        };
        assert_eq!(s, "es");
        let mapped = ctx.call("Lk", "a", &[]).unwrap();
        let JValue::Obj(o) = mapped else {
            panic!("not an object")
        };
        let s = match &ctx.vm().arena.objects[o as usize].native {
            Some(Native::Str(s)) => s.clone(),
            _ => panic!("not a string"),
        };
        assert_eq!(s, "spanish");
    }

    #[test]
    fn multidex_cross_dex_calls() {
        // classes.dex: m2.Base (superclass) + m2.Main; classes2.dex:
        // m2.Helper which extends Base. Exercises cross-dex class loading,
        // static/instance calls, static fields and superclass resolution.
        let data = std::fs::read("fixtures/multidex.apk").unwrap();
        let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
        let v = ctx.call("Lm2/Main;", "run", &[]).unwrap();
        assert_eq!(v, JValue::Int(36));
        // the primary dex view is still the first file
        assert!(ctx.dex().strings.iter().any(|s| s.as_ref() == "Lm2/Base;"));
        // Helper must resolve from the secondary dex and inherit Base
        let v = ctx
            .call("Lm2/Helper;", "add", &[JValue::Int(5), JValue::Int(6)])
            .unwrap();
        assert_eq!(v, JValue::Int(11));
        let v = ctx.call("Lm2/Helper;", "VERSION", &[]).unwrap_err();
        assert!(matches!(v, JvmError::Resolution(_)));
    }

    #[test]
    fn additional_apk_acts_as_boot_classpath() {
        fn must_not_override_bytecode(_vm: &mut Vm, _args: &[JValue]) -> Result<JValue, NatErr> {
            Ok(JValue::Int(999))
        }
        fn fill_missing_method(_vm: &mut Vm, _args: &[JValue]) -> Result<JValue, NatErr> {
            Ok(JValue::Int(123))
        }
        static NATIVE_FALLBACKS: &[NativeEntry] = &[
            NativeEntry {
                class: "Lm2/Helper;",
                name: "add",
                sig: "(II)I",
                instance: false,
                f: must_not_override_bytecode,
            },
            NativeEntry {
                class: "Lm2/Helper;",
                name: "hostFallback",
                sig: "()I",
                instance: false,
                f: fill_missing_method,
            },
        ];
        crate::vm::native::register_global(NATIVE_FALLBACKS);

        let app = std::fs::read("fixtures/classes.dex").unwrap();
        let library = std::fs::read("fixtures/multidex.apk").unwrap();
        let mut ctx =
            Context::new_with_libraries(&app, &[library.as_slice()], SandboxOptions::allow_all())
                .unwrap();

        let value = ctx
            .call("Lm2/Helper;", "add", &[JValue::Int(8), JValue::Int(9)])
            .unwrap();
        assert_eq!(value, JValue::Int(17));
        assert_eq!(
            ctx.call("Lm2/Helper;", "hostFallback", &[]).unwrap(),
            JValue::Int(123)
        );
        let vm = ctx.vm();
        let helper = vm.ensure_class_by_desc("Lm2/Helper;").unwrap();
        let method = vm.classes[helper as usize]
            .methods
            .iter()
            .find(|method| vm.str_of(method.name) == "add")
            .unwrap();
        assert!(method.native_key.is_none());
        assert!(method.code.is_some());
        // The application's primary DEX remains the public primary view.
        assert!(ctx.dex().strings.iter().any(|s| s.as_ref() == "Lk;"));
    }

    #[test]
    #[cfg(feature = "tachiyomi")]
    fn load_raw_dex() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
        let v = ctx.call("Lk", "getBaseUrl", &[]).unwrap();
        assert!(matches!(v, JValue::Obj(_)));
    }

    #[test]
    fn deny_by_default() {
        let data = fixture();
        let ctx = Context::new(&data).unwrap();
        assert!(!ctx.has_permission(&Permission::Network(NetworkPermission::Any)));
        assert!(!ctx.has_permission(&Permission::Env));
    }

    #[test]
    fn grant_revoke_has() {
        let data = fixture();
        let mut ctx = Context::new(&data).unwrap();
        ctx.grant(Permission::Network(NetworkPermission::Connect(
            "api.akuma.moe".into(),
        )));
        assert!(
            ctx.has_permission(&Permission::Network(NetworkPermission::Connect(
                "api.akuma.moe:443".into()
            )))
        );
        assert!(
            !ctx.has_permission(&Permission::Network(NetworkPermission::Connect(
                "evil.example:443".into()
            )))
        );
        ctx.revoke(&Permission::Network(NetworkPermission::Connect(
            "api.akuma.moe:443".into(),
        )));
        assert!(
            !ctx.has_permission(&Permission::Network(NetworkPermission::Connect(
                "api.akuma.moe:443".into()
            )))
        );
        ctx.grant(Permission::Network(NetworkPermission::Any));
        assert!(
            ctx.has_permission(&Permission::Network(NetworkPermission::Connect(
                "evil.example:443".into()
            )))
        );
    }

    #[test]
    fn host_api_with_permission_check() {
        // Host API: a fake network call native that refuses to run unless
        // network access to the host is granted.
        fn fetch(vm: &mut Vm, args: &[JValue]) -> Result<JValue, NatErr> {
            vm.check_permission(&Permission::Network(NetworkPermission::Connect(
                "api.akuma.moe".into(),
            )))
            .map_err(NatErr::Fatal)?;
            let s = match &vm.arena.objects[args[1].as_obj() as usize].native {
                Some(crate::vm::object::Native::Str(s)) => s.clone(),
                _ => String::new(),
            };
            Ok(vm.alloc_string(&format!("fake:body:{s}")))
        }
        let entry = NativeEntry {
            class: "Lcom/example/host/Api;",
            name: "fetch",
            sig: "(Ljava/lang/String;)Ljava/lang/String;",
            instance: false,
            f: fetch,
        };
        let data = fixture();
        let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
        ctx.register_native(entry).unwrap();
        // denv: deny network -> the native must fail
        ctx.revoke(&Permission::Network(NetworkPermission::Any));
        ctx.revoke(&Permission::Network(NetworkPermission::Connect(
            "api.akuma.moe".into(),
        )));
        assert!(ctx
            .vm()
            .check_permission(&Permission::Network(NetworkPermission::Connect(
                "api.akuma.moe".into()
            )))
            .is_err());
        ctx.grant(Permission::Network(NetworkPermission::Connect(
            "api.akuma.moe".into(),
        )));
        assert!(ctx
            .vm()
            .check_permission(&Permission::Network(NetworkPermission::Connect(
                "api.akuma.moe".into()
            )))
            .is_ok());
    }

    #[test]
    fn global_natives_are_callable_in_every_context() {
        fn ping(vm: &mut Vm, _args: &[JValue]) -> Result<JValue, NatErr> {
            Ok(vm.alloc_string("pong"))
        }
        static PING: &[NativeEntry] = &[NativeEntry {
            class: "Lcom/example/host/Ping;",
            name: "ping",
            sig: "()Ljava/lang/String;",
            instance: false,
            f: ping,
        }];
        crate::vm::native::register_global(PING);
        let mut ctx = Context::new(&fixture()).unwrap();
        let JValue::Obj(o) = ctx.call("Lcom/example/host/Ping;", "ping", &[]).unwrap() else {
            panic!("ping returned non-object");
        };
        let s = match &ctx.vm().arena.objects[o as usize].native {
            Some(Native::Str(s)) => s.clone(),
            _ => panic!("not a string"),
        };
        assert_eq!(s, "pong");
    }

    #[test]
    fn per_context_natives_do_not_leak() {
        fn local(vm: &mut Vm, _args: &[JValue]) -> Result<JValue, NatErr> {
            Ok(vm.alloc_string("local"))
        }
        static LOCAL: &[NativeEntry] = &[NativeEntry {
            class: "Lcom/example/host/Local;",
            name: "probe",
            sig: "()Ljava/lang/String;",
            instance: false,
            f: local,
        }];
        let mut a = Context::new(&fixture()).unwrap();
        a.register_natives(&[LOCAL]).unwrap();
        let mut b = Context::new(&fixture()).unwrap();
        // a sees the native...
        a.call("Lcom/example/host/Local;", "probe", &[]).unwrap();
        // ...b (no registration) must not see it at all.
        assert!(matches!(
            b.call("Lcom/example/host/Local;", "probe", &[]),
            Err(JvmError::Resolution(_))
        ));
    }

    #[test]
    #[cfg(all(feature = "tachiyomi", feature = "okhttp"))]
    fn native_registered_after_class_loaded_patches_dispatch() {
        fn late(vm: &mut Vm, _args: &[JValue]) -> Result<JValue, NatErr> {
            Ok(vm.alloc_string("late"))
        }
        let entry = NativeEntry {
            class: "Lk;",
            name: "hostProbe",
            sig: "()Ljava/lang/String;",
            instance: false,
            f: late,
        };
        let mut ctx = Context::new(&fixture()).unwrap();
        // Load "Lk" first (it exists in the fixture apk), then patch it.
        ctx.call("Lk", "<init>", &[JValue::Int(1)]).unwrap();
        ctx.register_native(entry).unwrap();
        let JValue::Obj(o) = ctx.call("Lk", "hostProbe", &[]).unwrap() else {
            panic!("not an object");
        };
        let s = match &ctx.vm().arena.objects[o as usize].native {
            Some(Native::Str(s)) => s.clone(),
            _ => panic!("not a string"),
        };
        assert_eq!(s, "late");
    }

    #[test]
    fn host_native_fatal_error_propagates() {
        fn boom(_vm: &mut Vm, _args: &[JValue]) -> Result<JValue, NatErr> {
            Err(NatErr::Fatal(JvmError::Fatal("boom".into())))
        }
        let entry = NativeEntry {
            class: "Lcom/example/host/Boom;",
            name: "fail",
            sig: "()V",
            instance: false,
            f: boom,
        };
        let mut ctx = Context::new(&fixture()).unwrap();
        ctx.register_native(entry).unwrap();
        assert!(matches!(
            ctx.call("Lcom/example/host/Boom;", "fail", &[]),
            Err(JvmError::Fatal(m)) if m == "boom"
        ));
    }

    #[test]
    fn bad_archive() {
        let err = match Context::new(b"not a dex or zip at all") {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert!(matches!(
            err,
            ContextError::Dex(_) | ContextError::BadArchive(_)
        ));
        assert!(!err.to_string().is_empty());
    }
}
