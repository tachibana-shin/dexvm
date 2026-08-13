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

fn summary_of(v: &JsonVal) -> String {
    match v {
        JsonVal::Array(items) => format!("array[{}]", items.len()),
        JsonVal::Object(m) => format!("object{{{}}}", m.len()),
        JsonVal::Str(s) => format!("str({s})"),
        JsonVal::Int(i) => format!("int({i})"),
        JsonVal::Double(d) => format!("double({d})"),
        JsonVal::Bool(b) => format!("bool({b})"),
        JsonVal::Null => "null".into(),
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
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!("DEXVM_TRACE native invoke_deserialize");
    }
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

fn json_decoder(vm: &mut Vm, element: JValue, module: Option<JValue>) -> R {
    alloc(
        vm,
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        Native::JsonDecoder {
            element,
            members: None,
            index: 0,
            module,
        },
    )
}

/// SerializersModule attached to a Json object, if any.
fn json_module_of(vm: &mut Vm, json: JValue) -> Option<JValue> {
    match payload(vm, json) {
        Some(Native::JsonWithModule { module }) => Some(*module),
        _ => None,
    }
}

/// Runs a serializer over an element tree: `JsonElementSerializer` returns
/// the element itself; `ArrayListSerializer` decodes each array item through
/// its child serializer.
fn run_serializer(vm: &mut Vm, serializer: JValue, element: JValue, module: Option<JValue>) -> R {
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
        Some(Native::LinkedHashMapSerializer { value, .. }) => {
            return decode_map(vm, serializer, element, module, *value);
        }
        _ => {
            if let JValue::Obj(_) = serializer {
                let dec = json_decoder(vm, element, module)?;
                return invoke_deserialize(vm, serializer, dec);
            }
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
    if std::env::var("DEXVM_TRACE").is_ok() {
        let cls: &str = match payload(vm, element) {
            Some(Native::Json(JsonVal::Array(_))) => "array",
            Some(Native::Json(JsonVal::Object(_))) => "object",
            Some(Native::Json(_)) => "scalar",
            _ => "none",
        };
        eprintln!(
            "DEXVM_TRACE run_serializer array-branch element={cls} items={}",
            items.len()
        );
    }
    let mut out = Vec::with_capacity(items.len());
    for item in &items {
        let node = alloc_json_node(vm, item)?;
        let dec = json_decoder(vm, node, module)?;
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
    let module = json_module_of(vm, args[0]);
    run_serializer(vm, args[1], args[2], module)
}

/// `Json.decodeFromString(strategy, text)`.
pub(crate) fn json_decode_from_string(vm: &mut Vm, args: &[JValue]) -> R {
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!("DEXVM_TRACE native json_decode_from_string");
    }
    let text = jstr(vm, args[2])?;
    let val =
        parse_json(&text).map_err(|e| nat_fatal(JvmError::Resolution(format!("json: {e}"))))?;
    let module = json_module_of(vm, args[0]);
    let node = alloc_json_node(vm, &val)?;
    run_serializer(vm, args[1], node, module)
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
    let module = json_module_of(vm, args[0]);
    let node = alloc_json_node(vm, &val)?;
    run_serializer(vm, args[1], node, module)
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
    .unwrap_or(JValue::Null)
}

/// `OkioZstd.zstdDecompress(source)` / `zstdCompress(sink)` — identity: the
/// cache stores plain JSON (this VM never writes zstd frames).
pub(crate) fn zstd_identity(vm: &mut Vm, args: &[JValue]) -> R {
    let _ = vm;
    Ok(args[0])
}

pub(crate) fn image_decoder_get_dimension(vm: &mut Vm, _args: &[JValue]) -> R {
    let _ = vm;
    log::warn!("tachiyomi.decoder.ImageDecoder is a stub; returning 0");
    Ok(JValue::Int(0))
}

pub(crate) fn image_decoder_recycle(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

// ---------------------------------------------------------------------------
// StreamingJsonDecoder (Decoder / CompositeDecoder)
// ---------------------------------------------------------------------------

/// `Decoder.beginStructure(descriptor)` — the host decoder is reused across
/// nesting levels, like kotlinx's StreamingJsonDecoder.
pub(crate) fn dec_begin_structure(vm: &mut Vm, args: &[JValue]) -> R {
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!("DEXVM_TRACE native dec_begin_structure");
    }
    let _ = vm;
    Ok(args[0])
}

/// `CompositeDecoder.decodeSequentially()` — always sequential; the
/// generated serializers then walk elements by descriptor index.
pub(crate) fn dec_decode_sequentially(_vm: &mut Vm, _args: &[JValue]) -> R {
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!("DEXVM_TRACE native dec_decode_sequentially");
    }
    Ok(JValue::Int(1))
}

/// `CompositeDecoder.decodeElementIndex(descriptor)` — walks the object
/// members in order, returning descriptor indexes of matching keys, or -1
/// when exhausted (non-sequential fallback; unused in the sequential path).
pub(crate) fn dec_decode_element_index(vm: &mut Vm, args: &[JValue]) -> R {
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!("DEXVM_TRACE native dec_decode_element_index");
    }
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
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!("DEXVM_TRACE native dec_decode_string_element");
    }
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
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!("DEXVM_TRACE native dec_decode_int_element");
    }
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
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!("DEXVM_TRACE native dec_decode_serializable_element");
    }
    let (element, module) = match payload(vm, args[0]) {
        Some(Native::JsonDecoder {
            element, module, ..
        }) => (*element, *module),
        _ => return Err(npe(vm)),
    };
    let index = int_of(vm, args[2]);
    let Some(child) = member_by_index(vm, element, args[1], index) else {
        return Ok(JValue::Null);
    };
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!(
            "DEXVM_TRACE decodeSerializableElement idx={index} child={:?}",
            summary_of(&child)
        );
    }
    let child_node = alloc_json_node(vm, &child)?;
    run_serializer(vm, args[3], child_node, module)
}

/// `CompositeDecoder.decodeNullableSerializableElement(...)` — null members
/// decode to null, otherwise forwards to the non-null path.
pub(crate) fn dec_decode_nullable_serializable_element(vm: &mut Vm, args: &[JValue]) -> R {
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!("DEXVM_TRACE native dec_decode_nullable_serializable_element");
    }
    let (element, module) = match payload(vm, args[0]) {
        Some(Native::JsonDecoder {
            element, module, ..
        }) => (*element, *module),
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
    run_serializer(vm, args[3], child_node, module)
}

/// `CompositeDecoder.endStructure(descriptor)`.
pub(crate) fn dec_end_structure(_vm: &mut Vm, _args: &[JValue]) -> R {
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!("DEXVM_TRACE native dec_end_structure");
    }
    Ok(JValue::Null)
}

