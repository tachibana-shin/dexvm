//! okio host shims: in-memory `BufferedSource`/`Buffer` over a byte cursor.

use super::*;
use crate::permission::{FilesystemPermission, Permission};
use crate::vm::native::okhttp::resp_body_bytes;
use std::io::Write as _;

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
