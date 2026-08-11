//! kotlinx.serialization host implementation: a small JSON parser producing
//! the `JsonElement` tree, plus the decoder / descriptor natives that the
//! extension's generated serializers (real dex bytecode, e.g. `Lg.deserialize`
//! and `Lg.serialize`) drive to decode and encode cached filter lists.
//!
//! The `JsonElement` tree is represented by [`JsonVal`]; each tree node is a
//! host object of class `JsonObject` / `JsonArray` / `JsonPrimitive` /
//! `JsonNull`. Decoding runs the *real* dex deserializers over a
//! `StreamingJsonDecoder` host object, so the result obeys the extension's
//! own serializer bytecode.

use super::*;

// ---------------------------------------------------------------------------
// JSON parsing
// ---------------------------------------------------------------------------

/// Parses a JSON document into a [`JsonVal`] tree.
pub(crate) fn parse_json(text: &str) -> Result<JsonVal, String> {
    let mut p = Parser {
        b: text.as_bytes(),
        i: 0,
    };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.i < p.b.len() {
        return Err(format!("trailing data at byte {}", p.i));
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let c = self.peek();
        if c.is_some() {
            self.i += 1;
        }
        c
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        match self.next() {
            Some(x) if x == c => Ok(()),
            other => Err(format!("expected {c:?} at byte {} got {other:?}", self.i)),
        }
    }

    fn value(&mut self) -> Result<JsonVal, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(JsonVal::Str),
            Some(b't') => self.literal("true").map(|_| JsonVal::Bool(true)),
            Some(b'f') => self.literal("false").map(|_| JsonVal::Bool(false)),
            Some(b'n') => self.literal("null").map(|_| JsonVal::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            other => Err(format!("unexpected token {other:?} at byte {}", self.i)),
        }
    }

    fn literal(&mut self, lit: &str) -> Result<(), String> {
        if self.b[self.i..].starts_with(lit.as_bytes()) {
            self.i += lit.len();
            Ok(())
        } else {
            Err(format!("expected literal {lit:?} at byte {}", self.i))
        }
    }

    fn object(&mut self) -> Result<JsonVal, String> {
        self.expect(b'{')?;
        let mut out = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(JsonVal::Object(out));
        }
        loop {
            self.skip_ws();
            let k = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            let v = self.value()?;
            out.push((k, v));
            self.skip_ws();
            match self.next() {
                Some(b',') => continue,
                Some(b'}') => break,
                other => return Err(format!("expected , or }} at byte {} got {other:?}", self.i)),
            }
        }
        Ok(JsonVal::Object(out))
    }

    fn array(&mut self) -> Result<JsonVal, String> {
        self.expect(b'[')?;
        let mut out = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(JsonVal::Array(out));
        }
        loop {
            let v = self.value()?;
            out.push(v);
            self.skip_ws();
            match self.next() {
                Some(b',') => continue,
                Some(b']') => break,
                other => return Err(format!("expected , or ] at byte {} got {other:?}", self.i)),
            }
        }
        Ok(JsonVal::Array(out))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = Vec::new();
        loop {
            let Some(c) = self.next() else {
                return Err("unterminated string".into());
            };
            match c {
                b'"' => break,
                b'\\' => {
                    let Some(e) = self.next() else {
                        return Err("unterminated escape".into());
                    };
                    match e {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0c),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let hex = self
                                .b
                                .get(self.i..self.i + 4)
                                .ok_or("truncated \\u escape")?;
                            let code = u16::from_str_radix(
                                std::str::from_utf8(hex).map_err(|_| "bad \\u escape")?,
                                16,
                            )
                            .map_err(|_| "bad \\u escape")?;
                            self.i += 4;
                            // encode the code point as UTF-8
                            let c = char::from_u32(u32::from(code)).unwrap_or('\u{fffd}');
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                        }
                        other => return Err(format!("bad escape \\{other:?}")),
                    }
                }
                c if c < 0x20 => return Err(format!("control char in string at byte {}", self.i)),
                _ => out.push(c),
            }
        }
        String::from_utf8(out).map_err(|_| "string is not utf-8".to_string())
    }

    fn number(&mut self) -> Result<JsonVal, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.i += 1;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        let text = std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "bad number")?;
        if is_float {
            text.parse::<f64>()
                .map(JsonVal::Double)
                .map_err(|_| format!("bad number {text:?}"))
        } else {
            text.parse::<i64>()
                .map(JsonVal::Int)
                .map_err(|_| format!("bad number {text:?}"))
        }
    }
}

// ---------------------------------------------------------------------------
// element tree
// ---------------------------------------------------------------------------

fn json_node_class(v: &JsonVal) -> &'static str {
    match v {
        JsonVal::Object(_) => "Lkotlinx/serialization/json/JsonObject;",
        JsonVal::Array(_) => "Lkotlinx/serialization/json/JsonArray;",
        JsonVal::Null => "Lkotlinx/serialization/json/JsonNull;",
        _ => "Lkotlinx/serialization/json/JsonPrimitive;",
    }
}

fn alloc_json_node(vm: &mut Vm, v: &JsonVal) -> R {
    alloc(vm, json_node_class(v), Native::Json(v.clone()))
}

fn jsonval_to_string(v: &JsonVal) -> String {
    match v {
        JsonVal::Str(s) => s.clone(),
        JsonVal::Int(i) => i.to_string(),
        JsonVal::Double(d) => d.to_string(),
        JsonVal::Bool(b) => b.to_string(),
        JsonVal::Null | JsonVal::Object(_) | JsonVal::Array(_) => String::new(),
    }
}

fn jsonval_to_json(v: &JsonVal) -> String {
    fn quoted(value: &str) -> String {
        let mut out = String::with_capacity(value.len() + 2);
        out.push('"');
        for c in value.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
    match v {
        JsonVal::Object(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(key, value)| format!("{}:{}", quoted(key), jsonval_to_json(value)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        JsonVal::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(jsonval_to_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        JsonVal::Str(value) => quoted(value),
        JsonVal::Int(value) => value.to_string(),
        JsonVal::Double(value) => value.to_string(),
        JsonVal::Bool(value) => value.to_string(),
        JsonVal::Null => "null".into(),
    }
}

fn json_parse_to_element(vm: &mut Vm, args: &[JValue]) -> R {
    let text = jstr(vm, args[1])?;
    let value = parse_json(&text).map_err(|error| iae(vm, error))?;
    alloc_json_node(vm, &value)
}

fn json_primitive_content(vm: &mut Vm, args: &[JValue]) -> R {
    let content = match payload(vm, args[0]) {
        Some(Native::Json(value)) => jsonval_to_string(value),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &content))
}

fn json_element_get_primitive(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Json(
            JsonVal::Str(_) | JsonVal::Int(_) | JsonVal::Double(_) | JsonVal::Bool(_),
        )) => Ok(args[0]),
        _ => Err(iae(vm, "JsonElement is not a JsonPrimitive")),
    }
}

fn json_element_get_array(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Array(_))) => Ok(args[0]),
        _ => Err(iae(vm, "JsonElement is not a JsonArray")),
    }
}

