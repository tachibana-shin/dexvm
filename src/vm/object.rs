//! Heap objects: the object arena, native-backed objects, and arrays.

use crate::vm::value::JValue;

/// Deferred RxJava 1 operations. Keeping callbacks in the heap payload lets
/// `fromCallable` and its operator chain run when the stream is consumed.
#[derive(Debug, Clone)]
pub enum RxOperator {
    Map(JValue),
    FlatMap(JValue),
    DoOnNext(JValue),
    ToList,
}

#[derive(Debug, Clone)]
pub enum ArrayData {
    Byte(Vec<i8>),
    Char(Vec<u16>),
    Short(Vec<i16>),
    Int(Vec<i32>),
    Long(Vec<i64>),
    Float(Vec<f32>),
    Double(Vec<f64>),
    Bool(Vec<bool>),
    Obj(Vec<JValue>),
}

impl ArrayData {
    /// Allocate a fresh zeroed array for the given element descriptor.
    pub fn new(elem_desc: &str, len: usize) -> ArrayData {
        match elem_desc {
            "B" => ArrayData::Byte(vec![0; len]),
            "C" => ArrayData::Char(vec![0; len]),
            "S" => ArrayData::Short(vec![0; len]),
            "I" => ArrayData::Int(vec![0; len]),
            "J" => ArrayData::Long(vec![0; len]),
            "F" => ArrayData::Float(vec![0.0; len]),
            "D" => ArrayData::Double(vec![0.0; len]),
            "Z" => ArrayData::Bool(vec![false; len]),
            _ => ArrayData::Obj(vec![JValue::Null; len]),
        }
    }

    pub fn elem_desc(&self) -> &'static str {
        match self {
            ArrayData::Byte(_) => "B",
            ArrayData::Char(_) => "C",
            ArrayData::Short(_) => "S",
            ArrayData::Int(_) => "I",
            ArrayData::Long(_) => "J",
            ArrayData::Float(_) => "F",
            ArrayData::Double(_) => "D",
            ArrayData::Bool(_) => "Z",
            ArrayData::Obj(_) => "Ljava/lang/Object;",
        }
    }

    pub fn len(&self) -> usize {
        match self {
            ArrayData::Byte(v) => v.len(),
            ArrayData::Char(v) => v.len(),
            ArrayData::Short(v) => v.len(),
            ArrayData::Int(v) => v.len(),
            ArrayData::Long(v) => v.len(),
            ArrayData::Float(v) => v.len(),
            ArrayData::Double(v) => v.len(),
            ArrayData::Bool(v) => v.len(),
            ArrayData::Obj(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, i: usize) -> JValue {
        match self {
            ArrayData::Byte(v) => JValue::Int(i32::from(v[i])),
            ArrayData::Char(v) => JValue::Int(i32::from(v[i])),
            ArrayData::Short(v) => JValue::Int(i32::from(v[i])),
            ArrayData::Int(v) => JValue::Int(v[i]),
            ArrayData::Long(v) => JValue::Long(v[i]),
            ArrayData::Float(v) => JValue::Float(v[i]),
            ArrayData::Double(v) => JValue::Double(v[i]),
            ArrayData::Bool(v) => JValue::Int(i32::from(v[i])),
            ArrayData::Obj(v) => v[i],
        }
    }

    pub fn set(&mut self, i: usize, v: JValue) {
        match self {
            ArrayData::Byte(d) => d[i] = v.as_int() as i8,
            ArrayData::Char(d) => d[i] = v.as_int() as u16,
            ArrayData::Short(d) => d[i] = v.as_int() as i16,
            ArrayData::Int(d) => d[i] = v.as_int(),
            ArrayData::Long(d) => d[i] = v.as_long(),
            ArrayData::Float(d) => {
                d[i] = match v {
                    JValue::Float(f) => f,
                    JValue::Int(i) => i as f32,
                    _ => panic!("float array store of {v:?}"),
                }
            }
            ArrayData::Double(d) => {
                d[i] = match v {
                    JValue::Double(f) => f,
                    JValue::Float(f) => f64::from(f),
                    JValue::Long(l) => l as f64,
                    JValue::Int(i) => f64::from(i),
                    _ => panic!("double array store of {v:?}"),
                }
            }
            ArrayData::Bool(d) => d[i] = v.truthy(),
            ArrayData::Obj(d) => d[i] = v,
        }
    }
}

/// Opaque handle to a parsed HTML document; Debug prints only the pointer.
#[cfg(feature = "jsoup")]
#[derive(Clone)]
pub struct JsoupDocRef {
    pub doc: std::rc::Rc<dom_query::Document>,
    /// Base URI (the response URL) used to resolve `abs:` attributes.
    pub base: Option<String>,
}

#[cfg(feature = "jsoup")]
impl std::fmt::Debug for JsoupDocRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JsoupDoc({:p})", std::rc::Rc::as_ptr(&self.doc))
    }
}

