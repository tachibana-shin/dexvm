//! Kotlin time value classes and instant helpers.
use crate::vm::native::*;

fn unit_millis(vm: &mut Vm, v: JValue) -> Result<i64, NatErr> {
    if v.is_null_ref() {
        return Ok(1000);
    }
    let desc = vm.class_desc_str(obj_class(vm, v.as_obj()));
    Ok(if desc.starts_with("Lkotlin/time/DurationUnit;") {
        1000
    } else {
        1
    })
}

pub(super) fn duration_get_zero(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Long(0))
}
pub(super) fn duration_to_duration_int(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(
        int_of(vm, args[0]) as i64 * unit_millis(vm, args[1])?,
    ))
}
pub(super) fn duration_to_duration_long(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(
        long_of(vm, args[0]) * unit_millis(vm, args[1])?,
    ))
}

pub(super) fn kotlin_instant_to_epoch_millis(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::EpochMillis(m)) => Ok(JValue::Long(*m)),
        _ => Err(npe(vm)),
    }
}
pub(super) fn kotlin_instant_now(vm: &mut Vm, _args: &[JValue]) -> R {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| iae(vm, "clock before epoch"))?
        .as_millis() as i64;
    alloc(vm, "Lkotlin/time/Instant;", Native::EpochMillis(millis))
}
pub(super) fn kotlin_instant_minus(vm: &mut Vm, args: &[JValue]) -> R {
    let base = match payload(vm, args[0]) {
        Some(Native::EpochMillis(m)) => *m,
        _ => return Err(npe(vm)),
    };
    alloc(
        vm,
        "Lkotlin/time/Instant;",
        Native::EpochMillis(base.saturating_sub(long_of(vm, args[1]))),
    )
}
pub(super) fn kotlin_instant_parse_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let text = jstr(vm, args[1]).unwrap_or_default();
    let Some((date, time)) = text.split_once('T') else {
        return Ok(JValue::Null);
    };
    let mut d = date.split('-');
    let (Ok(y), Ok(m), Ok(day)) = (
        d.next().unwrap_or("").parse::<i64>(),
        d.next().unwrap_or("").parse::<i64>(),
        d.next().unwrap_or("").parse::<i64>(),
    ) else {
        return Ok(JValue::Null);
    };
    let time = time.strip_suffix('Z').unwrap_or(time);
    let (clock, frac) = time.split_once('.').map_or((time, ""), |v| v);
    let mut c = clock.split(':');
    let (Ok(h), Ok(min), Ok(sec)) = (
        c.next().unwrap_or("").parse::<i64>(),
        c.next().unwrap_or("").parse::<i64>(),
        c.next().unwrap_or("").parse::<i64>(),
    ) else {
        return Ok(JValue::Null);
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) || h > 23 || min > 59 || sec > 60 {
        return Ok(JValue::Null);
    }
    // Howard Hinnant's civil-date conversion, using Euclidean division so
    // dates before the Unix epoch are handled correctly as well.
    let adjusted_year = y - i64::from(m <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let shifted_month = m + if m > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    let fraction_millis = frac
        .bytes()
        .take(3)
        .try_fold((0_i64, 0_u8), |(value, digits), byte| {
            byte.is_ascii_digit()
                .then_some((value * 10 + i64::from(byte - b'0'), digits + 1))
        })
        .map(|(value, digits)| value * 10_i64.pow(u32::from(3 - digits)))
        .unwrap_or(0);
    let millis = (days * 86_400 + h * 3600 + min * 60 + sec) * 1000 + fraction_millis;
    alloc(vm, "Lkotlin/time/Instant;", Native::EpochMillis(millis))
}