fn json_element_get_object(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Object(_))) => Ok(args[0]),
        _ => Err(iae(vm, "JsonElement is not a JsonObject")),
    }
}

fn json_object_get(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let value = match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Object(entries))) => entries
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.clone()),
        _ => return Err(npe(vm)),
    };
    match value {
        Some(value) => alloc_json_node(vm, &value),
        None => Ok(JValue::Null),
    }
}

fn json_object_init(vm: &mut Vm, args: &[JValue]) -> R {
    let source = match payload(vm, args[1]) {
        Some(Native::Map(entries)) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let mut entries = Vec::with_capacity(source.len());
    for (key, value) in source {
        let key = jstr(vm, key)?;
        let value = match payload(vm, value) {
            Some(Native::Json(value)) => value.clone(),
            _ => return Err(iae(vm, "JsonObject map value is not a JsonElement")),
        };
        entries.push((key, value));
    }
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Json(JsonVal::Object(entries)));
    Ok(JValue::Null)
}

fn json_object_contains_key(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let found = match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Object(entries))) => {
            entries.iter().any(|(name, _)| name == &key)
        }
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(i32::from(found)))
}

fn json_object_builder_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Json(JsonVal::Object(Vec::new())));
    Ok(JValue::Null)
}

fn json_object_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let object = match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Object(entries))) => JsonVal::Object(entries.clone()),
        _ => return Err(npe(vm)),
    };
    alloc_json_node(vm, &object)
}

fn json_object_builder_put(vm: &mut Vm, args: &[JValue]) -> R {
    let key = jstr(vm, args[1])?;
    let value = match payload(vm, args[2]) {
        Some(Native::Json(value)) => value.clone(),
        _ => return Err(iae(vm, "JsonObjectBuilder value is not a JsonElement")),
    };
    let previous = match payload_mut(vm, args[0]) {
        Some(Native::Json(JsonVal::Object(entries))) => {
            if let Some((_, old)) = entries.iter_mut().find(|(name, _)| name == &key) {
                Some(std::mem::replace(old, value))
            } else {
                entries.push((key, value));
                None
            }
        }
        _ => return Err(npe(vm)),
    };
    match previous {
        Some(previous) => alloc_json_node(vm, &previous),
        None => Ok(JValue::Null),
    }
}

fn json_builder_put_string(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[2])?;
    let element = alloc_json_node(vm, &JsonVal::Str(value))?;
    json_object_builder_put(vm, &[args[0], args[1], element])?;
    Ok(element)
}

fn json_builder_put_number(vm: &mut Vm, args: &[JValue]) -> R {
    let text = to_string_of(vm, args[2])?;
    let value = text
        .parse::<i64>()
        .map(JsonVal::Int)
        .unwrap_or_else(|_| JsonVal::Double(text.parse().unwrap_or(0.0)));
    let element = alloc_json_node(vm, &value)?;
    json_object_builder_put(vm, &[args[0], args[1], element])?;
    Ok(element)
}

fn json_get_serializers_module(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlinx/serialization/modules/SerializersModule;",
        Native::Opaque,
    )
}

fn json_primitive_string(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    alloc_json_node(vm, &JsonVal::Str(value))
}

fn json_primitive_is_string(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(i32::from(matches!(
        payload(vm, args[0]),
        Some(Native::Json(JsonVal::Str(_)))
    ))))
}

fn json_content_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Null)) | None => Ok(JValue::Null),
        Some(Native::Json(value)) => {
            let content = jsonval_to_string(value);
            Ok(new_str(vm, &content))
        }
        _ => Err(npe(vm)),
    }
}

fn json_array_init(vm: &mut Vm, args: &[JValue]) -> R {
    let values = coll_elems(vm, args[1])?
        .into_iter()
        .map(|value| match payload(vm, value) {
            Some(Native::Json(value)) => Ok(value.clone()),
            _ => Err(npe(vm)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let Some(native) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *native = Native::Json(JsonVal::Array(values));
    Ok(JValue::Null)
}

fn json_array_get(vm: &mut Vm, args: &[JValue]) -> R {
    let index = int_of(vm, args[1]);
    let value = match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Array(values))) if index >= 0 => {
            values.get(index as usize).cloned()
        }
        _ => None,
    }
    .ok_or_else(|| ioobe(vm, index))?;
    alloc_json_node(vm, &value)
}

fn json_array_size(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Array(values))) => Ok(JValue::Int(values.len() as i32)),
        _ => Err(npe(vm)),
    }
}

fn json_array_iterator(vm: &mut Vm, args: &[JValue]) -> R {
    let values = match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Array(values))) => values.clone(),
        _ => return Err(npe(vm)),
    };
    let mut nodes = Vec::with_capacity(values.len());
    for value in values {
        nodes.push(alloc_json_node(vm, &value)?);
    }
    let list = list_alloc(vm, nodes)?;
    list_iterator(vm, &[list])
}

fn json_object_values(vm: &mut Vm, args: &[JValue]) -> R {
    let values = match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Object(entries))) => entries
            .iter()
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>(),
        _ => return Err(npe(vm)),
    };
    let mut nodes = Vec::with_capacity(values.len());
    for value in values {
        nodes.push(alloc_json_node(vm, &value)?);
    }
    list_alloc(vm, nodes)
}

fn descriptor_push_annotation(_vm: &mut Vm, _args: &[JValue]) -> R {
    // Runtime annotations do not affect the generated serializer's field map.
    Ok(JValue::Null)
}

fn object_members(vm: &Vm, element: JValue) -> Vec<(String, JsonVal)> {
    match payload(vm, element) {
        Some(Native::Json(JsonVal::Object(m))) => m.clone(),
        _ => Vec::new(),
    }
}

