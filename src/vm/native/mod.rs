//! Native method implementations for the shim (host-provided) classes.
//!
//! Each class is registered by its own table (see `java/`, `kotlin.rs`,
//! `injekt.rs`, and the feature-gated `okhttp.rs`, `jsoup.rs`,
//! `android.rs`, `keiyoushi.rs`); `native_tables()` flattens them for
//! `register` and for shim class method dispatch, and `register` installs
//! the functions into `Vm::natives` keyed by (interned class, name, sig).

pub(crate) use std::cmp::Ordering;
pub(crate) use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
pub(crate) use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) use ::fancy_regex::Regex;

pub(crate) use crate::dex::insn::InvokeKind;
pub(crate) use crate::vm::error::JvmError;
pub(crate) use crate::vm::object::{ArrayData, ClassOrPrim, IterKind, MatcherState, Native};
pub(crate) use crate::vm::value::JValue;
pub use crate::vm::{MethodRef, NatErr, NativeEntry, NativeFn, Target, Vm};

pub(crate) type R = Result<JValue, NatErr>;

// table plumbing
// ---------------------------------------------------------------------------
macro_rules! ne {
    ($class:expr, $name:expr, $sig:expr, $instance:expr, $f:expr) => {
        NativeEntry {
            class: $class,
            name: $name,
            sig: $sig,
            instance: $instance,
            f: $f,
        }
    };
}

// ---------------------------------------------------------------------------
// table plumbing
// ---------------------------------------------------------------------------

/// The four standard `<init>` overloads shared by every throwable shim class.
/// Generated as a whole-array macro expansion because array literals cannot
/// spread a single macro invocation across multiple elements.
macro_rules! throwable_ctors_table {
    ($($class:expr),* $(,)?) => {
        &[
            $(
                ne!($class, "<init>", "()V", true, tinit0),
                ne!($class, "<init>", "(Ljava/lang/String;)V", true, tinit_str),
                ne!(
                    $class,
                    "<init>",
                    "(Ljava/lang/String;Ljava/lang/Throwable;)V",
                    true,
                    tinit_str_cause
                ),
                ne!($class, "<init>", "(Ljava/lang/Throwable;)V", true, tinit_cause),
            )*
        ]
    };
}

/// The four standard `<init>` overloads shared by every throwable shim class.
pub const THROWABLE_CTORS: &[NativeEntry] = throwable_ctors_table![
    "Ljava/lang/Throwable;",
    "Ljava/lang/Exception;",
    "Ljava/lang/RuntimeException;",
    "Ljava/lang/Error;",
    "Ljava/lang/AssertionError;",
    "Ljava/lang/StackOverflowError;",
    "Ljava/lang/OutOfMemoryError;",
    "Ljava/lang/NullPointerException;",
    "Ljava/lang/ArithmeticException;",
    "Ljava/lang/IllegalArgumentException;",
    "Ljava/lang/IllegalStateException;",
    "Ljava/lang/NumberFormatException;",
    "Ljava/lang/UnsupportedOperationException;",
    "Ljava/lang/IndexOutOfBoundsException;",
    "Ljava/lang/ArrayIndexOutOfBoundsException;",
    "Ljava/lang/StringIndexOutOfBoundsException;",
    "Ljava/lang/ClassCastException;",
    "Ljava/lang/NegativeArraySizeException;",
    "Ljava/lang/NoSuchElementException;",
    "Ljava/lang/NoSuchMethodError;",
    "Ljava/lang/NoClassDefFoundError;",
    "Ljava/lang/ClassNotFoundException;",
    "Ljava/io/IOException;",
    "Ljava/net/MalformedURLException;",
    "Ljava/lang/InterruptedException;",
    "Ljava/lang/SecurityException;",
];

#[cfg(feature = "android")]
mod android;
mod injekt;
mod java;
#[cfg(feature = "jsoup")]
mod jsoup;
#[cfg(feature = "tachiyomi")]
pub mod keiyoushi;
mod kotlin;
#[cfg(feature = "okhttp")]
mod okhttp;

#[cfg(feature = "tachiyomi")]
pub(crate) use self::keiyoushi::*;
#[cfg(feature = "okhttp")]
pub(crate) use self::okhttp::*;
pub(crate) use self::{java::*, kotlin::*};

// ---------------------------------------------------------------------------
// HTTP bridge helpers (okhttp request objects -> plain data)
// ---------------------------------------------------------------------------

