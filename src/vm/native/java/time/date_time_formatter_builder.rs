//! java.time.format.DateTimeFormatterBuilder host shim: accumulates a
//! pattern string (same `Native::DateFormatter` payload as
//! `DateTimeFormatter`/`SimpleDateFormat`) and hands it off via
//! `toFormatter`.

use crate::vm::native::*;

fn builder_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::DateFormatter { pattern, zone } => {
            pattern.clear();
            zone.clear();
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

fn builder_append_pattern(vm: &mut Vm, args: &[JValue]) -> R {
    let extra = jstr(vm, args[1]).unwrap_or_default();
    let Some(Native::DateFormatter { pattern, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    pattern.push_str(&extra);
    Ok(args[0])
}

/// `parseDefaulting` only matters for fields the input text omits; this
/// simplified formatter doesn't track defaults, so it's a no-op that just
/// returns `this` for chaining.
fn builder_parse_defaulting(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

fn builder_to_formatter(vm: &mut Vm, args: &[JValue]) -> R {
    let pattern = match payload(vm, args[0]) {
        Some(Native::DateFormatter { pattern, .. }) => pattern.clone(),
        _ => return Err(npe(vm)),
    };
    alloc(
        vm,
        "Ljava/time/format/DateTimeFormatter;",
        Native::DateFormatter {
            pattern,
            zone: String::new(),
        },
    )
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/time/format/DateTimeFormatterBuilder;",
        "<init>",
        "()V",
        true,
        builder_init
    ),
    ne!(
        "Ljava/time/format/DateTimeFormatterBuilder;",
        "appendPattern",
        "(Ljava/lang/String;)Ljava/time/format/DateTimeFormatterBuilder;",
        true,
        builder_append_pattern
    ),
    ne!(
        "Ljava/time/format/DateTimeFormatterBuilder;",
        "parseDefaulting",
        "(Ljava/time/temporal/TemporalField;J)Ljava/time/format/DateTimeFormatterBuilder;",
        true,
        builder_parse_defaulting
    ),
    ne!(
        "Ljava/time/format/DateTimeFormatterBuilder;",
        "toFormatter",
        "(Ljava/util/Locale;)Ljava/time/format/DateTimeFormatter;",
        true,
        builder_to_formatter
    ),
];