fn member_by_index(vm: &Vm, element: JValue, descriptor: JValue, index: i32) -> Option<JsonVal> {
    let names = match payload(vm, descriptor) {
        Some(Native::SerialDescriptor { elements, .. }) => elements.clone(),
        _ => return None,
    };
    let name = names.get(index as usize)?.clone();
    object_members(vm, element)
        .into_iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

// ---------------------------------------------------------------------------
// serializer dispatch
// ---------------------------------------------------------------------------

/// Invokes `deserializer.deserialize(decoder)` (interface dispatch into real
/// dex bytecode where the serializer is a dex class).
fn invoke_deserialize(vm: &mut Vm, serializer: JValue, decoder: JValue) -> R {
    let JValue::Obj(o) = serializer else {
        return Err(nat_fatal(JvmError::Resolution(
            "deserialize: null serializer".into(),
        )));
    };
    let mref = MethodRef {
        name: vm.intern("deserialize"),
        sig: vm.intern("(Lkotlinx/serialization/encoding/Decoder;)Ljava/lang/Object;"),
        ret: 0,
        args: Vec::new(),
        class_desc: 0,
    };
    let target = vm
        .resolve_target(InvokeKind::Interface, &mref, Some(o), 0)
        .map_err(nat_fatal)?;
    vm.call_target(target, vec![serializer, decoder])
        .map_err(nat_fatal)
}

/// Invokes the extension's generated `serializer.serialize(encoder, value)`.
fn invoke_serialize(
    vm: &mut Vm,
    serializer: JValue,
    encoder: JValue,
    value: JValue,
) -> Result<(), NatErr> {
    let JValue::Obj(o) = serializer else {
        return Err(nat_fatal(JvmError::Resolution(
            "serialize: null serializer".into(),
        )));
    };
    let mref = MethodRef {
        name: vm.intern("serialize"),
        sig: vm.intern("(Lkotlinx/serialization/encoding/Encoder;Ljava/lang/Object;)V"),
        ret: 0,
        args: Vec::new(),
        class_desc: 0,
    };
    let target = vm
        .resolve_target(InvokeKind::Interface, &mref, Some(o), 0)
        .map_err(nat_fatal)?;
    vm.call_target(target, vec![serializer, encoder, value])
        .map_err(nat_fatal)?;
    Ok(())
}

fn json_decoder(vm: &mut Vm, element: JValue) -> R {
    alloc(
        vm,
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        Native::JsonDecoder {
            element,
            members: None,
            index: 0,
        },
    )
}

/// Runs a serializer over an element tree: `JsonElementSerializer` returns
/// the element itself; `ArrayListSerializer` decodes each array item through
/// its child serializer.
fn run_serializer(vm: &mut Vm, serializer: JValue, element: JValue) -> R {
    let child = match payload(vm, serializer) {
        Some(Native::JsonElementSerializer) => return Ok(element),
        Some(Native::PrimitiveSerializer(kind)) => {
            let kind = *kind;
            let value = match payload(vm, element) {
                Some(Native::Json(value)) => value.clone(),
                _ => JsonVal::Null,
            };
            return Ok(match kind {
                PrimitiveSerializerKind::String => new_str(vm, &jsonval_to_string(&value)),
                PrimitiveSerializerKind::Int => JValue::Int(match value {
                    JsonVal::Int(v) => v as i32,
                    JsonVal::Double(v) => v as i32,
                    JsonVal::Str(v) => v.parse().unwrap_or_default(),
                    _ => 0,
                }),
                PrimitiveSerializerKind::Long => JValue::Long(match value {
                    JsonVal::Int(v) => v,
                    JsonVal::Double(v) => v as i64,
                    JsonVal::Str(v) => v.parse().unwrap_or_default(),
                    _ => 0,
                }),
            });
        }
        Some(Native::ArrayListSerializer { child }) => *child,
        _ => {
            let desc = match serializer {
                JValue::Obj(o) => vm.arena.objects[o as usize].class.to_string(),
                _ => String::new(),
            };
            return Err(nat_fatal(JvmError::Resolution(format!(
                "decode: unsupported serializer class {desc}"
            ))));
        }
    };
    let items = match payload(vm, element) {
        Some(Native::Json(JsonVal::Array(items))) => items.clone(),
        _ => Vec::new(),
    };
    let mut out = Vec::with_capacity(items.len());
    for item in &items {
        let node = alloc_json_node(vm, item)?;
        let dec = json_decoder(vm, node)?;
        out.push(invoke_deserialize(vm, child, dec)?);
    }
    alloc(vm, "Ljava/util/ArrayList;", Native::List(out))
}

fn json_encoder(vm: &mut Vm) -> R {
    alloc(
        vm,
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        Native::JsonEncoder {
            value: None,
            elements: Vec::new(),
        },
    )
}

/// Encodes a value using either a host serializer marker or the extension's
/// generated serializer bytecode.
fn run_serializer_encode(
    vm: &mut Vm,
    serializer: JValue,
    value: JValue,
) -> Result<JsonVal, NatErr> {
    match payload(vm, serializer) {
        Some(Native::JsonElementSerializer) => {
            return match payload(vm, value) {
                Some(Native::Json(value)) => Ok(value.clone()),
                _ => Err(iae(vm, "JsonElement serializer expects JsonElement")),
            };
        }
        Some(Native::PrimitiveSerializer(kind)) => {
            return Ok(match kind {
                PrimitiveSerializerKind::String => JsonVal::Str(jstr(vm, value)?),
                PrimitiveSerializerKind::Int => JsonVal::Int(i64::from(int_of(vm, value))),
                PrimitiveSerializerKind::Long => JsonVal::Int(long_of(vm, value)),
            });
        }
        Some(Native::ArrayListSerializer { child }) => {
            let child = *child;
            let values = coll_elems(vm, value)?;
            let mut encoded = Vec::with_capacity(values.len());
            for value in values {
                encoded.push(run_serializer_encode(vm, child, value)?);
            }
            return Ok(JsonVal::Array(encoded));
        }
        _ => {}
    }

    let encoder = json_encoder(vm)?;
    invoke_serialize(vm, serializer, encoder, value)?;
    match payload(vm, encoder) {
        Some(Native::JsonEncoder {
            value: Some(value), ..
        }) => Ok(value.clone()),
        _ => Err(nat_fatal(JvmError::Resolution(
            "serializer produced no JSON value".into(),
        ))),
    }
}

// ---------------------------------------------------------------------------
// Json / Okio entry points
// ---------------------------------------------------------------------------

/// `Json.decodeFromJsonElement(strategy, element)`.
pub(crate) fn json_decode_from_json_element(vm: &mut Vm, args: &[JValue]) -> R {
    run_serializer(vm, args[1], args[2])
}

/// `Json.decodeFromString(strategy, text)`.
pub(crate) fn json_decode_from_string(vm: &mut Vm, args: &[JValue]) -> R {
    let text = jstr(vm, args[2])?;
    let val =
        parse_json(&text).map_err(|e| nat_fatal(JvmError::Resolution(format!("json: {e}"))))?;
    let node = alloc_json_node(vm, &val)?;
    run_serializer(vm, args[1], node)
}

/// `Json.encodeToString(strategy, value)`.
pub(crate) fn json_encode_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let value = run_serializer_encode(vm, args[1], args[2])?;
    Ok(new_str(vm, &jsonval_to_json(&value)))
}