#[cfg(any(feature = "okhttp", feature = "tachiyomi"))]
#[cfg_attr(not(feature = "tachiyomi"), allow(dead_code))]
pub(crate) fn form_body_to_string(vm: &mut Vm, body: &Option<JValue>) -> Option<String> {
    let Some(JValue::Obj(id)) = body.as_ref() else {
        return None;
    };
    let o = vm.arena.get(*id)?;
    match o.native.as_ref()? {
        Native::FormBody(fields) => Some(
            fields
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("&"),
        ),
        _ => None,
    }
}

#[cfg(any(feature = "okhttp", feature = "tachiyomi"))]
#[cfg_attr(not(feature = "tachiyomi"), allow(dead_code))]
pub(crate) type RequestParts = (String, String, Vec<(String, String)>, Option<JValue>);

#[cfg(any(feature = "okhttp", feature = "tachiyomi"))]
#[cfg_attr(not(feature = "tachiyomi"), allow(dead_code))]
pub(crate) fn request_parts(vm: &mut Vm, v: JValue) -> Result<RequestParts, NatErr> {
    let Some(Native::Request {
        url,
        method,
        headers,
        body,
    }) = payload(vm, v)
    else {
        return Err(npe(vm));
    };
    Ok((url.clone(), method.clone(), headers.clone(), *body))
}

// ---------------------------------------------------------------------------
pub fn register(vm: &mut Vm) {
    for e in native_tables()
        .into_iter()
        .flatten()
        .chain(global_native_entries())
    {
        let key = (vm.intern(e.class), vm.intern(e.name), vm.intern(e.sig));
        vm.natives.insert(key, e.f);
    }
}

/// Process-wide dynamic native tables.
///
/// Tables registered here are installed into **every** [`Context`] created
/// afterwards (on top of the statically compiled tables), letting embedders
/// plug their own host libraries without recompiling dexvm. Registrations
/// are additive and append-only; call as early as possible.
static GLOBAL_NATIVES: std::sync::OnceLock<std::sync::RwLock<Vec<&'static NativeEntry>>> =
    std::sync::OnceLock::new();

/// Appends `table` to the process-wide native registry.
///
/// ```no_run
/// # use dexvm::vm::native::register_global;
/// # use dexvm::vm::{NativeEntry, NatErr, Vm};
/// # use dexvm::vm::value::JValue;
/// # static TABLE: &[NativeEntry] = &[];
/// register_global(TABLE);
/// ```
pub fn register_global(table: &'static [NativeEntry]) {
    let lock = GLOBAL_NATIVES.get_or_init(Default::default);
    lock.write().unwrap().extend(table.iter());
}

/// Read-only view of every globally registered native.
pub fn global_native_entries() -> Vec<&'static NativeEntry> {
    GLOBAL_NATIVES
        .get()
        .map_or_else(Vec::new, |l| l.read().unwrap().clone())
}

/// Number of globally registered natives (introspection / tests).
pub fn global_count() -> usize {
    global_native_entries().len()
}

/// Every native table in the crate, flattened into per-class tables, for
/// `register` and for shim class method dispatch in `Vm::load_shim_class`.
pub(crate) fn native_tables() -> Vec<&'static [NativeEntry]> {
    let mut out: Vec<&'static [NativeEntry]> = Vec::new();
    java::java_tables(&mut out);
    out.push(kotlin::KOTLIN_TABLE);
    out.push(injekt::INJEKT_TABLE);
    #[cfg(feature = "okhttp")]
    out.push(okhttp::OKHTTP_TABLE);
    #[cfg(feature = "jsoup")]
    out.push(jsoup::JSOUP_TABLE);
    #[cfg(feature = "android")]
    out.push(android::ANDROID_TABLE);
    #[cfg(feature = "tachiyomi")]
    out.push(keiyoushi::KEIYOUSHI_TABLE);
    out.push(THROWABLE_CTORS);
    out
}

// ---------------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------------
pub(crate) fn nat_fatal(e: JvmError) -> NatErr {
    match e {
        JvmError::Uncaught(t) => NatErr::Throw(t),
        e => NatErr::Fatal(e),
    }
}

