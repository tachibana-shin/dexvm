//! java.io host shims (PrintStream, ByteArrayInputStream).

use crate::vm::native::*;

// ---------------------------------------------------------------------------
// java.io.ByteArrayInputStream
// ---------------------------------------------------------------------------

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

pub(crate) fn ps_println(vm: &mut Vm, args: &[JValue]) -> R {
    let s = if args.len() > 1 {
        to_string_of(vm, args[1])?
    } else {
        String::new()
    };
    vm.write_out(&format!("{s}\n"));
    Ok(JValue::Null)
}

pub(crate) fn ps_print(vm: &mut Vm, args: &[JValue]) -> R {
    let s = to_string_of(vm, args[1])?;
    vm.write_out(&s);
    Ok(JValue::Null)
}

pub(crate) fn ps_println_char(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[1]) as u16;
    vm.write_out(&format!("{}\n", u16str(&[c])));
    Ok(JValue::Null)
}

pub(crate) fn ps_print_char(vm: &mut Vm, args: &[JValue]) -> R {
    let c = int_of(vm, args[1]) as u16;
    vm.write_out(&u16str(&[c]));
    Ok(JValue::Null)
}

pub(crate) fn ps_println_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Char(cs))) => u16str(cs),
        _ => return Err(npe(vm)),
    };
    vm.write_out(&format!("{s}\n"));
    Ok(JValue::Null)
}

pub(crate) fn ps_flush(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn ps_close(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub fn lazy_print_stream(vm: &mut Vm) -> JValue {
    let class = vm
        .ensure_class_by_desc("Ljava/io/PrintStream;")
        .expect("PrintStream shim");
    JValue::Obj(vm.arena.alloc(class, Vec::new(), Some(Native::PrintStream)))
}

// java.io.PrintStream.<init> (objects constructed by dex)
// ---------------------------------------------------------------------------

pub(crate) fn ps_init(vm: &mut Vm, args: &[JValue]) -> R {
    if payload_mut(vm, args[0]).is_none() {
        return Err(npe(vm));
    }
    Ok(JValue::Null)
}

// ---------------------------------------------------------------------------

/// Native methods for Ljava/io/PrintStream;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/io/PrintStream;",
        "<init>",
        "(Ljava/io/OutputStream;)V",
        true,
        ps_init
    ),
    ne!("Ljava/io/PrintStream;", "println", "()V", true, ps_println),
    ne!(
        "Ljava/io/PrintStream;",
        "println",
        "(Ljava/lang/String;)V",
        true,
        ps_println
    ),
    ne!("Ljava/io/PrintStream;", "println", "(I)V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(J)V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(Z)V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(F)V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(D)V", true, ps_println),
    ne!(
        "Ljava/io/PrintStream;",
        "println",
        "(Ljava/lang/Object;)V",
        true,
        ps_println
    ),
    ne!(
        "Ljava/io/PrintStream;",
        "println",
        "(C)V",
        true,
        ps_println_char
    ),
    ne!(
        "Ljava/io/PrintStream;",
        "println",
        "([C)V",
        true,
        ps_println_chars
    ),
    ne!(
        "Ljava/io/PrintStream;",
        "print",
        "(Ljava/lang/String;)V",
        true,
        ps_print
    ),
    ne!("Ljava/io/PrintStream;", "print", "(I)V", true, ps_print),
    ne!("Ljava/io/PrintStream;", "print", "(J)V", true, ps_print),
    ne!("Ljava/io/PrintStream;", "print", "(Z)V", true, ps_print),
    ne!("Ljava/io/PrintStream;", "print", "(F)V", true, ps_print),
    ne!("Ljava/io/PrintStream;", "print", "(D)V", true, ps_print),
    ne!(
        "Ljava/io/PrintStream;",
        "print",
        "(Ljava/lang/Object;)V",
        true,
        ps_print
    ),
    ne!(
        "Ljava/io/PrintStream;",
        "print",
        "(C)V",
        true,
        ps_print_char
    ),
    ne!("Ljava/io/PrintStream;", "flush", "()V", true, ps_flush),
    ne!("Ljava/io/PrintStream;", "close", "()V", true, ps_close),
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
