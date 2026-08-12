//! java.io.OutputStream host shims.

use crate::vm::native::*;

use super::byte_array_input_stream::bais_close;

pub(crate) fn output_stream_append(vm: &mut Vm, stream: JValue, bytes: &[u8]) -> R {
    let target = match payload(vm, stream) {
        Some(Native::OkioOutputStream(target)) => Some(*target),
        _ => None,
    };
    if let Some(target) = target {
        match payload_mut(vm, JValue::Obj(target)) {
            Some(Native::OkioBuf { bytes: output, .. }) => output.extend_from_slice(bytes),
            _ => return Err(npe(vm)),
        }
    } else {
        match payload_mut(vm, stream) {
            Some(Native::ByteArrayOutputStream(output)) => output.extend_from_slice(bytes),
            _ => return Err(npe(vm)),
        }
    }
    Ok(JValue::Null)
}

pub(crate) fn output_stream_write_byte(vm: &mut Vm, args: &[JValue]) -> R {
    output_stream_append(vm, args[0], &[int_of(vm, args[1]) as u8])
}

pub(crate) fn output_stream_write_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    output_stream_append(vm, args[0], &bytes)
}

pub(crate) fn output_stream_write_range(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let offset = usize::try_from(int_of(vm, args[2])).unwrap_or(usize::MAX);
    let length = usize::try_from(int_of(vm, args[3])).unwrap_or(usize::MAX);
    let Some(end) = offset.checked_add(length).filter(|end| *end <= bytes.len()) else {
        return Err(iae(vm, "byte range out of bounds"));
    };
    output_stream_append(vm, args[0], &bytes[offset..end])
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/io/OutputStream;",
        "write",
        "(I)V",
        true,
        output_stream_write_byte
    ),
    ne!(
        "Ljava/io/OutputStream;",
        "write",
        "([B)V",
        true,
        output_stream_write_bytes
    ),
    ne!(
        "Ljava/io/OutputStream;",
        "write",
        "([BII)V",
        true,
        output_stream_write_range
    ),
    ne!("Ljava/io/OutputStream;", "flush", "()V", true, bais_close),
    ne!("Ljava/io/OutputStream;", "close", "()V", true, bais_close),
];
