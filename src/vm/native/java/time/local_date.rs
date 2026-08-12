//! java.time.LocalDate host shims. Dates are dd/MM/yyyy, carrying days
//! since epoch.

use super::super::civil::{civil_from_days, days_from_civil, DAY_MS};
use crate::vm::native::*;

/// LocalDate.parse(str, dtf) -> LocalDate carrying days-since-epoch.
pub(crate) fn localdate_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let text = jstr(vm, args[0])?;
    let days = parse_ddmmyyyy(&text)
        .map_err(|_| NatErr::Throw(vm.err_iae(format!("unparseable date: {text}"))))?;
    alloc(vm, "Ljava/time/LocalDate;", Native::LocalDay(days))
}

pub(crate) fn localdate_at_start_of_day(vm: &mut Vm, args: &[JValue]) -> R {
    let days = match payload(vm, args[0]) {
        Some(Native::LocalDay(d)) => *d,
        _ => return Err(npe(vm)),
    };
    // fixed +07:00 offset for the HCM zone the extensions use
    let millis = i64::from(days) * 86_400_000 - 7 * 3_600_000;
    alloc(vm, "Ljava/time/ZonedDateTime;", Native::EpochMillis(millis))
}

pub(crate) fn localdate_now(vm: &mut Vm, _args: &[JValue]) -> R {
    let days = now_millis().div_euclid(DAY_MS) as u32;
    alloc(vm, "Ljava/time/LocalDate;", Native::LocalDay(days))
}

pub(crate) fn localdate_of(vm: &mut Vm, args: &[JValue]) -> R {
    let y = int_of(vm, args[0]);
    let m = int_of(vm, args[1]);
    let d = int_of(vm, args[2]);
    let days = days_from_civil(y, m, d) as u32;
    alloc(vm, "Ljava/time/LocalDate;", Native::LocalDay(days))
}

pub(crate) fn localdate_parse_iso(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let parts: Vec<&str> = text.trim().split('-').collect();
    let (Some(y), Some(m), Some(d)) = (
        parts.first().and_then(|p| p.parse::<i32>().ok()),
        parts.get(1).and_then(|p| p.parse::<i32>().ok()),
        parts.get(2).and_then(|p| p.parse::<i32>().ok()),
    ) else {
        return Err(NatErr::Throw(
            vm.err_iae(format!("unparseable date: {text}")),
        ));
    };
    let days = days_from_civil(y, m, d) as u32;
    alloc(vm, "Ljava/time/LocalDate;", Native::LocalDay(days))
}

pub(crate) fn localdate_minus_days(vm: &mut Vm, args: &[JValue]) -> R {
    let days = match payload(vm, args[0]) {
        Some(Native::LocalDay(d)) => *d,
        _ => return Err(npe(vm)),
    };
    let amount = long_of(vm, args[1]);
    let new_days = (i64::from(days) - amount).max(0) as u32;
    alloc(vm, "Ljava/time/LocalDate;", Native::LocalDay(new_days))
}

pub(crate) fn localdate_get_year(vm: &mut Vm, args: &[JValue]) -> R {
    let days = match payload(vm, args[0]) {
        Some(Native::LocalDay(d)) => *d,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(civil_from_days(i64::from(days)).0))
}

pub(crate) fn localdate_at_start_of_day_noarg(vm: &mut Vm, args: &[JValue]) -> R {
    let days = match payload(vm, args[0]) {
        Some(Native::LocalDay(d)) => *d,
        _ => return Err(npe(vm)),
    };
    let millis = i64::from(days) * DAY_MS;
    alloc(vm, "Ljava/time/LocalDateTime;", Native::EpochMillis(millis))
}

/// days since 1970-01-01 for a dd/MM/yyyy string.
fn parse_ddmmyyyy(s: &str) -> Result<u32, ()> {
    let parts: Vec<&str> = s.trim().split('/').collect();
    if parts.len() != 3 {
        return Err(());
    }
    let (d, m, y): (u32, u32, i64) = (
        parts[0].parse().map_err(|_| ())?,
        parts[1].parse().map_err(|_| ())?,
        parts[2].parse().map_err(|_| ())?,
    );
    if !(1..=31).contains(&d) || !(1..=12).contains(&m) {
        return Err(());
    }
    let (y0, m0) = if m < 3 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = if y0 >= 0 { y0 / 400 } else { (y0 - 399) / 400 };
    let yoe = y0 - era * 400;
    let doy = i64::from(153 * m0 + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok((era * 146_097 + doe) as u32)
}

/// Native methods for Ljava/time/LocalDate;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/time/LocalDate;",
        "parse",
        "(Ljava/lang/CharSequence;Ljava/time/format/DateTimeFormatter;)Ljava/time/LocalDate;",
        false,
        localdate_parse
    ),
    ne!(
        "Ljava/time/LocalDate;",
        "atStartOfDay",
        "(Ljava/time/ZoneId;)Ljava/time/ZonedDateTime;",
        true,
        localdate_at_start_of_day
    ),
    ne!(
        "Ljava/time/LocalDate;",
        "atStartOfDay",
        "()Ljava/time/LocalDateTime;",
        true,
        localdate_at_start_of_day_noarg
    ),
    ne!(
        "Ljava/time/LocalDate;",
        "now",
        "()Ljava/time/LocalDate;",
        false,
        localdate_now
    ),
    ne!(
        "Ljava/time/LocalDate;",
        "now",
        "(Ljava/time/ZoneId;)Ljava/time/LocalDate;",
        false,
        localdate_now
    ),
    ne!(
        "Ljava/time/LocalDate;",
        "of",
        "(III)Ljava/time/LocalDate;",
        false,
        localdate_of
    ),
    ne!(
        "Ljava/time/LocalDate;",
        "parse",
        "(Ljava/lang/CharSequence;)Ljava/time/LocalDate;",
        false,
        localdate_parse_iso
    ),
    ne!(
        "Ljava/time/LocalDate;",
        "minusDays",
        "(J)Ljava/time/LocalDate;",
        true,
        localdate_minus_days
    ),
    ne!(
        "Ljava/time/LocalDate;",
        "getYear",
        "()I",
        true,
        localdate_get_year
    ),
];
