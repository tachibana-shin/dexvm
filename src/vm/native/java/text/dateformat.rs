//! java.text.DateFormat host shims.

use crate::vm::native::*;

pub(crate) fn format_object(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[1]).unwrap_or_default();
    Ok(new_str(vm, &value))
}

// DateFormat shares the date_format_* impls with SimpleDateFormat (see simpledateformat.rs).

/// Native methods for Ljava/text/DateFormat;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/text/DateFormat;",
        "setTimeZone",
        "(Ljava/util/TimeZone;)V",
        true,
        date_format_set_time_zone
    ),
    ne!(
        "Ljava/text/DateFormat;",
        "parse",
        "(Ljava/lang/String;)Ljava/util/Date;",
        true,
        simple_date_format_parse
    ),
    ne!(
        "Ljava/text/Format;",
        "format",
        "(Ljava/lang/Object;)Ljava/lang/String;",
        true,
        format_object
    ),
];