fn keiyoushi_duration_minus(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0]) - long_of(vm, args[1])))
}
fn keiyoushi_duration_compare(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(
        long_of(vm, args[0]).cmp(&long_of(vm, args[1])) as i32
    ))
}
fn keiyoushi_duration_equals(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(
        long_of(vm, args[0]) == long_of(vm, args[1]),
    )))
}
fn duration_box(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlin/time/Duration;",
        Native::Duration(long_of(vm, args[0])),
    )
}
fn duration_unbox(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Duration(raw)) => Ok(JValue::Long(*raw)),
        _ => Err(npe(vm)),
    }
}
fn duration_nanos_impl(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0]).saturating_mul(1_000_000)))
}
fn duration_millis_impl(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0])))
}
/// Formats the millisecond raw value the way `Duration.toString()` renders
/// multi-unit durations (e.g. "1h 5m 30s", "250ms", "1.5s").
fn format_duration_millis(millis: i64) -> String {
    if millis == 0 {
        return "0s".to_string();
    }
    let neg = millis < 0;
    let mut ms = millis.unsigned_abs();
    let body = if ms < 1000 {
        format!("{ms}ms")
    } else {
        let days = ms / 86_400_000;
        ms %= 86_400_000;
        let hours = ms / 3_600_000;
        ms %= 3_600_000;
        let minutes = ms / 60_000;
        ms %= 60_000;
        let seconds = ms / 1000;
        let sub_ms = ms % 1000;
        let mut parts = Vec::new();
        if days > 0 {
            parts.push(format!("{days}d"));
        }
        if hours > 0 {
            parts.push(format!("{hours}h"));
        }
        if minutes > 0 {
            parts.push(format!("{minutes}m"));
        }
        if seconds > 0 || sub_ms > 0 || parts.is_empty() {
            if sub_ms > 0 {
                let frac = format!("{sub_ms:03}");
                let frac = frac.trim_end_matches('0');
                parts.push(if frac.is_empty() {
                    format!("{seconds}s")
                } else {
                    format!("{seconds}.{frac}s")
                });
            } else {
                parts.push(format!("{seconds}s"));
            }
        }
        parts.join(" ")
    };
    if neg {
        format!("-{body}")
    } else {
        body
    }
}
fn duration_to_string_impl(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = long_of(vm, args[0]);
    Ok(new_str(vm, &format_duration_millis(millis)))
}
fn duration_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    let a = match payload(vm, args[0]) {
        Some(Native::Duration(raw)) => *raw,
        _ => return Err(npe(vm)),
    };
    let b = match payload(vm, args[1]) {
        Some(Native::Duration(raw)) => *raw,
        _ => long_of(vm, args[1]),
    };
    Ok(JValue::Int(a.cmp(&b) as i32))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Lkotlin/time/Duration$Companion;",
        "getZERO-UwyO8pc",
        "()J",
        true,
        duration_get_zero
    ),
    ne!(
        "Lkotlin/time/DurationKt;",
        "toDuration",
        "(ILkotlin/time/DurationUnit;)J",
        false,
        duration_to_duration_int
    ),
    ne!(
        "Lkotlin/time/DurationKt;",
        "toDuration",
        "(JLkotlin/time/DurationUnit;)J",
        false,
        duration_to_duration_long
    ),
    ne!(
        "Lkotlin/time/Instant;",
        "toEpochMilliseconds",
        "()J",
        true,
        kotlin_instant_to_epoch_millis
    ),
    ne!(
        "Lkotlin/time/Instant;",
        "minus-LRDsOJo",
        "(J)Lkotlin/time/Instant;",
        true,
        kotlin_instant_minus
    ),
    ne!(
        "Lkotlin/time/Clock$System;",
        "now",
        "()Lkotlin/time/Instant;",
        true,
        kotlin_instant_now
    ),
    ne!(
        "Lkotlin/time/Instant$Companion;",
        "parseOrNull",
        "(Ljava/lang/CharSequence;)Lkotlin/time/Instant;",
        true,
        kotlin_instant_parse_or_null
    ),
    ne!(
        "Lkotlin/time/Instant$Companion;",
        "parse",
        "(Ljava/lang/CharSequence;)Lkotlin/time/Instant;",
        true,
        kotlin_instant_parse_or_null
    ),
    ne!(
        "Lkotlin/time/Duration;",
        "minus-LRDsOJo",
        "(JJ)J",
        false,
        keiyoushi_duration_minus
    ),
    ne!(
        "Lkotlin/time/Duration;",
        "compareTo-LRDsOJo",
        "(JJ)I",
        false,
        keiyoushi_duration_compare
    ),
    ne!(
        "Lkotlin/time/Duration;",
        "equals-impl0",
        "(JJ)Z",
        false,
        keiyoushi_duration_equals
    ),
    ne!(
        "Lkotlin/time/Duration;",
        "box-impl",
        "(J)Lkotlin/time/Duration;",
        false,
        duration_box
    ),
    ne!(
        "Lkotlin/time/Duration;",
        "unbox-impl",
        "()J",
        true,
        duration_unbox
    ),
    ne!(
        "Lkotlin/time/Duration;",
        "getInWholeNanoseconds-impl",
        "(J)J",
        false,
        duration_nanos_impl
    ),
    ne!(
        "Lkotlin/time/Duration;",
        "getInWholeMilliseconds-impl",
        "(J)J",
        false,
        duration_millis_impl
    ),
    ne!(
        "Lkotlin/time/Duration;",
        "compareTo",
        "(Ljava/lang/Object;)I",
        true,
        duration_compare_to
    ),
    ne!(
        "Lkotlin/time/Duration;",
        "toString-impl",
        "(J)Ljava/lang/String;",
        false,
        duration_to_string_impl
    ),
];
