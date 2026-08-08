//! Embeddable runtime API: a QuickJS/d4rt-style [`Context`] that owns a
//! loaded extension dex (from a raw `.dex` or an `.apk`/`.zip` container)
//! plus its sandbox permissions and host API registrations.
//!
//! ```
//! use dexvm::Context;
//! use dexvm::permission::{NetworkPermission, Permission};
//!
//! let apk = std::fs::read("fixtures/tachiyomi-all.akuma-v1.4.10.apk").unwrap();
//! let mut ctx = Context::new(&apk).unwrap();
//! ctx.grant(Permission::Network(NetworkPermission::Any));
//! let _ = ctx.call("Lk", "<init>", &[dexvm::JValue::Int(1)]).unwrap();
//! let lang = ctx.call("Lk", "getLang", &[]).unwrap();
//! assert!(matches!(lang, dexvm::JValue::Obj(_))); // a String object in the vm arena
//! ```
//!
//! By default a context denies every host capability ([`SandboxOptions`]
//! is deny-first, like `d4rt_rs::SandboxOptions` and Deno's `--allow-*`).

use std::io::Read;

use crate::dex::DexFile;
use crate::permission::{FilesystemPermission, NetworkPermission, Permission, ProcessPermission};
use crate::vm::error::JvmError;
use crate::vm::{interpret, NativeEntry};
use crate::vm::value::JValue;
use crate::Vm;

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
}

/// Extract the first `classes*.dex` entry from a zip/apk container.
fn dex_from_apk(data: &[u8]) -> Result<Vec<u8>, ContextError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))
        .map_err(|e| ContextError::BadArchive(e.to_string()))?;
    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| ContextError::BadArchive(e.to_string()))?;
        if f.name().starts_with("classes") && f.name().ends_with(".dex") {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)
                .map_err(|e| ContextError::BadArchive(format!("read {}: {e}", f.name())))?;
            return Ok(buf);
        }
    }
    Err(ContextError::BadArchive("no classes*.dex entry".into()))
}

impl Context {
    /// Loads a dex from raw bytes: either a plain `.dex` file or an
    /// `.apk`/`.zip` container (the first `classes*.dex` entry is used).
    /// All capabilities are denied by default.
    pub fn new(data: &[u8]) -> Result<Context, ContextError> {
        Context::new_with(data, SandboxOptions::default())
    }