/// `Json.encodeToJsonElement(strategy, value)`.
pub(crate) fn json_encode_to_json_element(vm: &mut Vm, args: &[JValue]) -> R {
    let value = run_serializer_encode(vm, args[1], args[2])?;
    alloc_json_node(vm, &value)
}

/// `OkioStreamsKt.decodeFromBufferedSource(json, strategy, source)` — reads
/// the buffered source to the end and decodes it.
pub(crate) fn okio_decode_from_buffered_source(vm: &mut Vm, args: &[JValue]) -> R {
    let (bytes, pos) = match payload(vm, args[2]) {
        Some(Native::OkioBuf { bytes, pos }) => (bytes.clone(), *pos),
        _ => return Err(npe(vm)),
    };
    let text = String::from_utf8_lossy(&bytes[pos..]);
    let val =
        parse_json(&text).map_err(|e| nat_fatal(JvmError::Resolution(format!("json: {e}"))))?;
    let node = alloc_json_node(vm, &val)?;
    run_serializer(vm, args[1], node)
}

/// `OkioStreamsKt.encodeToBufferedSink(json, strategy, value, sink)`.
pub(crate) fn okio_encode_to_buffered_sink(vm: &mut Vm, args: &[JValue]) -> R {
    let value = run_serializer_encode(vm, args[1], args[2])?;
    let text = jsonval_to_json(&value);
    let Some(Native::OkioSink { bytes, closed, .. }) = payload_mut(vm, args[3]) else {
        return Err(npe(vm));
    };
    if *closed {
        return Err(ioe(vm, "closed"));
    }
    bytes.extend_from_slice(text.as_bytes());
    Ok(JValue::Null)
}

/// `JsonElement$Companion.serializer()` — the JsonElement serializer marker.
pub(crate) fn json_element_companion_serializer(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlinx/serialization/DeserializationStrategy;",
        Native::JsonElementSerializer,
    )
}

/// Lazy materializer for the `JsonElement.Companion` static field.
pub(crate) fn lazy_json_element_companion(vm: &mut Vm) -> JValue {
    alloc(
        vm,
        "Lkotlinx/serialization/json/JsonElement$Companion;",
        Native::Opaque,
    )
    .expect("JsonElement$Companion shim")
}

/// `OkioZstd.zstdDecompress(source)` / `zstdCompress(sink)` — identity: the
/// cache stores plain JSON (this VM never writes zstd frames).
pub(crate) fn zstd_identity(vm: &mut Vm, args: &[JValue]) -> R {
    let _ = vm;
    Ok(args[0])
}

// ---------------------------------------------------------------------------
// StreamingJsonDecoder (Decoder / CompositeDecoder)
// ---------------------------------------------------------------------------

/// `Decoder.beginStructure(descriptor)` — the host decoder is reused across
/// nesting levels, like kotlinx's StreamingJsonDecoder.
pub(crate) fn dec_begin_structure(vm: &mut Vm, args: &[JValue]) -> R {
    let _ = vm;
    Ok(args[0])
}

/// `CompositeDecoder.decodeSequentially()` — always sequential; the
/// generated serializers then walk elements by descriptor index.
pub(crate) fn dec_decode_sequentially(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(1))
}

/// `CompositeDecoder.decodeElementIndex(descriptor)` — walks the object
/// members in order, returning descriptor indexes of matching keys, or -1
/// when exhausted (non-sequential fallback; unused in the sequential path).
pub(crate) fn dec_decode_element_index(vm: &mut Vm, args: &[JValue]) -> R {
    let element = match payload(vm, args[0]) {
        Some(Native::JsonDecoder { element, .. }) => *element,
        _ => return Err(npe(vm)),
    };
    let members = object_members(vm, element);
    let names = match payload(vm, args[1]) {
        Some(Native::SerialDescriptor { elements, .. }) => elements.clone(),
        _ => Vec::new(),
    };
    let index = match payload_mut(vm, args[0]) {
        Some(Native::JsonDecoder { index, .. }) => *index,
        _ => return Err(npe(vm)),
    };
    let mut i = index as usize;
    while i < members.len() {
        let name = &members[i].0;
        if names.get(i).is_some_and(|n| n == name) {
            if let Some(Native::JsonDecoder { index, .. }) = payload_mut(vm, args[0]) {
                *index = (i + 1) as i32;
            }
            return Ok(JValue::Int(i as i32));
        }
        i += 1;
    }
    Ok(JValue::Int(-1))
}

/// `CompositeDecoder.decodeStringElement(descriptor, index)`.
pub(crate) fn dec_decode_string_element(vm: &mut Vm, args: &[JValue]) -> R {
    let element = match payload(vm, args[0]) {
        Some(Native::JsonDecoder { element, .. }) => *element,
        _ => return Err(npe(vm)),
    };
    let index = int_of(vm, args[2]);
    let s = member_by_index(vm, element, args[1], index)
        .map(|v| jsonval_to_string(&v))
        .unwrap_or_default();
    Ok(new_str(vm, &s))
}

fn member_primitive(vm: &Vm, args: &[JValue]) -> Option<JsonVal> {
    let element = match payload(vm, args[0]) {
        Some(Native::JsonDecoder { element, .. }) => *element,
        _ => return None,
    };
    let index = int_of(vm, args[2]);
    member_by_index(vm, element, args[1], index)
}

/// `CompositeDecoder.decodeIntElement(descriptor, index)`.
pub(crate) fn dec_decode_int_element(vm: &mut Vm, args: &[JValue]) -> R {
    let v = member_primitive(vm, args).unwrap_or(JsonVal::Int(0));
    Ok(JValue::Int(match v {
        JsonVal::Int(i) => i as i32,
        JsonVal::Double(d) => d as i32,
        JsonVal::Bool(b) => i32::from(b),
        JsonVal::Str(s) => s.parse().unwrap_or(0),
        _ => 0,
    }))
}

/// `CompositeDecoder.decodeLongElement(descriptor, index)`.
pub(crate) fn dec_decode_long_element(vm: &mut Vm, args: &[JValue]) -> R {
    let v = member_primitive(vm, args).unwrap_or(JsonVal::Int(0));
    Ok(JValue::Long(match v {
        JsonVal::Int(i) => i,
        JsonVal::Double(d) => d as i64,
        JsonVal::Bool(b) => i64::from(b),
        JsonVal::Str(s) => s.parse().unwrap_or(0),
        _ => 0,
    }))
}