/// `CompositeDecoder.decodeJsonElement()` (Decoder extension) — returns the
/// element currently being decoded.
pub(crate) fn dec_decode_json_element(vm: &mut Vm, args: &[JValue]) -> R {
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!("DEXVM_TRACE native dec_decode_json_element");
    }
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
    alloc(vm, desc, Native::PrimitiveSerializer(kind)).unwrap_or(JValue::Null)
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
    let (element, module) = match payload(vm, args[1]) {
        Some(Native::JsonDecoder {
            element, module, ..
        }) => (*element, *module),
        _ => return Err(npe(vm)),
    };
    run_serializer(vm, args[0], element, module)
}

// ---------------------------------------------------------------------------
// serializer construction / JsonElement builders
// ---------------------------------------------------------------------------

/// Shared no-op `<init>` for serializer shim classes that only carry the
/// serializer object around (the real dex bytecode drives the encode/decode).
fn serializer_opaque_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Opaque);
    Ok(JValue::Null)
}

/// `JsonObject$Companion.serializer()` / `JsonArray$Companion.serializer()`
/// — the JsonElement serializer marker (decode/encode return the tree node).
fn json_element_serializer_marker(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlinx/serialization/DeserializationStrategy;",
        Native::JsonElementSerializer,
    )
}

/// `JsonNames.names()` — annotation alternatives; none by default.
fn json_names_names(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc_empty_arr(vm, "Ljava/lang/String;")
}

/// `BinaryFormat.getSerializersModule()` — the module is opaque.
fn serializers_module_alloc(_vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        _vm,
        "Lkotlinx/serialization/modules/SerializersModule;",
        Native::Opaque,
    )
}

/// `Json.getSerializersModule()` — returns the module carried by Json, if any.
fn json_get_serializers_module(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::JsonWithModule { module }) => Ok(*module),
        _ => alloc(
            vm,
            "Lkotlinx/serialization/modules/SerializersModule;",
            Native::Opaque,
        ),
    }
}

/// `SerializersModuleBuilder.<init>()` — starts an empty registry.
fn serializers_module_builder_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::SerializersModule { polys: Vec::new() });
    Ok(JValue::Null)
}

/// `PolymorphicModuleBuilder.<init>(base, serializer)` — opens one
/// polymorphic base registration.
fn polymorphic_module_builder_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::SerializersModule {
        polys: vec![(args[1], Vec::new(), None)],
    });
    Ok(JValue::Null)
}

/// `PolymorphicModuleBuilder.subclass(kclass, serializer)` — registers a
/// subtype on the currently open base.
fn polymorphic_module_builder_subclass(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    let Some(Native::SerializersModule { polys }) = payload_mut(vm, JValue::Obj(this)) else {
        return Err(npe(vm));
    };
    let Some((_, subs, _)) = polys.last_mut() else {
        return Err(npe(vm));
    };
    subs.push((args[1], args[2]));
    Ok(JValue::Null)
}

/// `PolymorphicModuleBuilder.defaultDeserializer(lambda)` — fallback for
/// unknown discriminator values.
fn polymorphic_module_builder_default_deserializer(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    let Some(Native::SerializersModule { polys }) = payload_mut(vm, JValue::Obj(this)) else {
        return Err(npe(vm));
    };
    let Some((_, _, slot)) = polys.last_mut() else {
        return Err(npe(vm));
    };
    *slot = Some(args[1]);
    Ok(JValue::Null)
}

/// `PolymorphicModuleBuilder.buildTo(builder)` — merges the registration
/// into the SerializersModuleBuilder.
fn polymorphic_module_builder_build_to(vm: &mut Vm, args: &[JValue]) -> R {
    let polys = match payload(vm, args[0]) {
        Some(Native::SerializersModule { polys }) => polys.clone(),
        _ => Vec::new(),
    };
    let Some(Native::SerializersModule { polys: target }) = payload_mut(vm, args[1]) else {
        return Err(npe(vm));
    };
    target.extend(polys);
    Ok(JValue::Null)
}

/// `SerializersModuleBuilder.build()` — yields the completed module.
fn serializers_module_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let polys = match payload(vm, args[0]) {
        Some(Native::SerializersModule { polys }) => polys.clone(),
        _ => Vec::new(),
    };
    alloc(
        vm,
        "Lkotlinx/serialization/modules/SerializersModule;",
        Native::SerializersModule { polys },
    )
}

/// `SerializersModuleKt.plus(a, b)` — folds the second module into the first.
fn serializers_module_plus(vm: &mut Vm, args: &[JValue]) -> R {
    let mut polys = match payload(vm, args[0]) {
        Some(Native::SerializersModule { polys }) => polys.clone(),
        _ => Vec::new(),
    };
    if let Some(Native::SerializersModule { polys: more }) = payload(vm, args[1]) {
        polys.extend(more.clone());
    }
    alloc(
        vm,
        "Lkotlinx/serialization/modules/SerializersModule;",
        Native::SerializersModule { polys },
    )
}

/// `JsonBuilder.setSerializersModule(module)` — keeps the module on the
/// builder so `Json$default` can attach it to the resulting Json.
fn json_builder_set_serializers_module(vm: &mut Vm, args: &[JValue]) -> R {
    let polys = match payload(vm, args[1]) {
        Some(Native::SerializersModule { polys }) => polys.clone(),
        _ => Vec::new(),
    };
    if let JValue::Obj(o) = args[0] {
        vm.arena.objects[o as usize].native = Some(Native::SerializersModule { polys });
    }
    Ok(JValue::Null)
}

/// `PolymorphicSerializer.<init>(kclass, annotations)` — remembers the base.
fn polymorphic_serializer_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Polymorphic { base: args[1] });
    Ok(JValue::Null)
}

/// `PolymorphicSerializer.getDescriptor()` — minimal base descriptor.
fn polymorphic_get_descriptor(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;",
        Native::SerialDescriptor {
            name: "Polymorphic".into(),
            elements: Vec::new(),
        },
    )
}

/// Resolves a serializer object's descriptor name through the guest's
/// `getDescriptor()` (plugin generated serializers carry the serial name).
fn serializer_descriptor_name(vm: &mut Vm, serializer: JValue) -> Result<String, NatErr> {
    let JValue::Obj(o) = serializer else {
        return Ok(String::new());
    };
    let mref = MethodRef {
        name: vm.intern("getDescriptor"),
        sig: vm.intern("()Lkotlinx/serialization/descriptors/SerialDescriptor;"),
        ret: 0,
        args: Vec::new(),
        class_desc: 0,
    };
    let target = vm
        .resolve_target(InvokeKind::Interface, &mref, Some(o), 0)
        .map_err(nat_fatal)?;
    let desc = vm
        .call_target(target, vec![serializer])
        .map_err(nat_fatal)?;
    match payload(vm, desc) {
        Some(Native::SerialDescriptor { name, .. }) => Ok(name.clone()),
        _ => Ok(String::new()),
    }
}