pub(crate) fn npe(vm: &mut Vm) -> NatErr {
    NatErr::Throw(vm.err_npe())
}
pub(crate) fn iae(vm: &mut Vm, m: impl Into<String>) -> NatErr {
    NatErr::Throw(vm.err_iae(m))
}
pub(crate) fn uoe(vm: &mut Vm, m: impl Into<String>) -> NatErr {
    NatErr::Throw(vm.err_uoe(m))
}
pub(crate) fn nfe(vm: &mut Vm, m: impl Into<String>) -> NatErr {
    NatErr::Throw(vm.err_nfe(m))
}
pub(crate) fn aioobe(vm: &mut Vm, idx: i32, len: i32) -> NatErr {
    NatErr::Throw(vm.err_aioobe(idx, len))
}
pub(crate) fn ioobe(vm: &mut Vm, idx: i32) -> NatErr {
    NatErr::Throw(vm.err_ioobe(idx))
}
pub(crate) fn sioobe(vm: &mut Vm, m: impl Into<String>) -> NatErr {
    NatErr::Throw(vm.err_sioobe(m))
}
pub(crate) fn cce(vm: &mut Vm, m: impl Into<String>) -> NatErr {
    NatErr::Throw(vm.err_cce(m))
}
pub(crate) fn no_such_elem(vm: &mut Vm) -> NatErr {
    NatErr::Throw(vm.throwable_of("Ljava/util/NoSuchElementException;", "No value present"))
}

/// Immutable access to an object's native payload.
pub(crate) fn payload(vm: &Vm, v: JValue) -> Option<&Native> {
    match v {
        JValue::Obj(id) => vm.arena.get(id).and_then(|o| o.native.as_ref()),
        _ => None,
    }
}

/// Mutable access to an object's native payload.
///
/// Objects created by `new-instance` start with `native: None`; the first
/// mutable access lazily installs a class-derived default payload so `<init>`
/// natives (and later reads) work on fresh objects.
pub(crate) fn payload_mut(vm: &mut Vm, v: JValue) -> Option<&mut Native> {
    let id = match v {
        JValue::Obj(id) => id,
        _ => return None,
    };
    let has_native = vm.arena.get(id).is_some_and(|o| o.native.is_some());
    if !has_native {
        if let Some(make) = default_native_for(vm, id) {
            vm.arena.get_mut(id)?.native = Some(make);
        }
    }
    vm.arena.get_mut(id)?.native.as_mut()
}

