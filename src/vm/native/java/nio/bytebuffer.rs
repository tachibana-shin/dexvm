//! java.nio.ByteBuffer / java.nio.Buffer host shims: a backing byte vector
//! plus a read/write cursor and endianness flag.

use crate::vm::native::*;

fn bb_alloc(vm: &mut Vm, data: Vec<u8>) -> R {
    let limit = data.len();
    alloc(
        vm,
        "Ljava/nio/ByteBuffer;",
        Native::ByteBuffer {
            data,
            pos: 0,
            limit,
            big_endian: true,
        },
    )
}

fn bb_allocate(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[0]).max(0) as usize;
    bb_alloc(vm, vec![0u8; n])
}

fn bb_wrap(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    bb_alloc(vm, bytes)
}

fn bb_wrap_range(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[0]).ok_or_else(|| npe(vm))?;
    let off = int_of(vm, args[1]).max(0) as usize;
    let len = int_of(vm, args[2]).max(0) as usize;
    let end = off.saturating_add(len).min(bytes.len());
    let slice = if off <= end { bytes[off..end].to_vec() } else { Vec::new() };
    bb_alloc(vm, slice)
}

fn bb_order(vm: &mut Vm, args: &[JValue]) -> R {
    let big_endian = !matches!(payload(vm, args[1]), Some(Native::Str(s)) if s == "LITTLE");
    let Some(Native::ByteBuffer { big_endian: be, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *be = big_endian;
    Ok(args[0])
}

/// Reads `n` bytes at the cursor, respecting the buffer's endianness.
fn bb_take(vm: &mut Vm, this: JValue, n: usize) -> Result<Vec<u8>, NatErr> {
    let Some(Native::ByteBuffer {
        data,
        pos,
        limit,
        big_endian,
    }) = payload_mut(vm, this)
    else {
        return Err(npe(vm));
    };
    if *pos + n > *limit {
        return Err(NatErr::Throw(
            vm.throwable_of("Ljava/nio/BufferUnderflowException;", "buffer underflow"),
        ));
    }
    let mut chunk = data[*pos..*pos + n].to_vec();
    *pos += n;
    if !*big_endian {
        chunk.reverse();
    }
    Ok(chunk)
}

fn bb_get(vm: &mut Vm, args: &[JValue]) -> R {
    let b = bb_take(vm, args[0], 1)?;
    Ok(JValue::Int(b[0] as i8 as i32))
}
fn bb_get_short(vm: &mut Vm, args: &[JValue]) -> R {
    let b = bb_take(vm, args[0], 2)?;
    Ok(JValue::Int(i16::from_be_bytes([b[0], b[1]]) as i32))
}
fn bb_get_int(vm: &mut Vm, args: &[JValue]) -> R {
    let b = bb_take(vm, args[0], 4)?;
    Ok(JValue::Int(i32::from_be_bytes([b[0], b[1], b[2], b[3]])))
}
fn bb_get_int_at(vm: &mut Vm, args: &[JValue]) -> R {
    let i = int_of(vm, args[1]).max(0) as usize;
    let (data, big_endian) = match payload(vm, args[0]) {
        Some(Native::ByteBuffer { data, big_endian, .. }) => (data.clone(), *big_endian),
        _ => return Err(npe(vm)),
    };
    if i + 4 > data.len() {
        return Err(iae(vm, "index out of bounds"));
    }
    let mut b: [u8; 4] = data[i..i + 4].try_into().unwrap();
    if !big_endian {
        b.reverse();
    }
    Ok(JValue::Int(i32::from_be_bytes(b)))
}
fn bb_get_long(vm: &mut Vm, args: &[JValue]) -> R {
    let b = bb_take(vm, args[0], 8)?;
    Ok(JValue::Long(i64::from_be_bytes(b.try_into().unwrap())))
}
fn bb_get_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let n = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Byte(dst))) => dst.len(),
        _ => return Err(npe(vm)),
    };
    let bytes = bb_take(vm, args[0], n)?;
    if let Some(Native::Array(ArrayData::Byte(dst))) = payload_mut(vm, args[1]) {
        for (i, b) in bytes.iter().enumerate() {
            dst[i] = *b as i8;
        }
    }
    Ok(args[0])
}

