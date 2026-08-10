//! okio host shims: in-memory `BufferedSource`/`Buffer` over a byte cursor.

use super::*;
use crate::vm::native::okhttp::resp_body_bytes;

pub(crate) fn okio_source_input_stream(vm: &mut Vm, args: &[JValue]) -> R {
    let (bytes, pos) = match payload(vm, args[1]) {
        Some(Native::ByteArrayInputStream { bytes, pos }) => (bytes.clone(), *pos),
        Some(Native::Str(s)) => (s.as_bytes().to_vec(), 0),
        _ => return Err(npe(vm)),
    };
    alloc(vm, "Lokio/BufferedSource;", Native::OkioBuf { bytes, pos })
}

pub(crate) fn okio_source_response_body(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = resp_body_bytes(vm, args[0])?;
    alloc(
        vm,
        "Lokio/BufferedSource;",
        Native::OkioBuf { bytes, pos: 0 },
    )
}

/// `source()` is already a BufferedSource for our in-memory payloads.
pub(crate) fn okio_identity(vm: &mut Vm, args: &[JValue]) -> R {
    let _ = vm;
    Ok(args[1])
}

pub(crate) fn okio_close(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn okio_get_buffer(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::OkioBuf { bytes, pos }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let (bytes, pos) = (bytes.clone(), *pos);
    alloc(vm, "Lokio/Buffer;", Native::OkioBuf { bytes, pos })
}

pub(crate) fn okio_request(vm: &mut Vm, args: &[JValue]) -> R {
    let want = int_of(vm, args[1]).max(0) as usize;
    let Some(Native::OkioBuf { bytes, pos }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(pos + want <= bytes.len())))
}

pub(crate) fn okio_read_byte_array(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::OkioBuf { bytes, pos }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let data = bytes[*pos..].iter().map(|&b| b as i8).collect::<Vec<_>>();
    *pos = bytes.len();
    let len = data.len();
    alloc_arr(vm, "B", len, move || ArrayData::Byte(data))
}

pub(crate) fn okio_read_utf8(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::OkioBuf { bytes, pos }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let s = String::from_utf8_lossy(&bytes[*pos..]).into_owned();
    *pos = bytes.len();
    Ok(new_str(vm, &s))
}

pub(crate) fn okio_exhausted(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::OkioBuf { bytes, pos }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(i32::from(*pos >= bytes.len())))
}

/// okio.Buffer.get(long): indexed byte access, relative to the unread cursor.
pub(crate) fn okio_buffer_get(vm: &mut Vm, args: &[JValue]) -> R {
    let i = match args[1] {
        JValue::Long(n) => n,
        JValue::Int(n) => n as i64,
        _ => return Err(iae(vm, "invalid index")),
    };
    let Some(Native::OkioBuf { bytes, pos }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let idx = *pos as i64 + i;
    match bytes.get(idx as usize) {
        Some(&b) => Ok(JValue::Int(b as i8 as i32)),
        None => Err(iae(vm, format!("index {i} out of bounds"))),
    }
}

pub(crate) const OKIO_TABLE: &[NativeEntry] = &[
    ne!(
        "Lokio/Okio;",
        "source",
        "(Ljava/io/InputStream;)Lokio/Source;",
        false,
        okio_source_input_stream
    ),
    ne!(
        "Lokio/Okio;",
        "buffer",
        "(Lokio/Source;)Lokio/BufferedSource;",
        false,
        okio_identity
    ),
    ne!(
        "Lokio/ByteStreams;",
        "source",
        "(Ljava/io/InputStream;)Lokio/Source;",
        false,
        okio_source_input_stream
    ),
    ne!(
        "Lokio/ByteStreams;",
        "buffer",
        "(Lokio/Source;)Lokio/BufferedSource;",
        false,
        okio_identity
    ),
    ne!(
        "Lokio/ByteStreamsKt;",
        "source",
        "(Ljava/io/InputStream;)Lokio/Source;",
        false,
        okio_source_input_stream
    ),
    ne!(
        "Lokio/ByteStreamsKt;",
        "buffer",
        "(Lokio/Source;)Lokio/BufferedSource;",
        false,
        okio_identity
    ),
    ne!(
        "Lokio/BufferedSource;",
        "getBuffer",
        "()Lokio/Buffer;",
        true,
        okio_get_buffer
    ),
    ne!(
        "Lokio/BufferedSource;",
        "request",
        "(J)Z",
        true,
        okio_request
    ),
    ne!(
        "Lokio/BufferedSource;",
        "readByteArray",
        "()[B",
        true,
        okio_read_byte_array
    ),
    ne!(
        "Lokio/BufferedSource;",
        "readUtf8",
        "()Ljava/lang/String;",
        true,
        okio_read_utf8
    ),
    ne!(
        "Lokio/BufferedSource;",
        "exhausted",
        "()Z",
        true,
        okio_exhausted
    ),
    ne!("Lokio/Buffer;", "get", "(J)B", true, okio_buffer_get),
    ne!("Lokio/Buffer;", "getByte", "(J)B", true, okio_buffer_get),
    ne!(
        "Lokio/Buffer;",
        "readByteArray",
        "()[B",
        true,
        okio_read_byte_array
    ),
    ne!("Lokio/Buffer;", "close", "()V", true, okio_close),
    ne!("Lokio/Source;", "close", "()V", true, okio_close),
];
