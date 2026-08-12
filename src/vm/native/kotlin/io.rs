//! Kotlin I/O extension bridges.
use crate::vm::native::*;

fn read_text(vm: &mut Vm, args: &[JValue]) -> R {
    let reader = args[0];
    if reader.is_null() {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/IllegalStateException;",
            "null Reader in readText",
        )));
    }
    if let Some(Native::Reader(text)) = payload(vm, reader) {
        return alloc(vm, "Ljava/lang/String;", Native::Str(text.clone()));
    }
    const CAP: i32 = 4096;
    let buffer = alloc_arr(vm, "C", CAP as usize, || {
        ArrayData::Char(vec![0; CAP as usize])
    })?;
    let mut output = Vec::new();
    loop {
        let count = vm
            .invoke_virtual_args(
                reader,
                "read",
                "([CII)I",
                vec![buffer, JValue::Int(0), JValue::Int(CAP)],
            )
            .map_err(nat_fatal)?;
        let count = int_of(vm, count);
        if count <= 0 {
            break;
        }
        if let Some(Native::Array(ArrayData::Char(chars))) = payload(vm, buffer) {
            output.extend_from_slice(&chars[..count as usize]);
        }
    }
    alloc(
        vm,
        "Ljava/lang/String;",
        Native::Str(String::from_utf16_lossy(&output)),
    )
}

fn close_finally(vm: &mut Vm, args: &[JValue]) -> R {
    if args[0].is_null() {
        return Ok(JValue::Null);
    }
    let result = vm
        .invoke_virtual_args(args[0], "close", "()V", vec![])
        .map_err(nat_fatal);
    if args[1].is_null() {
        result
    } else {
        Ok(JValue::Null)
    }
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Lkotlin/io/CloseableKt;",
        "closeFinally",
        "(Ljava/io/Closeable;Ljava/lang/Throwable;)V",
        false,
        close_finally
    ),
    ne!(
        "Lkotlin/io/TextStreamsKt;",
        "readText",
        "(Ljava/io/Reader;)Ljava/lang/String;",
        false,
        read_text
    ),
];