/// `CompositeDecoder.decodeBooleanElement(descriptor, index)`.
pub(crate) fn dec_decode_bool_element(vm: &mut Vm, args: &[JValue]) -> R {
    let v = member_primitive(vm, args).unwrap_or(JsonVal::Bool(false));
    Ok(JValue::Int(match v {
        JsonVal::Bool(b) => i32::from(b),
        JsonVal::Str(s) => i32::from(!matches!(s.as_str(), "false" | "0" | "")),
        _ => 0,
    }))
}

/// `CompositeDecoder.decodeCollectionSize(descriptor)`.
pub(crate) fn dec_decode_collection_size(vm: &mut Vm, args: &[JValue]) -> R {
    let element = match payload(vm, args[0]) {
        Some(Native::JsonDecoder { element, .. }) => *element,
        _ => return Err(npe(vm)),
    };
    let n = match payload(vm, element) {
        Some(Native::Json(JsonVal::Array(items))) => items.len(),
        _ => 0,
    };
    Ok(JValue::Int(n as i32))
}

/// `CompositeDecoder.decodeSerializableElement(descriptor, index, serializer,
/// previous)`.
pub(crate) fn dec_decode_serializable_element(vm: &mut Vm, args: &[JValue]) -> R {
    let element = match payload(vm, args[0]) {
        Some(Native::JsonDecoder { element, .. }) => *element,
        _ => return Err(npe(vm)),
    };
    let index = int_of(vm, args[2]);
    let Some(child) = member_by_index(vm, element, args[1], index) else {
        return Ok(JValue::Null);
    };
    let child_node = alloc_json_node(vm, &child)?;
    run_serializer(vm, args[3], child_node)
}

/// `CompositeDecoder.decodeNullableSerializableElement(...)` — null members
/// decode to null, otherwise forwards to the non-null path.
pub(crate) fn dec_decode_nullable_serializable_element(vm: &mut Vm, args: &[JValue]) -> R {
    let element = match payload(vm, args[0]) {
        Some(Native::JsonDecoder { element, .. }) => *element,
        _ => return Err(npe(vm)),
    };
    let index = int_of(vm, args[2]);
    let Some(child) = member_by_index(vm, element, args[1], index) else {
        return Ok(JValue::Null);
    };
    if matches!(child, JsonVal::Null) {
        return Ok(JValue::Null);
    }
    let child_node = alloc_json_node(vm, &child)?;
    run_serializer(vm, args[3], child_node)
}

/// `CompositeDecoder.endStructure(descriptor)`.
pub(crate) fn dec_end_structure(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// `CompositeDecoder.decodeJsonElement()` (Decoder extension) — returns the
/// element currently being decoded.
pub(crate) fn dec_decode_json_element(vm: &mut Vm, args: &[JValue]) -> R {
    let element = match payload(vm, args[0]) {
        Some(Native::JsonDecoder { element, .. }) => *element,
        _ => return Err(npe(vm)),
    };
    Ok(element)
}

/// Top-level `Decoder.decodeString()`.
pub(crate) fn dec_decode_string(vm: &mut Vm, args: &[JValue]) -> R {
    let element = match payload(vm, args[0]) {
        Some(Native::JsonDecoder { element, .. }) => *element,
        _ => return Err(npe(vm)),
    };
    let s = match payload(vm, element) {
        Some(Native::Json(v)) => jsonval_to_string(v),
        _ => String::new(),
    };
    Ok(new_str(vm, &s))
}

/// Top-level `Decoder.decodeInt()`.
pub(crate) fn dec_decode_int(vm: &mut Vm, args: &[JValue]) -> R {
    let element = match payload(vm, args[0]) {
        Some(Native::JsonDecoder { element, .. }) => *element,
        _ => return Err(npe(vm)),
    };
    let v = match payload(vm, element) {
        Some(Native::Json(JsonVal::Int(i))) => *i as i32,
        Some(Native::Json(JsonVal::Double(d))) => *d as i32,
        Some(Native::Json(JsonVal::Bool(b))) => i32::from(*b),
        _ => 0,
    };
    Ok(JValue::Int(v))
}

// ---------------------------------------------------------------------------
// StreamingJsonEncoder (Encoder / CompositeEncoder)
// ---------------------------------------------------------------------------

pub(crate) fn enc_begin_structure(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::JsonEncoder { value, elements }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *value = None;
    elements.clear();
    Ok(args[0])
}

fn descriptor_element_name(vm: &Vm, descriptor: JValue, index: i32) -> String {
    match payload(vm, descriptor) {
        Some(Native::SerialDescriptor { elements, .. }) => elements
            .get(index as usize)
            .cloned()
            .unwrap_or_else(|| index.to_string()),
        _ => index.to_string(),
    }
}

fn encoder_push_member(vm: &mut Vm, args: &[JValue], value: JsonVal) -> R {
    let name = descriptor_element_name(vm, args[1], int_of(vm, args[2]));
    let Some(Native::JsonEncoder { elements, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    elements.push((name, value));
    Ok(JValue::Null)
}

pub(crate) fn enc_encode_string_element(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[3])?;
    encoder_push_member(vm, args, JsonVal::Str(value))
}

pub(crate) fn enc_encode_int_element(vm: &mut Vm, args: &[JValue]) -> R {
    encoder_push_member(vm, args, JsonVal::Int(i64::from(int_of(vm, args[3]))))
}

pub(crate) fn enc_encode_long_element(vm: &mut Vm, args: &[JValue]) -> R {
    encoder_push_member(vm, args, JsonVal::Int(long_of(vm, args[3])))
}

pub(crate) fn enc_encode_bool_element(vm: &mut Vm, args: &[JValue]) -> R {
    encoder_push_member(vm, args, JsonVal::Bool(bool_of(vm, args[3])))
}

pub(crate) fn enc_encode_float_element(vm: &mut Vm, args: &[JValue]) -> R {
    encoder_push_member(vm, args, JsonVal::Double(f64::from(float_of(vm, args[3]))))
}

pub(crate) fn enc_encode_double_element(vm: &mut Vm, args: &[JValue]) -> R {
    encoder_push_member(vm, args, JsonVal::Double(double_of(vm, args[3])))
}

pub(crate) fn enc_encode_serializable_element(vm: &mut Vm, args: &[JValue]) -> R {
    let value = run_serializer_encode(vm, args[3], args[4])?;
    encoder_push_member(vm, args, value)
}

pub(crate) fn enc_encode_nullable_serializable_element(vm: &mut Vm, args: &[JValue]) -> R {
    if args[4].is_null_ref() {
        return encoder_push_member(vm, args, JsonVal::Null);
    }
    enc_encode_serializable_element(vm, args)
}

pub(crate) fn enc_should_encode_element_default(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(1))
}

pub(crate) fn enc_end_structure(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::JsonEncoder { value, elements }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *value = Some(JsonVal::Object(std::mem::take(elements)));
    Ok(JValue::Null)
}

fn encoder_set_value(vm: &mut Vm, encoder: JValue, value: JsonVal) -> R {
    let Some(Native::JsonEncoder { value: slot, .. }) = payload_mut(vm, encoder) else {
        return Err(npe(vm));
    };
    *slot = Some(value);
    Ok(JValue::Null)
}

pub(crate) fn enc_encode_string(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[1])?;
    encoder_set_value(vm, args[0], JsonVal::Str(value))
}

