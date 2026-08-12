//! java.time.Instant host shim: Instant.toEpochMilli on the millis payload
//! produced by the LocalDate.atStartOfDay chain.

use crate::vm::native::*;

pub(crate) fn instant_to_epoch_milli(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::EpochMillis(m)) => Ok(JValue::Long(*m)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn instant_at_zone(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = match payload(vm, args[0]) {
        Some(Native::EpochMillis(m)) => *m,
        _ => return Err(npe(vm)),
    };
    alloc(vm, "Ljava/time/ZonedDateTime;", Native::EpochMillis(millis))
}

pub(crate) fn instant_of_epoch_milli(vm: &mut Vm, args: &[JValue]) -> R {
    alloc(vm, "Ljava/time/Instant;", Native::EpochMillis(long_of(vm, args[0])))
}

pub(crate) fn instant_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let millis = super::zoned_date_time::parse_pattern_millis(&text, "yyyy-MM-dd'T'HH:mm:ss")
        .ok_or_else(|| NatErr::Throw(vm.err_iae(format!("unparseable instant: {text}"))))?;
    alloc(vm, "Ljava/time/Instant;", Native::EpochMillis(millis))
}

/// Native methods for Ljava/time/Instant;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/time/Instant;",
        "toEpochMilli",
        "()J",
        true,
        instant_to_epoch_milli
    ),
    ne!(
        "Ljava/time/Instant;",
        "atZone",
        "(Ljava/time/ZoneId;)Ljava/time/ZonedDateTime;",
        true,
        instant_at_zone
    ),
    ne!(
        "Ljava/time/Instant;",
        "ofEpochMilli",
        "(J)Ljava/time/Instant;",
        false,
        instant_of_epoch_milli
    ),
    ne!(
        "Ljava/time/Instant;",
        "parse",
        "(Ljava/lang/CharSequence;)Ljava/time/Instant;",
        false,
        instant_parse
    ),
];
