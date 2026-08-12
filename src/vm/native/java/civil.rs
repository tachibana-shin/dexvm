//! Shared proleptic-Gregorian civil-date arithmetic (Howard Hinnant's
//! algorithm), used by the java.time shims to break epoch millis into
//! year/month/day/... components and back, entirely in UTC (no DST).

pub(crate) const DAY_MS: i64 = 86_400_000;

pub(crate) fn days_from_civil(mut year: i32, month: i32, day: i32) -> i64 {
    year -= i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * shifted_month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i64::from(era * 146_097 + doe - 719_468)
}

pub(crate) fn civil_from_days(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as i32, day as i32)
}

/// (year, month 1-12, day, hour, minute, second, milli) for a UTC epoch-millis instant.
pub(crate) fn components(millis: i64) -> (i32, i32, i32, i32, i32, i32, i32) {
    let days = millis.div_euclid(DAY_MS);
    let day_ms = millis.rem_euclid(DAY_MS);
    let (year, month, day) = civil_from_days(days);
    let hour = (day_ms / 3_600_000) as i32;
    let minute = (day_ms % 3_600_000 / 60_000) as i32;
    let second = (day_ms % 60_000 / 1_000) as i32;
    let milli = (day_ms % 1_000) as i32;
    (year, month, day, hour, minute, second, milli)
}

/// Inverse of `components`: `month0` is a zero-based, unbounded month offset
/// from January of `year` (so passing -1 rolls back into December of the
/// previous year) — this is what lets month/year arithmetic just add deltas
/// and let this function normalize the carry.
pub(crate) fn compose(
    year: i32,
    month0: i32,
    day: i32,
    hour: i32,
    minute: i32,
    second: i32,
    milli: i32,
) -> i64 {
    let year = year + month0.div_euclid(12);
    let month = month0.rem_euclid(12) + 1;
    days_from_civil(year, month, day) * DAY_MS
        + i64::from(hour) * 3_600_000
        + i64::from(minute) * 60_000
        + i64::from(second) * 1_000
        + i64::from(milli)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrip_handles_leap_days_and_pre_epoch_dates() {
        for date in [(1970, 1, 1), (2024, 2, 29), (1969, 12, 31), (2000, 1, 1)] {
            assert_eq!(
                civil_from_days(days_from_civil(date.0, date.1, date.2)),
                date
            );
        }
    }
}
