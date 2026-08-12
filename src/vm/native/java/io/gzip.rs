//! java.util.zip.GZIPInputStream host shim: eagerly decompresses the
//! wrapped stream (this VM has no real streaming), then behaves like any
//! other in-memory `ByteArrayInputStream` from then on.

use crate::vm::native::*;

fn gzip_decompress(data: &[u8]) -> Result<Vec<u8>, ()> {
    if data.len() < 10 || data[0] != 0x1f || data[1] != 0x8b || data[2] != 8 {
        return Err(());
    }
    let flags = data[3];
    let mut pos = 10usize;
    if flags & 0x04 != 0 {
        if pos + 2 > data.len() {
            return Err(());
        }
        let xlen = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2 + xlen;
    }
    if flags & 0x08 != 0 {
        while pos < data.len() && data[pos] != 0 {
            pos += 1;
        }
        pos += 1;
    }
    if flags & 0x10 != 0 {
        while pos < data.len() && data[pos] != 0 {
            pos += 1;
        }
        pos += 1;
    }
    if flags & 0x02 != 0 {
        pos += 2;
    }
    if pos > data.len() {
        return Err(());
    }
    miniz_oxide::inflate::decompress_to_vec(&data[pos..]).map_err(|_| ())
}

fn gzip_input_stream_init(vm: &mut Vm, args: &[JValue]) -> R {
    let compressed = match payload(vm, args[1]) {
        Some(Native::ByteArrayInputStream { bytes, pos }) => bytes[*pos..].to_vec(),
        Some(Native::Str(s)) => s.as_bytes().to_vec(),
        _ => return Err(npe(vm)),
    };
    let decompressed = gzip_decompress(&compressed).map_err(|_| ioe(vm, "not a gzip stream"))?;
    let JValue::Obj(id) = args[0] else {
        return Err(npe(vm));
    };
    vm.arena.objects[id as usize].native = Some(Native::ByteArrayInputStream {
        bytes: decompressed,
        pos: 0,
    });
    Ok(JValue::Null)
}

pub(crate) const TABLE: &[NativeEntry] = &[ne!(
    "Ljava/util/zip/GZIPInputStream;",
    "<init>",
    "(Ljava/io/InputStream;)V",
    true,
    gzip_input_stream_init
)];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decompresses_a_real_gzip_stream() {
        let original = b"the quick brown fox jumps over the lazy dog".repeat(3);
        let mut gz = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 0xff];
        gz.extend(miniz_oxide::deflate::compress_to_vec(&original, 6));
        gz.extend(0u32.to_le_bytes());
        gz.extend((original.len() as u32).to_le_bytes());
        assert_eq!(gzip_decompress(&gz).unwrap(), original);
    }
}
