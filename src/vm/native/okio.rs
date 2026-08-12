//! okio host shims: in-memory `BufferedSource`/`Buffer` over a byte cursor.

use super::*;
use crate::permission::{FilesystemPermission, Permission};
use crate::vm::native::okhttp::resp_body_bytes;
use base64::engine::general_purpose::STANDARD as BASE64_STD;
use base64::Engine as _;
use sha2::{Digest, Sha256, Sha512};
use std::io::Write as _;

// ---- okio.ByteString: an immutable byte sequence, backed by a plain byte array ----

fn byte_string_alloc(vm: &mut Vm, bytes: Vec<u8>) -> R {
    let data: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
    alloc(vm, "Lokio/ByteString;", Native::Array(ArrayData::Byte(data)))
}

fn byte_string_decode_base64(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    match BASE64_STD.decode(s.trim()) {
        Ok(bytes) => byte_string_alloc(vm, bytes),
        Err(_) => Ok(JValue::Null),
    }
}

fn byte_string_encode_utf8(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    byte_string_alloc(vm, s.into_bytes())
}

fn byte_string_bytes(vm: &Vm, args: &[JValue]) -> Option<Vec<u8>> {
    match payload(vm, args[0]) {
        Some(Native::Array(ArrayData::Byte(bs))) => Some(bs.iter().map(|&b| b as u8).collect()),
        _ => None,
    }
}

fn byte_string_hex(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = byte_string_bytes(vm, args).ok_or_else(|| npe(vm))?;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(new_str(vm, &hex))
}

fn byte_string_to_byte_array(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = byte_string_bytes(vm, args).ok_or_else(|| npe(vm))?;
    let data: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

fn byte_string_sha256(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = byte_string_bytes(vm, args).ok_or_else(|| npe(vm))?;
    byte_string_alloc(vm, Sha256::digest(&bytes).to_vec())
}

fn byte_string_sha512(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = byte_string_bytes(vm, args).ok_or_else(|| npe(vm))?;
    byte_string_alloc(vm, Sha512::digest(&bytes).to_vec())
}

fn byte_string_get_byte(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = byte_string_bytes(vm, args).ok_or_else(|| npe(vm))?;
    let i = int_of(vm, args[1]);
    match bytes.get(i as usize) {
        Some(&b) => Ok(JValue::Int(b as i8 as i32)),
        None => Err(iae(vm, format!("index {i} out of bounds"))),
    }
}

fn byte_string_size(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = byte_string_bytes(vm, args).ok_or_else(|| npe(vm))?;
    Ok(JValue::Int(bytes.len() as i32))
}

fn byte_string_utf8(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = byte_string_bytes(vm, args).ok_or_else(|| npe(vm))?;
    let s = String::from_utf8_lossy(&bytes).into_owned();
    Ok(new_str(vm, &s))
}

pub(crate) fn okio_source_input_stream(vm: &mut Vm, args: &[JValue]) -> R {
    let (bytes, pos) = match payload(vm, args[0]) {
        Some(Native::ByteArrayInputStream { bytes, pos }) => (bytes.clone(), *pos),
        Some(Native::Str(s)) => (s.as_bytes().to_vec(), 0),
        _ => return Err(npe(vm)),
    };
    alloc(vm, "Lokio/BufferedSource;", Native::OkioBuf { bytes, pos })
}

/// `Okio.source(File)` — reads the file's real bytes; throws
/// `java.io.FileNotFoundException` when the file does not exist, exactly
/// like okio. Callers rely on the exception to fall back to the network.
pub(crate) fn okio_source_file(vm: &mut Vm, args: &[JValue]) -> R {
    let path = match payload(vm, args[0]) {
        Some(Native::File { path }) => path.clone(),
        _ => return Err(npe(vm)),
    };
    check_native_permission(
        vm,
        &Permission::Filesystem(FilesystemPermission::ReadPath(path.clone())),
    )?;
    let bytes =
        std::fs::read(&path).map_err(|_| fnf(vm, format!("{path} (No such file or directory)")))?;
    alloc(
        vm,
        "Lokio/BufferedSource;",
        Native::OkioBuf { bytes, pos: 0 },
    )
}

pub(crate) fn okio_source_response_body(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = resp_body_bytes(vm, args[0])?;
    alloc(
        vm,
        "Lokio/BufferedSource;",
        Native::OkioBuf { bytes, pos: 0 },
    )
}

/// Reads the unread bytes of any of our eager, in-memory `Source`-shaped
/// payloads (this VM has no real streaming — every source is fully
/// buffered up front).
fn okio_bytes_of(vm: &Vm, v: JValue) -> Option<Vec<u8>> {
    match payload(vm, v) {
        Some(Native::OkioBuf { bytes, pos }) => Some(bytes[*pos..].to_vec()),
        Some(Native::ByteArrayInputStream { bytes, pos }) => Some(bytes[*pos..].to_vec()),
        Some(Native::Str(s)) => Some(s.as_bytes().to_vec()),
        _ => None,
    }
}

/// `InflaterSource(source, inflater)`: eagerly reads and decompresses the
/// wrapped source, then behaves exactly like any other in-memory
/// `BufferedSource` from then on.
pub(crate) fn okio_inflater_source_init(vm: &mut Vm, args: &[JValue]) -> R {
    let compressed = okio_bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let nowrap = matches!(payload(vm, args[2]), Some(Native::Inflater { nowrap: true, .. }));
    let decompressed = if nowrap {
        miniz_oxide::inflate::decompress_to_vec(&compressed)
    } else {
        miniz_oxide::inflate::decompress_to_vec_zlib(&compressed)
    }
    .map_err(|_| ioe(vm, "invalid deflate data"))?;
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::OkioBuf {
        bytes: decompressed,
        pos: 0,
    });
    Ok(JValue::Null)
}

