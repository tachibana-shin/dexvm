//! kotlinx.serialization host implementation: a small JSON parser producing
//! the `JsonElement` tree, plus the decoder / descriptor natives that the
//! extension's generated serializers (real dex bytecode, e.g. `Lg.deserialize`)
//! drive to decode cached filter lists.
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
    let text = match payload(vm, args[2]) {
        Some(Native::Json(value)) => jsonval_to_json(value),
        _ => return Err(iae(vm, "encodeToBufferedSink expects JsonElement")),
    };
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
        "Lcom/squareup/zstd/okio/OkioZstd;",
        "zstdDecompress",
        "(Lokio/Source;)Lokio/Source;",
        false,
        zstd_identity
    ),
    ne!(
        "Lcom/squareup/zstd/okio/OkioZstd;",
        "zstdCompress",
        "(Lokio/Sink;)Lokio/Sink;",
        false,
        zstd_identity
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