pub(crate) fn enc_encode_int(vm: &mut Vm, args: &[JValue]) -> R {
    encoder_set_value(vm, args[0], JsonVal::Int(i64::from(int_of(vm, args[1]))))
}

pub(crate) fn enc_encode_long(vm: &mut Vm, args: &[JValue]) -> R {
    encoder_set_value(vm, args[0], JsonVal::Int(long_of(vm, args[1])))
}

pub(crate) fn enc_encode_bool(vm: &mut Vm, args: &[JValue]) -> R {
    encoder_set_value(vm, args[0], JsonVal::Bool(bool_of(vm, args[1])))
}

pub(crate) fn enc_encode_float(vm: &mut Vm, args: &[JValue]) -> R {
    encoder_set_value(
        vm,
        args[0],
        JsonVal::Double(f64::from(float_of(vm, args[1]))),
    )
}

pub(crate) fn enc_encode_double(vm: &mut Vm, args: &[JValue]) -> R {
    encoder_set_value(vm, args[0], JsonVal::Double(double_of(vm, args[1])))
}

/// The nullable wrapper only changes null handling at call sites in the
/// generated serializer, so retaining the underlying serializer is enough.
pub(crate) fn builtin_get_nullable(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) fn throw_missing_field(vm: &mut Vm, _args: &[JValue]) -> R {
    Err(iae(vm, "required serialized field is missing"))
}

fn lazy_primitive_serializer(vm: &mut Vm, desc: &str, kind: PrimitiveSerializerKind) -> JValue {
    alloc(vm, desc, Native::PrimitiveSerializer(kind)).expect("primitive serializer shim")
}

pub(crate) fn lazy_string_serializer(vm: &mut Vm) -> JValue {
    lazy_primitive_serializer(
        vm,
        "Lkotlinx/serialization/internal/StringSerializer;",
        PrimitiveSerializerKind::String,
    )
}

pub(crate) fn lazy_int_serializer(vm: &mut Vm) -> JValue {
    lazy_primitive_serializer(
        vm,
        "Lkotlinx/serialization/internal/IntSerializer;",
        PrimitiveSerializerKind::Int,
    )
}

pub(crate) fn lazy_long_serializer(vm: &mut Vm) -> JValue {
    lazy_primitive_serializer(
        vm,
        "Lkotlinx/serialization/internal/LongSerializer;",
        PrimitiveSerializerKind::Long,
    )
}

// ---------------------------------------------------------------------------
// descriptors
// ---------------------------------------------------------------------------

/// `PluginGeneratedSerialDescriptor.<init>(serialName, previous, count)`.
pub(crate) fn descriptor_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1]).unwrap_or_default();
    let desc = Native::SerialDescriptor {
        name,
        elements: Vec::new(),
    };
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *n = desc;
    Ok(JValue::Null)
}