#[cfg(feature = "jsoup")]
impl JsoupDocRef {
    pub fn new(doc: dom_query::Document) -> Self {
        JsoupDocRef {
            doc: std::rc::Rc::new(doc),
            base: None,
        }
    }
}

/// A parsed JSON value tree (kotlinx.serialization JsonElement model).
#[derive(Debug, Clone, PartialEq)]
pub enum JsonVal {
    Object(Vec<(String, JsonVal)>),
    Array(Vec<JsonVal>),
    Str(String),
    Int(i64),
    Double(f64),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PreferenceValue {
    Bool(bool),
    String(String),
    Int(i32),
    Long(i64),
    Float(f32),
}

#[derive(Debug, Clone)]
pub enum PreferenceEdit {
    Put(String, PreferenceValue),
    Remove(String),
}

/// Primitive serializers used by compiler-generated kotlinx.serialization
/// bytecode.  The serializer implementations themselves are host-backed; the
/// generated model serializers remain ordinary DEX bytecode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveSerializerKind {
    String,
    Int,
    Long,
}

/// Native (Rust-backed) objects. Objects of shim classes carry one of these
/// instead of interpreted fields.
#[derive(Clone)]
pub enum Native {
    /// java.lang.String payload.
    Str(String),
    /// Java array storage.
    Array(ArrayData),
    /// java.lang.Class instance: wraps a class id, or a primitive descriptor.
    ClassObj(ClassOrPrim),
    /// java.lang.Throwable and subclasses.
    Throwable {
        message: Option<String>,
        cause: JValue,
    },
    /// java.lang.StringBuilder.
    StringBuilder(String),
    // boxed primitives
    IntBox(i32),
    LongBox(i64),
    FloatBox(f32),
    DoubleBox(f64),
    CharBox(u16),
    BoolBox(bool),
    ShortBox(i16),
    ByteBox(i8),
    /// java.lang.Enum instances.
    Enum {
        name: String,
        ordinal: i32,
    },
    /// java.lang.reflect.Field (from Class.getDeclaredField).
    Field {
        class: u32,
        name: u32,
    },
    /// java.util.concurrent.atomic.AtomicBoolean.
    AtomicBool(bool),
    /// java.util.concurrent.atomic.AtomicInteger.
    AtomicInt(i32),
    /// java.time.LocalDate (days since epoch).
    LocalDay(u32),
    /// java.time.Instant / ZonedDateTime (epoch millis).
    EpochMillis(i64),
    List(Vec<JValue>),
    /// java.util.HashMap / LinkedHashMap.
    Map(Vec<(JValue, JValue)>),
    /// java.util.HashSet / LinkedHashSet.
    Set(Vec<JValue>),
    /// Iterators.
    Iter(IterKind),
    /// java.util.Map.Entry.
    MapEntry {
        map: u32,
        idx: usize,
    },
    /// java.util.regex.Pattern (source kept for `pattern()`/`toString()`).
    Pattern {
        re: fancy_regex::Regex,
        source: String,
    },
    /// java.util.regex.Matcher.
    Matcher(MatcherState),
    /// okhttp3.ResponseBody with a binary payload (image bytes etc.).
    RespBody(Vec<u8>),
    /// java.io.ByteArrayInputStream: cursor over in-memory bytes.
    ByteArrayInputStream {
        bytes: Vec<u8>,
        pos: usize,
    },
    /// okio.BufferedSource / okio.Buffer sharing a byte cursor.
    OkioBuf {
        bytes: Vec<u8>,
        pos: usize,
    },
    /// okio Sink / BufferedSink backed by a permission-checked host file.
    OkioSink {
        path: String,
        bytes: Vec<u8>,
        flushed: usize,
        closed: bool,
    },
    /// okhttp3.OkHttpClient built from a builder: keeps interceptor lists.
    OkHttpClient {
        interceptors: Vec<JValue>,
        network_interceptors: Vec<JValue>,
    },
    /// okhttp3.Call: request plus the originating client (chain execution).
    Call {
        request: JValue,
        client: JValue,
        canceled: bool,
    },
    /// okhttp3.Interceptor$Chain under execution.
    Chain {
        interceptors: Vec<JValue>,
        pos: usize,
        request: JValue,
        call: JValue,
    },
    /// okhttp3.Response$Builder for interceptor rewrites.
    ResponseBuilder {
        code: i32,
        message: String,
        headers: Vec<(String, String)>,
        body: Option<JValue>,
        request: Option<JValue>,
        prior: Option<JValue>,
    },
    /// java.security.MessageDigest: algorithm code + accumulated input.
    ///
    /// 0 = SHA-256, 1 = SHA-1, 2 = MD5, 3 = SHA-384, 4 = SHA-512.
    MessageDigest {
        algo: u8,
        buf: Vec<u8>,
    },
    /// java.security.SecureRandom: xorshift64* state.
    SecureRandom(u64),
    /// javax.crypto.Cipher: AES-256-GCM state machine.
    ///
    /// `mode` mirrors the JCE constants: 1 = ENCRYPT, 2 = DECRYPT, 0 = unset.
    AesGcm {
        mode: u8,
        secret: [u8; 32],
        iv: Vec<u8>,
        tag_bits: usize,
        aad: Vec<u8>,
    },
    /// javax.crypto.spec.SecretKeySpec key bytes.
    Key(Vec<u8>),
    /// javax.crypto.spec.GCMParameterSpec: tag bits + IV.
    GcmSpec {
        tag_bits: i32,
        iv: Vec<u8>,
    },
    /// java.io.PrintStream (writes to the VM output sink).
    PrintStream,
    /// java.util.Random (xorshift64*).
    Random(u64),
    /// java.util.Date (epoch millis).
    Date(i64),
    /// java.util.Locale and other inert Java objects.
    Opaque,
    /// java.lang.Thread. Execution is synchronous, but observable thread
    /// properties and interruption state follow the Java API.
    Thread {
        name: String,
        daemon: bool,
        alive: bool,
        interrupted: bool,
        started: bool,
        runnable: JValue,
    },
    /// Android SharedPreferences handle and a transactional editor.
    SharedPreferences(String),
    SharedPreferencesEditor {
        name: String,
        edits: Vec<PreferenceEdit>,
        clear: bool,
    },
    /// java.io.File: real host path. Every `File` method operates on the
    /// actual filesystem (mkdirs/exists/lastModified/resolve/...).
    File {
        path: String,
    },
    /// java.lang.reflect.Type produced by `FullTypeReference.getType()`:
    /// carries the concrete descriptor from the receiver's generic signature.
    Type {
        desc: String,
    },
    /// java.util.TimeZone (zone id string, e.g. "UTC", "GMT+07:00").
    TimeZone(String),
    /// java.text.SimpleDateFormat: pattern + resolved time zone id.
    DateFormatter {
        pattern: String,
        zone: String,
    },
    /// java.text.ParsePosition (current index).
    ParsePosition(i32),
    /// java.util.ArrayDeque (a FIFO list for our purposes).
    ArrayDeque(Vec<JValue>),
    /// java.util.concurrent.locks.ReentrantLock.
    ReentrantLock {
        locked: bool,
    },
    /// okhttp3.OkHttpClient$Builder: accumulated interceptor lists.
    OkHttpBuilder {
        interceptors: Vec<JValue>,
        network_interceptors: Vec<JValue>,
    },
    /// okhttp3.HttpUrl / HttpUrl$Builder: URL being built.
    HttpUrl(String),
    /// okhttp3.FormBody / FormBody$Builder: name/value pairs.
    FormBody(Vec<(String, String)>),
    /// okhttp3.Request produced by the RequestsKt helpers or Request$Builder.
    Request {
        url: String,
        method: String,
        headers: Vec<(String, String)>,
        body: Option<JValue>,
    },
    /// okhttp3.Request$Builder under construction.
    RequestBuilder {
        url: String,
        method: String,
        headers: Vec<(String, String)>,
        body: Option<JValue>,
    },
    /// okhttp3.Headers / Headers$Builder: name/value pairs.
    Headers(Vec<(String, String)>),
    /// okhttp3.Response produced by the host HTTP bridge. `body` is the raw
    /// payload (lossy UTF-8 for text responses); `None` means an empty body.
    /// `prior` is the response before a redirect (Null when there is none).
    Response {
        code: i32,
        message: String,
        headers: Vec<(String, String)>,
        body: Option<Vec<u8>>,
        request: JValue,
        prior: JValue,
    },
    /// okhttp3.CacheControl: parsed `Cache-Control` request header.
    CacheControl {
        max_age: i64,
        no_cache: bool,
    },
    /// okhttp3.CacheControl$Builder (chained `maxAge(...)` before `build()`).
    CacheControlBuilder {
        max_age: i64,
    },
    /// kotlin.Result failure marker: holds the wrapped throwable.
    ResultFailure(JValue),
    /// RxJava 1 Observable/BlockingObservable. Execution is synchronous, but
    /// callable sources and operators remain lazy until the stream is consumed.
    RxObservable {
        values: Vec<JValue>,
        error: JValue,
        callable: JValue,
        operators: Vec<RxOperator>,
    },
    /// java.net.URI (raw string form, parsed on demand).
    URI(String),
    /// okhttp3.Timeout: configurable timeout values on a call.
    Timeout {
        millis: i64,
    },
    /// okhttp3.Cookie.
    Cookie {
        name: String,
        value: String,
    },
    /// eu.kanade.tachiyomi.source.online.HttpSource: name from its ctor.
    HttpSource {
        name: String,
    },
    /// eu.kanade.tachiyomi.source.model.SManga.
    SManga {
        title: String,
        author: Option<String>,
        artist: Option<String>,
        description: Option<String>,
        genre: Option<String>,
        status: i32,
        thumbnail_url: String,
        url: String,
        update_strategy: JValue,
    },
    /// eu.kanade.tachiyomi.source.model.SChapter.
    SChapter {
        name: String,
        url: String,
        date_upload: i64,
        scanlator: String,
        chapter_number: f32,
    },
    /// eu.kanade.tachiyomi.source.model.Page.
    SPPage {
        index: i32,
        name: String,
        url: String,
        image_url: String,
    },
    /// eu.kanade.tachiyomi.source.model.MangasPage.
    SMangasPage {
        mangas: Vec<JValue>,
        has_next: bool,
    },
    /// eu.kanade.tachiyomi.source.model.Filter and its subtypes.
    SFilter {
        name: String,
        state: i32,
        is_checked: bool,
        children: Vec<JValue>,
        options: Vec<JValue>,
        text_value: String,
    },
    /// eu.kanade.tachiyomi.source.model.FilterList.
    SFilterList(Vec<JValue>),
    /// kotlin.time.Duration value-class payload (nanoseconds).
    Duration(i64),
    /// org.jsoup parsed document. Payloads mirror the jsoup-compatible layer
    /// in rakuyomi's `html_element.rs`: `dom_query` documents with stable
    /// per-document node ids (a node id is only meaningful inside its own
    /// document tree).
    #[cfg(feature = "jsoup")]
    JsoupDoc(JsoupDocRef),
    /// org.jsoup single element: (document, node id).
    #[cfg(feature = "jsoup")]
    JsoupElement {
        doc: JsoupDocRef,
        id: dom_query::NodeId,
    },
    /// org.jsoup Elements: (document, node ids).
    #[cfg(feature = "jsoup")]
    JsoupElements {
        doc: JsoupDocRef,
        ids: Vec<dom_query::NodeId>,
    },
    /// kotlin Lazy wrapper holding the initializer function (Function0).
    Lazy(JValue),
    /// kotlin Pair (two elements).
    Pair(JValue, JValue),
    /// kotlin.ranges.IntRange (current, last) — the cursor doubles as iterator
    /// position for IntIterator subclasses.
    IntRange(i32, i32),
    /// Array descriptor stored on a Class instance of an array type.
    ArrayDesc(ArrayData),
    /// kotlinx.serialization JsonElement tree node (JsonObject/JsonArray/
    /// JsonPrimitive/JsonNull objects).
    Json(JsonVal),
    /// kotlinx.serialization decoder state over a JsonElement node.
    JsonDecoder {
        element: JValue,
        members: Option<Vec<(String, JValue)>>,
        index: i32,
    },
    /// kotlinx.serialization encoder state for one JSON value. Structured
    /// serializers append descriptor-named members before `endStructure`.
    JsonEncoder {
        value: Option<JsonVal>,
        elements: Vec<(String, JsonVal)>,
    },
    /// kotlinx.serialization PluginGeneratedSerialDescriptor.
    SerialDescriptor {
        name: String,
        elements: Vec<String>,
    },
    /// kotlinx.serialization JsonElement serializer marker.
    JsonElementSerializer,
    /// kotlinx.serialization ArrayListSerializer(elementSerializer).
    ArrayListSerializer {
        child: JValue,
    },
    /// StringSerializer / IntSerializer / LongSerializer singleton marker.
    PrimitiveSerializer(PrimitiveSerializerKind),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClassOrPrim {
    Class(u32),
    /// Primitive descriptor for `Integer.TYPE`-style class constants.
    Primitive(u8),
}

#[derive(Debug, Clone)]
pub enum IterKind {
    List {
        list: u32,
        idx: usize,
    },
    MapEntries {
        map: u32,
        idx: usize,
    },
    MapKeys {
        map: u32,
        idx: usize,
    },
    MapValues {
        map: u32,
        idx: usize,
    },
    Set {
        set: u32,
        idx: usize,
    },
    #[cfg(feature = "jsoup")]
    Jsoup {
        doc: JsoupDocRef,
        ids: Vec<dom_query::NodeId>,
        idx: usize,
    },
}

#[derive(Debug, Clone)]
pub struct MatcherState {
    pub pattern: fancy_regex::Regex,
    pub text: String,
    pub pos: usize,
    /// (start, end) of the most recent find()/matches().
    pub last: Option<(usize, usize)>,
}

/// A heap object. `class` is a class id; `fields` holds interpreted fields by
/// resolved offset; `native` carries the Rust-side payload for shim classes.
#[derive(Clone)]
pub struct JObject {
    pub class: u32,
    pub fields: Vec<JValue>,
    pub native: Option<Native>,
}

impl std::fmt::Debug for JObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JObject")
            .field("class", &self.class)
            .field("fields", &self.fields)
            .field("native", &self.native.as_ref().map(std::mem::discriminant))
            .finish()
    }
}

