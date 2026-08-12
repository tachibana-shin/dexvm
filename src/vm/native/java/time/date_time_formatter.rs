//! java.time.format.DateTimeFormatter host shims: DateTimeFormatter.ofPattern
//! + Locale.ROOT.

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

/// Native methods for Ljava/time/format/DateTimeFormatter;
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
];
