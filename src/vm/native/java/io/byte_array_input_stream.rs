//! java.io.ByteArrayInputStream host shims.

use crate::vm::native::*;

pub(crate) fn bais_init(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    alloc(
        vm,
        "Ljava/io/ByteArrayInputStream;",
        Native::ByteArrayInputStream { bytes, pos: 0 },
    )
}

pub(crate) fn bais_init_range(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let off = usize::try_from(int_of(vm, args[2]))
        .unwrap_or(0)
        .min(bytes.len());
    let len = usize::try_from(int_of(vm, args[3]))
        .unwrap_or(0)
        .min(bytes.len() - off);
    alloc(
        vm,
        "Ljava/io/ByteArrayInputStream;",
        Native::ByteArrayInputStream {
            bytes: bytes[off..off + len].to_vec(),
            pos: 0,
        },
    )
}

pub(crate) fn bais_read(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ByteArrayInputStream { bytes, pos }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(if *pos < bytes.len() {
        let b = bytes[*pos] as i32;
        *pos += 1;
        JValue::Int(b & 0xff)
    } else {
        JValue::Int(-1)
    })
}

pub(crate) fn bais_read_buf(vm: &mut Vm, args: &[JValue]) -> R {
    let off = usize::try_from(int_of(vm, args[2])).unwrap_or(0);
    let len = usize::try_from(int_of(vm, args[3])).unwrap_or(0);
    let (src, dst) = payload_mut_two(vm, args[0], args[1]);
    let Some(Native::ByteArrayInputStream { bytes, pos }) = src else {
        return Err(npe(vm));
    };
    let Some(Native::Array(ArrayData::Byte(dst))) = dst else {
        return Err(npe(vm));
    };
    let off = off.min(dst.len());
    let len = len.min(dst.len() - off);
    let n = len.min(bytes.len() - *pos);
    for i in 0..n {
        dst[off + i] = bytes[*pos + i] as i8;
    }
    let n = n as i32;
    *pos += n as usize;
    Ok(JValue::Int(if n == 0 && *pos >= bytes.len() {
        -1
    } else {
        n
    }))
}

pub(crate) fn bais_available(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::ByteArrayInputStream { bytes, pos }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int((bytes.len() - *pos) as i32))
}

pub(crate) fn bais_close(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/io/ByteArrayInputStream;",
        "<init>",
        "([B)V",
        true,
        bais_init
    ),
    ne!(
        "Ljava/io/ByteArrayInputStream;",
        "<init>",
        "([BII)V",
        true,
        bais_init_range
    ),
    ne!(
        "Ljava/io/ByteArrayInputStream;",
        "read",
        "()I",
        true,
        bais_read
    ),
    ne!(
        "Ljava/io/ByteArrayInputStream;",
        "read",
        "([BII)I",
        true,
        bais_read_buf
    ),
    ne!(
        "Ljava/io/ByteArrayInputStream;",
        "available",
        "()I",
        true,
        bais_available
    ),
    ne!(
        "Ljava/io/ByteArrayInputStream;",
        "close",
        "()V",
        true,
        bais_close
    ),
];