/// Default payload for a freshly allocated object of a shim class.
pub(crate) fn default_native_for(vm: &mut Vm, id: u32) -> Option<Native> {
    let (class_id, desc) = {
        let o = vm.arena.get(id)?;
        let cl = vm.classes.get(o.class as usize)?;
        (o.class, vm.str_of(cl.descriptor).to_string())
    };
    // Enum and Throwable defaults apply to dex subclasses as well, found by
    // walking the superclass chain (shim superclasses are already linked).
    let mut c = Some(class_id);
    while let Some(cc) = c {
        let cl = &vm.classes[cc as usize];
        match vm.str_of(cl.descriptor) {
            "Ljava/lang/Enum;" => {
                return Some(Native::Enum {
                    name: String::new(),
                    ordinal: 0,
                });
            }
            "Ljava/lang/Throwable;" => {
                return Some(Native::Throwable {
                    message: None,
                    cause: JValue::Null,
                });
            }
            #[cfg(feature = "tachiyomi")]
            "Leu/kanade/tachiyomi/source/model/Filter;"
            | "Leu/kanade/tachiyomi/source/model/Filter$Header;"
            | "Leu/kanade/tachiyomi/source/model/Filter$Separator;"
            | "Leu/kanade/tachiyomi/source/model/Filter$Select;"
            | "Leu/kanade/tachiyomi/source/model/Filter$Sort;"
            | "Leu/kanade/tachiyomi/source/model/Filter$Text;"
            | "Leu/kanade/tachiyomi/source/model/Filter$TriState;"
            | "Leu/kanade/tachiyomi/source/model/Filter$Group;" => {
                return Some(Native::SFilter {
                    name: String::new(),
                    state: 0,
                    is_checked: false,
                    children: Vec::new(),
                    options: Vec::new(),
                    text_value: String::new(),
                });
            }
            _ => {}
        }
        c = cl.superclass;
    }
    match desc.as_str() {
        "Ljava/lang/String;" => Some(Native::Str(String::new())),
        "Ljava/lang/StringBuilder;" => Some(Native::StringBuilder(String::new())),
        "Ljava/lang/Integer;" => Some(Native::IntBox(0)),
        "Ljava/lang/Long;" => Some(Native::LongBox(0)),
        "Ljava/lang/Short;" => Some(Native::ShortBox(0)),
        "Ljava/lang/Byte;" => Some(Native::ByteBox(0)),
        "Ljava/lang/Character;" => Some(Native::CharBox(0)),
        "Ljava/lang/Boolean;" => Some(Native::BoolBox(false)),
        "Ljava/lang/Float;" => Some(Native::FloatBox(0.0)),
        "Ljava/lang/Double;" => Some(Native::DoubleBox(0.0)),
        "Ljava/util/ArrayList;" => Some(Native::List(Vec::new())),
        "Ljava/util/ArrayDeque;" => Some(Native::ArrayDeque(Vec::new())),
        "Lokhttp3/FormBody$Builder;" => Some(Native::FormBody(Vec::new())),
        "Lokhttp3/Request$Builder;" => Some(Native::RequestBuilder {
            url: String::new(),
            method: String::new(),
            headers: Vec::new(),
            body: None,
        }),
        "Ljava/util/HashMap;" | "Ljava/util/LinkedHashMap;" => Some(Native::Map(Vec::new())),
        "Ljava/util/HashSet;" | "Ljava/util/LinkedHashSet;" => Some(Native::Set(Vec::new())),
        "Ljava/util/regex/Pattern;" => Some(Native::Pattern {
            re: Regex::new("").expect("empty regex"),
            source: String::new(),
        }),
        "Lkotlin/text/Regex;" => Some(Native::Pattern {
            re: Regex::new("").expect("empty regex"),
            source: String::new(),
        }),
        "Ljava/util/regex/Matcher;" => Some(Native::Matcher(MatcherState {
            pattern: Regex::new("").expect("empty regex"),
            text: String::new(),
            pos: 0,
            last: None,
        })),
        "Ljava/util/Random;" => Some(Native::Random(0)),
        "Ljava/util/Date;" => Some(Native::Date(0)),
        "Ljava/text/SimpleDateFormat;" => Some(Native::DateFormatter {
            pattern: String::new(),
            zone: String::new(),
        }),
        "Ljava/text/ParsePosition;" => Some(Native::ParsePosition(0)),
        "Ljava/util/Locale;" => Some(Native::Opaque),
        "Ljava/io/PrintStream;" => Some(Native::PrintStream),
        "Ljava/lang/Thread;" => Some(Native::Opaque),
        "Ljava/util/Map$Entry;" => Some(Native::MapEntry { map: 0, idx: 0 }),
        "Ljava/util/concurrent/locks/ReentrantLock;" => {
            Some(Native::ReentrantLock { locked: false })
        }
        #[cfg(feature = "tachiyomi")]
        "Leu/kanade/tachiyomi/source/model/SManga;" => Some(keiyoushi::empty_smanga()),
        #[cfg(feature = "tachiyomi")]
        "Leu/kanade/tachiyomi/source/model/SChapter;" => Some(keiyoushi::empty_schapter()),
        #[cfg(feature = "tachiyomi")]
        "Leu/kanade/tachiyomi/source/model/Page;" => Some(Native::SPPage {
            index: 0,
            name: String::new(),
            url: String::new(),
            image_url: String::new(),
        }),
        #[cfg(feature = "tachiyomi")]
        "Leu/kanade/tachiyomi/source/model/MangasPage;" => Some(Native::SMangasPage {
            mangas: Vec::new(),
            has_next: false,
        }),
        #[cfg(feature = "tachiyomi")]
        "Leu/kanade/tachiyomi/source/model/FilterList;" => Some(Native::SFilterList(Vec::new())),
        #[cfg(feature = "tachiyomi")]
        "Leu/kanade/tachiyomi/source/model/Filter;"
        | "Leu/kanade/tachiyomi/source/model/Filter$Header;"
        | "Leu/kanade/tachiyomi/source/model/Filter$Separator;"
        | "Leu/kanade/tachiyomi/source/model/Filter$Select;"
        | "Leu/kanade/tachiyomi/source/model/Filter$Sort;"
        | "Leu/kanade/tachiyomi/source/model/Filter$Text;"
        | "Leu/kanade/tachiyomi/source/model/Filter$TriState;"
        | "Leu/kanade/tachiyomi/source/model/Filter$Group;" => Some(Native::SFilter {
            name: String::new(),
            state: 0,
            is_checked: false,
            children: Vec::new(),
            options: Vec::new(),
            text_value: String::new(),
        }),
        _ => None,
    }
}

pub(crate) fn obj_class(vm: &Vm, id: u32) -> u32 {
    vm.arena.objects[id as usize].class
}

