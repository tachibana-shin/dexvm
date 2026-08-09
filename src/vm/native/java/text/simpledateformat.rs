//! java.text.SimpleDateFormat host shims.

use crate::vm::native::*;

// TimeZone helpers (moved from java/text timezone.rs section).


pub(crate) fn date_format_set_time_zone(vm: &mut Vm, args: &[JValue]) -> R {
    let zone = match payload(vm, args[1]) {
        Some(Native::TimeZone(z)) => z.clone(),
        _ => "UTC".to_string(),
    };
    let Some(Native::DateFormatter { zone: dst, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = zone;
    Ok(JValue::Null)
}

pub(crate) fn simple_date_format_init(vm: &mut Vm, args: &[JValue]) -> R {
    let pat = jstr(vm, args[1])?;
    let Some(Native::DateFormatter { pattern, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *pattern = pat;
    Ok(JValue::Null)
}

pub(crate) fn simple_date_format_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let pattern = match payload(vm, args[0]) {
        Some(Native::DateFormatter { pattern, .. }) => pattern.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &pattern))
}

/// Minimal `parse(String, ParsePosition)` for `yyyy-MM-dd HH:mm`-style
/// patterns: supports `y M d H m s` (1-4 digits), quoted literals, and
/// literal punctuation. On success the ParsePosition index advances past
/// the parsed text; on failure it is left unchanged and null is returned.
pub(crate) fn simple_date_format_parse(vm: &mut Vm, args: &[JValue]) -> R {
    let text = jstr(vm, args[1])?;
    let (pattern, zone) = match payload(vm, args[0]) {
        Some(Native::DateFormatter { pattern, zone }) => (pattern.clone(), zone.clone()),
        _ => return Err(npe(vm)),
    };
    let start = match payload(vm, args[2]) {
        Some(Native::ParsePosition(i)) => (*i).max(0) as usize,
        _ => 0,
    };
    let bytes = text.as_bytes();
    let mut pos = start;
    let mut y: i64 = 1970;
    let mut mo: i64 = 1;
    let mut d: i64 = 1;
    let mut h: i64 = 0;
    let mut mi: i64 = 0;
    let mut s: i64 = 0;
    let mut ok = true;
    let mut seen_any = false;
    let mut pi = 0;
    while pi < pattern.len() {
        let c = pattern.as_bytes()[pi];
        let mut run = 1;
        while pi + run < pattern.len() && pattern.as_bytes()[pi + run] == c {
            run += 1;
        }
        if c == '\'' as u8 {
            pi += 1;
            while pi < pattern.len() && pattern.as_bytes()[pi] != '\'' as u8 {
                if pos < bytes.len() && bytes[pos] == pattern.as_bytes()[pi] {
                    pos += 1;
                } else {
                    ok = false;
                }
                pi += 1;
            }
            pi += 1;
            continue;
        }
        if !c.is_ascii_alphabetic() {
            if pos < bytes.len() && bytes[pos] == c {
                pos += 1;
            } else if c == b' ' && pos < bytes.len() && bytes[pos] == b' ' {
                pos += 1;
            } else {
                ok = false;
            }
            pi += run;
            continue;
        }
        match c {
            b'y' | b'Y' => {
                let v = read_int(&bytes, &mut pos);
                if v.is_none() {
                    ok = false;
                } else {
                    y = v.unwrap() as i64;
                    seen_any = true;
                }
            }
            b'M' => {
                let v = read_int(&bytes, &mut pos);
                if v.is_none() {
                    ok = false;
                } else {
                    mo = v.unwrap() as i64;
                    seen_any = true;
                }
            }
            b'd' => {
                let v = read_int(&bytes, &mut pos);
                if v.is_none() {
                    ok = false;
                } else {
                    d = v.unwrap() as i64;
                    seen_any = true;
                }
            }
            b'H' => {
                let v = read_int(&bytes, &mut pos);
                if v.is_none() {
                    ok = false;
                } else {
                    h = v.unwrap() as i64;
                    seen_any = true;
                }
            }
            b'm' => {
                let v = read_int(&bytes, &mut pos);
                if v.is_none() {
                    ok = false;
                } else {
                    mi = v.unwrap() as i64;
                    seen_any = true;
                }
            }
            b's' => {
                let v = read_int(&bytes, &mut pos);
                if v.is_none() {
                    ok = false;
                } else {
                    s = v.unwrap() as i64;
                    seen_any = true;
                }
            }
            _ => pi += run,
        }
        if !ok {
            break;
        }
        pi += run;
    }
    if !ok || !seen_any {
        return Ok(JValue::Null);
    }
    if let Some(Native::ParsePosition(dst)) = payload_mut(vm, args[2]) {
        *dst = pos as i32;
    }
    let days = days_from_civil(y, mo, d);
    let mut ms = days * 86_400_000 + h * 3_600_000 + mi * 60_000 + s * 1000;
    ms -= zone_offset_ms(&zone);
    alloc(vm, "Ljava/util/Date;", Native::Date(ms))
}

fn read_int(bytes: &[u8], pos: &mut usize) -> Option<i32> {
    let start = *pos;
    while *pos < bytes.len() && bytes[*pos].is_ascii_digit() && *pos - start < 4 {
        *pos += 1;
    }
    if *pos == start {
        return None;
    }
    let s = std::str::from_utf8(&bytes[start..*pos]).ok()?;
    s.parse().ok()
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's
/// algorithm, inverse of civil_from_days).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Parse "UTC", "GMT", "GMT+HH:MM" style zone ids into an offset in millis.
fn zone_offset_ms(zone: &str) -> i64 {
    let z = zone.trim();
    if z == "UTC" || z == "GMT" || z == "Z" {
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

/// Native methods for Ljava/text/SimpleDateFormat;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/text/SimpleDateFormat;", "<init>", "(Ljava/lang/String;Ljava/util/Locale;)V", true, simple_date_format_init),
    ne!("Ljava/text/SimpleDateFormat;", "<init>", "(Ljava/lang/String;)V", true, simple_date_format_init),
    ne!("Ljava/text/SimpleDateFormat;", "parse", "(Ljava/lang/String;Ljava/text/ParsePosition;)Ljava/util/Date;", true, simple_date_format_parse),
    ne!("Ljava/text/SimpleDateFormat;", "toString", "()Ljava/lang/String;", true, simple_date_format_to_string),
];
