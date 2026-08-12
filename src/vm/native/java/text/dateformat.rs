//! java.text.DateFormat host shims.

use crate::vm::native::*;

pub(crate) fn format_object(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[1]).unwrap_or_default();
    Ok(new_str(vm, &value))
}

pub(crate) fn date_format_set_lenient(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// `DateFormat.format(Date)` for `y M d H m s`-style patterns (mirrors
/// `SimpleDateFormat.parse`'s pattern-letter subset, in reverse).
pub(crate) fn date_format_format_date(vm: &mut Vm, args: &[JValue]) -> R {
    let (pattern, zone) = match payload(vm, args[0]) {
        Some(Native::DateFormatter { pattern, zone }) => (pattern.clone(), zone.clone()),
        _ => return Err(npe(vm)),
    };
    let millis = match payload(vm, args[1]) {
        Some(Native::Date(m)) => *m,
        Some(Native::EpochMillis(m)) => *m,
        _ => return Err(npe(vm)),
    } + zone_offset_ms(&zone);
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

/// Parse "UTC", "GMT", "GMT+HH:MM" style zone ids into an offset in millis.
fn zone_offset_ms(zone: &str) -> i64 {
    let z = zone.trim();
    if z == "UTC" || z == "GMT" || z == "Z" || z.is_empty() {
        return 0;
    }
    let rest = z.strip_prefix("GMT+").or_else(|| z.strip_prefix("UTC+"));
    let sign: i64 = if z.contains('-') { -1 } else { 1 };
    let Some(r) = rest else { return 0 };
    let parts: Vec<&str> = r.split(':').collect();
    let h: i64 = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);
    let m: i64 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(0);
    sign * (h * 3_600_000 + m * 60_000)
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
    ne!(
        "Ljava/text/DateFormat;",
        "format",
        "(Ljava/util/Date;)Ljava/lang/String;",
        true,
        date_format_format_date
    ),
    ne!(
        "Ljava/text/DateFormat;",
        "setLenient",
        "(Z)V",
        true,
        date_format_set_lenient
    ),
];