/// `PolymorphicSerializer.deserialize(decoder)` — reads the "type"
/// discriminator and dispatches to the subtype serializer registered in the
/// Json's SerializersModule.
fn polymorphic_deserialize(vm: &mut Vm, args: &[JValue]) -> R {
    let base = match payload(vm, args[0]) {
        Some(Native::Polymorphic { base }) => *base,
        _ => {
            return Err(nat_fatal(JvmError::Resolution(
                "polymorphic: no base class".into(),
            )))
        }
    };
    let (element, module) = match payload(vm, args[1]) {
        Some(Native::JsonDecoder {
            element, module, ..
        }) => (*element, *module),
        _ => {
            return Err(nat_fatal(JvmError::Resolution(
                "polymorphic: no decoder".into(),
            )))
        }
    };
    let discriminator = match payload(vm, element) {
        Some(Native::Json(JsonVal::Object(entries))) => entries
            .iter()
            .find(|(k, _)| k == "type")
            .and_then(|(_, v)| match v {
                JsonVal::Str(s) => Some(s.clone()),
                _ => None,
            }),
        _ => None,
    };
    let Some(discriminator) = discriminator else {
        return Err(nat_fatal(JvmError::Resolution(
            "polymorphic: no type discriminator".into(),
        )));
    };
    let Some(module) = module else {
        return Err(nat_fatal(JvmError::Resolution(format!(
            "polymorphic: no module for {discriminator}"
        ))));
    };
    let polys = match payload(vm, module) {
        Some(Native::SerializersModule { polys }) => polys.clone(),
        _ => Vec::new(),
    };
    let mut found = None;
    let mut fallback = None;
    let mut candidates = Vec::new();
    for (poly_base, subs, default) in polys {
        if poly_base == base {
            fallback = default;
            for (kclass, serializer) in subs {
                let name = serializer_descriptor_name(vm, serializer)?;
                candidates.push(name.clone());
                if name == discriminator {
                    found = Some((kclass, serializer));
                    break;
                }
            }
        }
    }
    let serializer = if let Some((_, serializer)) = found {
        serializer
    } else {
        let Some(default) = fallback else {
            return Err(nat_fatal(JvmError::Resolution(format!(
                "polymorphic: no subtype for {discriminator} (candidates: {})",
                candidates.join(",")
            ))));
        };
        let key = new_str(vm, &discriminator);
        invoke_function1(vm, default, key)?
    };
    run_serializer(vm, serializer, element, Some(module))
}

/// `JsonBuilder.set*(...)` — builder configuration is not tracked.
fn json_builder_set(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// `JsonKt.Json$default(json, block, mask, marker)` — runs the builder block
/// over a fresh `JsonBuilder` and returns the original Json.
fn json_builder_default(vm: &mut Vm, args: &[JValue]) -> R {
    let mut module = None;
    if int_of(vm, args[2]) & 0x2 == 0 {
        let builder = alloc(
            vm,
            "Lkotlinx/serialization/json/JsonBuilder;",
            Native::Opaque,
        )?;
        invoke_function1(vm, args[1], builder)?;
        if let Some(Native::SerializersModule { polys }) = payload(vm, builder) {
            let polys = polys.clone();
            if !polys.is_empty() {
                module = Some(alloc(
                    vm,
                    "Lkotlinx/serialization/modules/SerializersModule;",
                    Native::SerializersModule { polys },
                )?);
            }
        }
    }
    let this = if args[0].is_null_ref() {
        alloc(vm, "Lkotlinx/serialization/json/Json;", Native::Opaque)?
    } else {
        args[0]
    };
    if let (Some(module), JValue::Obj(o)) = (module, this) {
        vm.arena.objects[o as usize].native = Some(Native::JsonWithModule { module });
    }
    Ok(this)
}

/// `ProtoNumber.number()` — protobuf field annotation; 0 by default.
fn proto_number_number(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

/// `JsonClassDiscriminator.discriminator()` — kotlinx default discriminator.
fn json_class_discriminator(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(new_str(vm, "type"))
}

/// `SerialDescriptorsKt.PrimitiveSerialDescriptor(name, kind)`.
fn primitive_serial_descriptor(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[0]).unwrap_or_default();
    alloc(
        vm,
        "Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;",
        Native::SerialDescriptor {
            name,
            elements: Vec::new(),
        },
    )
}

/// `SerialDescriptorsKt.buildClassSerialDescriptor$default(...)` — element
/// configuration is driven by the generated serializer's addElement calls.
fn build_class_descriptor_default(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[0]).unwrap_or_default();
    alloc(
        vm,
        "Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;",
        Native::SerialDescriptor {
            name,
            elements: Vec::new(),
        },
    )
}

/// `InlineClassDescriptor.<init>(name, serializer)`.
fn inline_class_descriptor_init(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1]).unwrap_or_default();
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::SerialDescriptor {
        name,
        elements: Vec::new(),
    });
    Ok(JValue::Null)
}

/// `InlineClassDescriptor.addElement(name, isInline)`.
fn inline_class_descriptor_add_element(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[1]).unwrap_or_default();
    let Some(Native::SerialDescriptor { elements, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    elements.push(name);
    Ok(JValue::Null)
}

/// `ArrayListSerializer.getDescriptor()`.
fn array_list_serializer_descriptor(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;",
        Native::SerialDescriptor {
            name: "kotlin.collections.ArrayList".into(),
            elements: Vec::new(),
        },
    )
}

/// `BuiltinSerializersKt.ListSerializer(child)`.
fn list_serializer(vm: &mut Vm, args: &[JValue]) -> R {
    let child = args[0];
    alloc(
        vm,
        "Lkotlinx/serialization/internal/ArrayListSerializer;",
        Native::ArrayListSerializer { child },
    )
}

/// `BuiltinSerializersKt.MapSerializer(k, v)`.
fn map_serializer(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlinx/serialization/internal/LinkedHashMapSerializer;",
        Native::Opaque,
    )
}

/// `BuiltinSerializersKt.serializer(StringCompanionObject)`.
fn string_serializer_of(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(lazy_primitive_serializer(
        vm,
        "Lkotlinx/serialization/internal/StringSerializer;",
        PrimitiveSerializerKind::String,
    ))
}

/// `EnumsKt.createSimpleEnumSerializer` / `createAnnotatedEnumSerializer`.
fn enum_serializer_create(vm: &mut Vm, args: &[JValue]) -> R {
    let values = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Obj(values))) => values.clone(),
        _ => Vec::new(),
    };
    let mut names = Vec::with_capacity(values.len());
    if let Some(Native::Array(ArrayData::Obj(ser_names))) = payload(vm, args[2]) {
        let ser_names = ser_names.clone();
        for n in ser_names {
            names.push(jstr(vm, n).unwrap_or_default());
        }
    }
    if names.len() != values.len() {
        for v in &values {
            let name = match payload(vm, *v) {
                Some(Native::Enum { name, .. }) => name.clone(),
                _ => String::new(),
            };
            names.push(name);
        }
    }
    alloc(
        vm,
        "Lkotlinx/serialization/internal/EnumSerializer;",
        Native::EnumSerializer { values, names },
    )
}

