//! Heap objects: the object arena, native-backed objects, and arrays.

use crate::vm::value::JValue;

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
            ArrayData::Float(d) => d[i] = match v {
                JValue::Float(f) => f,
                JValue::Int(i) => i as f32,
                _ => panic!("float array store of {v:?}"),
            },
            ArrayData::Double(d) => d[i] = match v {
                JValue::Double(f) => f,
                JValue::Float(f) => f64::from(f),
                JValue::Long(l) => l as f64,
                JValue::Int(i) => f64::from(i),
                _ => panic!("double array store of {v:?}"),
            },
            ArrayData::Bool(d) => d[i] = v.truthy(),
            ArrayData::Obj(d) => d[i] = v,
        }
    }
}

/// Native (Rust-backed) objects. Objects of shim classes carry one of these
/// instead of interpreted fields.
#[derive(Debug, Clone)]
pub enum Native {
    /// java.lang.String payload.
    Str(String),
    /// Java array storage.
    Array(ArrayData),
    /// java.lang.Class instance: wraps a class id, or a primitive descriptor.
    ClassObj(ClassOrPrim),
    /// java.lang.Throwable and subclasses.
    Throwable { message: Option<String>, cause: JValue },
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
    Enum { name: String, ordinal: i32 },
    /// java.util.ArrayList / Arrays$ArrayList / Collections wrappers.
    List(Vec<JValue>),
    /// java.util.HashMap / LinkedHashMap.
    Map(Vec<(JValue, JValue)>),
    /// java.util.HashSet / LinkedHashSet.
    Set(Vec<JValue>),
    /// Iterators.
    Iter(IterKind),
    /// java.util.Map.Entry.
    MapEntry { map: u32, idx: usize },
    /// java.util.regex.Pattern (source kept for `pattern()`/`toString()`).
    Pattern { re: regex::Regex, source: String },
    /// java.util.regex.Matcher.
    Matcher(MatcherState),
    /// java.io.PrintStream (writes to the VM output sink).
    PrintStream,
    /// java.util.Random (xorshift64*).
    Random(u64),
    /// java.util.Date (epoch millis).
    Date(i64),
    /// java.util.Locale and other inert Java objects.
    Opaque,
    /// java.util.TimeZone (zone id string, e.g. "UTC", "GMT+07:00").
    TimeZone(String),
    /// java.text.SimpleDateFormat: pattern + resolved time zone id.
    DateFormatter { pattern: String, zone: String },
    /// java.text.ParsePosition (current index).
    ParsePosition(i32),
    /// java.util.ArrayDeque (a FIFO list for our purposes).
    ArrayDeque(Vec<JValue>),
    /// java.util.concurrent.locks.ReentrantLock.
    ReentrantLock { locked: bool },
    /// okhttp3.OkHttpClient$Builder: accumulated interceptor lists.
    OkHttpBuilder {
        interceptors: Vec<JValue>,
        network_interceptors: Vec<JValue>,
    },
    /// okhttp3.HttpUrl / HttpUrl$Builder: URL being built.
    HttpUrl(String),
    /// okhttp3.FormBody / FormBody$Builder: name/value pairs.
    FormBody(Vec<(String, String)>),
    /// okhttp3.Request produced by the RequestsKt helpers.
    Request { url: String, body: Option<JValue> },
    /// kotlin Lazy wrapper holding the initializer function (Function0).
    Lazy(JValue),
    /// kotlin Pair (two elements).
    Pair(JValue, JValue),
    /// kotlin.ranges.IntRange (current, last) — the cursor doubles as iterator
    /// position for IntIterator subclasses.
    IntRange(i32, i32),
    /// Array descriptor stored on a Class instance of an array type.
    ArrayDesc(ArrayData),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClassOrPrim {
    Class(u32),
    /// Primitive descriptor for `Integer.TYPE`-style class constants.
    Primitive(u8),
}

#[derive(Debug, Clone)]
pub enum IterKind {
    List { list: u32, idx: usize },
    MapEntries { map: u32, idx: usize },
    MapKeys { map: u32, idx: usize },
    MapValues { map: u32, idx: usize },
    Set { set: u32, idx: usize },
}

#[derive(Debug, Clone)]
pub struct MatcherState {
    pub pattern: regex::Regex,
    pub text: String,
    pub pos: usize,
    /// (start, end) of the most recent find()/matches().
    pub last: Option<(usize, usize)>,
}

/// A heap object. `class` is a class id; `fields` holds interpreted fields by
/// resolved offset; `native` carries the Rust-side payload for shim classes.
#[derive(Debug, Clone)]
pub struct JObject {
    pub class: u32,
    pub fields: Vec<JValue>,
    pub native: Option<Native>,
}

/// Simple growing arena. Objects are never reclaimed within a run; the whole
/// arena is dropped when the VM is dropped (per-run teardown keeps GC simple).
#[derive(Debug, Default)]
pub struct Arena {
    pub objects: Vec<JObject>,
}

impl Arena {
    pub fn alloc(&mut self, class: u32, fields: Vec<JValue>, native: Option<Native>) -> u32 {
        let id = self.objects.len() as u32;
        self.objects.push(JObject { class, fields, native });
        id
    }

    pub fn get(&self, id: u32) -> Option<&JObject> {
        self.objects.get(id as usize)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut JObject> {
        self.objects.get_mut(id as usize)
    }
}
