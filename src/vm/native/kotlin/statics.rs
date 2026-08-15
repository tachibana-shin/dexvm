//! Lazy materializers for Kotlin singleton and companion fields.
use super::support::opaque_inst;
use crate::vm::native::*;

pub(crate) fn duration_companion(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/time/Duration$Companion;")
}
pub(crate) fn duration_unit_seconds(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/time/DurationUnit;")
}
pub(crate) fn duration_unit_hours(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/time/DurationUnit;")
}
pub(crate) fn duration_unit_days(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/time/DurationUnit;")
}
pub(crate) fn duration_unit_millis(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/time/DurationUnit;")
}
pub(crate) fn unit_instance(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/Unit;")
}
pub(crate) fn global_scope(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlinx/coroutines/GlobalScope;")
}
pub(crate) fn result_companion(vm: &mut Vm) -> JValue {
    opaque_inst(vm, "Lkotlin/Result$Companion;")
}
