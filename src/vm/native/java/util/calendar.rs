//! java.util.Calendar bridge using deterministic UTC civil-date arithmetic.

use crate::vm::native::*;

const DAY_MS: i64 = 86_400_000;

fn days_from_civil(mut year: i32, month: i32, day: i32) -> i64 {
    year -= i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * shifted_month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i64::from(era * 146_097 + doe - 719_468)
}

fn civil_from_days(days: i64) -> (i32, i32, i32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as i32, day as i32)
}

fn components(millis: i64) -> (i32, i32, i32, i32, i32, i32, i32) {
    let days = millis.div_euclid(DAY_MS);
    let day_ms = millis.rem_euclid(DAY_MS);
    let (year, month, day) = civil_from_days(days);
    let hour = (day_ms / 3_600_000) as i32;
    let minute = (day_ms % 3_600_000 / 60_000) as i32;
    let second = (day_ms % 60_000 / 1_000) as i32;
    let milli = (day_ms % 1_000) as i32;
    (year, month, day, hour, minute, second, milli)
}

fn compose(
    year: i32,
    month0: i32,
    day: i32,
    hour: i32,
    minute: i32,
    second: i32,
    milli: i32,
) -> i64 {
    let year = year + month0.div_euclid(12);
    let month = month0.rem_euclid(12) + 1;
    days_from_civil(year, month, day) * DAY_MS
        + i64::from(hour) * 3_600_000
        + i64::from(minute) * 60_000
        + i64::from(second) * 1_000
        + i64::from(milli)
}

fn calendar_millis(vm: &mut Vm, value: JValue) -> Result<i64, NatErr> {
    match payload(vm, value) {
        Some(Native::Calendar(millis)) => Ok(*millis),
        _ => Err(npe(vm)),
    }
}

fn set_calendar_millis(vm: &mut Vm, value: JValue, millis: i64) -> R {
    match payload_mut(vm, value) {
        Some(Native::Calendar(dst)) => {
            *dst = millis;
            Ok(JValue::Null)
        }
        _ => Err(npe(vm)),
    }
}

fn calendar_get_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/Calendar;", Native::Calendar(now_millis()))
}

fn gregorian_calendar_init(vm: &mut Vm, args: &[JValue]) -> R {
    let JValue::Obj(id) = args[0] else {
        return Err(npe(vm));
    };
    vm.arena.objects[id as usize].native = Some(Native::Calendar(now_millis()));
    Ok(JValue::Null)
}

fn calendar_get_time_in_millis(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(calendar_millis(vm, args[0])?))
}

fn calendar_set_time_in_millis(vm: &mut Vm, args: &[JValue]) -> R {
    set_calendar_millis(vm, args[0], long_of(vm, args[1]))
}

fn calendar_get_time(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = calendar_millis(vm, args[0])?;
    alloc(vm, "Ljava/util/Date;", Native::Date(millis))
}

fn calendar_set_time(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = match payload(vm, args[1]) {
        Some(Native::Date(millis)) => *millis,
        _ => return Err(npe(vm)),
    };
    set_calendar_millis(vm, args[0], millis)
}

fn calendar_get(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = calendar_millis(vm, args[0])?;
    let field = int_of(vm, args[1]);
    let (year, month, day, hour, minute, second, milli) = components(millis);
    let days = millis.div_euclid(DAY_MS);
    let value = match field {
        1 => year,
        2 => month - 1,
        5 => day,
        6 => (days - days_from_civil(year, 1, 1) + 1) as i32,
        7 => (days + 4).rem_euclid(7) as i32 + 1,
        9 => i32::from(hour >= 12),
        10 => hour % 12,
        11 => hour,
        12 => minute,
        13 => second,
        14 => milli,
        15 | 16 => 0,
        _ => 0,
    };
    Ok(JValue::Int(value))
}

fn calendar_set(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = calendar_millis(vm, args[0])?;
    let field = int_of(vm, args[1]);
    let value = int_of(vm, args[2]);
    let (mut year, mut month, mut day, mut hour, mut minute, mut second, mut milli) =
        components(millis);
    match field {
        1 => year = value,
        2 => month = value + 1,
        5 => day = value,
        9 => hour = hour % 12 + value * 12,
        10 => hour = hour / 12 * 12 + value,
        11 => hour = value,
        12 => minute = value,
        13 => second = value,
        14 => milli = value,
        _ => return Ok(JValue::Null),
    }
    set_calendar_millis(
        vm,
        args[0],
        compose(year, month - 1, day, hour, minute, second, milli),
    )
}

