//! java.time host shims.

use crate::vm::native::*;

mod chrono_unit;
mod date_time_formatter;
mod date_time_formatter_builder;
mod instant;
mod local_date;
mod local_date_time;
mod zone_id;
mod zoned_date_time;

pub(crate) use chrono_unit::{
    days as lazy_chrono_unit_days, hours as lazy_chrono_unit_hours,
    millis as lazy_chrono_unit_millis, minutes as lazy_chrono_unit_minutes,
    months as lazy_chrono_unit_months, seconds as lazy_chrono_unit_seconds,
    weeks as lazy_chrono_unit_weeks, years as lazy_chrono_unit_years,
};

/// All java.time native tables, grouped for `register`.
pub(crate) const TIME_TABLE: &[&[NativeEntry]] = &[
    date_time_formatter::TABLE,
    date_time_formatter_builder::TABLE,
    zone_id::TABLE,
    local_date::TABLE,
    local_date_time::TABLE,
    zoned_date_time::TABLE,
    instant::TABLE,
];