/// `EnumSerializer.deserialize(decoder)` — maps the JSON string value to the
/// enum constant via its serial name.
fn enum_serializer_deserialize(vm: &mut Vm, args: &[JValue]) -> R {
    let (values, names) = match payload(vm, args[0]) {
        Some(Native::EnumSerializer { values, names }) => (values.clone(), names.clone()),
        _ => return Err(npe(vm)),
    };
    let element = match payload(vm, args[1]) {
        Some(Native::JsonDecoder { element, .. }) => *element,
        _ => return Err(npe(vm)),
    };
    let value = match payload(vm, element) {
        Some(Native::Json(JsonVal::Str(s))) => s.clone(),
        _ => String::new(),
    };
    for (i, name) in names.iter().enumerate() {
        if *name == value {
            return Ok(values.get(i).copied().unwrap_or(JValue::Null));
        }
    }
    Err(nat_fatal(JvmError::Resolution(format!(
        "enum: no constant for {value}"
    ))))
}

fn decode_map(
    vm: &mut Vm,
    _serializer: JValue,
    element: JValue,
    module: Option<JValue>,
    value_ser: JValue,
) -> R {
    let entries = match payload(vm, element) {
        Some(Native::Json(JsonVal::Object(entries))) => entries.clone(),
        _ => Vec::new(),
    };
    let mut out = Vec::with_capacity(entries.len());
    for (key, val) in &entries {
        let kobj = new_str(vm, key);
        let node = alloc_json_node(vm, val)?;
        let v = run_serializer(vm, value_ser, node, module)?;
        if std::env::var("DEXVM_TRACE").is_ok() {
            let vc = match v {
                JValue::Obj(o) => vm
                    .class_desc_str(vm.arena.objects[o as usize].class)
                    .rsplit('/')
                    .next()
                    .unwrap_or("?")
                    .to_string(),
                other => format!("{other:?}"),
            };
            eprintln!("DEXVM_TRACE decode_map key={key} value={vc}");
        }
        out.push((kobj, v));
    }
    alloc(vm, "Ljava/util/LinkedHashMap;", Native::Map(out))
}

fn linked_hash_map_init(vm: &mut Vm, args: &[JValue]) -> R {
    let JValue::Obj(o) = args[0] else {
        return Err(npe(vm));
    };
    vm.arena.objects[o as usize].native = Some(Native::LinkedHashMapSerializer {
        key: args[1],
        value: args[2],
    });
    Ok(JValue::Null)
}

/// `LinkedHashMapSerializer.deserialize(decoder)` — decodes the decoder's
/// current JSON object into a LinkedHashMap.
fn linked_hash_map_deserialize(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::LinkedHashMapSerializer { value, .. }) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let (element, module) = match payload(vm, args[1]) {
        Some(Native::JsonDecoder {
            element, module, ..
        }) => (*element, *module),
        _ => return Err(npe(vm)),
    };
    decode_map(vm, args[0], element, module, *value)
}

fn linked_hash_map_descriptor(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(
        vm,
        "Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;",
        Native::SerialDescriptor {
            name: "LinkedHashMap".into(),
            elements: Vec::new(),
        },
    )
}

/// Direct `deserialize(Decoder)` call on primitive serializers
/// (StringSerializer, IntSerializer, LongSerializer).
fn primitive_serializer_deserialize(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::PrimitiveSerializer(kind)) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let kind = *kind;
    let value = match payload(vm, args[1]) {
        Some(Native::JsonDecoder { element, .. }) => match payload(vm, *element) {
            Some(Native::Json(value)) => value.clone(),
            _ => JsonVal::Null,
        },
        _ => JsonVal::Null,
    };
    Ok(match kind {
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
    })
}

/// `EnumSerializer.getDescriptor()` — the enum serial name.
fn enum_serializer_descriptor(vm: &mut Vm, args: &[JValue]) -> R {
    let (names, _) = match payload(vm, args[0]) {
        Some(Native::EnumSerializer { values, names }) => (names.clone(), values),
        _ => return Err(npe(vm)),
    };
    alloc(
        vm,
        "Lkotlinx/serialization/internal/EnumDescriptor;",
        Native::SerialDescriptor {
            name: String::new(),
            elements: names,
        },
    )
}

/// Invokes a `Function1` with one argument (unit lambdas like
/// `JsonObjectBuilder.() -> Unit` compile to Function1).
fn invoke_function1(vm: &mut Vm, f: JValue, arg: JValue) -> R {
    if f.is_null_ref() {
        return Ok(JValue::Null);
    }
    inv_virt(
        vm,
        f,
        "invoke",
        "(Ljava/lang/Object;)Ljava/lang/Object;",
        &[arg],
    )
}

/// `JsonArrayBuilder.<init>()`.
fn json_array_builder_init(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Json(JsonVal::Array(Vec::new())));
    Ok(JValue::Null)
}

/// `JsonArrayBuilder.add(element)Z` — appends and always returns true.
fn json_array_builder_add(vm: &mut Vm, args: &[JValue]) -> R {
    let element = match payload(vm, args[1]) {
        Some(Native::Json(value)) => value.clone(),
        _ => return Err(iae(vm, "JsonArrayBuilder value is not a JsonElement")),
    };
    match payload_mut(vm, args[0]) {
        Some(Native::Json(JsonVal::Array(values))) => values.push(element),
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Int(1))
}

/// `JsonArrayBuilder.build()`.
fn json_array_builder_build(vm: &mut Vm, args: &[JValue]) -> R {
    let array = match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Array(values))) => JsonVal::Array(values.clone()),
        _ => return Err(npe(vm)),
    };
    alloc_json_node(vm, &array)
}

/// `JsonElementBuildersKt.add(builder, String)Z`.
fn json_array_add_string(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[1])?;
    let node = alloc_json_node(vm, &JsonVal::Str(value))?;
    json_array_builder_add(vm, &[args[0], node])?;
    Ok(JValue::Int(1))
}

/// `JsonElementBuildersKt.addAllStrings(builder, collection)Z`.
fn json_array_add_all_strings(vm: &mut Vm, args: &[JValue]) -> R {
    let items = coll_elems(vm, args[1])?;
    for item in items {
        let value = jstr(vm, item)?;
        let node = alloc_json_node(vm, &JsonVal::Str(value))?;
        json_array_builder_add(vm, &[args[0], node])?;
    }
    Ok(JValue::Int(1))
}