/// `PluginGeneratedSerialDescriptor.addElement(name, isInline)`.
pub(crate) fn descriptor_add_element(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1]).unwrap_or_default();
    let Some(Native::SerialDescriptor { elements, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    elements.push(name);
    Ok(JValue::Null)
}

/// `getElementName(descriptor, index)`.
pub(crate) fn descriptor_get_element_name(vm: &mut Vm, args: &[JValue]) -> R {
    let index = int_of(vm, args[1]);
    let name = match payload(vm, args[0]) {
        Some(Native::SerialDescriptor { elements, .. }) => {
            elements.get(index as usize).cloned().unwrap_or_default()
        }
        _ => String::new(),
    };
    Ok(new_str(vm, &name))
}

/// `getElementsCount(descriptor)`.
pub(crate) fn descriptor_get_elements_count(vm: &mut Vm, args: &[JValue]) -> R {
    let n = match payload(vm, args[0]) {
        Some(Native::SerialDescriptor { elements, .. }) => elements.len(),
        _ => 0,
    };
    Ok(JValue::Int(n as i32))
}

/// `getSerialName(descriptor)`.
pub(crate) fn descriptor_get_serial_name(vm: &mut Vm, args: &[JValue]) -> R {
    let name = match payload(vm, args[0]) {
        Some(Native::SerialDescriptor { name, .. }) => name.clone(),
        _ => String::new(),
    };
    Ok(new_str(vm, &name))
}

// ---------------------------------------------------------------------------
// ArrayListSerializer
// ---------------------------------------------------------------------------

/// `ArrayListSerializer.<init>(elementSerializer)`.
pub(crate) fn array_list_serializer_init(vm: &mut Vm, args: &[JValue]) -> R {
    let child = args[1];
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *n = Native::ArrayListSerializer { child };
    Ok(JValue::Null)
}

/// `ArrayListSerializer.deserialize(decoder)` — decodes the array currently
/// under the decoder.
pub(crate) fn array_list_serializer_deserialize(vm: &mut Vm, args: &[JValue]) -> R {
    let element = match payload(vm, args[1]) {
        Some(Native::JsonDecoder { element, .. }) => *element,
        _ => return Err(npe(vm)),
    };
    run_serializer(vm, args[0], element)
}

// ---------------------------------------------------------------------------
// natives table
// ---------------------------------------------------------------------------

pub(crate) const SERIALIZATION_TABLE: &[NativeEntry] = &[
    ne!(
        "Lkotlinx/serialization/json/Json;",
        "parseToJsonElement",
        "(Ljava/lang/String;)Lkotlinx/serialization/json/JsonElement;",
        true,
        json_parse_to_element
    ),
    ne!(
        "Lkotlinx/serialization/json/Json;",
        "getSerializersModule",
        "()Lkotlinx/serialization/modules/SerializersModule;",
        true,
        json_get_serializers_module
    ),
    ne!(
        "Lkotlinx/serialization/json/JsonPrimitive;",
        "getContent",
        "()Ljava/lang/String;",
        true,
        json_primitive_content
    ),
    ne!(
        "Lkotlinx/serialization/json/JsonElementKt;",
        "getJsonPrimitive",
        "(Lkotlinx/serialization/json/JsonElement;)Lkotlinx/serialization/json/JsonPrimitive;",
        false,
        json_element_get_primitive
    ),
    ne!(
        "Lkotlinx/serialization/json/JsonElementKt;",
        "getJsonArray",
        "(Lkotlinx/serialization/json/JsonElement;)Lkotlinx/serialization/json/JsonArray;",
        false,
        json_element_get_array
    ),
    ne!(
        "Lkotlinx/serialization/json/JsonElementKt;",
        "getJsonObject",
        "(Lkotlinx/serialization/json/JsonElement;)Lkotlinx/serialization/json/JsonObject;",
        false,
        json_element_get_object
    ),
    ne!(
        "Lkotlinx/serialization/json/JsonObject;",
        "get",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        json_object_get
    ),
    ne!("Lkotlinx/serialization/json/JsonObject;", "<init>", "(Ljava/util/Map;)V", true, json_object_init),
    ne!("Lkotlinx/serialization/json/JsonObject;", "containsKey", "(Ljava/lang/Object;)Z", true, json_object_contains_key),
    ne!("Lkotlinx/serialization/json/JsonObjectBuilder;", "<init>", "()V", true, json_object_builder_init),
    ne!("Lkotlinx/serialization/json/JsonObjectBuilder;", "build", "()Lkotlinx/serialization/json/JsonObject;", true, json_object_builder_build),
    ne!("Lkotlinx/serialization/json/JsonObjectBuilder;", "put", "(Ljava/lang/String;Lkotlinx/serialization/json/JsonElement;)Lkotlinx/serialization/json/JsonElement;", true, json_object_builder_put),
    ne!("Lkotlinx/serialization/json/JsonElementBuildersKt;", "put", "(Lkotlinx/serialization/json/JsonObjectBuilder;Ljava/lang/String;Ljava/lang/String;)Lkotlinx/serialization/json/JsonElement;", false, json_builder_put_string),
    ne!("Lkotlinx/serialization/json/JsonElementBuildersKt;", "put", "(Lkotlinx/serialization/json/JsonObjectBuilder;Ljava/lang/String;Ljava/lang/Number;)Lkotlinx/serialization/json/JsonElement;", false, json_builder_put_number),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "JsonPrimitive", "(Ljava/lang/String;)Lkotlinx/serialization/json/JsonPrimitive;", false, json_primitive_string),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "getContentOrNull", "(Lkotlinx/serialization/json/JsonPrimitive;)Ljava/lang/String;", false, json_content_or_null),
    ne!("Lkotlinx/serialization/json/JsonPrimitive;", "isString", "()Z", true, json_primitive_is_string),
    ne!("Lkotlinx/serialization/json/JsonArray;", "<init>", "(Ljava/util/List;)V", true, json_array_init),
    ne!("Lkotlinx/serialization/json/JsonArray;", "get", "(I)Lkotlinx/serialization/json/JsonElement;", true, json_array_get),
    ne!("Lkotlinx/serialization/json/JsonArray;", "get", "(I)Ljava/lang/Object;", true, json_array_get),
    ne!("Lkotlinx/serialization/json/JsonArray;", "size", "()I", true, json_array_size),
    ne!("Lkotlinx/serialization/json/JsonArray;", "iterator", "()Ljava/util/Iterator;", true, json_array_iterator),
    ne!("Lkotlinx/serialization/json/JsonObject;", "values", "()Ljava/util/Collection;", true, json_object_values),
    ne!(
        "Lkotlinx/serialization/json/Json;",
        "decodeFromJsonElement",
        "(Lkotlinx/serialization/DeserializationStrategy;Lkotlinx/serialization/json/JsonElement;)Ljava/lang/Object;",
        true,
        json_decode_from_json_element
    ),
    ne!(
        "Lkotlinx/serialization/json/Json;",
        "decodeFromString",
        "(Lkotlinx/serialization/DeserializationStrategy;Ljava/lang/String;)Ljava/lang/Object;",
        true,
        json_decode_from_string
    ),
    ne!(
        "Lkotlinx/serialization/json/Json;",
        "encodeToString",
        "(Lkotlinx/serialization/SerializationStrategy;Ljava/lang/Object;)Ljava/lang/String;",
        true,
        json_encode_to_string
    ),
    ne!(
        "Lkotlinx/serialization/json/Json;",
        "encodeToJsonElement",
        "(Lkotlinx/serialization/SerializationStrategy;Ljava/lang/Object;)Lkotlinx/serialization/json/JsonElement;",
        true,
        json_encode_to_json_element
    ),
    ne!(
        "Lkotlinx/serialization/json/JsonElement$Companion;",
        "serializer",
        "()Lkotlinx/serialization/KSerializer;",
        true,
        json_element_companion_serializer
    ),
    ne!(
        "Lkotlinx/serialization/json/okio/OkioStreamsKt;",
        "decodeFromBufferedSource",
        "(Lkotlinx/serialization/json/Json;Lkotlinx/serialization/DeserializationStrategy;Lokio/BufferedSource;)Ljava/lang/Object;",
        false,
        okio_decode_from_buffered_source
    ),
    ne!(
        "Lkotlinx/serialization/json/okio/OkioStreamsKt;",
        "encodeToBufferedSink",
        "(Lkotlinx/serialization/json/Json;Lkotlinx/serialization/SerializationStrategy;Ljava/lang/Object;Lokio/BufferedSink;)V",
        false,
        okio_encode_to_buffered_sink
    ),
    ne!(
        "Lkotlinx/serialization/builtins/BuiltinSerializersKt;",
        "getNullable",
        "(Lkotlinx/serialization/KSerializer;)Lkotlinx/serialization/KSerializer;",
        false,
        builtin_get_nullable
    ),
    ne!(
        "Lkotlinx/serialization/internal/PluginExceptionsKt;",
        "throwMissingFieldException",
        "(IILkotlinx/serialization/descriptors/SerialDescriptor;)V",
        false,
        throw_missing_field
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "beginStructure",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;)Lkotlinx/serialization/encoding/CompositeEncoder;",
        true,
        enc_begin_structure
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeStringElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;ILjava/lang/String;)V",
        true,
        enc_encode_string_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeIntElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;II)V",
        true,
        enc_encode_int_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeLongElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;IJ)V",
        true,
        enc_encode_long_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeBooleanElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;IZ)V",
        true,
        enc_encode_bool_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeFloatElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;IF)V",
        true,
        enc_encode_float_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeDoubleElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;ID)V",
        true,
        enc_encode_double_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeSerializableElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;ILkotlinx/serialization/SerializationStrategy;Ljava/lang/Object;)V",
        true,
        enc_encode_serializable_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeNullableSerializableElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;ILkotlinx/serialization/SerializationStrategy;Ljava/lang/Object;)V",
        true,
        enc_encode_nullable_serializable_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "shouldEncodeElementDefault",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;I)Z",
        true,
        enc_should_encode_element_default
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "endStructure",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;)V",
        true,
        enc_end_structure
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeString",
        "(Ljava/lang/String;)V",
        true,
        enc_encode_string
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeInt",
        "(I)V",
        true,
        enc_encode_int
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeLong",
        "(J)V",
        true,
        enc_encode_long
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeBoolean",
        "(Z)V",
        true,
        enc_encode_bool
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeFloat",
        "(F)V",
        true,
        enc_encode_float
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonEncoder;",
        "encodeDouble",
        "(D)V",
        true,
        enc_encode_double
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "beginStructure",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;)Lkotlinx/serialization/encoding/CompositeDecoder;",
        true,
        dec_begin_structure
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeSequentially",
        "()Z",
        true,
        dec_decode_sequentially
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeElementIndex",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;)I",
        true,
        dec_decode_element_index
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeStringElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;I)Ljava/lang/String;",
        true,
        dec_decode_string_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeIntElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;I)I",
        true,
        dec_decode_int_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeLongElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;I)J",
        true,
        dec_decode_long_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeBooleanElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;I)Z",
        true,
        dec_decode_bool_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeCollectionSize",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;)I",
        true,
        dec_decode_collection_size
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeSerializableElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;ILkotlinx/serialization/DeserializationStrategy;Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        dec_decode_serializable_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeNullableSerializableElement",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;ILkotlinx/serialization/DeserializationStrategy;Ljava/lang/Object;)Ljava/lang/Object;",
        true,
        dec_decode_nullable_serializable_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "endStructure",
        "(Lkotlinx/serialization/descriptors/SerialDescriptor;)V",
        true,
        dec_end_structure
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeJsonElement",
        "()Lkotlinx/serialization/json/JsonElement;",
        true,
        dec_decode_json_element
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeString",
        "()Ljava/lang/String;",
        true,
        dec_decode_string
    ),
    ne!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        "decodeInt",
        "()I",
        true,
        dec_decode_int
    ),
    ne!(
        "Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;",
        "<init>",
        "(Ljava/lang/String;Lkotlinx/serialization/internal/GeneratedSerializer;I)V",
        true,
        descriptor_init
    ),
    ne!(
        "Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;",
        "addElement",
        "(Ljava/lang/String;Z)V",
        true,
        descriptor_add_element
    ),
    ne!(
        "Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;",
        "getElementName",
        "(I)Ljava/lang/String;",
        true,
        descriptor_get_element_name
    ),
    ne!(
        "Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;",
        "getElementsCount",
        "()I",
        true,
        descriptor_get_elements_count
    ),
    ne!(
        "Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;",
        "getSerialName",
        "()Ljava/lang/String;",
        true,
        descriptor_get_serial_name
    ),
    ne!("Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;", "pushAnnotation", "(Ljava/lang/annotation/Annotation;)V", true, descriptor_push_annotation),
    ne!(
        "Lkotlinx/serialization/internal/ArrayListSerializer;",
        "<init>",
        "(Lkotlinx/serialization/KSerializer;)V",
        true,
        array_list_serializer_init
    ),
    ne!(
        "Lkotlinx/serialization/internal/ArrayListSerializer;",
        "deserialize",
        "(Lkotlinx/serialization/encoding/Decoder;)Ljava/lang/Object;",
        true,
        array_list_serializer_deserialize
    ),
    ne!(
        "Lkotlinx/serialization/UnknownFieldException;",
        "<init>",
        "(I)V",
        true,
        descriptor_init_placeholder
    ),
];