fn bb_put_bytes(vm: &mut Vm, this: JValue, mut bytes: Vec<u8>) -> R {
    let Some(Native::ByteBuffer {
        data,
        pos,
        big_endian,
        ..
    }) = payload_mut(vm, this)
    else {
        return Err(npe(vm));
    };
    if !*big_endian {
        bytes.reverse();
    }
    let end = (*pos + bytes.len()).min(data.len());
    let n = end - *pos;
    data[*pos..end].copy_from_slice(&bytes[..n]);
    *pos += n;
    Ok(this)
}
fn bb_put(vm: &mut Vm, args: &[JValue]) -> R {
    bb_put_bytes(vm, args[0], vec![int_of(vm, args[1]) as u8])
}
fn bb_put_short(vm: &mut Vm, args: &[JValue]) -> R {
    bb_put_bytes(vm, args[0], (int_of(vm, args[1]) as i16).to_be_bytes().to_vec())
}
fn bb_put_int(vm: &mut Vm, args: &[JValue]) -> R {
    bb_put_bytes(vm, args[0], int_of(vm, args[1]).to_be_bytes().to_vec())
}
fn bb_put_array(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    bb_put_bytes(vm, args[0], bytes)
}

fn bb_array(vm: &mut Vm, args: &[JValue]) -> R {
    let data = match payload(vm, args[0]) {
        Some(Native::ByteBuffer { data, .. }) => data.clone(),
        _ => return Err(npe(vm)),
    };
    let out: Vec<i8> = data.iter().map(|&b| b as i8).collect();
    alloc_arr(vm, "B", out.len(), move || ArrayData::Byte(out))
}

fn bb_remaining(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::ByteBuffer { pos, limit, .. }) => Ok(JValue::Int((*limit - *pos) as i32)),
        _ => Err(npe(vm)),
    }
}
fn bb_position_get(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::ByteBuffer { pos, .. }) => Ok(JValue::Int(*pos as i32)),
        _ => Err(npe(vm)),
    }
}
fn bb_position_set(vm: &mut Vm, args: &[JValue]) -> R {
    let p = int_of(vm, args[1]).max(0) as usize;
    let Some(Native::ByteBuffer { pos, limit, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *pos = p.min(*limit);
    Ok(args[0])
}
fn bb_limit(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::ByteBuffer { limit, .. }) => Ok(JValue::Int(*limit as i32)),
        _ => Err(npe(vm)),
    }
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/nio/ByteBuffer;", "allocate", "(I)Ljava/nio/ByteBuffer;", false, bb_allocate),
    ne!("Ljava/nio/ByteBuffer;", "wrap", "([B)Ljava/nio/ByteBuffer;", false, bb_wrap),
    ne!("Ljava/nio/ByteBuffer;", "wrap", "([BII)Ljava/nio/ByteBuffer;", false, bb_wrap_range),
    ne!("Ljava/nio/ByteBuffer;", "order", "(Ljava/nio/ByteOrder;)Ljava/nio/ByteBuffer;", true, bb_order),
    ne!("Ljava/nio/ByteBuffer;", "get", "()B", true, bb_get),
    ne!("Ljava/nio/ByteBuffer;", "get", "([B)Ljava/nio/ByteBuffer;", true, bb_get_bytes),
    ne!("Ljava/nio/ByteBuffer;", "getShort", "()S", true, bb_get_short),
    ne!("Ljava/nio/ByteBuffer;", "getInt", "()I", true, bb_get_int),
    ne!("Ljava/nio/ByteBuffer;", "getInt", "(I)I", true, bb_get_int_at),
    ne!("Ljava/nio/ByteBuffer;", "getLong", "()J", true, bb_get_long),
    ne!("Ljava/nio/ByteBuffer;", "put", "(B)Ljava/nio/ByteBuffer;", true, bb_put),
    ne!("Ljava/nio/ByteBuffer;", "put", "([B)Ljava/nio/ByteBuffer;", true, bb_put_array),
    ne!("Ljava/nio/ByteBuffer;", "putShort", "(S)Ljava/nio/ByteBuffer;", true, bb_put_short),
    ne!("Ljava/nio/ByteBuffer;", "putInt", "(I)Ljava/nio/ByteBuffer;", true, bb_put_int),
    ne!("Ljava/nio/ByteBuffer;", "array", "()[B", true, bb_array),
    ne!("Ljava/nio/Buffer;", "remaining", "()I", true, bb_remaining),
    ne!("Ljava/nio/Buffer;", "position", "()I", true, bb_position_get),
    ne!("Ljava/nio/Buffer;", "position", "(I)Ljava/nio/Buffer;", true, bb_position_set),
    ne!("Ljava/nio/Buffer;", "limit", "()I", true, bb_limit),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::SandboxOptions;

    #[test]
    fn big_endian_get_short_matches_java_default() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut context = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
        let vm = context.vm();
        let arr = bytes_arr(vm, &[0x01, 0x02, 0x03, 0x04]);
        let buf = bb_wrap(vm, &[arr]).unwrap();
        assert_eq!(bb_get_short(vm, &[buf]).unwrap(), JValue::Int(0x0102));
        assert_eq!(bb_get_short(vm, &[buf]).unwrap(), JValue::Int(0x0304));
    }

    fn bytes_arr(vm: &mut Vm, bytes: &[u8]) -> JValue {
        let data: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
        alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data)).unwrap()
    }
}
