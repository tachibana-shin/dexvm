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

/// Reads every remaining byte from any of our eager, in-memory
/// `InputStream`-shaped payloads (no real streaming — everything is
/// already buffered).
fn stream_bytes_of(vm: &Vm, v: JValue) -> Option<Vec<u8>> {
    match payload(vm, v) {
        Some(Native::ByteArrayInputStream { bytes, pos }) => Some(bytes[*pos..].to_vec()),
        Some(Native::Str(s)) => Some(s.as_bytes().to_vec()),
        _ => None,
    }
}

fn read_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = stream_bytes_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let data: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

fn copy_to_default(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = stream_bytes_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let n = bytes.len() as i64;
    let data: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
    let arr = alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))?;
    vm.invoke_virtual_args(args[1], "write", "([B)V", vec![arr])
        .map_err(nat_fatal)?;
    Ok(JValue::Long(n))
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
    ne!(
        "Lkotlin/io/ByteStreamsKt;",
        "readBytes",
        "(Ljava/io/InputStream;)[B",
        false,
        read_bytes
    ),
    ne!(
        "Lkotlin/io/ByteStreamsKt;",
        "copyTo$default",
        "(Ljava/io/InputStream;Ljava/io/OutputStream;IILjava/lang/Object;)J",
        false,
        copy_to_default
    ),
];
