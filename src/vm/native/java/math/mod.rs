//! java.math host shims.

mod bigdecimal;
mod biginteger;

/// All java.math native tables, grouped for `register`.
pub(crate) const MATH_TABLE: &[&[crate::vm::native::NativeEntry]] =
    &[biginteger::TABLE, bigdecimal::TABLE];
