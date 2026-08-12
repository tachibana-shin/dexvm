//! java.text.DecimalFormat / DecimalFormatSymbols host shims. The pattern
//! is stored verbatim and reparsed on every `format` call — cheap enough
//! given these are one-off calls, not a hot loop.

use crate::vm::native::*;

fn decimal_format_init(vm: &mut Vm, args: &[JValue]) -> R {
    let pattern = jstr(vm, args[1]).unwrap_or_else(|_| "#,##0.###".to_string());
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Str(pattern));
    Ok(JValue::Null)
}

fn decimal_format_symbols_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Opaque);
    Ok(JValue::Null)
}

fn decimal_format_symbols_get_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/text/DecimalFormatSymbols;", Native::Opaque)
}

/// (min_fraction_digits, max_fraction_digits, grouping)
fn pattern_digits(pattern: &str) -> (usize, usize, bool) {
    let grouping = pattern.contains(',');
    let frac = pattern.split('.').nth(1).unwrap_or("");
    let frac = frac.split(';').next().unwrap_or(frac);
    let min = frac.chars().take_while(|&c| c == '0').count();
    let max = frac.chars().filter(|&c| c == '0' || c == '#').count();
    (min, max, grouping)
}

fn group_integer(digits: &str) -> String {
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

fn format_decimal(pattern: &str, value: f64) -> String {
    let (min_frac, max_frac, grouping) = pattern_digits(pattern);
    let neg = value.is_sign_negative() && value != 0.0;
    let scaled = (value.abs() * 10f64.powi(max_frac as i32)).round() as i64;
    let base = 10i64.pow(max_frac as u32).max(1);
    let int_part = scaled / base;
    let mut frac_part = format!("{:0width$}", scaled % base, width = max_frac);
    while frac_part.len() > min_frac && frac_part.ends_with('0') {
        frac_part.pop();
    }
    let int_str = if grouping {
        group_integer(&int_part.to_string())
    } else {
        int_part.to_string()
    };
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push_str(&int_str);
    if !frac_part.is_empty() {
        out.push('.');
        out.push_str(&frac_part);
    }
    out
}

fn decimal_format_format_double(vm: &mut Vm, args: &[JValue]) -> R {
    let pattern = match payload(vm, args[0]) {
        Some(Native::Str(p)) => p.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(
        vm,
        &format_decimal(&pattern, double_of(vm, args[1])),
    ))
}

fn decimal_format_format_long(vm: &mut Vm, args: &[JValue]) -> R {
    let pattern = match payload(vm, args[0]) {
        Some(Native::Str(p)) => p.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(
        vm,
        &format_decimal(&pattern, long_of(vm, args[1]) as f64),
    ))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/text/DecimalFormat;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        decimal_format_init
    ),
    ne!(
        "Ljava/text/DecimalFormat;",
        "<init>",
        "(Ljava/lang/String;Ljava/text/DecimalFormatSymbols;)V",
        true,
        decimal_format_init
    ),
    ne!(
        "Ljava/text/DecimalFormat;",
        "format",
        "(D)Ljava/lang/String;",
        true,
        decimal_format_format_double
    ),
    ne!(
        "Ljava/text/DecimalFormat;",
        "format",
        "(J)Ljava/lang/String;",
        true,
        decimal_format_format_long
    ),
    ne!(
        "Ljava/text/DecimalFormatSymbols;",
        "<init>",
        "(Ljava/util/Locale;)V",
        true,
        decimal_format_symbols_init
    ),
    ne!(
        "Ljava/text/DecimalFormatSymbols;",
        "getInstance",
        "(Ljava/util/Locale;)Ljava/text/DecimalFormatSymbols;",
        false,
        decimal_format_symbols_get_instance
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_grouping_and_fraction_digits() {
        assert_eq!(format_decimal("#,##0.00", 1234.5), "1,234.50");
        assert_eq!(format_decimal("0.##", 3.0), "3");
        assert_eq!(format_decimal("0.##", 3.14159), "3.14");
        assert_eq!(format_decimal("#,##0.###", -1234567.891), "-1,234,567.891");
    }
}
