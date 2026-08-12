//! java.time.ZonedDateTime / java.time.chrono.ChronoZonedDateTime /
//! java.time.OffsetDateTime host shims. All three are represented the same
//! way as a UTC `Native::EpochMillis` payload (no real timezone/offset or
//! DST tracking, consistent with the rest of `java.time`), so the same
//! arithmetic helpers back every registered class.

use super::super::civil::{components, compose, DAY_MS};
use crate::vm::native::*;

fn millis_of(vm: &Vm, v: JValue) -> Option<i64> {
    match payload(vm, v) {
        Some(Native::EpochMillis(m)) => Some(*m),
        _ => None,
    }
}

pub(crate) fn zdt_to_instant(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = millis_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    alloc(vm, "Ljava/time/Instant;", Native::EpochMillis(millis))
}

pub(crate) fn zdt_to_epoch_second(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = millis_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(JValue::Long(millis.div_euclid(1000)))
}

pub(crate) fn zdt_now_zoned(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Ljava/time/ZonedDateTime;",
        Native::EpochMillis(now_millis()),
    )
}

pub(crate) fn zdt_now_offset(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Ljava/time/OffsetDateTime;",
        Native::EpochMillis(now_millis()),
    )
}

pub(crate) fn zdt_get_year(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = millis_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    Ok(JValue::Int(components(millis).0))
}

fn delta(vm: &mut Vm, args: &[JValue], unit_ms: i64) -> R {
    let millis = millis_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let amount = long_of(vm, args[1]);
    let class = class_desc_of(vm, args[0]);
    alloc(vm, &class, Native::EpochMillis(millis + amount * unit_ms))
}

fn class_desc_of(vm: &Vm, v: JValue) -> String {
    match v {
        JValue::Obj(id) => {
            let class = vm.arena.objects[id as usize].class;
            vm.str_of(vm.classes[class as usize].descriptor).to_string()
        }
        _ => "Ljava/time/ZonedDateTime;".to_string(),
    }
}

fn delta_field(vm: &mut Vm, args: &[JValue], sign: i64, field: FieldUnit) -> R {
    let millis = millis_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let amount = sign * long_of(vm, args[1]);
    let class = class_desc_of(vm, args[0]);
    let (year, month, day, hour, minute, second, milli) = components(millis);
    let updated = match field {
        FieldUnit::Months => compose(
            year,
            (month - 1) as i64 as i32 + amount as i32,
            day,
            hour,
            minute,
            second,
            milli,
        ),
        FieldUnit::Years => compose(
            year + amount as i32,
            month - 1,
            day,
            hour,
            minute,
            second,
            milli,
        ),
    };
    alloc(vm, &class, Native::EpochMillis(updated))
}

enum FieldUnit {
    Months,
    Years,
}

pub(crate) fn zdt_plus_days(vm: &mut Vm, args: &[JValue]) -> R {
    delta(vm, args, DAY_MS)
}
pub(crate) fn zdt_minus_days(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = millis_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let amount = long_of(vm, args[1]);
    let class = class_desc_of(vm, args[0]);
    alloc(vm, &class, Native::EpochMillis(millis - amount * DAY_MS))
}
pub(crate) fn zdt_minus_hours(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = millis_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let amount = long_of(vm, args[1]);
    let class = class_desc_of(vm, args[0]);
    alloc(vm, &class, Native::EpochMillis(millis - amount * 3_600_000))
}
pub(crate) fn zdt_minus_minutes(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = millis_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let amount = long_of(vm, args[1]);
    let class = class_desc_of(vm, args[0]);
    alloc(vm, &class, Native::EpochMillis(millis - amount * 60_000))
}
pub(crate) fn zdt_minus_seconds(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = millis_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let amount = long_of(vm, args[1]);
    let class = class_desc_of(vm, args[0]);
    alloc(vm, &class, Native::EpochMillis(millis - amount * 1_000))
}
pub(crate) fn zdt_minus_weeks(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = millis_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let amount = long_of(vm, args[1]);
    let class = class_desc_of(vm, args[0]);
    alloc(
        vm,
        &class,
        Native::EpochMillis(millis - amount * 7 * DAY_MS),
    )
}
pub(crate) fn zdt_minus_months(vm: &mut Vm, args: &[JValue]) -> R {
    delta_field(vm, args, -1, FieldUnit::Months)
}
pub(crate) fn zdt_minus_years(vm: &mut Vm, args: &[JValue]) -> R {
    delta_field(vm, args, -1, FieldUnit::Years)
}

pub(crate) fn zdt_truncated_to(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = millis_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let unit = match payload(vm, args[1]) {
        Some(Native::Str(s)) => s.clone(),
        _ => "DAYS".to_string(),
    };
    let class = class_desc_of(vm, args[0]);
    let truncated = match unit.as_str() {
        "DAYS" => millis.div_euclid(DAY_MS) * DAY_MS,
        "HOURS" => millis.div_euclid(3_600_000) * 3_600_000,
        "MINUTES" => millis.div_euclid(60_000) * 60_000,
        "SECONDS" => millis.div_euclid(1_000) * 1_000,
        _ => millis,
    };
    alloc(vm, &class, Native::EpochMillis(truncated))
}

/// `ZonedDateTime.parse(text, formatter)` for `y M d H m s`-style patterns
/// (mirrors `SimpleDateFormat.parse`'s pattern-letter subset).
pub(crate) fn zdt_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let pattern = match payload(vm, args[1]) {
        Some(Native::DateFormatter { pattern, .. }) => pattern.clone(),
        _ => "yyyy-MM-dd'T'HH:mm:ss".to_string(),
    };
    let millis = parse_pattern_millis(&text, &pattern)
        .ok_or_else(|| NatErr::Throw(vm.err_iae(format!("unparseable date: {text}"))))?;
    alloc(vm, "Ljava/time/ZonedDateTime;", Native::EpochMillis(millis))
}