/// `JsonElementBuildersKt.addJsonObject(builder, block)Z`.
fn json_array_add_json_object(vm: &mut Vm, args: &[JValue]) -> R {
    let builder = alloc(
        vm,
        "Lkotlinx/serialization/json/JsonObjectBuilder;",
        Native::Json(JsonVal::Object(Vec::new())),
    )?;
    invoke_function1(vm, args[1], builder)?;
    let object = json_object_builder_build(vm, &[builder])?;
    json_array_builder_add(vm, &[args[0], object])?;
    Ok(JValue::Int(1))
}

/// `JsonElementBuildersKt.putJsonObject(builder, key, block)`.
fn json_builder_put_json_object(vm: &mut Vm, args: &[JValue]) -> R {
    let builder = alloc(
        vm,
        "Lkotlinx/serialization/json/JsonObjectBuilder;",
        Native::Json(JsonVal::Object(Vec::new())),
    )?;
    invoke_function1(vm, args[2], builder)?;
    let object = json_object_builder_build(vm, &[builder])?;
    json_object_builder_put(vm, &[args[0], args[1], object])?;
    Ok(object)
}

/// `JsonElementBuildersKt.putJsonArray(builder, key, block)`.
fn json_builder_put_json_array(vm: &mut Vm, args: &[JValue]) -> R {
    let builder = alloc(
        vm,
        "Lkotlinx/serialization/json/JsonArrayBuilder;",
        Native::Json(JsonVal::Array(Vec::new())),
    )?;
    invoke_function1(vm, args[2], builder)?;
    let array = json_array_builder_build(vm, &[builder])?;
    json_object_builder_put(vm, &[args[0], args[1], array])?;
    Ok(array)
}

/// `JsonElementBuildersKt.put(builder, key, Boolean)`.
fn json_builder_put_bool(vm: &mut Vm, args: &[JValue]) -> R {
    let element = alloc_json_node(vm, &JsonVal::Bool(bool_of(vm, args[2])))?;
    json_object_builder_put(vm, &[args[0], args[1], element])?;
    Ok(element)
}

/// `JsonElementBuildersKt.put(builder, key, null)`.
fn json_builder_put_null(vm: &mut Vm, args: &[JValue]) -> R {
    let element = alloc_json_node(vm, &JsonVal::Null)?;
    json_object_builder_put(vm, &[args[0], args[1], element])?;
    Ok(element)
}

/// `BinaryFormat.decodeFromByteArray(strategy, bytes)`.
fn binary_format_decode_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[2]).ok_or_else(|| npe(vm))?;
    let text = String::from_utf8_lossy(&bytes);
    let val =
        parse_json(&text).map_err(|e| nat_fatal(JvmError::Resolution(format!("json: {e}"))))?;
    let node = alloc_json_node(vm, &val)?;
    run_serializer(vm, args[1], node, None)
}

/// `BinaryFormat.encodeToByteArray(strategy, value)`.
fn binary_format_encode_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let value = run_serializer_encode(vm, args[1], args[2])?;
    let text = jsonval_to_json(&value);
    let data = text
        .into_bytes()
        .into_iter()
        .map(|b| b as i8)
        .collect::<Vec<_>>();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

/// `JvmStreamsKt.decodeFromStream(json, strategy, stream)` — reads the
/// ByteArrayInputStream payload to the end and decodes it.
fn jvm_streams_decode(vm: &mut Vm, args: &[JValue]) -> R {
    let (bytes, pos) = match payload(vm, args[2]) {
        Some(Native::ByteArrayInputStream { bytes, pos }) => (bytes.clone(), *pos),
        _ => return Err(npe(vm)),
    };
    let text = String::from_utf8_lossy(&bytes[pos..]);
    let val =
        parse_json(&text).map_err(|e| nat_fatal(JvmError::Resolution(format!("json: {e}"))))?;
    let node = alloc_json_node(vm, &val)?;
    run_serializer(vm, args[1], node, None)
}

// ---------------------------------------------------------------------------
// JsonObject / JsonArray collection surface
// ---------------------------------------------------------------------------

/// `JsonObject.size()`.
fn json_object_size(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Object(entries))) => Ok(JValue::Int(entries.len() as i32)),
        _ => Err(npe(vm)),
    }
}

/// `JsonObject.isEmpty()`.
fn json_object_is_empty(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Object(entries))) => {
            Ok(JValue::Int(i32::from(entries.is_empty())))
        }
        _ => Err(npe(vm)),
    }
}

/// `JsonArray.isEmpty()`.
fn json_array_is_empty(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Array(values))) => Ok(JValue::Int(i32::from(values.is_empty()))),
        _ => Err(npe(vm)),
    }
}

/// `JsonObject.entrySet()` — a Set of Map.Entry backed by a HashMap.
fn json_object_entry_set(vm: &mut Vm, args: &[JValue]) -> R {
    let entries = match payload(vm, args[0]) {
        Some(Native::Json(JsonVal::Object(entries))) => entries.clone(),
        _ => return Err(npe(vm)),
    };
    let mut map_entries = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        map_entries.push((new_str(vm, &key), alloc_json_node(vm, &value)?));
    }
    let map = alloc(vm, "Ljava/util/HashMap;", Native::Map(map_entries.clone()))?;
    let JValue::Obj(mid) = map else {
        return Err(npe(vm));
    };
    let mut items = Vec::with_capacity(map_entries.len());
    for idx in 0..map_entries.len() {
        items.push(alloc(
            vm,
            "Ljava/util/Map$Entry;",
            Native::MapEntry { map: mid, idx },
        )?);
    }
    alloc(vm, "Ljava/util/HashSet;", Native::Set(items))
}

// ---------------------------------------------------------------------------
// JsonPrimitive accessors
// ---------------------------------------------------------------------------

fn json_primitive_value(vm: &mut Vm, args: &[JValue]) -> Result<JsonVal, NatErr> {
    match payload(vm, args[0]) {
        Some(Native::Json(value)) => Ok(value.clone()),
        _ => Err(npe(vm)),
    }
}

/// `JsonElementKt.getInt(primitive)`.
fn json_primitive_int(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(match json_primitive_value(vm, args)? {
        JsonVal::Int(i) => i as i32,
        JsonVal::Double(d) => d as i32,
        JsonVal::Bool(b) => i32::from(b),
        JsonVal::Str(s) => s.trim().parse().unwrap_or_default(),
        _ => 0,
    }))
}

/// `JsonElementKt.getIntOrNull(primitive)`.
fn json_primitive_int_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let n = match json_primitive_value(vm, args)? {
        JsonVal::Int(i) => Some(i as i32),
        JsonVal::Double(d) => Some(d as i32),
        JsonVal::Str(s) => s.trim().parse().ok(),
        _ => None,
    };
    match n {
        Some(n) => boxed(vm, "Ljava/lang/Integer;", Native::IntBox(n)),
        None => Ok(JValue::Null),
    }
}

