//! Minimal java.time shims for synthetic extensions:
//! DateTimeFormatter.ofPattern + Locale.ROOT, ZoneId.of, and the
//! LocalDate.parse -> atStartOfDay -> Instant.toEpochMilli chain the
//! generated code uses for "last updated" bookkeeping. Zones are treated as
//! fixed offsets (Asia/Ho_Chi_Minh = +7, no DST); dates are dd/MM/yyyy.

use crate::vm::native::*;

pub(crate) fn dtf_of_pattern(vm: &mut Vm, args: &[JValue]) -> R {
    let pattern = jstr(vm, args[0])?;
    alloc(
        vm,
        "Ljava/time/format/DateTimeFormatter;",
        Native::DateFormatter {
            pattern,
            zone: String::new(),
        },
    )
}

pub(crate) fn dtf_tostring(vm: &mut Vm, args: &[JValue]) -> R {
    let pattern = match payload(vm, args[0]) {
        Some(Native::DateFormatter { pattern, .. }) => pattern.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &pattern))
}

pub(crate) fn zone_of(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/time/ZoneId;", Native::Opaque)
}

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

pub(crate) fn zdt_to_instant(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = match payload(vm, args[0]) {
        Some(Native::EpochMillis(m)) => *m,
        _ => return Err(npe(vm)),
    };
    alloc(vm, "Ljava/time/Instant;", Native::EpochMillis(millis))
}

pub(crate) fn instant_to_epoch_milli(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::EpochMillis(m)) => Ok(JValue::Long(*m)),
        _ => Err(npe(vm)),
    }
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

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/time/format/DateTimeFormatter;",
        "ofPattern",
        "(Ljava/lang/String;Ljava/util/Locale;)Ljava/time/format/DateTimeFormatter;",
        false,
        dtf_of_pattern
    ),
    ne!(
        "Ljava/time/format/DateTimeFormatter;",
        "toString",
        "()Ljava/lang/String;",
        true,
        dtf_tostring
    ),
    ne!(
        "Ljava/time/ZoneId;",
        "of",
        "(Ljava/lang/String;)Ljava/time/ZoneId;",
        false,
        zone_of
    ),
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
        "Ljava/time/chrono/ChronoZonedDateTime;",
        "toInstant",
        "()Ljava/time/Instant;",
        true,
        zdt_to_instant
    ),
    ne!(
        "Ljava/time/Instant;",
        "toEpochMilli",
        "()J",
        true,
        instant_to_epoch_milli
    ),
];