/// `UnknownFieldException.<init>(index)` — allocated but never thrown in the
/// sequential decode path.
pub(crate) fn descriptor_init_placeholder(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let _ = n;
    Ok(JValue::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    #[test]
    fn json_tree_accessors_parse_and_navigate() {
        let data = std::fs::read("fixtures/classes.dex").unwrap();
        let mut ctx = Context::new(&data).unwrap();
        let vm = ctx.vm();
        let json = alloc(vm, "Lkotlinx/serialization/json/Json;", Native::Opaque).unwrap();
        let text = new_str(vm, r#"{"name":"Dex","items":[1,2]}"#);
        let root = json_parse_to_element(vm, &[json, text]).unwrap();
        assert_eq!(json_element_get_object(vm, &[root]).unwrap(), root);

        let key = new_str(vm, "name");
        let name = json_object_get(vm, &[root, key]).unwrap();
        let content = json_primitive_content(vm, &[name]).unwrap();
        assert_eq!(jstr(vm, content).unwrap(), "Dex");

        let key = new_str(vm, "items");
        let items = json_object_get(vm, &[root, key]).unwrap();
        assert_eq!(json_element_get_array(vm, &[items]).unwrap(), items);
        assert_eq!(json_array_size(vm, &[items]).unwrap(), JValue::Int(2));
        let first = json_array_get(vm, &[items, JValue::Int(0)]).unwrap();
        let content = json_primitive_content(vm, &[first]).unwrap();
        assert_eq!(jstr(vm, content).unwrap(), "1");

        let iterator = json_array_iterator(vm, &[items]).unwrap();
        assert_eq!(iter_has_next(vm, &[iterator]).unwrap(), JValue::Int(1));
    }

    #[test]
    fn generated_dex_serializer_encodes_through_host_runtime() {
        let apk =
            std::fs::read("fixtures/tachiyomi-vi.moetruyen-v1.6.8.apk").expect("moetruyen fixture");
        let mut ctx = Context::new(&apk).expect("load fixture");

        let model_class = ctx.vm().ensure_class_by_desc("Li;").expect("model class");
        let model = ctx.vm().alloc_instance(model_class).expect("model");
        let name = ctx.vm().alloc_string("Action");
        let id = ctx.vm().alloc_string("1");
        ctx.invoke_on(
            model,
            "<init>",
            "(Ljava/lang/String;Ljava/lang/String;)V",
            &[name, id],
        )
        .expect("model constructor");

        let serializer_class = ctx
            .vm()
            .ensure_class_by_desc("Lg;")
            .expect("serializer class");
        let serializer = ctx
            .vm()
            .alloc_instance(serializer_class)
            .expect("serializer");
        let encoded = run_serializer_encode(ctx.vm(), JValue::Obj(serializer), JValue::Obj(model))
            .expect("generated serializer");
        assert_eq!(
            encoded,
            JsonVal::Object(vec![
                ("name".into(), JsonVal::Str("Action".into())),
                ("id".into(), JsonVal::Str("1".into())),
            ])
        );
    }
}
