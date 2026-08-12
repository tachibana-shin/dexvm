//! java.nio host shims.

use crate::vm::native::*;

mod byte_order;
mod bytebuffer;
mod charset;

pub(crate) use byte_order::{lazy_big_endian, lazy_little_endian};
pub(crate) use charset::*;

/// All java.nio native tables, grouped for `register`.
pub(crate) const NIO_TABLE: &[&[NativeEntry]] = &[charset::TABLE, bytebuffer::TABLE];