/// Owned copy of a java.lang.String payload.
pub(crate) fn jstr(vm: &mut Vm, v: JValue) -> Result<String, NatErr> {
    let Some(n) = payload(vm, v) else {
        return Err(npe(vm));
    };
    match n {
        Native::Str(s) => Ok(s.clone()),
        _ => Err(npe(vm)),
    }
}

/// Borrowed java.lang.String payload (no error helpers or allocs afterwards).
pub(crate) fn peek_str(vm: &Vm, v: JValue) -> Option<&str> {
    let n = payload(vm, v)?;
    match n {
        Native::Str(s) => Some(s),
        _ => None,
    }
}

/// String or StringBuilder payload (CharSequence positions).
pub(crate) fn charseq_of(vm: &mut Vm, v: JValue) -> Result<String, NatErr> {
    let Some(n) = payload(vm, v) else {
        return Err(npe(vm));
    };
    match n {
        Native::Str(s) | Native::StringBuilder(s) => Ok(s.clone()),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn new_str(vm: &mut Vm, s: &str) -> JValue {
    vm.alloc_string(s)
}

pub(crate) fn alloc(vm: &mut Vm, desc: &str, native: Native) -> Result<JValue, NatErr> {
    let class = vm.ensure_class_by_desc(desc).map_err(nat_fatal)?;
    Ok(JValue::Obj(vm.arena.alloc(class, Vec::new(), Some(native))))
}

pub(crate) fn boxed(vm: &mut Vm, desc: &str, native: Native) -> Result<JValue, NatErr> {
    alloc(vm, desc, native)
}

/// Allocate a Java array with the given element descriptor. The array class
/// is looked up through the dex type table (callers' signatures reference
/// these descriptors); if absent it falls back to Object[].
pub(crate) fn alloc_arr(
    vm: &mut Vm,
    elem_desc: &str,
    len: usize,
    fill: impl FnOnce() -> ArrayData,
) -> Result<JValue, NatErr> {
    let full = format!("[{elem_desc}");
    let (class, data) = match vm.ensure_class_by_desc(&full) {
        Ok(c) => (c, fill()),
        Err(_) => (
            vm.ensure_class_by_desc("[Ljava/lang/Object;")
                .map_err(nat_fatal)?,
            fill_fallback(elem_desc, len),
        ),
    };
    Ok(JValue::Obj(vm.arena.alloc(
        class,
        Vec::new(),
        Some(Native::Array(data)),
    )))
}

pub(crate) fn fill_fallback(elem_desc: &str, len: usize) -> ArrayData {
    if matches!(elem_desc, "B" | "C" | "S" | "I" | "J" | "F" | "D" | "Z") {
        ArrayData::new(elem_desc, len)
    } else {
        ArrayData::Obj(vec![JValue::Null; len])
    }
}

pub(crate) fn alloc_empty_arr(vm: &mut Vm, elem_desc: &str) -> Result<JValue, NatErr> {
    let e = elem_desc.to_string();
    alloc_arr(vm, elem_desc, 0, move || ArrayData::new(&e, 0))
}

// unboxing with tolerant conversions
pub(crate) fn int_of(vm: &Vm, v: JValue) -> i32 {
    match v {
        JValue::Int(i) => i,
        JValue::Long(l) => l as i32,
        JValue::Float(f) => f as i32,
        JValue::Double(d) => d as i32,
        JValue::Null => 0,
        JValue::Obj(_) => match payload(vm, v) {
            Some(Native::IntBox(i)) => *i,
            Some(Native::LongBox(l)) => *l as i32,
            Some(Native::ShortBox(s)) => i32::from(*s),
            Some(Native::ByteBox(b)) => i32::from(*b),
            Some(Native::CharBox(c)) => i32::from(*c),
            Some(Native::FloatBox(f)) => *f as i32,
            Some(Native::DoubleBox(d)) => *d as i32,
            Some(Native::BoolBox(b)) => i32::from(*b),
            _ => 0,
        },
    }
}

pub(crate) fn long_of(vm: &Vm, v: JValue) -> i64 {
    match v {
        JValue::Long(l) => l,
        JValue::Int(i) => i64::from(i),
        JValue::Float(f) => f as i64,
        JValue::Double(d) => d as i64,
        JValue::Null => 0,
        JValue::Obj(_) => match payload(vm, v) {
            Some(Native::LongBox(l)) => *l,
            Some(Native::IntBox(i)) => i64::from(*i),
            Some(Native::ShortBox(s)) => i64::from(*s),
            Some(Native::ByteBox(b)) => i64::from(*b),
            Some(Native::CharBox(c)) => i64::from(*c),
            Some(Native::FloatBox(f)) => *f as i64,
            Some(Native::DoubleBox(d)) => *d as i64,
            _ => 0,
        },
    }
}

pub(crate) fn float_of(vm: &Vm, v: JValue) -> f32 {
    match v {
        JValue::Float(f) => f,
        JValue::Int(i) => i as f32,
        JValue::Long(l) => l as f32,
        JValue::Double(d) => d as f32,
        JValue::Null => 0.0,
        JValue::Obj(_) => match payload(vm, v) {
            Some(Native::FloatBox(f)) => *f,
            Some(Native::IntBox(i)) => *i as f32,
            Some(Native::LongBox(l)) => *l as f32,
            Some(Native::DoubleBox(d)) => *d as f32,
            _ => 0.0,
        },
    }
}

pub(crate) fn double_of(vm: &Vm, v: JValue) -> f64 {
    match v {
        JValue::Double(d) => d,
        JValue::Float(f) => f64::from(f),
        JValue::Int(i) => f64::from(i),
        JValue::Long(l) => l as f64,
        JValue::Null => 0.0,
        JValue::Obj(_) => match payload(vm, v) {
            Some(Native::DoubleBox(d)) => *d,
            Some(Native::FloatBox(f)) => f64::from(*f),
            Some(Native::IntBox(i)) => f64::from(*i),
            Some(Native::LongBox(l)) => *l as f64,
            _ => 0.0,
        },
    }
}

pub(crate) fn bool_of(vm: &Vm, v: JValue) -> bool {
    match v {
        JValue::Int(i) => i != 0,
        JValue::Long(l) => l != 0,
        JValue::Null => false,
        JValue::Obj(_) => match payload(vm, v) {
            Some(Native::BoolBox(b)) => *b,
            _ => false,
        },
        _ => false,
    }
}

/// Java-style string rendering (Float.toString / Double.toString rules).
pub(crate) fn fmt_f32(v: f32) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v.is_sign_positive() {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        };
    }
    let mut s = v.to_string();
    if !s.contains(['.', 'e', 'E']) {
        s.push_str(".0");
    }
    s
}