    /// Loads a dex with the given sandbox grants pre-applied.
    pub fn new_with(data: &[u8], options: SandboxOptions) -> Result<Context, ContextError> {
        let dex_data = if data.starts_with(b"PK\x03\x04") || data.starts_with(b"PK\x05\x06") {
            dex_from_apk(data)?
        } else {
            data.to_vec()
        };
        let dex = DexFile::parse(&dex_data).map_err(|e| ContextError::Dex(e.to_string()))?;
        let mut vm = Vm::new(dex, Box::new(std::io::sink()))?;
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
        Ok(Context { vm, last_instance: None })
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

    /// Withdraws exactly this permission. A broader grant (`Any`) keeps
    /// covering the capability afterwards.
    pub fn revoke(&mut self, p: &Permission) {
        self.vm.perms.revoke(p);
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

    /// Calls a method (by dex name) on `class` with the given arguments.
    /// Instance methods get a fresh instance unless one was already created
    /// by a previous call (see [`Context::invoke`]).
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
        let mut call_args = Vec::with_capacity(args.len() + 1);
        if !is_static {
            let obj = match self.last_instance {
                Some(o) if self.vm.classes[self.vm.arena.objects[o as usize].class as usize].descriptor
                    == self.vm.intern(&class) => o,
                _ => self.vm.alloc_instance(cid)?,
            };
            call_args.push(JValue::Obj(obj));
            self.last_instance = Some(obj);
        }
        call_args.extend_from_slice(args);
        let v = interpret::run(&mut self.vm, cid, slot as u32, call_args)?;
        if let JValue::Obj(o) = v {
            if self.vm.arena.objects[o as usize].class != self.vm.hot.string {
                self.last_instance = Some(o);
            }
        }
        Ok(v)
    }

    /// Dispatches `method` on the object returned by the most recent
    /// [`Context::call`], mirroring `d4rt_rs::Context::invoke`.
    pub fn invoke(&mut self, method: &str, args: &[JValue]) -> Result<JValue, JvmError> {
        let obj = self.last_instance.ok_or_else(|| {
            JvmError::Resolution("no instance from a previous call; call an instance method first".into())
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

    /// Read-only access to the loaded dex file.
    pub fn dex(&self) -> &DexFile {
        &self.vm.dex
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
    use crate::vm::object::Native;
    use crate::vm::NatErr;

    const APK: &str = "fixtures/tachiyomi-all.akuma-v1.4.10.apk";

    fn fixture() -> Vec<u8> {
        std::fs::read(APK).unwrap()
    }

    #[test]
    fn load_apk_and_call() {
        let data = fixture();
        let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
        // Lk's <init> takes an int language code (1 = "es"); getLang maps it
        // back to the code string via a packed switch.
        let v = ctx.call("Lk", "<init>", &[JValue::Int(1)]).unwrap();
        assert_eq!(v, JValue::Null);
        let lang = ctx.call("Lk", "getLang", &[]).unwrap();
        let JValue::Obj(o) = lang else { panic!("not an object") };
        let s = match &ctx.vm().arena.objects[o as usize].native {
            Some(Native::Str(s)) => s.clone(),
            _ => panic!("not a string"),
        };
        assert_eq!(s, "es");
        let mapped = ctx.call("Lk", "a", &[]).unwrap();
        let JValue::Obj(o) = mapped else { panic!("not an object") };
        let s = match &ctx.vm().arena.objects[o as usize].native {
            Some(Native::Str(s)) => s.clone(),
            _ => panic!("not a string"),
        };
        assert_eq!(s, "spanish");
    }

    #[test]
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
        ctx.grant(Permission::Network(NetworkPermission::Connect("api.akuma.moe".into())));
        assert!(ctx.has_permission(&Permission::Network(NetworkPermission::Connect("api.akuma.moe:443".into()))));
        assert!(!ctx.has_permission(&Permission::Network(NetworkPermission::Connect("evil.example:443".into()))));
        ctx.revoke(&Permission::Network(NetworkPermission::Connect("api.akuma.moe:443".into())));
        assert!(!ctx.has_permission(&Permission::Network(NetworkPermission::Connect("api.akuma.moe:443".into()))));
        ctx.grant(Permission::Network(NetworkPermission::Any));
        assert!(ctx.has_permission(&Permission::Network(NetworkPermission::Connect("evil.example:443".into()))));
    }

    #[test]
    fn host_api_with_permission_check() {
        // Host API: a fake network call native that refuses to run unless
        // network access to the host is granted.
        fn fetch(vm: &mut Vm, args: &[JValue]) -> Result<JValue, NatErr> {
            vm.check_permission(&Permission::Network(NetworkPermission::Connect("api.akuma.moe".into())))
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
        ctx.revoke(&Permission::Network(NetworkPermission::Connect("api.akuma.moe".into())));
        assert!(matches!(
            ctx.vm().check_permission(&Permission::Network(NetworkPermission::Connect("api.akuma.moe".into()))),
            Err(_)
        ));
        ctx.grant(Permission::Network(NetworkPermission::Connect("api.akuma.moe".into())));
        assert!(ctx.vm().check_permission(&Permission::Network(NetworkPermission::Connect("api.akuma.moe".into()))).is_ok());
    }

    #[test]
    fn bad_archive() {
        let err = match Context::new(b"not a dex or zip at all") {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert!(matches!(err, ContextError::Dex(_) | ContextError::BadArchive(_)));
        assert!(!err.to_string().is_empty());
    }
}
