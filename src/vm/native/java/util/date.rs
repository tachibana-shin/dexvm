//! java.util.Date host shims.

use crate::vm::native::*;

// java.util.Date
// ---------------------------------------------------------------------------

pub(crate) fn date_init(vm: &mut Vm, args: &[JValue]) -> R {
    let t = now_millis();
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Date(dst) => *dst = t,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn date_init_ms(vm: &mut Vm, args: &[JValue]) -> R {
    let t = long_of(vm, args[1]);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Date(dst) => *dst = t,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn date_get_time(vm: &mut Vm, args: &[JValue]) -> R {
    let t = match payload(vm, args[0]) {
        Some(Native::Date(t)) => *t,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Long(t))
}

pub(crate) fn date_set_time(vm: &mut Vm, args: &[JValue]) -> R {
    let t = long_of(vm, args[1]);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Date(dst) => *dst = t,
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn date_millis(vm: &mut Vm, v: JValue) -> Result<i64, NatErr> {
    match payload(vm, v) {
        Some(Native::Date(t)) => Ok(*t),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn date_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let t = date_millis(vm, args[0])?;
    Ok(new_str(vm, &format!("java.util.Date({t})")))
}

pub(crate) fn date_after(vm: &mut Vm, args: &[JValue]) -> R {
    let a = date_millis(vm, args[0])?;
    let b = date_millis(vm, args[1])?;
    Ok(JValue::Int(i32::from(a > b)))
}

pub(crate) fn date_before(vm: &mut Vm, args: &[JValue]) -> R {
    let a = date_millis(vm, args[0])?;
    let b = date_millis(vm, args[1])?;
    Ok(JValue::Int(i32::from(a < b)))
}

pub(crate) fn date_equals(vm: &mut Vm, args: &[JValue]) -> R {
    let a = date_millis(vm, args[0])?;
    match payload(vm, args[1]) {
        Some(Native::Date(b)) => Ok(JValue::Int(i32::from(a == *b))),
        _ => Ok(JValue::Int(0)),
    }
}

pub(crate) fn date_compare_to(vm: &mut Vm, args: &[JValue]) -> R {
    let a = date_millis(vm, args[0])?;
    let b = date_millis(vm, args[1])?;
    Ok(JValue::Int(a.cmp(&b) as i32))
}


/// Native methods for Ljava/util/Date;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/util/Date;", "<init>", "()V", true, date_init),
    ne!("Ljava/util/Date;", "<init>", "(J)V", true, date_init_ms),
    ne!("Ljava/util/Date;", "getTime", "()J", true, date_get_time),
    ne!("Ljava/util/Date;", "setTime", "(J)V", true, date_set_time),
    ne!("Ljava/util/Date;", "toString", "()Ljava/lang/String;", true, date_to_string),
    ne!("Ljava/util/Date;", "after", "(Ljava/util/Date;)Z", true, date_after),
    ne!("Ljava/util/Date;", "before", "(Ljava/util/Date;)Z", true, date_before),
    ne!("Ljava/util/Date;", "equals", "(Ljava/lang/Object;)Z", true, date_equals),
    ne!("Ljava/util/Date;", "compareTo", "(Ljava/util/Date;)I", true, date_compare_to),
    ne!("Ljava/util/Date;", "compareTo", "(Ljava/lang/Object;)I", true, date_compare_to),
];
