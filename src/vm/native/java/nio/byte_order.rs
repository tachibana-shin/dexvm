//! java.nio.ByteOrder constants: each is a `Native::Str` carrying "BIG" or
//! "LITTLE", which `ByteBuffer.order` reads back.

use crate::vm::native::*;

fn order_const(vm: &mut Vm, tag: &str) -> JValue {
    let Ok(class) = vm.ensure_class_by_desc("Ljava/nio/ByteOrder;") else {
        return JValue::Null;
    };
    JValue::Obj(
        vm.arena
            .alloc(class, Vec::new(), Some(Native::Str(tag.to_string()))),
    )
}

pub fn lazy_big_endian(vm: &mut Vm) -> JValue {
    order_const(vm, "BIG")
}
pub fn lazy_little_endian(vm: &mut Vm) -> JValue {
    order_const(vm, "LITTLE")
}
