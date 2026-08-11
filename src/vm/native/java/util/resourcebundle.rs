//! java.util.PropertyResourceBundle backed by APK `.properties` resources.

use crate::vm::native::*;

fn unescape_property(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\u{000c}'),
            Some('u') => {
                let digits: String = chars.by_ref().take(4).collect();
                if let Ok(code) = u32::from_str_radix(&digits, 16) {
                    if let Some(decoded) = char::from_u32(code) {
                        out.push(decoded);
                    }
                }
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

fn logical_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for raw in text.lines() {
        current.push_str(raw);
        let slashes = current.chars().rev().take_while(|c| *c == '\\').count();
        if slashes % 2 == 1 {
            current.pop();
            continue;
        }
        lines.push(std::mem::take(&mut current));
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn parse_properties(text: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for line in logical_lines(text) {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let mut escaped = false;
        let mut separator = None;
        for (idx, ch) in line.char_indices() {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '=' || ch == ':' || ch.is_whitespace() {
                separator = Some(idx);
                break;
            }
        }
        let (key, value) = match separator {
            Some(idx) => {
                let mut value = &line[idx..];
                value = value.trim_start_matches(char::is_whitespace);
                value = value.strip_prefix(['=', ':']).unwrap_or(value);
                (line[..idx].trim_end(), value.trim_start())
            }
            None => (line, ""),
        };
        entries.push((unescape_property(key), unescape_property(value)));
    }
    entries
}

fn property_resource_bundle_init(vm: &mut Vm, args: &[JValue]) -> R {
    let text = match payload(vm, args[1]) {
        Some(Native::Reader(text)) => text.clone(),
        _ => return Err(npe(vm)),
    };
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::ResourceBundle(parse_properties(&text)));
    Ok(JValue::Null)
}

fn resource_bundle_contains_key(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let found = match payload(vm, args[0]) {
        Some(Native::ResourceBundle(entries)) => entries.iter().any(|(k, _)| k == &key),
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(i32::from(found)))
}

fn resource_bundle_get_string(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let value = match payload(vm, args[0]) {
        Some(Native::ResourceBundle(entries)) => entries
            .iter()
            .find(|(k, _)| k == &key)
            .map(|(_, value)| value.clone()),
        _ => return Err(npe(vm)),
    };
    value
        .map(|value| new_str(vm, &value))
        .ok_or_else(|| no_such_elem(vm))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/PropertyResourceBundle;",
        "<init>",
        "(Ljava/io/Reader;)V",
        true,
        property_resource_bundle_init
    ),
    ne!(
        "Ljava/util/ResourceBundle;",
        "containsKey",
        "(Ljava/lang/String;)Z",
        true,
        resource_bundle_contains_key
    ),
    ne!(
        "Ljava/util/ResourceBundle;",
        "getString",
        "(Ljava/lang/String;)Ljava/lang/String;",
        true,
        resource_bundle_get_string
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_escapes_comments_and_continuations() {
        let parsed =
            parse_properties("# comment\nhello = xin\\ chào\nunicode=\\u0111ẹp\nlong=a\\\nb\n");
        assert_eq!(
            parsed,
            vec![
                ("hello".into(), "xin chào".into()),
                ("unicode".into(), "đẹp".into()),
                ("long".into(), "ab".into()),
            ]
        );
    }
}