pub(crate) fn fmt_f64(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v.is_sign_positive() {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        };
    }
    let mut s = v.to_string();
    if !s.contains(['.', 'e', 'E']) {
        s.push_str(".0");
    }
    s
}

/// Virtual call helper for natives (extra args appended after the receiver).
pub(crate) fn inv_virt(
    vm: &mut Vm,
    recv: JValue,
    name: &str,
    sig: &str,
    extra: &[JValue],
) -> Result<JValue, NatErr> {
    let mref = MethodRef {
        name: vm.intern(name),
        sig: vm.intern(sig),
        ret: 0,
        args: Vec::new(),
        class_desc: 0,
    };
    let target = vm
        .resolve_target(InvokeKind::Virtual, &mref, Some(recv.as_obj()))
        .map_err(nat_fatal)?;
    let mut args = Vec::with_capacity(1 + extra.len());
    args.push(recv);
    args.extend_from_slice(extra);
    vm.call_target(target, args).map_err(nat_fatal)
}

/// `String.valueOf(x)`-ish conversion, including null and boxed values.
pub(crate) fn to_string_of(vm: &mut Vm, v: JValue) -> Result<String, NatErr> {
    match v {
        JValue::Null => Ok("null".to_string()),
        JValue::Int(i) => Ok(i.to_string()),
        JValue::Long(l) => Ok(l.to_string()),
        JValue::Float(f) => Ok(fmt_f32(f)),
        JValue::Double(d) => Ok(fmt_f64(d)),
        JValue::Obj(_) => {
            if let Some(s) = peek_str(vm, v) {
                return Ok(s.to_string());
            }
            match inv_virt(vm, v, "toString", "()Ljava/lang/String;", &[]) {
                Ok(JValue::Obj(id)) => match payload(vm, JValue::Obj(id)) {
                    Some(Native::Str(s)) => Ok(s.clone()),
                    _ => Ok("<object>".to_string()),
                },
                _ => Ok("<error>".to_string()),
            }
        }
    }
}

