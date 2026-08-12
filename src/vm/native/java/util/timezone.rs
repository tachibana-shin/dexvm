//! java.util.TimeZone host shims.

use crate::vm::native::*;

pub(crate) fn time_zone_get_time_zone(vm: &mut Vm, args: &[JValue]) -> R {
    let id = jstr(vm, args[0])?;
    alloc(vm, "Ljava/util/TimeZone;", Native::TimeZone(id))
}

pub(crate) fn time_zone_get_default(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Ljava/util/TimeZone;",
        Native::TimeZone("UTC".to_string()),
    )
}

/// Native methods for Ljava/util/TimeZone;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/TimeZone;",
        "getTimeZone",
        "(Ljava/lang/String;)Ljava/util/TimeZone;",
        false,
        time_zone_get_time_zone
    ),
    ne!(
        "Ljava/util/TimeZone;",
        "getDefault",
        "()Ljava/util/TimeZone;",
        false,
        time_zone_get_default
    ),
];
