//! java.security host shims.

use crate::vm::native::*;

mod message_digest;
mod secure_random;
mod ssl_context;

#[cfg(test)]
mod tests;

/// All java.security native tables, grouped for `register`.
pub(crate) const SECURITY_TABLE: &[&[NativeEntry]] = &[
    message_digest::TABLE,
    secure_random::TABLE,
    ssl_context::TABLE,
];
