//! java.time.ZoneId host shim. Zones are treated as fixed offsets
//! (Asia/Ho_Chi_Minh = +7, no DST).

use crate::vm::native::*;

pub(crate) fn zone_of(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/time/ZoneId;", Native::Opaque)
}

pub(crate) fn zone_system_default(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/time/ZoneId;", Native::Opaque)
}

/// Native methods for Ljava/time/ZoneId;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/time/ZoneId;",
        "of",
        "(Ljava/lang/String;)Ljava/time/ZoneId;",
        false,
        zone_of
    ),
    ne!(
        "Ljava/time/ZoneId;",
        "systemDefault",
        "()Ljava/time/ZoneId;",
        false,
        zone_system_default
    ),
];
