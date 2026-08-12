//! Kotlin native bridge modules.
//!
//! This file intentionally contains declarations only.  Implementations and
//! the aggregate Kotlin registration table live in [`text`], while frequently
//! used API families have their own registration modules.

mod text;

pub(crate) use text::*;
