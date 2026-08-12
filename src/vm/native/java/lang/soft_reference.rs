//! java.lang.ref.SoftReference host shim. This VM never runs under memory
//! pressure the way a real JVM GC would clear a soft reference, so `get()`
//! always returns the referent.

use crate::vm::native::*;

fn soft_reference_init(vm: &mut Vm, args: &[JValue]) -> R {
    let JValue::Obj(id) = args[0] else {
        return Err(npe(vm));
    };
    vm.arena.objects[id as usize].native = Some(Native::Lazy(args[1]));
    Ok(JValue::Null)
}

fn soft_reference_get(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Lazy(v)) => Ok(*v),
        _ => Err(npe(vm)),
    }
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/lang/ref/SoftReference;",
        "<init>",
        "(Ljava/lang/Object;)V",
        true,
        soft_reference_init
    ),
    ne!(
        "Ljava/lang/ref/SoftReference;",
        "get",
        "()Ljava/lang/Object;",
        true,
        soft_reference_get
    ),
];