/// java.util.Objects.equals semantics.
pub(crate) fn java_equals(vm: &mut Vm, a: JValue, b: JValue) -> Result<bool, NatErr> {
    let r = match (a, b) {
        (JValue::Null, JValue::Null) => true,
        (JValue::Null, _) | (_, JValue::Null) => false,
        (JValue::Int(x), JValue::Int(y)) => x == y,
        (JValue::Long(x), JValue::Long(y)) => x == y,
        (JValue::Float(x), JValue::Float(y)) => x.to_bits() == y.to_bits(),
        (JValue::Double(x), JValue::Double(y)) => x.to_bits() == y.to_bits(),
        (a, b) => match inv_virt(vm, a, "equals", "(Ljava/lang/Object;)Z", &[b]) {
            Ok(JValue::Int(i)) => i != 0,
            _ => false,
        },
    };
    Ok(r)
}

/// Boxed-equivalent hash (identity hash falls back to the arena id).
pub(crate) fn java_hash(vm: &mut Vm, v: JValue) -> i32 {
    match v {
        JValue::Null => 0,
        JValue::Int(i) => i,
        JValue::Long(l) => (l ^ (l >> 32)) as i32,
        JValue::Float(f) => f.to_bits() as i32,
        JValue::Double(d) => {
            let b = d.to_bits();
            (b ^ (b >> 32)) as i32
        }
        JValue::Obj(_) => match inv_virt(vm, v, "hashCode", "()I", &[]) {
            Ok(JValue::Int(i)) => i,
            _ => 0,
        },
    }
}

pub(crate) fn java_cmp(vm: &mut Vm, a: JValue, b: JValue) -> Result<Ordering, NatErr> {
    match inv_virt(vm, a, "compareTo", "(Ljava/lang/Object;)I", &[b]) {
        Ok(JValue::Int(i)) => Ok(i.cmp(&0)),
        _ => Err(npe(vm)),
    }
}

