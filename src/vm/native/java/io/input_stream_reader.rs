//! java.io.InputStreamReader host shims.

use crate::vm::native::*;

fn input_stream_reader_init(vm: &mut Vm, args: &[JValue]) -> R {
    let charset = jstr(vm, args[2])?;
    let bytes = match payload(vm, args[1]) {
        Some(Native::ByteArrayInputStream { bytes, pos }) => bytes[*pos..].to_vec(),
        _ => return Err(npe(vm)),
    };
    let text = if charset.eq_ignore_ascii_case("UTF-8") || charset.eq_ignore_ascii_case("UTF8") {
        String::from_utf8_lossy(&bytes).into_owned()
    } else {
        bytes.into_iter().map(char::from).collect()
    };
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Reader(text));
    Ok(JValue::Null)
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/io/InputStreamReader;",
        "<init>",
        "(Ljava/io/InputStream;Ljava/lang/String;)V",
        true,
        input_stream_reader_init
    ),
    ne!(
        "Ljava/io/InputStreamReader;",
        "<init>",
        "(Ljava/io/InputStream;Ljava/nio/charset/Charset;)V",
        true,
        input_stream_reader_init
    ),
];
