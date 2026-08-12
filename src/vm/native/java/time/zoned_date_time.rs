//! java.time.chrono.ChronoZonedDateTime host shim: the
//! LocalDate.parse -> atStartOfDay -> Instant.toEpochMilli chain the
//! generated code uses for "last updated" bookkeeping.

use crate::vm::native::*;

pub(crate) fn zdt_to_instant(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = match payload(vm, args[0]) {
        Some(Native::EpochMillis(m)) => *m,
        _ => return Err(npe(vm)),
    };
    alloc(vm, "Ljava/time/Instant;", Native::EpochMillis(millis))
}

/// Native methods for Ljava/time/chrono/ChronoZonedDateTime;
pub(crate) const TABLE: &[NativeEntry] = &[ne!(
    "Ljava/time/chrono/ChronoZonedDateTime;",
    "toInstant",
    "()Ljava/time/Instant;",
    true,
    zdt_to_instant
)];