/// `JsonElementKt.getLong(primitive)`.
fn json_primitive_long(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Long(match json_primitive_value(vm, args)? {
        JsonVal::Int(i) => i,
        JsonVal::Double(d) => d as i64,
        JsonVal::Bool(b) => i64::from(b),
        JsonVal::Str(s) => s.trim().parse().unwrap_or_default(),
        _ => 0,
    }))
}

/// `JsonElementKt.getLongOrNull(primitive)`.
fn json_primitive_long_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let n = match json_primitive_value(vm, args)? {
        JsonVal::Int(i) => Some(i),
        JsonVal::Double(d) => Some(d as i64),
        JsonVal::Str(s) => s.trim().parse().ok(),
        _ => None,
    };
    match n {
        Some(n) => boxed(vm, "Ljava/lang/Long;", Native::LongBox(n)),
        None => Ok(JValue::Null),
    }
}

/// `JsonElementKt.getFloat(primitive)`.
fn json_primitive_float(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Float(match json_primitive_value(vm, args)? {
        JsonVal::Double(d) => d as f32,
        JsonVal::Int(i) => i as f32,
        JsonVal::Str(s) => s.trim().parse().unwrap_or_default(),
        _ => 0.0,
    }))
}

/// `JsonElementKt.getFloatOrNull(primitive)`.
fn json_primitive_float_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let n = match json_primitive_value(vm, args)? {
        JsonVal::Double(d) => Some(d as f32),
        JsonVal::Int(i) => Some(i as f32),
        JsonVal::Str(s) => s.trim().parse().ok(),
        _ => None,
    };
    match n {
        Some(n) => boxed(vm, "Ljava/lang/Float;", Native::FloatBox(n)),
        None => Ok(JValue::Null),
    }
}

/// `JsonElementKt.getDouble(primitive)`.
fn json_primitive_double(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Double(match json_primitive_value(vm, args)? {
        JsonVal::Double(d) => d,
        JsonVal::Int(i) => i as f64,
        JsonVal::Str(s) => s.trim().parse().unwrap_or_default(),
        _ => 0.0,
    }))
}

/// `JsonElementKt.getBoolean(primitive)`.
fn json_primitive_bool(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(match json_primitive_value(vm, args)? {
        JsonVal::Bool(b) => i32::from(b),
        JsonVal::Str(s) => i32::from(!matches!(s.trim(), "false" | "0" | "")),
        _ => 0,
    }))
}

/// `JsonElementKt.getBooleanOrNull(primitive)`.
fn json_primitive_bool_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let b = match json_primitive_value(vm, args)? {
        JsonVal::Bool(b) => Some(b),
        JsonVal::Str(s) => match s.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    };
    match b {
        Some(b) => boxed(vm, "Ljava/lang/Boolean;", Native::BoolBox(b)),
        None => Ok(JValue::Null),
    }
}

/// `JsonElementKt.JsonPrimitive(Boolean)`.
fn json_primitive_of_bool(vm: &mut Vm, args: &[JValue]) -> R {
    alloc_json_node(vm, &JsonVal::Bool(bool_of(vm, args[0])))
}

