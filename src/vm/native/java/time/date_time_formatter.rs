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

/// `withLocale` doesn't affect this simplified (ASCII, fixed-pattern)
/// formatter — returns `this` for chaining.
pub(crate) fn dtf_with_locale(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn dtf_of_localized_date(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Ljava/time/format/DateTimeFormatter;",
        Native::DateFormatter {
            pattern: "yyyy-MM-dd".to_string(),
            zone: String::new(),
        },
    )
}

pub(crate) fn dtf_format(vm: &mut Vm, args: &[JValue]) -> R {
    let pattern = match payload(vm, args[0]) {
        Some(Native::DateFormatter { pattern, .. }) => pattern.clone(),
        _ => return Err(npe(vm)),
    };
    let millis = match payload(vm, args[1]) {
        Some(Native::EpochMillis(m)) => *m,
        Some(Native::LocalDay(d)) => i64::from(*d) * super::super::civil::DAY_MS,
        Some(Native::Date(m)) => *m,
        _ => return Err(npe(vm)),
    };
    let (year, month, day, hour, minute, second, _) = super::super::civil::components(millis);
    let mut out = String::new();
    let pb = pattern.as_bytes();
    let mut pi = 0;
    while pi < pb.len() {
        let c = pb[pi];
        if c == b'\'' {
            pi += 1;
            while pi < pb.len() && pb[pi] != b'\'' {
                out.push(pb[pi] as char);
                pi += 1;
            }
            pi += 1;
            continue;
        }
        let mut run = 1;
        while pi + run < pb.len() && pb[pi + run] == c {
            run += 1;
        }
        match c {
            b'y' | b'Y' => out.push_str(&format!("{:0width$}", year, width = run.max(4))),
            b'M' => out.push_str(&format!("{:0width$}", month, width = run)),
            b'd' => out.push_str(&format!("{:0width$}", day, width = run)),
            b'H' => out.push_str(&format!("{:0width$}", hour, width = run)),
            b'm' => out.push_str(&format!("{:0width$}", minute, width = run)),
            b's' => out.push_str(&format!("{:0width$}", second, width = run)),
            _ => {
                for _ in 0..run {
                    out.push(c as char);
                }
            }
        }
        pi += run;
    }
    Ok(new_str(vm, &out))
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
    ne!(
        "Ljava/time/format/DateTimeFormatter;",
        "withLocale",
        "(Ljava/util/Locale;)Ljava/time/format/DateTimeFormatter;",
        true,
        dtf_with_locale
    ),
    ne!(
        "Ljava/time/format/DateTimeFormatter;",
        "ofLocalizedDate",
        "(Ljava/time/format/FormatStyle;)Ljava/time/format/DateTimeFormatter;",
        false,
        dtf_of_localized_date
    ),
    ne!(
        "Ljava/time/format/DateTimeFormatter;",
        "format",
        "(Ljava/time/temporal/TemporalAccessor;)Ljava/lang/String;",
        true,
        dtf_format
    ),
];
