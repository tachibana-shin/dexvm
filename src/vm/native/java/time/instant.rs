//! java.time.Instant host shim: Instant.toEpochMilli on the millis payload
//! produced by the LocalDate.atStartOfDay chain.

use crate::vm::native::*;

pub(crate) fn instant_to_epoch_milli(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::EpochMillis(m)) => Ok(JValue::Long(*m)),
        _ => Err(npe(vm)),
    }
}

/// Native methods for Ljava/time/Instant;
pub(crate) const TABLE: &[NativeEntry] = &[ne!(
    "Ljava/time/Instant;",
    "toEpochMilli",
    "()J",
    true,
    instant_to_epoch_milli
)];
