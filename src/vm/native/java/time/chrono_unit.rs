//! java.time.temporal.ChronoUnit constants: each is represented as a plain
//! `Native::Str` carrying the enum constant name, which `truncatedTo` reads
//! back to pick a granularity. Good enough for the fixed-offset, no-DST
//! time model the rest of `java.time` uses.

use crate::vm::native::*;

macro_rules! chrono_unit_const {
    ($name:ident, $tag:expr) => {
        pub fn $name(vm: &mut Vm) -> JValue {
            let Ok(class) = vm.ensure_class_by_desc("Ljava/time/temporal/ChronoUnit;") else {
                return JValue::Null;
            };
            JValue::Obj(
                vm.arena
                    .alloc(class, Vec::new(), Some(Native::Str($tag.into()))),
            )
        }
    };
}

chrono_unit_const!(millis, "MILLIS");
chrono_unit_const!(seconds, "SECONDS");
chrono_unit_const!(minutes, "MINUTES");
chrono_unit_const!(hours, "HOURS");
chrono_unit_const!(days, "DAYS");
chrono_unit_const!(weeks, "WEEKS");
chrono_unit_const!(months, "MONTHS");
chrono_unit_const!(years, "YEARS");
