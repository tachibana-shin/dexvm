//! java.io.BufferedReader host shim: this VM has no real streaming, so
//! wrapping a Reader just aliases its already-fully-read `Native::Reader`
//! text (the buffer-size parameter is meaningless here and ignored).

use crate::vm::native::*;

fn buffered_reader_init(vm: &mut Vm, args: &[JValue]) -> R {
    let text = match payload(vm, args[1]) {
        Some(Native::Reader(text)) => text.clone(),
        _ => return Err(npe(vm)),
    };
    let JValue::Obj(id) = args[0] else {
        return Err(npe(vm));
    };
    vm.arena.objects[id as usize].native = Some(Native::Reader(text));
    Ok(JValue::Null)
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/io/BufferedReader;",
        "<init>",
        "(Ljava/io/Reader;)V",
        true,
        buffered_reader_init
    ),
    ne!(
        "Ljava/io/BufferedReader;",
        "<init>",
        "(Ljava/io/Reader;I)V",
        true,
        buffered_reader_init
    ),
];