/// `JsonElementKt.JsonPrimitive(Number)`.
fn json_primitive_of_number(vm: &mut Vm, args: &[JValue]) -> R {
    let text = to_string_of(vm, args[0])?;
    let value = text
        .parse::<i64>()
        .map(JsonVal::Int)
        .unwrap_or_else(|_| JsonVal::Double(text.parse().unwrap_or(0.0)));
    alloc_json_node(vm, &value)
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
    ne!("Lkotlinx/serialization/internal/LinkedHashMapSerializer;", "<init>", "(Lkotlinx/serialization/KSerializer;Lkotlinx/serialization/KSerializer;)V", true, linked_hash_map_init),
    ne!("Lkotlinx/serialization/internal/LinkedHashMapSerializer;", "deserialize", "(Lkotlinx/serialization/encoding/Decoder;)Ljava/lang/Object;", true, linked_hash_map_deserialize),
    ne!("Lkotlinx/serialization/internal/StringSerializer;", "deserialize", "(Lkotlinx/serialization/encoding/Decoder;)Ljava/lang/Object;", true, primitive_serializer_deserialize),
    ne!("Lkotlinx/serialization/internal/IntSerializer;", "deserialize", "(Lkotlinx/serialization/encoding/Decoder;)Ljava/lang/Object;", true, primitive_serializer_deserialize),
    ne!("Lkotlinx/serialization/internal/LongSerializer;", "deserialize", "(Lkotlinx/serialization/encoding/Decoder;)Ljava/lang/Object;", true, primitive_serializer_deserialize),
    ne!("Lkotlinx/serialization/internal/LinkedHashMapSerializer;", "getDescriptor", "()Lkotlinx/serialization/descriptors/SerialDescriptor;", true, linked_hash_map_descriptor),
    ne!("Lkotlinx/serialization/json/JsonObject$Companion;", "serializer", "()Lkotlinx/serialization/KSerializer;", true, json_element_serializer_marker),
    ne!("Lkotlinx/serialization/json/JsonArray$Companion;", "serializer", "()Lkotlinx/serialization/KSerializer;", true, json_element_serializer_marker),
    ne!("Lkotlinx/serialization/json/JsonNames;", "names", "()[Ljava/lang/String;", true, json_names_names),
    ne!("Lkotlinx/serialization/BinaryFormat;", "getSerializersModule", "()Lkotlinx/serialization/modules/SerializersModule;", true, serializers_module_alloc),
    ne!("Lkotlinx/serialization/BinaryFormat;", "decodeFromByteArray", "(Lkotlinx/serialization/DeserializationStrategy;[B)Ljava/lang/Object;", true, binary_format_decode_bytes),
    ne!("Lkotlinx/serialization/BinaryFormat;", "encodeToByteArray", "(Lkotlinx/serialization/SerializationStrategy;Ljava/lang/Object;)[B", true, binary_format_encode_bytes),
    ne!("Lkotlinx/serialization/json/JsonTransformingSerializer;", "<init>", "(Lkotlinx/serialization/KSerializer;)V", true, serializer_opaque_init),
    ne!("Lkotlinx/serialization/internal/PairSerializer;", "<init>", "(Lkotlinx/serialization/KSerializer;Lkotlinx/serialization/KSerializer;)V", true, serializer_opaque_init),
    ne!("Lkotlinx/serialization/json/JsonArrayBuilder;", "<init>", "()V", true, json_array_builder_init),
    ne!("Lkotlinx/serialization/json/JsonArrayBuilder;", "build", "()Lkotlinx/serialization/json/JsonArray;", true, json_array_builder_build),
    ne!("Lkotlinx/serialization/json/JsonArrayBuilder;", "add", "(Lkotlinx/serialization/json/JsonElement;)Z", true, json_array_builder_add),
    ne!("Lkotlinx/serialization/protobuf/ProtoNumber;", "number", "()I", true, proto_number_number),
    ne!("Lkotlinx/serialization/internal/ReferenceArraySerializer;", "<init>", "(Lkotlin/reflect/KClass;Lkotlinx/serialization/KSerializer;)V", true, serializer_opaque_init),
    ne!("Lkotlinx/serialization/json/JsonKt;", "Json$default", "(Lkotlinx/serialization/json/Json;Lkotlin/jvm/functions/Function1;ILjava/lang/Object;)Lkotlinx/serialization/json/Json;", false, json_builder_default),
    ne!("Lkotlinx/serialization/json/JsonBuilder;", "setIgnoreUnknownKeys", "(Z)V", true, json_builder_set),
    ne!("Lkotlinx/serialization/json/JsonBuilder;", "setLenient", "(Z)V", true, json_builder_set),
    ne!("Lkotlinx/serialization/json/JsonBuilder;", "setEncodeDefaults", "(Z)V", true, json_builder_set),
    ne!("Lkotlinx/serialization/json/JsonBuilder;", "setExplicitNulls", "(Z)V", true, json_builder_set),
    ne!("Lkotlinx/serialization/json/JsonBuilder;", "setSerializersModule", "(Lkotlinx/serialization/modules/SerializersModule;)V", true, json_builder_set_serializers_module),
    ne!("Lkotlinx/serialization/json/JsonBuilder;", "setAllowSpecialFloatingPointValues", "(Z)V", true, json_builder_set),
    ne!("Lkotlinx/serialization/json/JsonBuilder;", "setPrettyPrint", "(Z)V", true, json_builder_set),
    ne!("Lkotlinx/serialization/json/JsonBuilder;", "setAllowTrailingComma", "(Z)V", true, json_builder_set),
    ne!("Lkotlinx/serialization/json/JsonBuilder;", "setUseArrayPolymorphism", "(Z)V", true, json_builder_set),
    ne!("Lkotlinx/serialization/json/JsonBuilder;", "getSerializersModule", "()Lkotlinx/serialization/modules/SerializersModule;", true, serializers_module_alloc),
    ne!("Lkotlinx/serialization/json/JvmStreamsKt;", "decodeFromStream", "(Lkotlinx/serialization/json/Json;Lkotlinx/serialization/DeserializationStrategy;Ljava/io/InputStream;)Ljava/lang/Object;", false, jvm_streams_decode),
    ne!("Lkotlinx/serialization/descriptors/SerialDescriptorsKt;", "PrimitiveSerialDescriptor", "(Ljava/lang/String;Lkotlinx/serialization/descriptors/PrimitiveKind;)Lkotlinx/serialization/descriptors/SerialDescriptor;", false, primitive_serial_descriptor),
    ne!("Lkotlinx/serialization/descriptors/SerialDescriptorsKt;", "buildClassSerialDescriptor$default", "(Ljava/lang/String;[Lkotlinx/serialization/descriptors/SerialDescriptor;Lkotlin/jvm/functions/Function1;ILjava/lang/Object;)Lkotlinx/serialization/descriptors/SerialDescriptor;", false, build_class_descriptor_default),
    ne!("Lkotlinx/serialization/SealedClassSerializer;", "<init>", "(Ljava/lang/String;Lkotlin/reflect/KClass;[Lkotlin/reflect/KClass;[Lkotlinx/serialization/KSerializer;[Ljava/lang/annotation/Annotation;)V", true, serializer_opaque_init),
    ne!("Lkotlinx/serialization/internal/ObjectSerializer;", "<init>", "(Ljava/lang/String;Ljava/lang/Object;[Ljava/lang/annotation/Annotation;)V", true, serializer_opaque_init),
    ne!("Lkotlinx/serialization/internal/EnumsKt;", "createSimpleEnumSerializer", "(Ljava/lang/String;[Ljava/lang/Enum;)Lkotlinx/serialization/KSerializer;", false, enum_serializer_create),
    ne!("Lkotlinx/serialization/internal/EnumSerializer;", "deserialize", "(Lkotlinx/serialization/encoding/Decoder;)Ljava/lang/Object;", true, enum_serializer_deserialize),
    ne!("Lkotlinx/serialization/internal/EnumSerializer;", "getDescriptor", "()Lkotlinx/serialization/descriptors/SerialDescriptor;", true, enum_serializer_descriptor),
    ne!("Lkotlinx/serialization/internal/EnumsKt;", "createAnnotatedEnumSerializer", "(Ljava/lang/String;[Ljava/lang/Enum;[Ljava/lang/String;[[Ljava/lang/annotation/Annotation;[Ljava/lang/annotation/Annotation;)Lkotlinx/serialization/KSerializer;", false, enum_serializer_create),
    ne!("Lkotlinx/serialization/internal/LinkedHashSetSerializer;", "<init>", "(Lkotlinx/serialization/KSerializer;)V", true, serializer_opaque_init),
    ne!("Lkotlinx/serialization/modules/PolymorphicModuleBuilder;", "<init>", "(Lkotlin/reflect/KClass;Lkotlinx/serialization/KSerializer;)V", true, polymorphic_module_builder_init),
    ne!("Lkotlinx/serialization/modules/PolymorphicModuleBuilder;", "subclass", "(Lkotlin/reflect/KClass;Lkotlinx/serialization/KSerializer;)V", true, polymorphic_module_builder_subclass),
    ne!("Lkotlinx/serialization/modules/PolymorphicModuleBuilder;", "buildTo", "(Lkotlinx/serialization/modules/SerializersModuleBuilder;)V", true, polymorphic_module_builder_build_to),
    ne!("Lkotlinx/serialization/modules/PolymorphicModuleBuilder;", "defaultDeserializer", "(Lkotlin/jvm/functions/Function1;)V", true, polymorphic_module_builder_default_deserializer),
    ne!("Lkotlinx/serialization/ContextualSerializer;", "<init>", "(Lkotlin/reflect/KClass;Lkotlinx/serialization/KSerializer;[Lkotlinx/serialization/KSerializer;)V", true, serializer_opaque_init),
    ne!("Lkotlinx/serialization/json/Json$Default;", "parseToJsonElement", "(Ljava/lang/String;)Lkotlinx/serialization/json/JsonElement;", true, json_parse_to_element),
    ne!("Lkotlinx/serialization/modules/SerializersModuleBuilder;", "<init>", "()V", true, serializers_module_builder_init),
    ne!("Lkotlinx/serialization/modules/SerializersModuleBuilder;", "build", "()Lkotlinx/serialization/modules/SerializersModule;", true, serializers_module_builder_build),
    ne!("Lkotlinx/serialization/modules/SerializersModuleBuilder;", "contextual", "(Lkotlin/reflect/KClass;Lkotlin/jvm/functions/Function1;)V", true, json_builder_set),
    ne!("Lkotlinx/serialization/modules/SerializersModuleKt;", "plus", "(Lkotlinx/serialization/modules/SerializersModule;Lkotlinx/serialization/modules/SerializersModule;)Lkotlinx/serialization/modules/SerializersModule;", false, serializers_module_plus),
    ne!("Lkotlinx/serialization/internal/InlineClassDescriptor;", "<init>", "(Ljava/lang/String;Lkotlinx/serialization/internal/GeneratedSerializer;)V", true, inline_class_descriptor_init),
    ne!("Lkotlinx/serialization/internal/InlineClassDescriptor;", "addElement", "(Ljava/lang/String;Z)V", true, inline_class_descriptor_add_element),
    ne!("Lkotlinx/serialization/PolymorphicSerializer;", "<init>", "(Lkotlin/reflect/KClass;[Ljava/lang/annotation/Annotation;)V", true, polymorphic_serializer_init),
    ne!("Lkotlinx/serialization/PolymorphicSerializer;", "deserialize", "(Lkotlinx/serialization/encoding/Decoder;)Ljava/lang/Object;", true, polymorphic_deserialize),
    ne!("Lkotlinx/serialization/PolymorphicSerializer;", "getDescriptor", "()Lkotlinx/serialization/descriptors/SerialDescriptor;", true, polymorphic_get_descriptor),
    ne!("Lkotlinx/serialization/SerializationException;", "<init>", "(Ljava/lang/String;)V", true, serializer_opaque_init),
    ne!("Lkotlinx/serialization/json/JsonClassDiscriminator;", "discriminator", "()Ljava/lang/String;", true, json_class_discriminator),
    ne!("Lkotlinx/serialization/json/JsonElementBuildersKt;", "putJsonObject", "(Lkotlinx/serialization/json/JsonObjectBuilder;Ljava/lang/String;Lkotlin/jvm/functions/Function1;)Lkotlinx/serialization/json/JsonElement;", false, json_builder_put_json_object),
    ne!("Lkotlinx/serialization/json/JsonElementBuildersKt;", "putJsonArray", "(Lkotlinx/serialization/json/JsonObjectBuilder;Ljava/lang/String;Lkotlin/jvm/functions/Function1;)Lkotlinx/serialization/json/JsonElement;", false, json_builder_put_json_array),
    ne!("Lkotlinx/serialization/json/JsonElementBuildersKt;", "add", "(Lkotlinx/serialization/json/JsonArrayBuilder;Ljava/lang/String;)Z", false, json_array_add_string),
    ne!("Lkotlinx/serialization/json/JsonElementBuildersKt;", "addAllStrings", "(Lkotlinx/serialization/json/JsonArrayBuilder;Ljava/util/Collection;)Z", false, json_array_add_all_strings),
    ne!("Lkotlinx/serialization/json/JsonElementBuildersKt;", "addJsonObject", "(Lkotlinx/serialization/json/JsonArrayBuilder;Lkotlin/jvm/functions/Function1;)Z", false, json_array_add_json_object),
    ne!("Lkotlinx/serialization/json/JsonElementBuildersKt;", "put", "(Lkotlinx/serialization/json/JsonObjectBuilder;Ljava/lang/String;Ljava/lang/Boolean;)Lkotlinx/serialization/json/JsonElement;", false, json_builder_put_bool),
    ne!("Lkotlinx/serialization/json/JsonElementBuildersKt;", "put", "(Lkotlinx/serialization/json/JsonObjectBuilder;Ljava/lang/String;Ljava/lang/Void;)Lkotlinx/serialization/json/JsonElement;", false, json_builder_put_null),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "getInt", "(Lkotlinx/serialization/json/JsonPrimitive;)I", false, json_primitive_int),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "getIntOrNull", "(Lkotlinx/serialization/json/JsonPrimitive;)Ljava/lang/Integer;", false, json_primitive_int_or_null),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "getLong", "(Lkotlinx/serialization/json/JsonPrimitive;)J", false, json_primitive_long),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "getLongOrNull", "(Lkotlinx/serialization/json/JsonPrimitive;)Ljava/lang/Long;", false, json_primitive_long_or_null),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "getFloat", "(Lkotlinx/serialization/json/JsonPrimitive;)F", false, json_primitive_float),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "getFloatOrNull", "(Lkotlinx/serialization/json/JsonPrimitive;)Ljava/lang/Float;", false, json_primitive_float_or_null),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "getDouble", "(Lkotlinx/serialization/json/JsonPrimitive;)D", false, json_primitive_double),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "getBoolean", "(Lkotlinx/serialization/json/JsonPrimitive;)Z", false, json_primitive_bool),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "getBooleanOrNull", "(Lkotlinx/serialization/json/JsonPrimitive;)Ljava/lang/Boolean;", false, json_primitive_bool_or_null),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "JsonPrimitive", "(Ljava/lang/Boolean;)Lkotlinx/serialization/json/JsonPrimitive;", false, json_primitive_of_bool),
    ne!("Lkotlinx/serialization/json/JsonElementKt;", "JsonPrimitive", "(Ljava/lang/Number;)Lkotlinx/serialization/json/JsonPrimitive;", false, json_primitive_of_number),
    ne!("Lkotlinx/serialization/json/JsonObject;", "entrySet", "()Ljava/util/Set;", true, json_object_entry_set),
    ne!("Lkotlinx/serialization/json/JsonObject;", "size", "()I", true, json_object_size),
    ne!("Lkotlinx/serialization/json/JsonObject;", "isEmpty", "()Z", true, json_object_is_empty),
    ne!("Lkotlinx/serialization/json/JsonArray;", "isEmpty", "()Z", true, json_array_is_empty),
    ne!("Lkotlinx/serialization/internal/ArrayListSerializer;", "getDescriptor", "()Lkotlinx/serialization/descriptors/SerialDescriptor;", true, array_list_serializer_descriptor),
    ne!("Lkotlinx/serialization/builtins/BuiltinSerializersKt;", "ListSerializer", "(Lkotlinx/serialization/KSerializer;)Lkotlinx/serialization/KSerializer;", false, list_serializer),
    ne!("Lkotlinx/serialization/builtins/BuiltinSerializersKt;", "MapSerializer", "(Lkotlinx/serialization/KSerializer;Lkotlinx/serialization/KSerializer;)Lkotlinx/serialization/KSerializer;", false, map_serializer),
    ne!("Lkotlinx/serialization/builtins/BuiltinSerializersKt;", "serializer", "(Lkotlin/jvm/internal/StringCompanionObject;)Lkotlinx/serialization/KSerializer;", false, string_serializer_of),
    ne!("Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;", "pushClassAnnotation", "(Ljava/lang/annotation/Annotation;)V", true, descriptor_push_annotation),
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
