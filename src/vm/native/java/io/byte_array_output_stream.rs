//! java.io.ByteArrayOutputStream host shims.

use crate::vm::native::*;

use super::output_stream::{output_stream_write_byte, output_stream_write_range};

fn byte_array_output_stream_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::ByteArrayOutputStream(Vec::new()));
    Ok(JValue::Null)
}

fn byte_array_output_stream_to_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = match payload(vm, args[0]) {
        Some(Native::ByteArrayOutputStream(bytes)) => bytes.clone(),
        _ => return Err(npe(vm)),
    };
    let data = bytes.into_iter().map(|byte| byte as i8).collect::<Vec<_>>();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

fn byte_array_output_stream_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = match payload(vm, args[0]) {
        Some(Native::ByteArrayOutputStream(bytes)) => bytes.clone(),
        _ => return Err(npe(vm)),
    };
    let charset = jstr(vm, args[1]).unwrap_or_default();
    let data: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
    Ok(new_str(vm, &decode_bytes(&data, &charset)))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/io/ByteArrayOutputStream;",
        "<init>",
        "()V",
        true,
        byte_array_output_stream_init
    ),
    ne!(
        "Ljava/io/ByteArrayOutputStream;",
        "write",
        "(I)V",
        true,
        output_stream_write_byte
    ),
    ne!(
        "Ljava/io/ByteArrayOutputStream;",
        "write",
        "([BII)V",
        true,
        output_stream_write_range
    ),
    ne!(
        "Ljava/io/ByteArrayOutputStream;",
        "toByteArray",
        "()[B",
        true,
        byte_array_output_stream_to_bytes
    ),
    ne!(
        "Ljava/io/ByteArrayOutputStream;",
        "toString",
        "(Ljava/lang/String;)Ljava/lang/String;",
        true,
        byte_array_output_stream_to_string
    ),
];
