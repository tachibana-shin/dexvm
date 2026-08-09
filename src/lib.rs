//! dexvm: a minimal DEX (Dalvik bytecode) VM in Rust, embeddable as a library.
//!
//! The runtime loads a bare `.dex` file or an `.apk` (dex-in-zip) archive and
//! executes the bytecode directly — no Android runtime, no JVM, no subprocess.
//! Java standard-library entries are provided as host-backed shims, so any
//! dex that does not reach into platform internals can run unmodified.
//!
//! The VM is special-cased (and extensively tested) for **one workload**:
//! **mihon / Tachiyomi manga extensions** (the `keiyoushi` collection). With
//! the default features the extension API (`eu.kanade.tachiyomi.*`) is shimmed
//! and the [`keiyoushi`](crate::keiyoushi) module offers a typed bridge;
//! without them this is still a general-purpose DEX runtime ([`Context`]).
//!
//! # Quick start
//!
//! ```
//! use dexvm::vm::value::JValue;
//! use dexvm::{Context, SandboxOptions};
//!
//! let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/classes.dex")).unwrap();
//! let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
//! ctx.call("Lk", "<init>", &[JValue::Int(1)]).unwrap();
//! let v = ctx.call("Lk", "getLang", &[]).unwrap();
//! assert!(matches!(v, JValue::Obj(_)));
//! ```
//!
//! [`SandboxOptions::allow_all`] disables the sandbox; see [`permission`] for
//! the capability model.
//!
//! # Feature flags
//!
//! Every shim family is a feature, all enabled in `default`:
//!
//! | feature      | provides                                    |
//! |--------------|---------------------------------------------|
//! | `java`       | `java.*` stdlib shims                       |
//! | `kotlin`     | `kotlin.*` stdlib shims                     |
//! | `injekt`     | koan / injekt shims                         |
//! | `jsoup`      | `org.jsoup.*` shims (DOM on `dom_query`)    |
//! | `android`    | `android.*` stubs                           |
//! | `okhttp`     | `okhttp3.*` request shims (never networked) |
//! | `tachiyomi`  | `eu.kanade.tachiyomi.*` mihon source shims  |
//! | `keiyoushi`  | compat alias for the full mihon bundle      |
//!
//! # Custom host libraries
//!
//! Register native tables per context via [`Context::register_native`] /
//! [`Context::register_natives`], or process-wide with
//! [`vm::native::register_global`]. See [`vm::NativeEntry`] and
//! `examples/host_native.rs`.

pub mod context;
pub mod dex;
#[cfg(feature = "tachiyomi")]
pub mod keiyoushi;
pub mod permission;
pub mod vm;

pub use context::{Context, ContextError, SandboxOptions};
pub use vm::value::JValue;
pub use vm::Vm;