/// Shared by `LocalDateTime.parse` too.
pub(crate) fn parse_pattern_millis(text: &str, pattern: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    let mut pos = 0usize;
    let (mut y, mut mo, mut d, mut h, mut mi, mut s) = (1970i64, 1i64, 1i64, 0i64, 0i64, 0i64);
    let mut seen = false;
    let pb = pattern.as_bytes();
    let mut pi = 0;
    while pi < pb.len() {
        let c = pb[pi];
        let mut run = 1;
        while pi + run < pb.len() && pb[pi + run] == c {
            run += 1;
        }
        if c == b'\'' {
            pi += 1;
            while pi < pb.len() && pb[pi] != b'\'' {
                if pos < bytes.len() && bytes[pos] == pb[pi] {
                    pos += 1;
                }
                pi += 1;
            }
            pi += 1;
            continue;
        }
        if !c.is_ascii_alphabetic() {
            if pos < bytes.len() && bytes[pos] == c {
                pos += 1;
            }
            pi += run;
            continue;
        }
        let read = |pos: &mut usize| -> Option<i64> {
            let start = *pos;
            while *pos < bytes.len() && bytes[*pos].is_ascii_digit() && *pos - start < 4 {
                *pos += 1;
            }
            if *pos == start {
                return None;
            }
            std::str::from_utf8(&bytes[start..*pos]).ok()?.parse().ok()
        };
        match c {
            b'y' | b'Y' => {
                y = read(&mut pos)?;
                seen = true;
            }
            b'M' => {
                mo = read(&mut pos)?;
                seen = true;
            }
            b'd' => {
                d = read(&mut pos)?;
                seen = true;
            }
            b'H' => {
                h = read(&mut pos)?;
                seen = true;
            }
            b'm' => {
                mi = read(&mut pos)?;
                seen = true;
            }
            b's' => {
                s = read(&mut pos)?;
                seen = true;
            }
            _ => {}
        }
        pi += run;
    }
    if !seen {
        return None;
    }
    Some(compose(
        y as i32,
        mo as i32 - 1,
        d as i32,
        h as i32,
        mi as i32,
        s as i32,
        0,
    ))
}

/// Native methods for Ljava/time/chrono/ChronoZonedDateTime;,
/// Ljava/time/ZonedDateTime; and Ljava/time/OffsetDateTime; (all share the
/// same UTC-epoch-millis representation).
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/time/chrono/ChronoZonedDateTime;",
        "toInstant",
        "()Ljava/time/Instant;",
        true,
        zdt_to_instant
    ),
    ne!(
        "Ljava/time/chrono/ChronoZonedDateTime;",
        "toEpochSecond",
        "()J",
        true,
        zdt_to_epoch_second
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "toInstant",
        "()Ljava/time/Instant;",
        true,
        zdt_to_instant
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "toEpochSecond",
        "()J",
        true,
        zdt_to_epoch_second
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "now",
        "()Ljava/time/ZonedDateTime;",
        false,
        zdt_now_zoned
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "now",
        "(Ljava/time/ZoneId;)Ljava/time/ZonedDateTime;",
        false,
        zdt_now_zoned
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "getYear",
        "()I",
        true,
        zdt_get_year
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "plusDays",
        "(J)Ljava/time/ZonedDateTime;",
        true,
        zdt_plus_days
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "minusDays",
        "(J)Ljava/time/ZonedDateTime;",
        true,
        zdt_minus_days
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "minusHours",
        "(J)Ljava/time/ZonedDateTime;",
        true,
        zdt_minus_hours
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "minusMinutes",
        "(J)Ljava/time/ZonedDateTime;",
        true,
        zdt_minus_minutes
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "minusSeconds",
        "(J)Ljava/time/ZonedDateTime;",
        true,
        zdt_minus_seconds
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "minusWeeks",
        "(J)Ljava/time/ZonedDateTime;",
        true,
        zdt_minus_weeks
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "minusMonths",
        "(J)Ljava/time/ZonedDateTime;",
        true,
        zdt_minus_months
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "minusYears",
        "(J)Ljava/time/ZonedDateTime;",
        true,
        zdt_minus_years
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "truncatedTo",
        "(Ljava/time/temporal/TemporalUnit;)Ljava/time/ZonedDateTime;",
        true,
        zdt_truncated_to
    ),
    ne!(
        "Ljava/time/ZonedDateTime;",
        "parse",
        "(Ljava/lang/CharSequence;Ljava/time/format/DateTimeFormatter;)Ljava/time/ZonedDateTime;",
        false,
        zdt_parse
    ),
    ne!(
        "Ljava/time/OffsetDateTime;",
        "toInstant",
        "()Ljava/time/Instant;",
        true,
        zdt_to_instant
    ),
    ne!(
        "Ljava/time/OffsetDateTime;",
        "now",
        "(Ljava/time/ZoneId;)Ljava/time/OffsetDateTime;",
        false,
        zdt_now_offset
    ),
    ne!(
        "Ljava/time/OffsetDateTime;",
        "minusHours",
        "(J)Ljava/time/OffsetDateTime;",
        true,
        zdt_minus_hours
    ),
    ne!(
        "Ljava/time/OffsetDateTime;",
        "minusMinutes",
        "(J)Ljava/time/OffsetDateTime;",
        true,
        zdt_minus_minutes
    ),
    ne!(
        "Ljava/time/OffsetDateTime;",
        "minusSeconds",
        "(J)Ljava/time/OffsetDateTime;",
        true,
        zdt_minus_seconds
    ),
];
