//! java.time.LocalDateTime host shims. Represented the same way as
//! ZonedDateTime — a UTC `Native::EpochMillis` payload, no real zone/offset.

use super::zoned_date_time::parse_pattern_millis;
use crate::vm::native::*;

pub(crate) fn ldt_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let millis = parse_pattern_millis(&text, "yyyy-MM-dd'T'HH:mm:ss")
        .ok_or_else(|| NatErr::Throw(vm.err_iae(format!("unparseable date: {text}"))))?;
    alloc(vm, "Ljava/time/LocalDateTime;", Native::EpochMillis(millis))
}

pub(crate) fn ldt_parse_with_formatter(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let pattern = match payload(vm, args[1]) {
        Some(Native::DateFormatter { pattern, .. }) => pattern.clone(),
        _ => "yyyy-MM-dd'T'HH:mm:ss".to_string(),
    };
    let millis = parse_pattern_millis(&text, &pattern)
        .ok_or_else(|| NatErr::Throw(vm.err_iae(format!("unparseable date: {text}"))))?;
    alloc(vm, "Ljava/time/LocalDateTime;", Native::EpochMillis(millis))
}

pub(crate) fn ldt_at_zone(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = match payload(vm, args[0]) {
        Some(Native::EpochMillis(m)) => *m,
        _ => return Err(npe(vm)),
    };
    alloc(vm, "Ljava/time/ZonedDateTime;", Native::EpochMillis(millis))
}

/// Native methods for Ljava/time/LocalDateTime;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/time/LocalDateTime;",
        "parse",
        "(Ljava/lang/CharSequence;)Ljava/time/LocalDateTime;",
        false,
        ldt_parse
    ),
    ne!(
        "Ljava/time/LocalDateTime;",
        "parse",
        "(Ljava/lang/CharSequence;Ljava/time/format/DateTimeFormatter;)Ljava/time/LocalDateTime;",
        false,
        ldt_parse_with_formatter
    ),
    ne!(
        "Ljava/time/LocalDateTime;",
        "atZone",
        "(Ljava/time/ZoneId;)Ljava/time/ZonedDateTime;",
        true,
        ldt_at_zone
    ),
];