// utf16-based string helpers (Java indexes strings by code units)
pub(crate) fn u16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
pub(crate) fn u16str(v: &[u16]) -> String {
    String::from_utf16_lossy(v)
}
pub(crate) fn u16len(s: &str) -> usize {
    s.encode_utf16().count()
}
pub(crate) fn utf16_cmp(a: &str, b: &str) -> Ordering {
    let mut ia = a.encode_utf16();
    let mut ib = b.encode_utf16();
    loop {
        match (ia.next(), ib.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => match x.cmp(&y) {
                Ordering::Equal => continue,
                o => return o,
            },
        }
    }
}
pub(crate) fn u16_index_of(hay: &str, needle: &str, from: usize) -> Option<usize> {
    let h = u16(hay);
    let n = u16(needle);
    if n.is_empty() {
        return Some(from.min(h.len()));
    }
    let mut i = from;
    while i + n.len() <= h.len() {
        if h[i..i + n.len()] == n[..] {
            return Some(i);
        }
        i += 1;
    }
    None
}
pub(crate) fn u16_last_index_of(hay: &str, needle: &str, from: i64) -> Option<usize> {
    let h = u16(hay);
    let n = u16(needle);
    if from < 0 {
        return None;
    }
    if n.is_empty() {
        return Some((from as usize).min(h.len()));
    }
    let mut i = (from as usize).min(h.len().saturating_sub(n.len()));
    loop {
        if h[i..i + n.len()] == n[..] {
            return Some(i);
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    None
}
pub(crate) fn char_at(s: &str, i: usize) -> Option<u16> {
    u16(s).get(i).copied()
}

/// java.util.regex.Pattern.split semantics for the given limit.
pub(crate) fn split_java(re: &Regex, text: &str, limit: i32) -> Vec<String> {
    let matches: Vec<(usize, usize)> = re
        .find_iter(text)
        .flatten()
        .map(|m| (m.start(), m.end()))
        .collect();
    let mut parts = Vec::new();
    let mut pos = 0usize;
    let mut count = 0usize;
    let max = if limit <= 0 {
        usize::MAX
    } else {
        limit as usize
    };
    for (s, e) in &matches {
        if *s < pos {
            continue;
        }
        if count + 1 >= max {
            break;
        }
        parts.push(text[pos..*s].to_string());
        pos = *e;
        count += 1;
    }
    parts.push(text[pos..].to_string());
    if limit == 0 {
        while parts.last().is_some_and(|p| p.is_empty()) {
            parts.pop();
        }
    }
    parts
}

const DIGITS: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

pub(crate) fn int_to_string(v: i32, radix: u32) -> String {
    let radix = radix.clamp(2, 36);
    if v == 0 {
        return "0".to_string();
    }
    let neg = v < 0;
    let mut u = v.wrapping_abs() as u32;
    let mut buf = Vec::new();
    while u > 0 {
        buf.push(DIGITS[(u % radix) as usize]);
        u /= radix;
    }
    if neg {
        buf.push('-');
    }
    buf.iter().rev().collect()
}

pub(crate) fn long_to_string_help(v: i64, radix: u32) -> String {
    let radix = radix.clamp(2, 36);
    if v == 0 {
        return "0".to_string();
    }
    let neg = v < 0;
    let mut u = v.wrapping_abs() as u64;
    let mut buf = Vec::new();
    while u > 0 {
        buf.push(DIGITS[(u % u64::from(radix)) as usize]);
        u /= u64::from(radix);
    }
    if neg {
        buf.push('-');
    }
    buf.iter().rev().collect()
}

pub(crate) fn parse_int_radix(vm: &mut Vm, s: &str, radix: u32) -> Result<i32, NatErr> {
    if !(2..=36).contains(&radix) {
        return Err(nfe(vm, format!("radix {radix} out of range")));
    }
    i32::from_str_radix(s, radix).map_err(|_| nfe(vm, format!("For input string: \"{s}\"")))
}

pub(crate) fn parse_long_radix(vm: &mut Vm, s: &str, radix: u32) -> Result<i64, NatErr> {
    if !(2..=36).contains(&radix) {
        return Err(nfe(vm, format!("radix {radix} out of range")));
    }
    i64::from_str_radix(s, radix).map_err(|_| nfe(vm, format!("For input string: \"{s}\"")))
}

pub(crate) fn parse_float(vm: &mut Vm, raw: &str) -> Result<f32, NatErr> {
    let s: String = raw.chars().filter(|&c| c != '_').collect();
    let s = s.trim().to_ascii_lowercase();
    let s = if let Some(stripped) = s.strip_suffix('f') {
        stripped.to_string()
    } else {
        s
    };
    if s == "infinity" || s == "+infinity" {
        return Ok(f32::INFINITY);
    }
    if s == "-infinity" {
        return Ok(f32::NEG_INFINITY);
    }
    if s == "nan" {
        return Ok(f32::NAN);
    }
    s.parse::<f32>()
        .map_err(|_| nfe(vm, format!("For input string: \"{raw}\"")))
}

pub(crate) fn parse_double(vm: &mut Vm, raw: &str) -> Result<f64, NatErr> {
    let s: String = raw.chars().filter(|&c| c != '_').collect();
    let s = s.trim().to_ascii_lowercase();
    if s == "infinity" || s == "+infinity" {
        return Ok(f64::INFINITY);
    }
    if s == "-infinity" {
        return Ok(f64::NEG_INFINITY);
    }
    if s == "nan" {
        return Ok(f64::NAN);
    }
    s.parse::<f64>()
        .map_err(|_| nfe(vm, format!("For input string: \"{raw}\"")))
}

pub(crate) fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

static RNG: AtomicU64 = AtomicU64::new(0x9E3779B97F4A7C15);
pub(crate) fn next_random_u64() -> u64 {
    let mut cur = RNG.load(AtomicOrdering::Relaxed);
    loop {
        let next = cur
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        if RNG
            .compare_exchange_weak(cur, next, AtomicOrdering::Relaxed, AtomicOrdering::Relaxed)
            .is_ok()
        {
            return next;
        }
        cur = RNG.load(AtomicOrdering::Relaxed);
    }
}

pub(crate) fn floor_div_i(a: i32, b: i32) -> i32 {
    let q = a / b;
    let r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}
pub(crate) fn floor_div_l(a: i64, b: i64) -> i64 {
    let q = a / b;
    let r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}
pub(crate) fn floor_mod_i(a: i32, b: i32) -> i32 {
    a - floor_div_i(a, b) * b
}
pub(crate) fn floor_mod_l(a: i64, b: i64) -> i64 {
    a - floor_div_l(a, b) * b
}

pub(crate) fn fmax32(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else {
        a.max(b)
    }
}
pub(crate) fn fmin32(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else {
        a.min(b)
    }
}
pub(crate) fn fmax64(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}
pub(crate) fn fmin64(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

pub(crate) fn comma_group(mut s: String) -> String {
    let neg = s.starts_with('-');
    if neg {
        s = s[1..].to_string();
    }
    let b = s.as_bytes();
    let rem = b.len() % 3;
    let mut out = String::with_capacity(b.len() + b.len() / 3);
    for (idx, &c) in b.iter().enumerate() {
        if idx > 0 && (idx - rem).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c as char);
    }
    if neg {
        out.insert(0, '-');
    }
    out
}
