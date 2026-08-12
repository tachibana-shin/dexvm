//! java.util.zip.Inflater host shim: accumulates `setInput` bytes and
//! decompresses eagerly (via miniz_oxide) on the first `inflate` call,
//! rather than modeling incremental streaming state.

use crate::vm::native::*;

pub(crate) fn inflater_init(vm: &mut Vm, args: &[JValue]) -> R {
    let nowrap = args.get(1).map(|v| int_of(vm, *v) != 0).unwrap_or(false);
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Inflater {
            input,
            nowrap: nw,
            output,
            out_pos,
        } => {
            input.clear();
            *nw = nowrap;
            *output = None;
            *out_pos = 0;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn inflater_set_input(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let Some(Native::Inflater { input, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    input.extend_from_slice(&bytes);
    Ok(JValue::Null)
}

pub(crate) fn inflater_inflate(vm: &mut Vm, args: &[JValue]) -> R {
    let dst_len = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Byte(b))) => b.len(),
        _ => return Err(npe(vm)),
    };
    let (input, nowrap, has_output) = match payload(vm, args[0]) {
        Some(Native::Inflater {
            input,
            nowrap,
            output,
            ..
        }) => (input.clone(), *nowrap, output.is_some()),
        _ => return Err(npe(vm)),
    };
    if !has_output {
        let decompressed = if nowrap {
            miniz_oxide::inflate::decompress_to_vec(&input)
        } else {
            miniz_oxide::inflate::decompress_to_vec_zlib(&input)
        }
        .map_err(|_| ioe(vm, "invalid deflate data"))?;
        let Some(Native::Inflater { output, .. }) = payload_mut(vm, args[0]) else {
            return Err(npe(vm));
        };
        *output = Some(decompressed);
    }
    let Some(Native::Inflater {
        output, out_pos, ..
    }) = payload_mut(vm, args[0])
    else {
        return Err(npe(vm));
    };
    let out = output.as_ref().expect("output populated above");
    let remaining = out.len() - *out_pos;
    let n = remaining.min(dst_len);
    let chunk = out[*out_pos..*out_pos + n].to_vec();
    *out_pos += n;
    if let Some(Native::Array(ArrayData::Byte(dst))) = payload_mut(vm, args[1]) {
        for (i, b) in chunk.iter().enumerate() {
            dst[i] = *b as i8;
        }
    }
    Ok(JValue::Int(n as i32))
}

pub(crate) fn inflater_finished(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Inflater {
            output: Some(out),
            out_pos,
            ..
        }) => Ok(JValue::Int(i32::from(out_pos >= &out.len()))),
        Some(Native::Inflater { .. }) => Ok(JValue::Int(0)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn inflater_end(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Inflater {
            input,
            output,
            out_pos,
            ..
        } => {
            input.clear();
            *output = None;
            *out_pos = 0;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/zip/Inflater;",
        "<init>",
        "()V",
        true,
        inflater_init
    ),
    ne!(
        "Ljava/util/zip/Inflater;",
        "<init>",
        "(Z)V",
        true,
        inflater_init
    ),
    ne!(
        "Ljava/util/zip/Inflater;",
        "setInput",
        "([B)V",
        true,
        inflater_set_input
    ),
    ne!(
        "Ljava/util/zip/Inflater;",
        "inflate",
        "([B)I",
        true,
        inflater_inflate
    ),
    ne!(
        "Ljava/util/zip/Inflater;",
        "finished",
        "()Z",
        true,
        inflater_finished
    ),
    ne!("Ljava/util/zip/Inflater;", "end", "()V", true, inflater_end),
];

#[cfg(test)]
mod tests {
    #[test]
    fn raw_deflate_roundtrips_through_miniz_oxide() {
        let original =
            b"the quick brown fox jumps over the lazy dog, repeatedly, for compression".repeat(4);
        let compressed = miniz_oxide::deflate::compress_to_vec(&original, 6);
        let decompressed = miniz_oxide::inflate::decompress_to_vec(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }

    #[test]
    fn zlib_wrapped_deflate_roundtrips_through_miniz_oxide() {
        let original = b"zlib-wrapped payload".to_vec();
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&original, 6);
        let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }
}
