use super::*;

// java.lang.Math
// ---------------------------------------------------------------------------

pub(crate) fn math_abs_int(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).wrapping_abs()))
}

pub(crate) fn math_abs_long(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0]).wrapping_abs()))
}

pub(crate) fn math_abs_float(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(float_of(vm, args[0]).abs()))
}

pub(crate) fn math_abs_double(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(double_of(vm, args[0]).abs()))
}

pub(crate) fn math_max_int(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).max(int_of(vm, args[1]))))
}

pub(crate) fn math_min_int(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]).min(int_of(vm, args[1]))))
}

pub(crate) fn math_max_long(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0]).max(long_of(vm, args[1]))))
}

pub(crate) fn math_min_long(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0]).min(long_of(vm, args[1]))))
}

pub(crate) fn math_max_float(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(fmax32(float_of(vm, args[0]), float_of(vm, args[1]))))
}

pub(crate) fn math_min_float(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(fmin32(float_of(vm, args[0]), float_of(vm, args[1]))))
}

pub(crate) fn math_max_double(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(fmax64(double_of(vm, args[0]), double_of(vm, args[1]))))
}

pub(crate) fn math_min_double(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(fmin64(double_of(vm, args[0]), double_of(vm, args[1]))))
}

pub(crate) fn math_unop_f64(vm: &mut Vm, args: &[JValue], f: impl Fn(f64) -> f64) -> R {
    Ok(JValue::Double(f(double_of(vm, args[0]))))
}

pub(crate) fn math_sqrt(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::sqrt)
}

pub(crate) fn math_cbrt(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::cbrt)
}

pub(crate) fn math_pow(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(double_of(vm, args[0]).powf(double_of(vm, args[1]))))
}

pub(crate) fn math_exp(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::exp)
}

pub(crate) fn math_log(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::ln)
}

pub(crate) fn math_log10(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::log10)
}

pub(crate) fn math_log1p(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::ln_1p)
}

pub(crate) fn math_floor(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::floor)
}

pub(crate) fn math_ceil(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::ceil)
}

pub(crate) fn math_rint(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::round)
}

pub(crate) fn math_floor_div_int(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(floor_div_i(int_of(vm, args[0]), int_of(vm, args[1]))))
}

pub(crate) fn math_floor_div_long(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(floor_div_l(long_of(vm, args[0]), long_of(vm, args[1]))))
}

pub(crate) fn math_floor_mod_int(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(floor_mod_i(int_of(vm, args[0]), int_of(vm, args[1]))))
}

pub(crate) fn math_floor_mod_long(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(floor_mod_l(long_of(vm, args[0]), long_of(vm, args[1]))))
}

pub(crate) fn math_round_float(vm: &mut Vm, args: &[JValue]) -> R {
    let v = float_of(vm, args[0]);
    if v.is_nan() {
        return Ok(JValue::Int(0));
    }
    if v >= 2_147_483_647.0 {
        return Ok(JValue::Int(i32::MAX));
    }
    if v < -2_147_483_648.0 {
        return Ok(JValue::Int(i32::MIN));
    }
    Ok(JValue::Int((v + 0.5).floor() as i32))
}

pub(crate) fn math_round_double(vm: &mut Vm, args: &[JValue]) -> R {
    let v = double_of(vm, args[0]);
    if v.is_nan() {
        return Ok(JValue::Long(0));
    }
    if v >= 9_223_372_036_854_775_807.0 {
        return Ok(JValue::Long(i64::MAX));
    }
    if v < -9_223_372_036_854_776_000.0 {
        return Ok(JValue::Long(i64::MIN));
    }
    Ok(JValue::Long((v + 0.5).floor() as i64))
}

pub(crate) fn math_signum_float(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(float_of(vm, args[0]).signum()))
}

pub(crate) fn math_signum_double(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(double_of(vm, args[0]).signum()))
}

pub(crate) fn math_random(_vm: &mut Vm, _args: &[JValue]) -> R {
    let bits = next_random_u64() >> 11;
    Ok(JValue::Double((bits as f64) / ((1u64 << 53) as f64)))
}

pub(crate) fn math_sin(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::sin)
}
pub(crate) fn math_cos(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::cos)
}
pub(crate) fn math_tan(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::tan)
}
pub(crate) fn math_asin(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::asin)
}
pub(crate) fn math_acos(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::acos)
}
pub(crate) fn math_atan(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::atan)
}
pub(crate) fn math_atan2(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(double_of(vm, args[0]).atan2(double_of(vm, args[1]))))
}
pub(crate) fn math_to_radians(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::to_radians)
}
pub(crate) fn math_to_degrees(vm: &mut Vm, args: &[JValue]) -> R {
    math_unop_f64(vm, args, f64::to_degrees)
}
pub(crate) fn math_copy_sign_float(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(float_of(vm, args[0]).copysign(float_of(vm, args[1]))))
}
pub(crate) fn math_copy_sign_double(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(double_of(vm, args[0]).copysign(double_of(vm, args[1]))))
}

// ---------------------------------------------------------------------------
