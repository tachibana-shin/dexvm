//! java.net host shims.

use crate::vm::native::*;

mod inet_address;
mod uri;
mod url;

/// All java.net native tables, grouped for `register`.
pub(crate) const NET_TABLE: &[&[NativeEntry]] = &[inet_address::TABLE, uri::TABLE, url::TABLE];
