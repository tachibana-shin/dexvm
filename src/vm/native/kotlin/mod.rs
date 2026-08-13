//! Kotlin native bridge modules.
//!
//! This file intentionally contains module declarations and table exports only.

mod collections;
mod intrinsics;
mod io;
mod jvm;
mod lazy;
mod ranges;
mod result;
mod sequences;
mod statics;
mod support;
mod text;
mod time;
mod tuples;
mod unsigned;

pub(crate) use collections::TABLE as COLLECTIONS_TABLE;
pub(crate) use intrinsics::TABLE as INTRINSICS_TABLE;
pub(crate) use io::TABLE as IO_TABLE;
pub(crate) use jvm::TABLE as JVM_TABLE;
pub(crate) use lazy::{
    lazy_lazy_mode_none, lazy_lazy_mode_publication, lazy_lazy_mode_synchronized,
    TABLE as LAZY_TABLE,
};
pub(crate) use ranges::TABLE as RANGES_TABLE;
pub(crate) use result::TABLE as RESULT_TABLE;
pub(crate) use sequences::TABLE as SEQUENCES_TABLE;
pub(crate) use statics::{
    duration_companion as lazy_duration_companion, duration_unit_days as lazy_duration_unit_days,
    duration_unit_millis as lazy_duration_unit_millis,
    duration_unit_seconds as lazy_duration_unit_seconds, global_scope as lazy_global_scope,
    result_companion as lazy_result_companion, unit_instance as lazy_unit_instance,
};
pub(crate) use support::opaque_inst;
pub(crate) use support::TABLE as SUPPORT_TABLE;
pub(crate) use text::*;
pub(crate) use time::TABLE as TIME_TABLE;
pub(crate) use tuples::TABLE as TUPLES_TABLE;
pub(crate) use unsigned::TABLE as UNSIGNED_TABLE;
