//! java.time host shims.

use crate::vm::native::*;

mod date_time_formatter;
mod instant;
mod local_date;
mod zone_id;
mod zoned_date_time;

/// All java.time native tables, grouped for `register`.
pub(crate) const TIME_TABLE: &[&[NativeEntry]] = &[
    date_time_formatter::TABLE,
    zone_id::TABLE,
    local_date::TABLE,
    zoned_date_time::TABLE,
    instant::TABLE,
];