/// `Okio.cipherSource(source, cipher)`: eagerly reads the wrapped source and
/// runs it through the already-initialized `Cipher` (decrypt or encrypt,
/// whichever mode the caller set up), yielding another in-memory source.
pub(crate) fn okio_cipher_source(vm: &mut Vm, args: &[JValue]) -> R {
    let input = okio_bytes_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let len = input.len() as i32;
    let data: Vec<i8> = input.iter().map(|&b| b as i8).collect();
    let arr = alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))?;
    let out = crate::vm::native::java::javax_crypto::cipher_do_final(
        vm,
        &[args[1], arr, JValue::Int(0), JValue::Int(len)],
    )?;
    let bytes = bytes_of(vm, out).unwrap_or_default();
    alloc(vm, "Lokio/CipherSource;", Native::OkioBuf { bytes, pos: 0 })
}

/// `ForwardingSource(delegate)`: this VM has no partial reads to forward
/// through, so it's equivalent to just aliasing the delegate's buffer.
pub(crate) fn okio_forwarding_source_init(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = okio_bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::OkioBuf { bytes, pos: 0 });
    Ok(JValue::Null)
}

/// `ForwardingSource.read(sink, byteCount)`: drains up to `byteCount` bytes
/// into `sink`'s buffer (both share the same in-memory `OkioBuf` shape).
pub(crate) fn okio_forwarding_source_read(vm: &mut Vm, args: &[JValue]) -> R {
    let want = long_of(vm, args[2]).max(0) as usize;
    let chunk = match payload_mut(vm, args[0]) {
        Some(Native::OkioBuf { bytes, pos }) => {
            let n = (bytes.len() - *pos).min(want);
            let chunk = bytes[*pos..*pos + n].to_vec();
            *pos += n;
            chunk
        }
        _ => return Err(npe(vm)),
    };
    if chunk.is_empty() {
        return Ok(JValue::Long(-1));
    }
    let n = chunk.len() as i64;
    match payload_mut(vm, args[1]) {
        Some(Native::OkioBuf { bytes, .. }) => bytes.extend_from_slice(&chunk),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Long(n))
}

