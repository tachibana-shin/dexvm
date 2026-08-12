//! `java.net.URLEncoder` / `URLDecoder` form encoding shims.

use crate::vm::native::*;

fn form_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' => {
                out.push(*b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn form_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    out.push((hi * 16 + lo) as u8);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub(crate) fn url_encoder_encode(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[1])?;
    let encoded = form_encode(&value);
    Ok(new_str(vm, &encoded))
}

pub(crate) fn url_decoder_decode(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[1])?;
    let decoded = form_decode(&value);
    Ok(new_str(vm, &decoded))
}

fn url_init(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[1])?;
    let JValue::Obj(id) = args[0] else {
        return Err(npe(vm));
    };
    vm.arena.objects[id as usize].native = Some(Native::URI(value));
    Ok(JValue::Null)
}

fn url_get_host(vm: &mut Vm, args: &[JValue]) -> R {
    let value = match payload(vm, args[0]) {
        Some(Native::URI(value)) => value.clone(),
        _ => return Err(npe(vm)),
    };
    let authority = value
        .split("://")
        .last()
        .and_then(|part| part.strip_prefix("//").or(Some(part)))
        .unwrap_or(&value);
    Ok(new_str(vm, authority.split(['/', ':']).next().unwrap_or("")))
}

fn url_get_path(vm: &mut Vm, args: &[JValue]) -> R {
    let value = match payload(vm, args[0]) {
        Some(Native::URI(value)) => value.clone(),
        _ => return Err(npe(vm)),
    };
    let after_scheme = value.split("://").last().unwrap_or(&value);
    let path = if let Some(rest) = after_scheme.strip_prefix("//") {
        rest.find('/').map(|i| &rest[i..]).unwrap_or("")
    } else {
        after_scheme
    };
    Ok(new_str(vm, path.split(['?', '#']).next().unwrap_or("")))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/net/URLEncoder;",
        "encode",
        "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
        false,
        url_encoder_encode
    ),
    ne!(
        "Ljava/net/URLDecoder;",
        "decode",
        "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
        false,
        url_decoder_decode
    ),
    ne!("Ljava/net/URL;", "<init>", "(Ljava/lang/String;)V", true, url_init),
    ne!("Ljava/net/URL;", "getHost", "()Ljava/lang/String;", true, url_get_host),
    ne!("Ljava/net/URL;", "getPath", "()Ljava/lang/String;", true, url_get_path),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_encoding_round_trips_utf8() {
        let encoded = form_encode("a b+c/你好");
        assert_eq!(encoded, "a+b%2Bc%2F%E4%BD%A0%E5%A5%BD");
        assert_eq!(form_decode(&encoded), "a b+c/你好");
    }
}