fn calendar_set_fields(vm: &mut Vm, args: &[JValue]) -> R {
    let old = calendar_millis(vm, args[0])?;
    let milli = components(old).6;
    let millis = compose(
        int_of(vm, args[1]),
        int_of(vm, args[2]),
        int_of(vm, args[3]),
        int_of(vm, args[4]),
        int_of(vm, args[5]),
        int_of(vm, args[6]),
        milli,
    );
    set_calendar_millis(vm, args[0], millis)
}

fn calendar_add(vm: &mut Vm, args: &[JValue]) -> R {
    let millis = calendar_millis(vm, args[0])?;
    let field = int_of(vm, args[1]);
    let amount = int_of(vm, args[2]);
    let (year, month, day, hour, minute, second, milli) = components(millis);
    let updated = match field {
        1 => compose(year + amount, month - 1, day, hour, minute, second, milli),
        2 => compose(year, month - 1 + amount, day, hour, minute, second, milli),
        3 | 4 => millis + i64::from(amount) * 7 * DAY_MS,
        5..=7 => millis + i64::from(amount) * DAY_MS,
        10 | 11 => millis + i64::from(amount) * 3_600_000,
        12 => millis + i64::from(amount) * 60_000,
        13 => millis + i64::from(amount) * 1_000,
        14 => millis + i64::from(amount),
        _ => millis,
    };
    set_calendar_millis(vm, args[0], updated)
}

fn calendar_get_display_name(vm: &mut Vm, args: &[JValue]) -> R {
    let field = int_of(vm, args[1]);
    let field_value = calendar_get(vm, &[args[0], args[1]])?;
    let value = int_of(vm, field_value);
    let name = match field {
        2 => [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ]
        .get(value as usize)
        .copied(),
        7 => [
            "",
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ]
        .get(value as usize)
        .copied(),
        _ => None,
    };
    Ok(name.map_or(JValue::Null, |name| new_str(vm, name)))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/Calendar;",
        "getInstance",
        "()Ljava/util/Calendar;",
        false,
        calendar_get_instance
    ),
    ne!(
        "Ljava/util/Calendar;",
        "getInstance",
        "(Ljava/util/TimeZone;)Ljava/util/Calendar;",
        false,
        calendar_get_instance
    ),
    ne!(
        "Ljava/util/Calendar;",
        "getInstance",
        "(Ljava/util/Locale;)Ljava/util/Calendar;",
        false,
        calendar_get_instance
    ),
    ne!(
        "Ljava/util/Calendar;",
        "getInstance",
        "(Ljava/util/TimeZone;Ljava/util/Locale;)Ljava/util/Calendar;",
        false,
        calendar_get_instance
    ),
    ne!(
        "Ljava/util/Calendar;",
        "getTimeInMillis",
        "()J",
        true,
        calendar_get_time_in_millis
    ),
    ne!(
        "Ljava/util/Calendar;",
        "setTimeInMillis",
        "(J)V",
        true,
        calendar_set_time_in_millis
    ),
    ne!(
        "Ljava/util/Calendar;",
        "getTime",
        "()Ljava/util/Date;",
        true,
        calendar_get_time
    ),
    ne!(
        "Ljava/util/Calendar;",
        "setTime",
        "(Ljava/util/Date;)V",
        true,
        calendar_set_time
    ),
    ne!("Ljava/util/Calendar;", "get", "(I)I", true, calendar_get),
    ne!("Ljava/util/Calendar;", "set", "(II)V", true, calendar_set),
    ne!(
        "Ljava/util/Calendar;",
        "set",
        "(IIIIII)V",
        true,
        calendar_set_fields
    ),
    ne!("Ljava/util/Calendar;", "add", "(II)V", true, calendar_add),
    ne!(
        "Ljava/util/Calendar;",
        "getDisplayName",
        "(IILjava/util/Locale;)Ljava/lang/String;",
        true,
        calendar_get_display_name
    ),
    ne!(
        "Ljava/util/GregorianCalendar;",
        "<init>",
        "()V",
        true,
        gregorian_calendar_init
    ),
    ne!(
        "Ljava/util/GregorianCalendar;",
        "<init>",
        "(Ljava/util/TimeZone;)V",
        true,
        gregorian_calendar_init
    ),
    ne!(
        "Ljava/util/GregorianCalendar;",
        "add",
        "(II)V",
        true,
        calendar_add
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrip_handles_leap_days_and_pre_epoch_dates() {
        for date in [(1970, 1, 1), (2024, 2, 29), (1969, 12, 31), (2000, 1, 1)] {
            assert_eq!(
                civil_from_days(days_from_civil(date.0, date.1, date.2)),
                date
            );
        }
    }
}