impl JObject {
    /// Appends every object id referenced from this object (fields plus any
    /// JValue held inside a native payload) to `out`. Used by the mark phase
    /// of [`Arena::gc`].
    pub(crate) fn collect_refs(&self, out: &mut Vec<u32>) {
        for f in &self.fields {
            if let JValue::Obj(o) = f {
                out.push(*o);
            }
        }
        if let Some(n) = &self.native {
            n.collect_refs(out);
        }
    }
}

impl Native {
    pub(crate) fn collect_refs(&self, out: &mut Vec<u32>) {
        let push = |v: Option<&JValue>, out: &mut Vec<u32>| {
            if let Some(JValue::Obj(o)) = v {
                out.push(*o);
            }
        };
        let push_all = |v: &[JValue], out: &mut Vec<u32>| {
            for f in v {
                if let JValue::Obj(o) = f {
                    out.push(*o);
                }
            }
        };
        match self {
            Native::Array(ArrayData::Obj(v)) => push_all(v, out),
            Native::Throwable { cause, .. } => push(Some(cause), out),
            Native::ResultFailure(t) => push(Some(t), out),
            Native::RxObservable {
                values,
                error,
                callable,
                operators,
            } => {
                push_all(values, out);
                push(Some(error), out);
                push(Some(callable), out);
                for operator in operators {
                    match operator {
                        RxOperator::Map(callback)
                        | RxOperator::FlatMap(callback)
                        | RxOperator::DoOnNext(callback) => push(Some(callback), out),
                        RxOperator::ToList => {}
                    }
                }
            }
            Native::List(v) | Native::Set(v) | Native::ArrayDeque(v) => push_all(v, out),
            Native::Map(v) => {
                for (k, val) in v {
                    push(Some(k), out);
                    push(Some(val), out);
                }
            }
            Native::Iter(k) => match k {
                IterKind::List { list, .. } => out.push(*list),
                IterKind::MapEntries { map, .. }
                | IterKind::MapKeys { map, .. }
                | IterKind::MapValues { map, .. } => out.push(*map),
                IterKind::Set { set, .. } => out.push(*set),
                #[cfg(feature = "jsoup")]
                IterKind::Jsoup { .. } => {}
            },
            Native::MapEntry { map, .. } => out.push(*map),
            Native::Lazy(v) => push(Some(v), out),
            Native::Thread { runnable, .. } => push(Some(runnable), out),
            Native::Pair(a, b) => {
                push(Some(a), out);
                push(Some(b), out);
            }
            Native::OkHttpBuilder {
                interceptors,
                network_interceptors,
            }
            | Native::OkHttpClient {
                interceptors,
                network_interceptors,
            } => {
                push_all(interceptors, out);
                push_all(network_interceptors, out);
            }
            Native::Call {
                request, client, ..
            } => {
                push(Some(request), out);
                push(Some(client), out);
            }
            Native::Chain {
                interceptors,
                request,
                call,
                ..
            } => {
                push_all(interceptors, out);
                push(Some(request), out);
                push(Some(call), out);
            }
            Native::ResponseBuilder { body, request, .. } => {
                push(body.as_ref(), out);
                push(request.as_ref(), out);
            }
            Native::Request { body, .. } => push(body.as_ref(), out),
            Native::RequestBuilder { body, .. } => push(body.as_ref(), out),
            Native::Response { request, .. } => push(Some(request), out),
            Native::SManga {
                update_strategy, ..
            } => push(Some(update_strategy), out),
            Native::SMangasPage { mangas, .. } => push_all(mangas, out),
            Native::SFilter { children, .. } => push_all(children, out),
            Native::SFilterList(v) => push_all(v, out),
            Native::Json(_)
            | Native::JsonEncoder { .. }
            | Native::SerialDescriptor { .. }
            | Native::JsonElementSerializer
            | Native::PrimitiveSerializer(_) => {}
            Native::JsonDecoder {
                element, members, ..
            } => {
                push(Some(element), out);
                if let Some(m) = members {
                    for (_, v) in m {
                        push(Some(v), out);
                    }
                }
            }
            Native::ArrayListSerializer { child } => push(Some(child), out),
            #[cfg(feature = "jsoup")]
            Native::JsoupDoc(_) => {}
            #[cfg(feature = "jsoup")]
            Native::JsoupElement { .. } | Native::JsoupElements { .. } => {}
            Native::ArrayDesc(ArrayData::Obj(v)) => push_all(v, out),
            Native::HttpSource { .. }
            | Native::Str(_)
            | Native::Array(_)
            | Native::ClassObj(_)
            | Native::StringBuilder(_)
            | Native::IntBox(_)
            | Native::LongBox(_)
            | Native::FloatBox(_)
            | Native::DoubleBox(_)
            | Native::CharBox(_)
            | Native::BoolBox(_)
            | Native::ShortBox(_)
            | Native::ByteBox(_)
            | Native::Enum { .. }
            | Native::Pattern { .. }
            | Native::Matcher(_)
            | Native::MessageDigest { .. }
            | Native::SecureRandom(_)
            | Native::AesGcm { .. }
            | Native::Key(_)
            | Native::GcmSpec { .. }
            | Native::PrintStream
            | Native::Random(_)
            | Native::Date(_)
            | Native::Opaque
            | Native::SharedPreferences(_)
            | Native::SharedPreferencesEditor { .. }
            | Native::File { .. }
            | Native::TimeZone(_)
            | Native::DateFormatter { .. }
            | Native::ParsePosition(_)
            | Native::ReentrantLock { .. }
            | Native::HttpUrl(_)
            | Native::FormBody(_)
            | Native::Headers(_)
            | Native::Cookie { .. }
            | Native::Field { .. }
            | Native::AtomicBool(_)
            | Native::AtomicInt(_)
            | Native::LocalDay(_)
            | Native::EpochMillis(_)
            | Native::SChapter { .. }
            | Native::SPPage { .. }
            | Native::Duration(_)
            | Native::IntRange(..)
            | Native::ArrayDesc(_)
            | Native::RespBody(_)
            | Native::ByteArrayInputStream { .. }
            | Native::CacheControl { .. }
            | Native::CacheControlBuilder { .. }
            | Native::URI(_)
            | Native::Timeout { .. }
            | Native::OkioBuf { .. }
            | Native::OkioSink { .. }
            | Native::Type { .. } => {}
        }
    }
}

/// Simple growing arena with mark-sweep reclamation between top-level calls.
/// Object ids are stable `u32` handles; the garbage collector never moves
/// objects, so handles never dangle — dead objects are handed to a free list
/// and reused by later allocations.
#[derive(Debug, Default)]
pub struct Arena {
    pub objects: Vec<JObject>,
    free: Vec<u32>,
}

impl Arena {
    pub fn alloc(&mut self, class: u32, fields: Vec<JValue>, native: Option<Native>) -> u32 {
        if let Some(f) = self.free.pop() {
            self.objects[f as usize] = JObject {
                class,
                fields,
                native,
            };
            return f;
        }
        let id = self.objects.len() as u32;
        self.objects.push(JObject {
            class,
            fields,
            native,
        });
        id
    }

    pub fn get(&self, id: u32) -> Option<&JObject> {
        self.objects.get(id as usize)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut JObject> {
        self.objects.get_mut(id as usize)
    }

    /// Returns a slot to the free list for reuse by a later [`Arena::alloc`].
    pub(crate) fn reclaim(&mut self, id: u32) {
        self.free.push(id);
    }

    pub fn live_count(&self) -> usize {
        self.objects.len() - self.free.len()
    }
}
