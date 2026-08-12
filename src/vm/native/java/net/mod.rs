//! java.net host shims.

use crate::vm::native::*;

mod uri;

/// All java.net native tables, grouped for `register`.
pub(crate) const NET_TABLE: &[&[NativeEntry]] = &[uri::TABLE];
