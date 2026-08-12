//! java.nio host shims.

use crate::vm::native::*;

mod charset;

pub(crate) use charset::*;

/// All java.nio native tables, grouped for `register`.
pub(crate) const NIO_TABLE: &[&[NativeEntry]] = &[charset::TABLE];