fn file_arg_path(vm: &mut Vm, arg: JValue) -> Result<String, NatErr> {
    match payload(vm, arg) {
        Some(Native::File { path }) => Ok(path.clone()),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn okio_sink_file(vm: &mut Vm, args: &[JValue]) -> R {
    let path = file_arg_path(vm, args[0])?;
    check_native_permission(
        vm,
        &Permission::Filesystem(FilesystemPermission::WritePath(path.clone())),
    )?;
    let append = args.get(1).is_some_and(|v| int_of(vm, *v) != 0);
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(!append)
        .append(append)
        .open(&path)
        .map_err(|e| fnf(vm, e.to_string()))?;
    alloc(
        vm,
        "Lokio/BufferedSink;",
        Native::OkioSink {
            path,
            bytes: Vec::new(),
            flushed: 0,
            closed: false,
        },
    )
}

/// `source()` is already a BufferedSource for our in-memory payloads.
pub(crate) fn okio_identity(vm: &mut Vm, args: &[JValue]) -> R {
    let _ = vm;
    Ok(args[0])
}

fn flush_sink(vm: &mut Vm, sink: JValue) -> Result<(), NatErr> {
    let Some(Native::OkioSink {
        path,
        bytes,
        flushed,
        closed,
    }) = payload(vm, sink)
    else {
        return Ok(());
    };
    let (path, pending, start, closed) = (path.clone(), bytes.clone(), *flushed, *closed);
    if closed || start >= pending.len() {
        return Ok(());
    }
    check_native_permission(
        vm,
        &Permission::Filesystem(FilesystemPermission::WritePath(path.clone())),
    )?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| fnf(vm, e.to_string()))?;
    file.write_all(&pending[start..])
        .map_err(|e| ioe(vm, e.to_string()))?;
    if let Some(Native::OkioSink { flushed, .. }) = payload_mut(vm, sink) {
        *flushed = pending.len();
    }
    Ok(())
}

pub(crate) fn okio_flush(vm: &mut Vm, args: &[JValue]) -> R {
    flush_sink(vm, args[0])?;
    Ok(JValue::Null)
}

pub(crate) fn okio_close(vm: &mut Vm, args: &[JValue]) -> R {
    flush_sink(vm, args[0])?;
    if let Some(Native::OkioSink { closed, .. }) = payload_mut(vm, args[0]) {
        *closed = true;
    }
    Ok(JValue::Null)
}

pub(crate) fn okio_sink_write_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let data = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let (offset, length) = if args.len() >= 4 {
        let offset = usize::try_from(int_of(vm, args[2])).unwrap_or(usize::MAX);
        let length = usize::try_from(int_of(vm, args[3])).unwrap_or(usize::MAX);
        (offset, length)
    } else {
        (0, data.len())
    };
    let Some(end) = offset.checked_add(length).filter(|end| *end <= data.len()) else {
        return Err(iae(vm, "byte range out of bounds"));
    };
    let Some(Native::OkioSink { bytes, closed, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if *closed {
        return Err(ioe(vm, "closed"));
    }
    bytes.extend_from_slice(&data[offset..end]);
    Ok(args[0])
}

pub(crate) fn okio_sink_write_utf8(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[1])?;
    let Some(Native::OkioSink { bytes, closed, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if *closed {
        return Err(ioe(vm, "closed"));
    }
    bytes.extend_from_slice(value.as_bytes());
    Ok(args[0])
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

fn okio_buffer_size(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::OkioBuf { bytes, pos }) => Ok(JValue::Long((bytes.len() - pos) as i64)),
        _ => Err(npe(vm)),
    }
}

fn okio_buffer_write(vm: &mut Vm, args: &[JValue]) -> R {
    let input = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    match payload_mut(vm, args[0]) {
        Some(Native::OkioBuf { bytes, .. }) => bytes.extend_from_slice(&input),
        _ => return Err(npe(vm)),
    }
    Ok(args[0])
}

fn okio_buffer_write_all(vm: &mut Vm, args: &[JValue]) -> R {
    let source = okio_bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let n = source.len() as i64;
    match payload_mut(vm, args[0]) {
        Some(Native::OkioBuf { bytes, .. }) => bytes.extend_from_slice(&source),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Long(n))
}

fn okio_buffer_read_into(vm: &mut Vm, args: &[JValue]) -> R {
    let want = long_of(vm, args[2]).max(0) as usize;
    let chunk = match payload_mut(vm, args[1]) {
        Some(Native::OkioBuf { bytes, pos }) => {
            let n = (bytes.len() - *pos).min(want);
            let chunk = bytes[*pos..*pos + n].to_vec();
            *pos += n;
            chunk
        }
        _ => return Err(npe(vm)),
    };
    if chunk.is_empty() {
        return Ok(JValue::Long(-1));
    }
    let n = chunk.len() as i64;
    match payload_mut(vm, args[0]) {
        Some(Native::OkioBuf { bytes, .. }) => bytes.extend_from_slice(&chunk),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Long(n))
}

fn okio_buffer_write_byte(vm: &mut Vm, args: &[JValue]) -> R {
    let b = int_of(vm, args[1]) as u8;
    match payload_mut(vm, args[0]) {
        Some(Native::OkioBuf { bytes, .. }) => bytes.push(b),
        _ => return Err(npe(vm)),
    }
    Ok(args[0])
}

fn okio_buffer_read_byte(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::OkioBuf { bytes, pos }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    if *pos >= bytes.len() {
        return Err(NatErr::Throw(
            vm.throwable_of("Ljava/io/EOFException;", "end of buffer"),
        ));
    }
    let b = bytes[*pos];
    *pos += 1;
    Ok(JValue::Int(b as i8 as i32))
}

fn okio_buffer_output_stream(vm: &mut Vm, args: &[JValue]) -> R {
    let buffer = args[0].as_obj();
    alloc(
        vm,
        "Ljava/io/OutputStream;",
        Native::OkioOutputStream(buffer),
    )
}

pub(crate) const OKIO_TABLE: &[NativeEntry] = &[
    ne!(
        "Lokio/ByteString$Companion;",
        "decodeBase64",
        "(Ljava/lang/String;)Lokio/ByteString;",
        true,
        byte_string_decode_base64
    ),
    ne!(
        "Lokio/ByteString$Companion;",
        "encodeUtf8",
        "(Ljava/lang/String;)Lokio/ByteString;",
        true,
        byte_string_encode_utf8
    ),
    ne!("Lokio/ByteString;", "hex", "()Ljava/lang/String;", true, byte_string_hex),
    ne!("Lokio/ByteString;", "toByteArray", "()[B", true, byte_string_to_byte_array),
    ne!("Lokio/ByteString;", "sha256", "()Lokio/ByteString;", true, byte_string_sha256),
    ne!("Lokio/ByteString;", "sha512", "()Lokio/ByteString;", true, byte_string_sha512),
    ne!("Lokio/ByteString;", "getByte", "(I)B", true, byte_string_get_byte),
    ne!("Lokio/ByteString;", "size", "()I", true, byte_string_size),
    ne!("Lokio/ByteString;", "utf8", "()Ljava/lang/String;", true, byte_string_utf8),
    ne!(
        "Lokio/Okio;",
        "source",
        "(Ljava/io/InputStream;)Lokio/Source;",
        false,
        okio_source_input_stream
    ),
    ne!(
        "Lokio/Okio;",
        "source",
        "(Ljava/io/File;)Lokio/Source;",
        false,
        okio_source_file
    ),
    ne!(
        "Lokio/Okio;",
        "sink",
        "(Ljava/io/File;)Lokio/Sink;",
        false,
        okio_sink_file
    ),
    ne!(
        "Lokio/Okio;",
        "sink",
        "(Ljava/io/File;Z)Lokio/Sink;",
        false,
        okio_sink_file
    ),
    ne!(
        "Lokio/Okio;",
        "sink$default",
        "(Ljava/io/File;ZILjava/lang/Object;)Lokio/Sink;",
        false,
        okio_sink_file
    ),
    ne!(
        "Lokio/Okio;",
        "buffer",
        "(Lokio/Source;)Lokio/BufferedSource;",
        false,
        okio_identity
    ),
    ne!(
        "Lokio/Okio;",
        "buffer",
        "(Lokio/Sink;)Lokio/BufferedSink;",
        false,
        okio_identity
    ),
    ne!(
        "Lokio/Okio;",
        "cipherSource",
        "(Lokio/Source;Ljavax/crypto/Cipher;)Lokio/CipherSource;",
        false,
        okio_cipher_source
    ),
    ne!(
        "Lokio/InflaterSource;",
        "<init>",
        "(Lokio/Source;Ljava/util/zip/Inflater;)V",
        true,
        okio_inflater_source_init
    ),
    ne!(
        "Lokio/ForwardingSource;",
        "<init>",
        "(Lokio/Source;)V",
        true,
        okio_forwarding_source_init
    ),
    ne!(
        "Lokio/ForwardingSource;",
        "read",
        "(Lokio/Buffer;J)J",
        true,
        okio_forwarding_source_read
    ),
    ne!("Lokio/Buffer;", "writeAll", "(Lokio/Source;)J", true, okio_buffer_write_all),
    ne!("Lokio/Buffer;", "read", "(Lokio/Buffer;J)J", true, okio_buffer_read_into),
    ne!("Lokio/Buffer;", "writeByte", "(I)Lokio/Buffer;", true, okio_buffer_write_byte),
    ne!("Lokio/Buffer;", "readByte", "()B", true, okio_buffer_read_byte),
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
    ne!("Lokio/Buffer;", "size", "()J", true, okio_buffer_size),
    ne!(
        "Lokio/Buffer;",
        "write",
        "([B)Lokio/Buffer;",
        true,
        okio_buffer_write
    ),
    ne!(
        "Lokio/Buffer;",
        "outputStream",
        "()Ljava/io/OutputStream;",
        true,
        okio_buffer_output_stream
    ),
    ne!(
        "Lokio/Buffer;",
        "readByteArray",
        "()[B",
        true,
        okio_read_byte_array
    ),
    ne!("Lokio/Buffer;", "close", "()V", true, okio_close),
    ne!("Lokio/Source;", "close", "()V", true, okio_close),
    ne!("Lokio/Sink;", "close", "()V", true, okio_close),
    ne!("Lokio/BufferedSink;", "close", "()V", true, okio_close),
    ne!("Lokio/BufferedSink;", "flush", "()V", true, okio_flush),
    ne!(
        "Lokio/BufferedSink;",
        "write",
        "([B)Lokio/BufferedSink;",
        true,
        okio_sink_write_bytes
    ),
    ne!(
        "Lokio/BufferedSink;",
        "write",
        "([BII)Lokio/BufferedSink;",
        true,
        okio_sink_write_bytes
    ),
    ne!(
        "Lokio/BufferedSink;",
        "writeUtf8",
        "(Ljava/lang/String;)Lokio/BufferedSink;",
        true,
        okio_sink_write_utf8
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    #[test]
    fn file_sink_writes_and_enforces_permission() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut ctx = Context::new(&data).unwrap();
        let vm = ctx.vm();
        let root = vm.cache_root_path().to_owned();
        let path = format!("{root}/out.bin");
        let file = alloc(vm, "Ljava/io/File;", Native::File { path: path.clone() }).unwrap();
        assert!(matches!(okio_sink_file(vm, &[file]), Err(NatErr::Throw(_))));

        vm.perms
            .grant(Permission::Filesystem(FilesystemPermission::Path(
                root.clone(),
            )));
        std::fs::create_dir_all(&root).unwrap();
        let sink = okio_sink_file(vm, &[file]).unwrap();
        let bytes = vec![1_i8, 2, -1];
        let input = alloc_arr(vm, "B", bytes.len(), move || ArrayData::Byte(bytes)).unwrap();
        okio_sink_write_bytes(vm, &[sink, input]).unwrap();
        okio_close(vm, &[sink]).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), vec![1, 2, 255]);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
