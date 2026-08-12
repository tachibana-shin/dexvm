//! java.util.Base64 / Base64.Decoder / Base64.Encoder host shims. Each
//! Decoder/Encoder instance is a `Native::Str` tag ("STD", "URL",
//! "STD_NOPAD", "URL_NOPAD") selecting which `base64` engine to use.

use crate::vm::native::*;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine as _;

fn engine_for(tag: &str) -> &'static base64::engine::GeneralPurpose {
    match tag {
        "URL" => &URL_SAFE,
        "URL_NOPAD" => &URL_SAFE_NO_PAD,
        "STD_NOPAD" => &STANDARD_NO_PAD,
        _ => &STANDARD,
    }
}

fn get_decoder(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/Base64$Decoder;", Native::Str("STD".into()))
}
fn get_encoder(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/Base64$Encoder;", Native::Str("STD".into()))
}
fn get_url_decoder(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/Base64$Decoder;", Native::Str("URL".into()))
}
fn get_url_encoder(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/util/Base64$Encoder;", Native::Str("URL".into()))
}

fn decoder_decode(vm: &mut Vm, args: &[JValue]) -> R {
    let tag = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    let s = jstr(vm, args[1])?;
    let bytes = engine_for(&tag)
        .decode(s.trim())
        .map_err(|_| iae(vm, "Base64 decode failed"))?;
    let data: Vec<i8> = bytes.iter().map(|&b| b as i8).collect();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

fn encoder_encode_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let tag = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    Ok(new_str(vm, &engine_for(&tag).encode(bytes)))
}

fn encoder_without_padding(vm: &mut Vm, args: &[JValue]) -> R {
    let tag = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    let no_pad = match tag.as_str() {
        "URL" | "URL_NOPAD" => "URL_NOPAD",
        _ => "STD_NOPAD",
    };
    alloc(
        vm,
        "Ljava/util/Base64$Encoder;",
        Native::Str(no_pad.to_string()),
    )
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/Base64;",
        "getDecoder",
        "()Ljava/util/Base64$Decoder;",
        false,
        get_decoder
    ),
    ne!(
        "Ljava/util/Base64;",
        "getEncoder",
        "()Ljava/util/Base64$Encoder;",
        false,
        get_encoder
    ),
    ne!(
        "Ljava/util/Base64;",
        "getUrlDecoder",
        "()Ljava/util/Base64$Decoder;",
        false,
        get_url_decoder
    ),
    ne!(
        "Ljava/util/Base64;",
        "getUrlEncoder",
        "()Ljava/util/Base64$Encoder;",
        false,
        get_url_encoder
    ),
    ne!(
        "Ljava/util/Base64$Decoder;",
        "decode",
        "(Ljava/lang/String;)[B",
        true,
        decoder_decode
    ),
    ne!(
        "Ljava/util/Base64$Encoder;",
        "encodeToString",
        "([B)Ljava/lang/String;",
        true,
        encoder_encode_to_string
    ),
    ne!(
        "Ljava/util/Base64$Encoder;",
        "withoutPadding",
        "()Ljava/util/Base64$Encoder;",
        true,
        encoder_without_padding
    ),
];
