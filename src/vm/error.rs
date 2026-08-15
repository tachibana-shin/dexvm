//! VM errors: fatal (abort run) vs throwable (normal JVM exception).

use std::fmt;

use crate::dex::read::DexError;
use crate::vm::value::JValue;

/// A fatal VM error. Aborts the current execution run.
#[derive(Debug, Clone)]
pub enum JvmError {
    /// A Java exception object (arena id) was thrown and not caught.
    Uncaught(u32),
    /// DEX/class resolution problem.
    Resolution(String),
    /// Instruction decode problem.
    Decode(String),
    /// Instruction budget exhausted (infinite-loop guard).
    BudgetExceeded,
    /// Stack depth limit hit.
    StackOverflow,
    /// System.exit(code)
    Exit(i32),
    /// Everything else.
    Fatal(String),
}

impl fmt::Display for JvmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JvmError::Uncaught(_) => write!(f, "uncaught java exception"),
            JvmError::Resolution(m) => write!(f, "resolution error: {m}"),
            JvmError::Decode(m) => write!(f, "decode error: {m}"),
            JvmError::BudgetExceeded => write!(f, "instruction budget exceeded"),
            JvmError::StackOverflow => write!(f, "stack overflow"),
            JvmError::Exit(c) => write!(f, "System.exit({c})"),
            JvmError::Fatal(m) => write!(f, "fatal: {m}"),
        }
    }
}

impl std::error::Error for JvmError {}

impl From<DexError> for JvmError {
    fn from(e: DexError) -> Self {
        JvmError::Decode(e.to_string())
    }
}

impl From<crate::vm::NatErr> for JvmError {
    fn from(e: crate::vm::NatErr) -> Self {
        match e {
            crate::vm::NatErr::Throw(t) => JvmError::Uncaught(t),
            crate::vm::NatErr::Fatal(j) => j,
        }
    }
}

/// Result of a VM operation: normal result or a thrown Java exception object.
pub type ExecResult<T = JValue> = Result<T, u32>;
