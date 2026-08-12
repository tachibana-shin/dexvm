//! java.io.PrintStream host shims.

use crate::vm::native::*;

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
    let Ok(class) = vm.ensure_class_by_desc("Ljava/io/PrintStream;") else {
        return JValue::Null;
    };
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
];
