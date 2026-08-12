//! java.lang.Number host shims shared by all boxed numeric values.

use crate::vm::native::*;

fn number_int_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0])))
}

fn number_long_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(long_of(vm, args[0])))
}

fn number_float_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(float_of(vm, args[0])))
}

fn number_double_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(double_of(vm, args[0])))
}

fn number_byte_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]) as i8 as i32))
}

fn number_short_value(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0]) as i16 as i32))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/Number;",
        "intValue",
        "()I",
        true,
        number_int_value
    ),
    ne!(
        "Ljava/lang/Number;",
        "longValue",
        "()J",
        true,
        number_long_value
    ),
    ne!(
        "Ljava/lang/Number;",
        "floatValue",
        "()F",
        true,
        number_float_value
    ),
    ne!(
        "Ljava/lang/Number;",
        "doubleValue",
        "()D",
        true,
        number_double_value
    ),
    ne!(
        "Ljava/lang/Number;",
        "byteValue",
        "()B",
        true,
        number_byte_value
    ),
    ne!(
        "Ljava/lang/Number;",
        "shortValue",
        "()S",
        true,
        number_short_value
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::SandboxOptions;

    #[test]
    fn number_conversions_dispatch_for_boxed_values() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut context = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
        let vm = context.vm();
        let value = boxed(vm, "Ljava/lang/Double;", Native::DoubleBox(42.75)).unwrap();
        assert_eq!(number_int_value(vm, &[value]).unwrap().as_int(), 42);
        assert_eq!(number_long_value(vm, &[value]).unwrap().as_long(), 42);
        assert!(matches!(
            number_float_value(vm, &[value]).unwrap(),
            JValue::Float(value) if value == 42.75
        ));
    }
}
