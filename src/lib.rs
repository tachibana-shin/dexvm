//! dexvm: a minimal JVM (DEX) interpreter for running keiyoushi/Tachiyomi
//! extension dex files, embeddable as a library.
//!
//! The main entry point is [`Context`](crate::Context): load a `.dex` or
//! `.apk`, grant sandbox permissions, register host APIs, and call into
//! extension methods — QuickJS/d4rt-style.

pub mod context;
pub mod dex;
pub mod permission;
pub mod vm;

pub use context::{Context, ContextError, SandboxOptions};
pub use vm::value::JValue;
pub use vm::Vm;
