//! Native method implementations for the shim (host-provided) classes.
//!
//! Every method of every shim class must appear in `NATIVE_TABLE`; class
//! loading picks its methods up from there, and `register` installs the
//! functions into `Vm::natives` keyed by (interned class, name, sig).

use std::cmp::Ordering;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{SystemTime, UNIX_EPOCH};

use ::regex::Regex;

use crate::dex::insn::InvokeKind;
use crate::vm::error::JvmError;
use crate::vm::object::{ArrayData, ClassOrPrim, IterKind, MatcherState, Native};
use crate::vm::value::JValue;
use crate::vm::{MethodRef, NatErr, NativeEntry, Vm};

type R = Result<JValue, NatErr>;

mod collections;
mod io;
mod kotlin;
mod lang;
mod math;
mod regex;
mod string;
mod sync;
mod text;
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


#[cfg(feature = "okhttp")]
mod okhttp;
#[cfg(feature = "keiyoushi")]
pub mod keiyoushi;


pub(crate) use self::{
    collections::*, io::*, kotlin::*, lang::*, math::*, regex::*, string::*, sync::*, text::*,
};
#[cfg(feature = "okhttp")]
pub(crate) use self::okhttp::*;
#[cfg(feature = "keiyoushi")]
pub(crate) use self::keiyoushi::*;

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

/// Native methods for the OkHttp and mihon-network shim classes.
#[cfg(feature = "okhttp")]
pub const OKHTTP_TABLE: &[NativeEntry] = &[
    // ---- eu.kanade.tachiyomi.network.NetworkHelper / source HttpSource ----
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getNetwork", "()Leu/kanade/tachiyomi/network/NetworkHelper;", true, http_source_get_network),
    ne!("Leu/kanade/tachiyomi/source/online/HttpSource;", "getHeaders", "()Lokhttp3/Headers;", true, http_source_get_headers),
    ne!("Leu/kanade/tachiyomi/network/NetworkHelper;", "getClient", "()Lokhttp3/OkHttpClient;", true, network_helper_get_client),
    // ---- okhttp3.OkHttpClient ----
    ne!("Lokhttp3/OkHttpClient;", "newBuilder", "()Lokhttp3/OkHttpClient$Builder;", true, okhttp_client_new_builder),
    // ---- okhttp3.OkHttpClient$Builder ----
    ne!("Lokhttp3/OkHttpClient$Builder;", "addInterceptor", "(Lokhttp3/Interceptor;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_add_interceptor),
    ne!("Lokhttp3/OkHttpClient$Builder;", "addNetworkInterceptor", "(Lokhttp3/Interceptor;)Lokhttp3/OkHttpClient$Builder;", true, okhttp_builder_add_network_interceptor),
    ne!("Lokhttp3/OkHttpClient$Builder;", "interceptors", "()Ljava/util/List;", true, okhttp_builder_interceptors),
    ne!("Lokhttp3/OkHttpClient$Builder;", "networkInterceptors", "()Ljava/util/List;", true, okhttp_builder_network_interceptors),
    ne!("Lokhttp3/OkHttpClient$Builder;", "build", "()Lokhttp3/OkHttpClient;", true, okhttp_builder_build),
    // ---- okhttp3.FormBody / FormBody$Builder ----
    ne!("Lokhttp3/FormBody$Builder;", "<init>", "(Ljava/nio/charset/Charset;ILkotlin/jvm/internal/DefaultConstructorMarker;)V", true, okhttp_form_builder_init),
    ne!("Lokhttp3/FormBody$Builder;", "add", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/FormBody$Builder;", true, okhttp_form_builder_add),
    ne!("Lokhttp3/FormBody$Builder;", "build", "()Lokhttp3/FormBody;", true, okhttp_form_builder_build),
    // ---- okhttp3.HttpUrl / HttpUrl$Builder ----
    ne!("Lokhttp3/HttpUrl$Companion;", "parse", "(Ljava/lang/String;)Lokhttp3/HttpUrl;", true, okhttp_http_url_parse),
    ne!("Lokhttp3/HttpUrl;", "newBuilder", "()Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_new_builder),
    ne!("Lokhttp3/HttpUrl$Builder;", "addQueryParameter", "(Ljava/lang/String;Ljava/lang/String;)Lokhttp3/HttpUrl$Builder;", true, okhttp_http_url_builder_add_query),
    ne!("Lokhttp3/HttpUrl$Builder;", "toString", "()Ljava/lang/String;", true, okhttp_http_url_builder_to_string),
    // ---- eu.kanade.tachiyomi.network.RequestsKt ----
    ne!("Leu/kanade/tachiyomi/network/RequestsKt;", "POST$default", "(Ljava/lang/String;Lokhttp3/Headers;Lokhttp3/RequestBody;Lokhttp3/CacheControl;ILjava/lang/Object;)Lokhttp3/Request;", false, requests_kt_post_default),
];

pub fn register(vm: &mut Vm) {
    #[allow(unused_mut)]
    let mut tables: Vec<&[NativeEntry]> = vec![NATIVE_TABLE, THROWABLE_CTORS];
    #[cfg(feature = "okhttp")]
    tables.push(OKHTTP_TABLE);
    #[cfg(feature = "keiyoushi")]
    tables.push(keiyoushi::KEIYOUSHI_TABLE);
    for e in tables.into_iter().flatten() {
        let key = (vm.intern(e.class), vm.intern(e.name), vm.intern(e.sig));
        vm.natives.insert(key, e.f);
    }
}

// ---------------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------------

fn nat_fatal(e: JvmError) -> NatErr {
    match e {
        JvmError::Uncaught(t) => NatErr::Throw(t),
        e => NatErr::Fatal(e),
    }
}

fn npe(vm: &mut Vm) -> NatErr {
    NatErr::Throw(vm.err_npe())
}
fn iae(vm: &mut Vm, m: impl Into<String>) -> NatErr {
    NatErr::Throw(vm.err_iae(m))
}
fn uoe(vm: &mut Vm, m: impl Into<String>) -> NatErr {
    NatErr::Throw(vm.err_uoe(m))
}
fn nfe(vm: &mut Vm, m: impl Into<String>) -> NatErr {
    NatErr::Throw(vm.err_nfe(m))
}
fn aioobe(vm: &mut Vm, idx: i32, len: i32) -> NatErr {
    NatErr::Throw(vm.err_aioobe(idx, len))
}
fn ioobe(vm: &mut Vm, idx: i32) -> NatErr {
    NatErr::Throw(vm.err_ioobe(idx))
}
fn sioobe(vm: &mut Vm, m: impl Into<String>) -> NatErr {
    NatErr::Throw(vm.err_sioobe(m))
}
fn cce(vm: &mut Vm, m: impl Into<String>) -> NatErr {
    NatErr::Throw(vm.err_cce(m))
}
fn no_such_elem(vm: &mut Vm) -> NatErr {
    NatErr::Throw(vm.throwable_of("Ljava/util/NoSuchElementException;", "No value present"))
}

/// Immutable access to an object's native payload.
fn payload<'a>(vm: &'a Vm, v: JValue) -> Option<&'a Native> {
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
fn payload_mut<'a>(vm: &'a mut Vm, v: JValue) -> Option<&'a mut Native> {
    let id = match v {
        JValue::Obj(id) => id,
        _ => return None,
    };
    let has_native = vm.arena.get(id).map_or(false, |o| o.native.is_some());
    if !has_native {
        if let Some(make) = default_native_for(vm, id) {
            vm.arena.get_mut(id)?.native = Some(make);
        }
    }
    vm.arena.get_mut(id)?.native.as_mut()
}

/// Default payload for a freshly allocated object of a shim class.
fn default_native_for(vm: &mut Vm, id: u32) -> Option<Native> {
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
                return Some(Native::Enum { name: String::new(), ordinal: 0 });
            }
            "Ljava/lang/Throwable;" => {
                return Some(Native::Throwable { message: None, cause: JValue::Null });
            }
            #[cfg(feature = "keiyoushi")]
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
        "Ljava/util/Locale;" => Some(Native::Opaque),
        "Ljava/io/PrintStream;" => Some(Native::PrintStream),
        "Ljava/lang/Thread;" => Some(Native::Opaque),
        "Ljava/util/Map$Entry;" => Some(Native::MapEntry { map: 0, idx: 0 }),
        #[cfg(feature = "keiyoushi")]
        "Leu/kanade/tachiyomi/source/model/SManga;" => Some(keiyoushi::empty_smanga()),
        #[cfg(feature = "keiyoushi")]
        "Leu/kanade/tachiyomi/source/model/SChapter;" => Some(keiyoushi::empty_schapter()),
        #[cfg(feature = "keiyoushi")]
        "Leu/kanade/tachiyomi/source/model/Page;" => Some(Native::SPPage {
            index: 0,
            name: String::new(),
            url: String::new(),
            image_url: String::new(),
        }),
        #[cfg(feature = "keiyoushi")]
        "Leu/kanade/tachiyomi/source/model/MangasPage;" => Some(Native::SMangasPage {
            mangas: Vec::new(),
            has_next: false,
        }),
        #[cfg(feature = "keiyoushi")]
        "Leu/kanade/tachiyomi/source/model/FilterList;" => Some(Native::SFilterList(Vec::new())),
        #[cfg(feature = "keiyoushi")]
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

fn obj_class(vm: &Vm, id: u32) -> u32 {
    vm.arena.objects[id as usize].class
}

/// Owned copy of a java.lang.String payload.
fn jstr(vm: &mut Vm, v: JValue) -> Result<String, NatErr> {
    let Some(n) = payload(vm, v) else {
        return Err(npe(vm));
    };
    match n {
        Native::Str(s) => Ok(s.clone()),
        _ => Err(npe(vm)),
    }
}

/// Borrowed java.lang.String payload (no error helpers or allocs afterwards).
fn peek_str<'a>(vm: &'a Vm, v: JValue) -> Option<&'a str> {
    let n = payload(vm, v)?;
    match n {
        Native::Str(s) => Some(s),
        _ => None,
    }
}

/// String or StringBuilder payload (CharSequence positions).
fn charseq_of(vm: &mut Vm, v: JValue) -> Result<String, NatErr> {
    let Some(n) = payload(vm, v) else {
        return Err(npe(vm));
    };
    match n {
        Native::Str(s) | Native::StringBuilder(s) => Ok(s.clone()),
        _ => Err(npe(vm)),
    }
}

fn new_str(vm: &mut Vm, s: &str) -> JValue {
    vm.alloc_string(s)
}

fn alloc(vm: &mut Vm, desc: &str, native: Native) -> Result<JValue, NatErr> {
    let class = vm.ensure_class_by_desc(desc).map_err(nat_fatal)?;
    Ok(JValue::Obj(vm.arena.alloc(class, Vec::new(), Some(native))))
}

// injekt DI (kohesive)
// ---------------------------------------------------------------------------

pub(crate) fn injekt_get_injekt(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Luy/kohesive/injekt/api/InjektScope;", Native::Opaque)
}

pub(crate) fn injekt_get_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/app/Application;", Native::Opaque)
}

pub(crate) fn injekt_full_type_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn injekt_full_type_get(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/lang/reflect/Type;", Native::Opaque)
}

fn boxed(vm: &mut Vm, desc: &str, native: Native) -> Result<JValue, NatErr> {
    alloc(vm, desc, native)
}

/// Allocate a Java array with the given element descriptor. The array class
/// is looked up through the dex type table (callers' signatures reference
/// these descriptors); if absent it falls back to Object[].
fn alloc_arr(
    vm: &mut Vm,
    elem_desc: &str,
    len: usize,
    fill: impl FnOnce() -> ArrayData,
) -> Result<JValue, NatErr> {
    let full = format!("[{elem_desc}");
    let (class, data) = match vm.ensure_class_by_desc(&full) {
        Ok(c) => (c, fill()),
        Err(_) => (
            vm.ensure_class_by_desc("[Ljava/lang/Object;").map_err(nat_fatal)?,
            fill_fallback(elem_desc, len),
        ),
    };
    Ok(JValue::Obj(vm.arena.alloc(class, Vec::new(), Some(Native::Array(data)))))
}

fn fill_fallback(elem_desc: &str, len: usize) -> ArrayData {
    if matches!(elem_desc, "B" | "C" | "S" | "I" | "J" | "F" | "D" | "Z") {
        ArrayData::new(elem_desc, len)
    } else {
        ArrayData::Obj(vec![JValue::Null; len])
    }
}

fn alloc_empty_arr(vm: &mut Vm, elem_desc: &str) -> Result<JValue, NatErr> {
    let e = elem_desc.to_string();
    alloc_arr(vm, elem_desc, 0, move || ArrayData::new(&e, 0))
}

// unboxing with tolerant conversions
fn int_of(vm: &Vm, v: JValue) -> i32 {
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

fn long_of(vm: &Vm, v: JValue) -> i64 {
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

fn float_of(vm: &Vm, v: JValue) -> f32 {
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

fn double_of(vm: &Vm, v: JValue) -> f64 {
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

fn bool_of(vm: &Vm, v: JValue) -> bool {
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
fn fmt_f32(v: f32) -> String {
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

fn fmt_f64(v: f64) -> String {
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
fn inv_virt(vm: &mut Vm, recv: JValue, name: &str, sig: &str, extra: &[JValue]) -> Result<JValue, NatErr> {
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
fn to_string_of(vm: &mut Vm, v: JValue) -> Result<String, NatErr> {
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
fn java_equals(vm: &mut Vm, a: JValue, b: JValue) -> Result<bool, NatErr> {
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
fn java_hash(vm: &mut Vm, v: JValue) -> i32 {
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

fn java_cmp(vm: &mut Vm, a: JValue, b: JValue) -> Result<Ordering, NatErr> {
    match inv_virt(vm, a, "compareTo", "(Ljava/lang/Object;)I", &[b]) {
        Ok(JValue::Int(i)) => Ok(i.cmp(&0)),
        _ => Err(npe(vm)),
    }
}

// utf16-based string helpers (Java indexes strings by code units)
fn u16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}
fn u16str(v: &[u16]) -> String {
    String::from_utf16_lossy(v)
}
fn u16len(s: &str) -> usize {
    s.encode_utf16().count()
}
fn utf16_cmp(a: &str, b: &str) -> Ordering {
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
fn u16_index_of(hay: &str, needle: &str, from: usize) -> Option<usize> {
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
fn u16_last_index_of(hay: &str, needle: &str, from: i64) -> Option<usize> {
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
fn char_at(s: &str, i: usize) -> Option<u16> {
    u16(s).get(i).copied()
}

/// java.util.regex.Pattern.split semantics for the given limit.
fn split_java(re: &Regex, text: &str, limit: i32) -> Vec<String> {
    if limit == 0 {
        let mut parts: Vec<String> = re.split(text).map(String::from).collect();
        while parts.last().is_some_and(|p| p.is_empty()) {
            parts.pop();
        }
        return parts;
    }
    if limit < 0 {
        return re.split(text).map(String::from).collect();
    }
    let max = limit as usize;
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut count = 0usize;
    while count + 1 < max {
        match re.find_at(text, pos) {
            Some(m) => {
                out.push(text[pos..m.start()].to_string());
                pos = m.end();
                count += 1;
            }
            None => break,
        }
    }
    out.push(text[pos..].to_string());
    out
}

const DIGITS: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

fn int_to_string(v: i32, radix: u32) -> String {
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

fn long_to_string_help(v: i64, radix: u32) -> String {
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

fn parse_int_radix(vm: &mut Vm, s: &str, radix: u32) -> Result<i32, NatErr> {
    if !(2..=36).contains(&radix) {
        return Err(nfe(vm, format!("radix {radix} out of range")));
    }
    i32::from_str_radix(s, radix).map_err(|_| nfe(vm, format!("For input string: \"{s}\"")))
}

fn parse_long_radix(vm: &mut Vm, s: &str, radix: u32) -> Result<i64, NatErr> {
    if !(2..=36).contains(&radix) {
        return Err(nfe(vm, format!("radix {radix} out of range")));
    }
    i64::from_str_radix(s, radix).map_err(|_| nfe(vm, format!("For input string: \"{s}\"")))
}

fn parse_float(vm: &mut Vm, raw: &str) -> Result<f32, NatErr> {
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

fn parse_double(vm: &mut Vm, raw: &str) -> Result<f64, NatErr> {
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

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

static RNG: AtomicU64 = AtomicU64::new(0x9E3779B97F4A7C15);
fn next_random_u64() -> u64 {
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

fn floor_div_i(a: i32, b: i32) -> i32 {
    let q = a / b;
    let r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}
fn floor_div_l(a: i64, b: i64) -> i64 {
    let q = a / b;
    let r = a % b;
    if r != 0 && ((r < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}
fn floor_mod_i(a: i32, b: i32) -> i32 {
    a - floor_div_i(a, b) * b
}
fn floor_mod_l(a: i64, b: i64) -> i64 {
    a - floor_div_l(a, b) * b
}

fn fmax32(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else {
        a.max(b)
    }
}
fn fmin32(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else {
        a.min(b)
    }
}
fn fmax64(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}
fn fmin64(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

fn comma_group(mut s: String) -> String {
    let neg = s.starts_with('-');
    if neg {
        s = s[1..].to_string();
    }
    let b = s.as_bytes();
    let rem = b.len() % 3;
    let mut out = String::with_capacity(b.len() + b.len() / 3);
    for (idx, &c) in b.iter().enumerate() {
        if idx > 0 && (idx - rem) % 3 == 0 {
            out.push(',');
        }
        out.push(c as char);
    }
    if neg {
        out.insert(0, '-');
    }
    out
}

// ---------------------------------------------------------------------------
// NATIVE_TABLE
// ---------------------------------------------------------------------------

pub const NATIVE_TABLE: &[NativeEntry] = &[
    // ---- java.lang.Object ----
    ne!("Ljava/lang/Object;", "<init>", "()V", true, object_init),
    ne!("Ljava/lang/Object;", "getClass", "()Ljava/lang/Class;", true, object_get_class),
    ne!("Ljava/lang/Object;", "hashCode", "()I", true, object_hash_code),
    ne!("Ljava/lang/Object;", "equals", "(Ljava/lang/Object;)Z", true, object_equals),
    ne!("Ljava/lang/Object;", "toString", "()Ljava/lang/String;", true, object_to_string),
    ne!("Ljava/lang/Object;", "clone", "()Ljava/lang/Object;", true, object_clone),
    // ---- java.lang.String ----
    ne!("Ljava/lang/String;", "<init>", "()V", true, string_init),
    ne!("Ljava/lang/String;", "<init>", "(Ljava/lang/String;)V", true, string_init_copy),
    ne!("Ljava/lang/String;", "<init>", "([C)V", true, string_init_chars),
    ne!("Ljava/lang/String;", "<init>", "([B)V", true, string_init_bytes),
    ne!("Ljava/lang/String;", "<init>", "([BLjava/lang/String;)V", true, string_init_bytes),
    ne!("Ljava/lang/String;", "length", "()I", true, string_length),
    ne!("Ljava/lang/String;", "isEmpty", "()Z", true, string_is_empty),
    ne!("Ljava/lang/String;", "charAt", "(I)C", true, string_char_at),
    ne!("Ljava/lang/String;", "equals", "(Ljava/lang/Object;)Z", true, string_equals),
    ne!("Ljava/lang/String;", "equalsIgnoreCase", "(Ljava/lang/String;)Z", true, string_equals_ignore_case),
    ne!("Ljava/lang/String;", "hashCode", "()I", true, string_hash_code),
    ne!("Ljava/lang/String;", "toString", "()Ljava/lang/String;", true, string_to_string),
    ne!("Ljava/lang/String;", "substring", "(I)Ljava/lang/String;", true, string_substring),
    ne!("Ljava/lang/String;", "substring", "(II)Ljava/lang/String;", true, string_substring),
    ne!("Ljava/lang/String;", "subSequence", "(II)Ljava/lang/CharSequence;", true, string_sub_sequence),
    ne!("Ljava/lang/String;", "concat", "(Ljava/lang/String;)Ljava/lang/String;", true, string_concat),
    ne!("Ljava/lang/String;", "contains", "(Ljava/lang/CharSequence;)Z", true, string_contains),
    ne!("Ljava/lang/String;", "startsWith", "(Ljava/lang/String;)Z", true, string_starts_with),
    ne!("Ljava/lang/String;", "startsWith", "(Ljava/lang/String;I)Z", true, string_starts_with),
    ne!("Ljava/lang/String;", "endsWith", "(Ljava/lang/String;)Z", true, string_ends_with),
    ne!("Ljava/lang/String;", "indexOf", "(I)I", true, string_index_of_char),
    ne!("Ljava/lang/String;", "indexOf", "(II)I", true, string_index_of_char),
    ne!("Ljava/lang/String;", "indexOf", "(Ljava/lang/String;)I", true, string_index_of_str),
    ne!("Ljava/lang/String;", "indexOf", "(Ljava/lang/String;I)I", true, string_index_of_str),
    ne!("Ljava/lang/String;", "lastIndexOf", "(I)I", true, string_last_index_of_char),
    ne!("Ljava/lang/String;", "lastIndexOf", "(II)I", true, string_last_index_of_char),
    ne!("Ljava/lang/String;", "lastIndexOf", "(Ljava/lang/String;)I", true, string_last_index_of_str),
    ne!("Ljava/lang/String;", "lastIndexOf", "(Ljava/lang/String;I)I", true, string_last_index_of_str),
    ne!("Ljava/lang/String;", "toLowerCase", "()Ljava/lang/String;", true, string_to_lower),
    ne!("Ljava/lang/String;", "toLowerCase", "(Ljava/util/Locale;)Ljava/lang/String;", true, string_to_lower),
    ne!("Ljava/lang/String;", "toUpperCase", "()Ljava/lang/String;", true, string_to_upper),
    ne!("Ljava/lang/String;", "toUpperCase", "(Ljava/util/Locale;)Ljava/lang/String;", true, string_to_upper),
    ne!("Ljava/lang/String;", "trim", "()Ljava/lang/String;", true, string_trim),
    ne!("Ljava/lang/String;", "getBytes", "()[B", true, string_get_bytes),
    ne!("Ljava/lang/String;", "getBytes", "(Ljava/lang/String;)[B", true, string_get_bytes),
    ne!("Ljava/lang/String;", "getBytes", "(Ljava/nio/charset/Charset;)[B", true, string_get_bytes),
    ne!("Ljava/lang/String;", "toCharArray", "()[C", true, string_to_char_array),
    ne!("Ljava/lang/String;", "getChars", "(II[CI)V", true, string_get_chars),
    ne!("Ljava/lang/String;", "split", "(Ljava/lang/String;)[Ljava/lang/String;", true, string_split),
    ne!("Ljava/lang/String;", "split", "(Ljava/lang/String;I)[Ljava/lang/String;", true, string_split),
    ne!("Ljava/lang/String;", "matches", "(Ljava/lang/String;)Z", true, string_matches),
    ne!("Ljava/lang/String;", "replace", "(CC)Ljava/lang/String;", true, string_replace_chars),
    ne!("Ljava/lang/String;", "replace", "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Ljava/lang/String;", true, string_replace_seq),
    ne!("Ljava/lang/String;", "compareTo", "(Ljava/lang/String;)I", true, string_compare_to),
    ne!("Ljava/lang/String;", "compareTo", "(Ljava/lang/Object;)I", true, string_compare_to),
    ne!("Ljava/lang/String;", "compareToIgnoreCase", "(Ljava/lang/String;)I", true, string_compare_to_ignore_case),
    ne!("Ljava/lang/String;", "intern", "()Ljava/lang/String;", true, string_intern),
    ne!("Ljava/lang/String;", "valueOf", "(I)Ljava/lang/String;", false, string_value_of_int),
    ne!("Ljava/lang/String;", "valueOf", "(J)Ljava/lang/String;", false, string_value_of_long),
    ne!("Ljava/lang/String;", "valueOf", "(Z)Ljava/lang/String;", false, string_value_of_bool),
    ne!("Ljava/lang/String;", "valueOf", "(C)Ljava/lang/String;", false, string_value_of_char),
    ne!("Ljava/lang/String;", "valueOf", "(F)Ljava/lang/String;", false, string_value_of_float),
    ne!("Ljava/lang/String;", "valueOf", "(D)Ljava/lang/String;", false, string_value_of_double),
    ne!("Ljava/lang/String;", "valueOf", "(Ljava/lang/Object;)Ljava/lang/String;", false, string_value_of_obj),
    ne!("Ljava/lang/String;", "valueOf", "([C)Ljava/lang/String;", false, string_value_of_chars),
    ne!("Ljava/lang/String;", "format", "(Ljava/lang/String;[Ljava/lang/Object;)Ljava/lang/String;", false, string_format),
    ne!("Ljava/lang/String;", "format", "(Ljava/util/Locale;Ljava/lang/String;[Ljava/lang/Object;)Ljava/lang/String;", false, string_format),
    // ---- java.lang.StringBuilder ----
    ne!("Ljava/lang/StringBuilder;", "<init>", "()V", true, sb_init),
    ne!("Ljava/lang/StringBuilder;", "<init>", "(I)V", true, sb_init),
    ne!("Ljava/lang/StringBuilder;", "<init>", "(Ljava/lang/String;)V", true, sb_init),
    ne!("Ljava/lang/StringBuilder;", "toString", "()Ljava/lang/String;", true, sb_to_string),
    ne!("Ljava/lang/StringBuilder;", "append", "(Ljava/lang/String;)Ljava/lang/StringBuilder;", true, sb_append_str),
    ne!("Ljava/lang/StringBuilder;", "append", "(Ljava/lang/CharSequence;)Ljava/lang/StringBuilder;", true, sb_append_charseq),
    ne!("Ljava/lang/StringBuilder;", "append", "(Ljava/lang/Object;)Ljava/lang/StringBuilder;", true, sb_append_obj),
    ne!("Ljava/lang/StringBuilder;", "append", "(I)Ljava/lang/StringBuilder;", true, sb_append_int),
    ne!("Ljava/lang/StringBuilder;", "append", "(J)Ljava/lang/StringBuilder;", true, sb_append_long),
    ne!("Ljava/lang/StringBuilder;", "append", "(Z)Ljava/lang/StringBuilder;", true, sb_append_bool),
    ne!("Ljava/lang/StringBuilder;", "append", "(C)Ljava/lang/StringBuilder;", true, sb_append_char),
    ne!("Ljava/lang/StringBuilder;", "append", "(F)Ljava/lang/StringBuilder;", true, sb_append_float),
    ne!("Ljava/lang/StringBuilder;", "append", "(D)Ljava/lang/StringBuilder;", true, sb_append_double),
    ne!("Ljava/lang/StringBuilder;", "append", "([C)Ljava/lang/StringBuilder;", true, sb_append_chars),
    ne!("Ljava/lang/StringBuilder;", "length", "()I", true, sb_length),
    ne!("Ljava/lang/StringBuilder;", "charAt", "(I)C", true, sb_char_at),
    ne!("Ljava/lang/StringBuilder;", "substring", "(II)Ljava/lang/String;", true, sb_substring),
    ne!("Ljava/lang/StringBuilder;", "delete", "(II)Ljava/lang/StringBuilder;", true, sb_delete),
    ne!("Ljava/lang/StringBuilder;", "setLength", "(I)V", true, sb_set_length),
    ne!("Ljava/lang/StringBuilder;", "capacity", "()I", true, sb_capacity),
    ne!("Ljava/lang/StringBuilder;", "indexOf", "(Ljava/lang/String;)I", true, sb_index_of),
    ne!("Ljava/lang/StringBuilder;", "indexOf", "(Ljava/lang/String;I)I", true, sb_index_of),
    // ---- java.lang.Class ----
    ne!("Ljava/lang/Class;", "getName", "()Ljava/lang/String;", true, class_get_name),
    ne!("Ljava/lang/Class;", "getSimpleName", "()Ljava/lang/String;", true, class_get_simple_name),
    ne!("Ljava/lang/Class;", "getCanonicalName", "()Ljava/lang/String;", true, class_get_canonical_name),
    ne!("Ljava/lang/Class;", "toString", "()Ljava/lang/String;", true, class_to_string),
    ne!("Ljava/lang/Class;", "isInstance", "(Ljava/lang/Object;)Z", true, class_is_instance),
    ne!("Ljava/lang/Class;", "isArray", "()Z", true, class_is_array),
    ne!("Ljava/lang/Class;", "isPrimitive", "()Z", true, class_is_primitive),
    ne!("Ljava/lang/Class;", "isInterface", "()Z", true, class_is_interface),
    ne!("Ljava/lang/Class;", "getComponentType", "()Ljava/lang/Class;", true, class_get_component_type),
    ne!("Ljava/lang/Class;", "getSuperclass", "()Ljava/lang/Class;", true, class_get_superclass),
    ne!("Ljava/lang/Class;", "cast", "(Ljava/lang/Object;)Ljava/lang/Object;", true, class_cast),
    ne!("Ljava/lang/Class;", "desiredAssertionStatus", "()Z", true, class_desired_assertion_status),
    ne!("Ljava/lang/Class;", "getClassLoader", "()Ljava/lang/ClassLoader;", true, class_get_class_loader),
    ne!("Ljava/lang/Class;", "getModifiers", "()I", true, class_get_modifiers),
    ne!("Ljava/lang/Class;", "isAssignableFrom", "(Ljava/lang/Class;)Z", true, class_is_assignable_from),
    ne!("Ljava/lang/Class;", "getInterfaces", "()[Ljava/lang/Class;", true, class_get_interfaces),
    ne!("Ljava/lang/Class;", "forName", "(Ljava/lang/String;)Ljava/lang/Class;", false, class_for_name),
    ne!("Ljava/lang/Class;", "forName", "(Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;", false, class_for_name),
    // ---- java.lang.Throwable and subclasses ----
    ne!("Ljava/lang/Throwable;", "getMessage", "()Ljava/lang/String;", true, throwable_get_message),
    ne!("Ljava/lang/Throwable;", "getLocalizedMessage", "()Ljava/lang/String;", true, throwable_get_localized_message),
    ne!("Ljava/lang/Throwable;", "getCause", "()Ljava/lang/Throwable;", true, throwable_get_cause),
    ne!("Ljava/lang/Throwable;", "initCause", "(Ljava/lang/Throwable;)Ljava/lang/Throwable;", true, throwable_init_cause),
    ne!("Ljava/lang/Throwable;", "toString", "()Ljava/lang/String;", true, throwable_to_string),
    ne!("Ljava/lang/Throwable;", "printStackTrace", "()V", true, throwable_print_stack_trace),
    ne!("Ljava/lang/Throwable;", "fillInStackTrace", "()Ljava/lang/Throwable;", true, throwable_fill_in_stack_trace),
    ne!("Ljava/lang/Throwable;", "addSuppressed", "(Ljava/lang/Throwable;)V", true, throwable_add_suppressed),
    ne!("Ljava/lang/Throwable;", "getSuppressed", "()[Ljava/lang/Throwable;", true, throwable_get_suppressed),
    ne!("Ljava/lang/Throwable;", "getStackTrace", "()[Ljava/lang/StackTraceElement;", true, throwable_get_stack_trace),
    // ---- java.lang.System ----
    ne!("Ljava/lang/System;", "currentTimeMillis", "()J", false, sys_current_time_millis),
    ne!("Ljava/lang/System;", "nanoTime", "()J", false, sys_nano_time),
    ne!("Ljava/lang/System;", "arraycopy", "(Ljava/lang/Object;ILjava/lang/Object;II)V", false, sys_arraycopy),
    ne!("Ljava/lang/System;", "exit", "(I)V", false, sys_exit),
    ne!("Ljava/lang/System;", "gc", "()V", false, sys_gc),
    ne!("Ljava/lang/System;", "identityHashCode", "(Ljava/lang/Object;)I", false, sys_identity_hash_code),
    ne!("Ljava/lang/System;", "getProperty", "(Ljava/lang/String;)Ljava/lang/String;", false, sys_get_property),
    ne!("Ljava/lang/System;", "lineSeparator", "()Ljava/lang/String;", false, sys_line_separator),
    // ---- java.io.PrintStream ----
    ne!("Ljava/io/PrintStream;", "<init>", "(Ljava/io/OutputStream;)V", true, ps_init),
    ne!("Ljava/io/PrintStream;", "println", "()V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(Ljava/lang/String;)V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(I)V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(J)V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(Z)V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(F)V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(D)V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(Ljava/lang/Object;)V", true, ps_println),
    ne!("Ljava/io/PrintStream;", "println", "(C)V", true, ps_println_char),
    ne!("Ljava/io/PrintStream;", "println", "([C)V", true, ps_println_chars),
    ne!("Ljava/io/PrintStream;", "print", "(Ljava/lang/String;)V", true, ps_print),
    ne!("Ljava/io/PrintStream;", "print", "(I)V", true, ps_print),
    ne!("Ljava/io/PrintStream;", "print", "(J)V", true, ps_print),
    ne!("Ljava/io/PrintStream;", "print", "(Z)V", true, ps_print),
    ne!("Ljava/io/PrintStream;", "print", "(F)V", true, ps_print),
    ne!("Ljava/io/PrintStream;", "print", "(D)V", true, ps_print),
    ne!("Ljava/io/PrintStream;", "print", "(Ljava/lang/Object;)V", true, ps_print),
    ne!("Ljava/io/PrintStream;", "print", "(C)V", true, ps_print_char),
    ne!("Ljava/io/PrintStream;", "flush", "()V", true, ps_flush),
    ne!("Ljava/io/PrintStream;", "close", "()V", true, ps_close),
    // ---- java.lang.Math ----
    ne!("Ljava/lang/Math;", "abs", "(I)I", false, math_abs_int),
    ne!("Ljava/lang/Math;", "abs", "(J)J", false, math_abs_long),
    ne!("Ljava/lang/Math;", "abs", "(F)F", false, math_abs_float),
    ne!("Ljava/lang/Math;", "abs", "(D)D", false, math_abs_double),
    ne!("Ljava/lang/Math;", "max", "(II)I", false, math_max_int),
    ne!("Ljava/lang/Math;", "max", "(JJ)J", false, math_max_long),
    ne!("Ljava/lang/Math;", "max", "(FF)F", false, math_max_float),
    ne!("Ljava/lang/Math;", "max", "(DD)D", false, math_max_double),
    ne!("Ljava/lang/Math;", "min", "(II)I", false, math_min_int),
    ne!("Ljava/lang/Math;", "min", "(JJ)J", false, math_min_long),
    ne!("Ljava/lang/Math;", "min", "(FF)F", false, math_min_float),
    ne!("Ljava/lang/Math;", "min", "(DD)D", false, math_min_double),
    ne!("Ljava/lang/Math;", "sqrt", "(D)D", false, math_sqrt),
    ne!("Ljava/lang/Math;", "cbrt", "(D)D", false, math_cbrt),
    ne!("Ljava/lang/Math;", "pow", "(DD)D", false, math_pow),
    ne!("Ljava/lang/Math;", "exp", "(D)D", false, math_exp),
    ne!("Ljava/lang/Math;", "log", "(D)D", false, math_log),
    ne!("Ljava/lang/Math;", "log10", "(D)D", false, math_log10),
    ne!("Ljava/lang/Math;", "log1p", "(D)D", false, math_log1p),
    ne!("Ljava/lang/Math;", "floor", "(D)D", false, math_floor),
    ne!("Ljava/lang/Math;", "ceil", "(D)D", false, math_ceil),
    ne!("Ljava/lang/Math;", "rint", "(D)D", false, math_rint),
    ne!("Ljava/lang/Math;", "floorDiv", "(II)I", false, math_floor_div_int),
    ne!("Ljava/lang/Math;", "floorDiv", "(JJ)J", false, math_floor_div_long),
    ne!("Ljava/lang/Math;", "floorMod", "(II)I", false, math_floor_mod_int),
    ne!("Ljava/lang/Math;", "floorMod", "(JJ)J", false, math_floor_mod_long),
    ne!("Ljava/lang/Math;", "round", "(F)I", false, math_round_float),
    ne!("Ljava/lang/Math;", "round", "(D)J", false, math_round_double),
    ne!("Ljava/lang/Math;", "signum", "(F)F", false, math_signum_float),
    ne!("Ljava/lang/Math;", "signum", "(D)D", false, math_signum_double),
    ne!("Ljava/lang/Math;", "random", "()D", false, math_random),
    ne!("Ljava/lang/Math;", "sin", "(D)D", false, math_sin),
    ne!("Ljava/lang/Math;", "cos", "(D)D", false, math_cos),
    ne!("Ljava/lang/Math;", "tan", "(D)D", false, math_tan),
    ne!("Ljava/lang/Math;", "asin", "(D)D", false, math_asin),
    ne!("Ljava/lang/Math;", "acos", "(D)D", false, math_acos),
    ne!("Ljava/lang/Math;", "atan", "(D)D", false, math_atan),
    ne!("Ljava/lang/Math;", "atan2", "(DD)D", false, math_atan2),
    ne!("Ljava/lang/Math;", "toRadians", "(D)D", false, math_to_radians),
    ne!("Ljava/lang/Math;", "toDegrees", "(D)D", false, math_to_degrees),
    ne!("Ljava/lang/Math;", "copySign", "(FF)F", false, math_copy_sign_float),
    ne!("Ljava/lang/Math;", "copySign", "(DD)D", false, math_copy_sign_double),
    // ---- boxed primitives ----
    ne!("Ljava/lang/Integer;", "valueOf", "(I)Ljava/lang/Integer;", false, integer_value_of),
    ne!("Ljava/lang/Integer;", "valueOf", "(Ljava/lang/String;)Ljava/lang/Integer;", false, integer_value_of_str),
    ne!("Ljava/lang/Integer;", "parseInt", "(Ljava/lang/String;)I", false, integer_parse_int),
    ne!("Ljava/lang/Integer;", "parseInt", "(Ljava/lang/String;I)I", false, integer_parse_int),
    ne!("Ljava/lang/Integer;", "toString", "(I)Ljava/lang/String;", false, integer_to_string_static),
    ne!("Ljava/lang/Integer;", "toString", "(II)Ljava/lang/String;", false, integer_to_string_radix),
    ne!("Ljava/lang/Integer;", "toHexString", "(I)Ljava/lang/String;", false, integer_to_hex),
    ne!("Ljava/lang/Integer;", "toBinaryString", "(I)Ljava/lang/String;", false, integer_to_binary),
    ne!("Ljava/lang/Integer;", "toOctalString", "(I)Ljava/lang/String;", false, integer_to_octal),
    ne!("Ljava/lang/Integer;", "compare", "(II)I", false, integer_compare),
    ne!("Ljava/lang/Integer;", "bitCount", "(I)I", false, integer_bit_count),
    ne!("Ljava/lang/Integer;", "highestOneBit", "(I)I", false, integer_highest_one_bit),
    ne!("Ljava/lang/Integer;", "signum", "(I)I", false, integer_signum),
    ne!("Ljava/lang/Integer;", "intValue", "()I", true, integer_int_value),
    ne!("Ljava/lang/Integer;", "longValue", "()J", true, integer_long_value),
    ne!("Ljava/lang/Integer;", "floatValue", "()F", true, integer_float_value),
    ne!("Ljava/lang/Integer;", "doubleValue", "()D", true, integer_double_value),
    ne!("Ljava/lang/Integer;", "byteValue", "()B", true, integer_byte_value),
    ne!("Ljava/lang/Integer;", "shortValue", "()S", true, integer_short_value),
    ne!("Ljava/lang/Integer;", "equals", "(Ljava/lang/Object;)Z", true, integer_equals),
    ne!("Ljava/lang/Integer;", "hashCode", "()I", true, integer_hash_code),
    ne!("Ljava/lang/Integer;", "toString", "()Ljava/lang/String;", true, integer_to_string),
    ne!("Ljava/lang/Integer;", "compareTo", "(Ljava/lang/Integer;)I", true, integer_compare_to),
    ne!("Ljava/lang/Integer;", "compareTo", "(Ljava/lang/Object;)I", true, integer_compare_to),
    ne!("Ljava/lang/Long;", "valueOf", "(J)Ljava/lang/Long;", false, long_value_of),
    ne!("Ljava/lang/Long;", "valueOf", "(Ljava/lang/String;)Ljava/lang/Long;", false, long_value_of_str),
    ne!("Ljava/lang/Long;", "parseLong", "(Ljava/lang/String;)J", false, long_parse_long),
    ne!("Ljava/lang/Long;", "parseLong", "(Ljava/lang/String;I)J", false, long_parse_long),
    ne!("Ljava/lang/Long;", "toString", "(J)Ljava/lang/String;", false, long_to_string_static),
    ne!("Ljava/lang/Long;", "toString", "(JI)Ljava/lang/String;", false, long_to_string_radix),
    ne!("Ljava/lang/Long;", "toHexString", "(J)Ljava/lang/String;", false, long_to_hex),
    ne!("Ljava/lang/Long;", "compare", "(JJ)I", false, long_compare),
    ne!("Ljava/lang/Long;", "bitCount", "(J)I", false, long_bit_count),
    ne!("Ljava/lang/Long;", "signum", "(J)I", false, long_signum),
    ne!("Ljava/lang/Long;", "intValue", "()I", true, long_int_value),
    ne!("Ljava/lang/Long;", "longValue", "()J", true, long_long_value),
    ne!("Ljava/lang/Long;", "floatValue", "()F", true, long_float_value),
    ne!("Ljava/lang/Long;", "doubleValue", "()D", true, long_double_value),
    ne!("Ljava/lang/Long;", "byteValue", "()B", true, long_byte_value),
    ne!("Ljava/lang/Long;", "shortValue", "()S", true, long_short_value),
    ne!("Ljava/lang/Long;", "equals", "(Ljava/lang/Object;)Z", true, long_equals),
    ne!("Ljava/lang/Long;", "hashCode", "()I", true, long_hash_code),
    ne!("Ljava/lang/Long;", "toString", "()Ljava/lang/String;", true, long_to_string),
    ne!("Ljava/lang/Long;", "compareTo", "(Ljava/lang/Long;)I", true, long_compare_to),
    ne!("Ljava/lang/Long;", "compareTo", "(Ljava/lang/Object;)I", true, long_compare_to),
    ne!("Ljava/lang/Short;", "valueOf", "(S)Ljava/lang/Short;", false, short_value_of),
    ne!("Ljava/lang/Short;", "parseShort", "(Ljava/lang/String;)S", false, short_parse_short),
    ne!("Ljava/lang/Short;", "parseShort", "(Ljava/lang/String;I)S", false, short_parse_short),
    ne!("Ljava/lang/Short;", "toString", "(S)Ljava/lang/String;", false, short_to_string),
    ne!("Ljava/lang/Short;", "intValue", "()I", true, integer_int_value),
    ne!("Ljava/lang/Short;", "shortValue", "()S", true, integer_short_value),
    ne!("Ljava/lang/Short;", "byteValue", "()B", true, integer_byte_value),
    ne!("Ljava/lang/Short;", "equals", "(Ljava/lang/Object;)Z", true, integer_equals),
    ne!("Ljava/lang/Short;", "hashCode", "()I", true, integer_hash_code),
    ne!("Ljava/lang/Short;", "toString", "()Ljava/lang/String;", true, short_to_string),
    ne!("Ljava/lang/Short;", "compareTo", "(Ljava/lang/Short;)I", true, short_compare_to),
    ne!("Ljava/lang/Short;", "compareTo", "(Ljava/lang/Object;)I", true, short_compare_to),
    ne!("Ljava/lang/Byte;", "valueOf", "(B)Ljava/lang/Byte;", false, byte_value_of),
    ne!("Ljava/lang/Byte;", "parseByte", "(Ljava/lang/String;)B", false, byte_parse_byte),
    ne!("Ljava/lang/Byte;", "parseByte", "(Ljava/lang/String;I)B", false, byte_parse_byte),
    ne!("Ljava/lang/Byte;", "toString", "(B)Ljava/lang/String;", false, byte_to_string),
    ne!("Ljava/lang/Byte;", "intValue", "()I", true, integer_int_value),
    ne!("Ljava/lang/Byte;", "shortValue", "()S", true, integer_short_value),
    ne!("Ljava/lang/Byte;", "byteValue", "()B", true, integer_byte_value),
    ne!("Ljava/lang/Byte;", "equals", "(Ljava/lang/Object;)Z", true, integer_equals),
    ne!("Ljava/lang/Byte;", "hashCode", "()I", true, integer_hash_code),
    ne!("Ljava/lang/Byte;", "toString", "()Ljava/lang/String;", true, byte_to_string),
    ne!("Ljava/lang/Byte;", "compareTo", "(Ljava/lang/Byte;)I", true, byte_compare_to),
    ne!("Ljava/lang/Byte;", "compareTo", "(Ljava/lang/Object;)I", true, byte_compare_to),
    ne!("Ljava/lang/Character;", "valueOf", "(C)Ljava/lang/Character;", false, char_value_of),
    ne!("Ljava/lang/Character;", "charValue", "()C", true, char_char_value),
    ne!("Ljava/lang/Character;", "equals", "(Ljava/lang/Object;)Z", true, char_equals),
    ne!("Ljava/lang/Character;", "hashCode", "()I", true, char_hash_code),
    ne!("Ljava/lang/Character;", "toString", "()Ljava/lang/String;", true, char_to_string),
    ne!("Ljava/lang/Character;", "toString", "(C)Ljava/lang/String;", false, char_to_string_static),
    ne!("Ljava/lang/Character;", "compareTo", "(Ljava/lang/Character;)I", true, char_compare_to),
    ne!("Ljava/lang/Character;", "compareTo", "(Ljava/lang/Object;)I", true, char_compare_to),
    ne!("Ljava/lang/Character;", "isDigit", "(C)Z", false, char_is_digit),
    ne!("Ljava/lang/Character;", "isLetter", "(C)Z", false, char_is_letter),
    ne!("Ljava/lang/Character;", "isLetterOrDigit", "(C)Z", false, char_is_letter_or_digit),
    ne!("Ljava/lang/Character;", "isWhitespace", "(C)Z", false, char_is_whitespace),
    ne!("Ljava/lang/Character;", "isUpperCase", "(C)Z", false, char_is_upper),
    ne!("Ljava/lang/Character;", "isLowerCase", "(C)Z", false, char_is_lower),
    ne!("Ljava/lang/Character;", "toUpperCase", "(C)C", false, char_to_upper),
    ne!("Ljava/lang/Character;", "toLowerCase", "(C)C", false, char_to_lower),
    ne!("Ljava/lang/Character;", "isHighSurrogate", "(C)Z", false, char_is_high_surrogate),
    ne!("Ljava/lang/Character;", "isLowSurrogate", "(C)Z", false, char_is_low_surrogate),
    ne!("Ljava/lang/Character;", "getNumericValue", "(C)I", false, char_get_numeric_value),
    ne!("Ljava/lang/Boolean;", "valueOf", "(Z)Ljava/lang/Boolean;", false, bool_value_of),
    ne!("Ljava/lang/Boolean;", "booleanValue", "()Z", true, bool_boolean_value),
    ne!("Ljava/lang/Boolean;", "equals", "(Ljava/lang/Object;)Z", true, bool_equals),
    ne!("Ljava/lang/Boolean;", "hashCode", "()I", true, bool_hash_code),
    ne!("Ljava/lang/Boolean;", "toString", "()Ljava/lang/String;", true, bool_to_string),
    ne!("Ljava/lang/Boolean;", "toString", "(Z)Ljava/lang/String;", false, bool_to_string_static),
    ne!("Ljava/lang/Boolean;", "parseBoolean", "(Ljava/lang/String;)Z", false, bool_parse_boolean),
    ne!("Ljava/lang/Boolean;", "compareTo", "(Ljava/lang/Boolean;)I", true, bool_compare_to),
    ne!("Ljava/lang/Boolean;", "compareTo", "(Ljava/lang/Object;)I", true, bool_compare_to),
    ne!("Ljava/lang/Float;", "valueOf", "(F)Ljava/lang/Float;", false, float_value_of),
    ne!("Ljava/lang/Float;", "valueOf", "(Ljava/lang/String;)Ljava/lang/Float;", false, float_value_of_str),
    ne!("Ljava/lang/Float;", "parseFloat", "(Ljava/lang/String;)F", false, float_parse_float),
    ne!("Ljava/lang/Float;", "intValue", "()I", true, float_int_value),
    ne!("Ljava/lang/Float;", "longValue", "()J", true, float_long_value),
    ne!("Ljava/lang/Float;", "floatValue", "()F", true, float_float_value),
    ne!("Ljava/lang/Float;", "doubleValue", "()D", true, float_double_value),
    ne!("Ljava/lang/Float;", "byteValue", "()B", true, float_byte_value),
    ne!("Ljava/lang/Float;", "shortValue", "()S", true, float_short_value),
    ne!("Ljava/lang/Float;", "equals", "(Ljava/lang/Object;)Z", true, float_equals),
    ne!("Ljava/lang/Float;", "hashCode", "()I", true, float_hash_code),
    ne!("Ljava/lang/Float;", "toString", "()Ljava/lang/String;", true, float_to_string),
    ne!("Ljava/lang/Float;", "toString", "(F)Ljava/lang/String;", false, float_to_string_static),
    ne!("Ljava/lang/Float;", "compareTo", "(Ljava/lang/Float;)I", true, float_compare_to),
    ne!("Ljava/lang/Float;", "compareTo", "(Ljava/lang/Object;)I", true, float_compare_to),
    ne!("Ljava/lang/Float;", "compare", "(FF)I", false, float_compare),
    ne!("Ljava/lang/Float;", "isNaN", "(F)Z", false, float_is_nan),
    ne!("Ljava/lang/Float;", "isInfinite", "(F)Z", false, float_is_infinite),
    ne!("Ljava/lang/Float;", "floatToIntBits", "(F)I", false, float_to_int_bits),
    ne!("Ljava/lang/Float;", "floatToRawIntBits", "(F)I", false, float_to_int_bits),
    ne!("Ljava/lang/Float;", "intBitsToFloat", "(I)F", false, float_int_bits_to_float),
    ne!("Ljava/lang/Double;", "valueOf", "(D)Ljava/lang/Double;", false, double_value_of),
    ne!("Ljava/lang/Double;", "valueOf", "(Ljava/lang/String;)Ljava/lang/Double;", false, double_value_of_str),
    ne!("Ljava/lang/Double;", "parseDouble", "(Ljava/lang/String;)D", false, double_parse_double),
    ne!("Ljava/lang/Double;", "intValue", "()I", true, double_int_value),
    ne!("Ljava/lang/Double;", "longValue", "()J", true, double_long_value),
    ne!("Ljava/lang/Double;", "floatValue", "()F", true, double_float_value),
    ne!("Ljava/lang/Double;", "doubleValue", "()D", true, double_double_value),
    ne!("Ljava/lang/Double;", "byteValue", "()B", true, double_byte_value),
    ne!("Ljava/lang/Double;", "shortValue", "()S", true, double_short_value),
    ne!("Ljava/lang/Double;", "equals", "(Ljava/lang/Object;)Z", true, double_equals),
    ne!("Ljava/lang/Double;", "hashCode", "()I", true, double_hash_code),
    ne!("Ljava/lang/Double;", "toString", "()Ljava/lang/String;", true, double_to_string),
    ne!("Ljava/lang/Double;", "toString", "(D)Ljava/lang/String;", false, double_to_string_static),
    ne!("Ljava/lang/Double;", "compareTo", "(Ljava/lang/Double;)I", true, double_compare_to),
    ne!("Ljava/lang/Double;", "compareTo", "(Ljava/lang/Object;)I", true, double_compare_to),
    ne!("Ljava/lang/Double;", "compare", "(DD)I", false, double_compare),
    ne!("Ljava/lang/Double;", "isNaN", "(D)Z", false, double_is_nan),
    ne!("Ljava/lang/Double;", "isInfinite", "(D)Z", false, double_is_infinite),
    ne!("Ljava/lang/Double;", "doubleToLongBits", "(D)J", false, double_to_long_bits),
    ne!("Ljava/lang/Double;", "doubleToRawLongBits", "(D)J", false, double_to_long_bits),
    ne!("Ljava/lang/Double;", "longBitsToDouble", "(J)D", false, double_long_bits_to_double),
    // ---- java.lang.Enum ----
    ne!("Ljava/lang/Enum;", "<init>", "(Ljava/lang/String;I)V", true, enum_init),
    ne!("Ljava/lang/Enum;", "name", "()Ljava/lang/String;", true, enum_name),
    ne!("Ljava/lang/Enum;", "ordinal", "()I", true, enum_ordinal),
    ne!("Ljava/lang/Enum;", "toString", "()Ljava/lang/String;", true, enum_to_string),
    ne!("Ljava/lang/Enum;", "compareTo", "(Ljava/lang/Enum;)I", true, enum_compare_to),
    ne!("Ljava/lang/Enum;", "compareTo", "(Ljava/lang/Object;)I", true, enum_compare_to),
    // ---- java.lang.Thread ----
    ne!("Ljava/lang/Thread;", "<init>", "()V", true, thread_init),
    ne!("Ljava/lang/Thread;", "<init>", "(Ljava/lang/Runnable;)V", true, thread_init),
    ne!("Ljava/lang/Thread;", "<init>", "(Ljava/lang/String;)V", true, thread_init),
    ne!("Ljava/lang/Thread;", "<init>", "(Ljava/lang/Runnable;Ljava/lang/String;)V", true, thread_init),
    ne!("Ljava/lang/Thread;", "currentThread", "()Ljava/lang/Thread;", false, thread_current),
    ne!("Ljava/lang/Thread;", "start", "()V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "run", "()V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "yield", "()V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "interrupt", "()V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "interrupted", "()Z", false, thread_is_interrupted),
    ne!("Ljava/lang/Thread;", "sleep", "(J)V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "sleep", "(JI)V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "setName", "(Ljava/lang/String;)V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "setDaemon", "(Z)V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "join", "()V", true, thread_noop),
    ne!("Ljava/lang/Thread;", "getName", "()Ljava/lang/String;", true, thread_get_name),
    ne!("Ljava/lang/Thread;", "getId", "()J", true, thread_get_id),
    ne!("Ljava/lang/Thread;", "isAlive", "()Z", true, thread_is_alive),
    ne!("Ljava/lang/Thread;", "isDaemon", "()Z", true, thread_is_daemon),
    ne!("Ljava/lang/Thread;", "isInterrupted", "()Z", true, thread_is_interrupted),
    // ---- java.util.ArrayList / LinkedHashMap / HashSet ----
    ne!("Ljava/util/ArrayList;", "<init>", "()V", true, list_init),
    ne!("Ljava/util/ArrayList;", "<init>", "(I)V", true, list_init),
    ne!("Ljava/util/ArrayList;", "<init>", "(Ljava/util/Collection;)V", true, list_init),
    ne!("Ljava/util/ArrayList;", "size", "()I", true, list_size),
    ne!("Ljava/util/ArrayList;", "isEmpty", "()Z", true, list_is_empty),
    ne!("Ljava/util/ArrayList;", "get", "(I)Ljava/lang/Object;", true, list_get),
    ne!("Ljava/util/ArrayList;", "set", "(ILjava/lang/Object;)Ljava/lang/Object;", true, list_set),
    ne!("Ljava/util/ArrayList;", "add", "(Ljava/lang/Object;)Z", true, list_add),
    ne!("Ljava/util/ArrayList;", "add", "(ILjava/lang/Object;)V", true, list_add_at),
    ne!("Ljava/util/ArrayList;", "remove", "(I)Ljava/lang/Object;", true, list_remove_at),
    ne!("Ljava/util/ArrayList;", "remove", "(Ljava/lang/Object;)Z", true, list_remove_obj),
    ne!("Ljava/util/ArrayList;", "clear", "()V", true, list_clear),
    ne!("Ljava/util/ArrayList;", "contains", "(Ljava/lang/Object;)Z", true, list_contains),
    ne!("Ljava/util/ArrayList;", "indexOf", "(Ljava/lang/Object;)I", true, list_index_of),
    ne!("Ljava/util/ArrayList;", "lastIndexOf", "(Ljava/lang/Object;)I", true, list_last_index_of),
    ne!("Ljava/util/ArrayList;", "iterator", "()Ljava/util/Iterator;", true, list_iterator),
    ne!("Ljava/util/ArrayList;", "listIterator", "()Ljava/util/ListIterator;", true, list_iterator),
    ne!("Ljava/util/ArrayList;", "toArray", "()[Ljava/lang/Object;", true, list_to_array),
    ne!("Ljava/util/ArrayList;", "toArray", "([Ljava/lang/Object;)[Ljava/lang/Object;", true, list_to_array_typed),
    ne!("Ljava/util/ArrayList;", "addAll", "(Ljava/util/Collection;)Z", true, list_add_all),
    ne!("Ljava/util/ArrayList;", "addAll", "(ILjava/util/Collection;)Z", true, list_add_all),
    ne!("Ljava/util/ArrayList;", "removeAll", "(Ljava/util/Collection;)Z", true, list_remove_all),
    ne!("Ljava/util/ArrayList;", "retainAll", "(Ljava/util/Collection;)Z", true, list_retain_all),
    ne!("Ljava/util/ArrayList;", "toString", "()Ljava/lang/String;", true, list_to_string),
    ne!("Ljava/util/ArrayList;", "sort", "(Ljava/util/Comparator;)V", true, list_sort_cmp),
    ne!("Ljava/util/HashMap;", "<init>", "()V", true, map_init),
    ne!("Ljava/util/HashMap;", "<init>", "(I)V", true, map_init),
    ne!("Ljava/util/HashMap;", "<init>", "(IF)V", true, map_init),
    ne!("Ljava/util/HashMap;", "<init>", "(Ljava/util/Map;)V", true, map_init),
    ne!("Ljava/util/HashMap;", "size", "()I", true, map_size),
    ne!("Ljava/util/HashMap;", "isEmpty", "()Z", true, map_is_empty),
    ne!("Ljava/util/HashMap;", "get", "(Ljava/lang/Object;)Ljava/lang/Object;", true, map_get),
    ne!("Ljava/util/HashMap;", "getOrDefault", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;", true, map_get_default),
    ne!("Ljava/util/HashMap;", "put", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;", true, map_put),
    ne!("Ljava/util/HashMap;", "putAll", "(Ljava/util/Map;)V", true, map_put_all),
    ne!("Ljava/util/HashMap;", "putIfAbsent", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;", true, map_put_if_absent),
    ne!("Ljava/util/HashMap;", "containsKey", "(Ljava/lang/Object;)Z", true, map_contains_key),
    ne!("Ljava/util/HashMap;", "containsValue", "(Ljava/lang/Object;)Z", true, map_contains_value),
    ne!("Ljava/util/HashMap;", "remove", "(Ljava/lang/Object;)Ljava/lang/Object;", true, map_remove),
    ne!("Ljava/util/HashMap;", "remove", "(Ljava/lang/Object;Ljava/lang/Object;)Z", true, map_remove),
    ne!("Ljava/util/HashMap;", "clear", "()V", true, map_clear),
    ne!("Ljava/util/HashMap;", "keySet", "()Ljava/util/Set;", true, map_keys),
    ne!("Ljava/util/HashMap;", "values", "()Ljava/util/Collection;", true, map_values),
    ne!("Ljava/util/HashMap;", "entrySet", "()Ljava/util/Set;", true, map_entries),
    ne!("Ljava/util/HashMap;", "toString", "()Ljava/lang/String;", true, map_to_string),
    ne!("Ljava/util/LinkedHashMap;", "<init>", "()V", true, map_init),
    ne!("Ljava/util/LinkedHashMap;", "<init>", "(I)V", true, map_init),
    ne!("Ljava/util/LinkedHashMap;", "<init>", "(IF)V", true, map_init),
    ne!("Ljava/util/LinkedHashMap;", "<init>", "(Ljava/util/Map;)V", true, map_init),
    ne!("Ljava/util/LinkedHashMap;", "size", "()I", true, map_size),
    ne!("Ljava/util/LinkedHashMap;", "isEmpty", "()Z", true, map_is_empty),
    ne!("Ljava/util/LinkedHashMap;", "get", "(Ljava/lang/Object;)Ljava/lang/Object;", true, map_get),
    ne!("Ljava/util/LinkedHashMap;", "getOrDefault", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;", true, map_get_default),
    ne!("Ljava/util/LinkedHashMap;", "put", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;", true, map_put),
    ne!("Ljava/util/LinkedHashMap;", "putAll", "(Ljava/util/Map;)V", true, map_put_all),
    ne!("Ljava/util/LinkedHashMap;", "putIfAbsent", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;", true, map_put_if_absent),
    ne!("Ljava/util/LinkedHashMap;", "containsKey", "(Ljava/lang/Object;)Z", true, map_contains_key),
    ne!("Ljava/util/LinkedHashMap;", "containsValue", "(Ljava/lang/Object;)Z", true, map_contains_value),
    ne!("Ljava/util/LinkedHashMap;", "remove", "(Ljava/lang/Object;)Ljava/lang/Object;", true, map_remove),
    ne!("Ljava/util/LinkedHashMap;", "remove", "(Ljava/lang/Object;Ljava/lang/Object;)Z", true, map_remove),
    ne!("Ljava/util/LinkedHashMap;", "clear", "()V", true, map_clear),
    ne!("Ljava/util/LinkedHashMap;", "keySet", "()Ljava/util/Set;", true, map_keys),
    ne!("Ljava/util/LinkedHashMap;", "values", "()Ljava/util/Collection;", true, map_values),
    ne!("Ljava/util/LinkedHashMap;", "entrySet", "()Ljava/util/Set;", true, map_entries),
    ne!("Ljava/util/LinkedHashMap;", "toString", "()Ljava/lang/String;", true, map_to_string),
    ne!("Ljava/util/HashSet;", "<init>", "()V", true, set_init),
    ne!("Ljava/util/HashSet;", "<init>", "(I)V", true, set_init),
    ne!("Ljava/util/HashSet;", "<init>", "(Ljava/util/Collection;)V", true, set_init),
    ne!("Ljava/util/HashSet;", "size", "()I", true, set_size),
    ne!("Ljava/util/HashSet;", "isEmpty", "()Z", true, set_is_empty),
    ne!("Ljava/util/HashSet;", "contains", "(Ljava/lang/Object;)Z", true, set_contains),
    ne!("Ljava/util/HashSet;", "add", "(Ljava/lang/Object;)Z", true, set_add),
    ne!("Ljava/util/HashSet;", "remove", "(Ljava/lang/Object;)Z", true, set_remove),
    ne!("Ljava/util/HashSet;", "clear", "()V", true, set_clear),
    ne!("Ljava/util/HashSet;", "iterator", "()Ljava/util/Iterator;", true, set_iterator),
    ne!("Ljava/util/HashSet;", "addAll", "(Ljava/util/Collection;)Z", true, set_add_all),
    ne!("Ljava/util/HashSet;", "toString", "()Ljava/lang/String;", true, set_to_string),
    ne!("Ljava/util/LinkedHashSet;", "<init>", "()V", true, set_init),
    ne!("Ljava/util/LinkedHashSet;", "<init>", "(I)V", true, set_init),
    ne!("Ljava/util/LinkedHashSet;", "<init>", "(Ljava/util/Collection;)V", true, set_init),
    ne!("Ljava/util/LinkedHashSet;", "size", "()I", true, set_size),
    ne!("Ljava/util/LinkedHashSet;", "isEmpty", "()Z", true, set_is_empty),
    ne!("Ljava/util/LinkedHashSet;", "contains", "(Ljava/lang/Object;)Z", true, set_contains),
    ne!("Ljava/util/LinkedHashSet;", "add", "(Ljava/lang/Object;)Z", true, set_add),
    ne!("Ljava/util/LinkedHashSet;", "remove", "(Ljava/lang/Object;)Z", true, set_remove),
    ne!("Ljava/util/LinkedHashSet;", "clear", "()V", true, set_clear),
    ne!("Ljava/util/LinkedHashSet;", "iterator", "()Ljava/util/Iterator;", true, set_iterator),
    ne!("Ljava/util/LinkedHashSet;", "addAll", "(Ljava/util/Collection;)Z", true, set_add_all),
    ne!("Ljava/util/LinkedHashSet;", "toString", "()Ljava/lang/String;", true, set_to_string),
    // ---- iterator & Map.Entry ----
    ne!("Ljava/util/Iterator;", "hasNext", "()Z", true, iter_has_next),
    ne!("Ljava/util/Iterator;", "next", "()Ljava/lang/Object;", true, iter_next),
    ne!("Ljava/util/Iterator;", "remove", "()V", true, iter_remove),
    ne!("Ljava/util/ListIterator;", "hasNext", "()Z", true, iter_has_next),
    ne!("Ljava/util/ListIterator;", "next", "()Ljava/lang/Object;", true, iter_next),
    ne!("Ljava/util/ListIterator;", "remove", "()V", true, iter_remove),
    ne!("Ljava/util/Map$Entry;", "getKey", "()Ljava/lang/Object;", true, entry_get_key),
    ne!("Ljava/util/Map$Entry;", "getValue", "()Ljava/lang/Object;", true, entry_get_value),
    ne!("Ljava/util/Map$Entry;", "setValue", "(Ljava/lang/Object;)Ljava/lang/Object;", true, entry_set_value),
    // ---- java.util.Collections (statics) ----
    ne!("Ljava/util/Collections;", "emptyList", "()Ljava/util/List;", false, collections_empty_list),
    ne!("Ljava/util/Collections;", "emptySet", "()Ljava/util/Set;", false, collections_empty_set),
    ne!("Ljava/util/Collections;", "emptyMap", "()Ljava/util/Map;", false, collections_empty_map),
    ne!("Ljava/util/Collections;", "singleton", "(Ljava/lang/Object;)Ljava/util/Set;", false, collections_singleton),
    ne!("Ljava/util/Collections;", "singletonList", "(Ljava/lang/Object;)Ljava/util/List;", false, collections_singleton_list),
    ne!("Ljava/util/Collections;", "singletonMap", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/util/Map;", false, collections_singleton_map),
    ne!("Ljava/util/Collections;", "unmodifiableCollection", "(Ljava/util/Collection;)Ljava/util/Collection;", false, collections_identity),
    ne!("Ljava/util/Collections;", "unmodifiableList", "(Ljava/util/List;)Ljava/util/List;", false, collections_identity),
    ne!("Ljava/util/Collections;", "unmodifiableSet", "(Ljava/util/Set;)Ljava/util/Set;", false, collections_identity),
    ne!("Ljava/util/Collections;", "unmodifiableMap", "(Ljava/util/Map;)Ljava/util/Map;", false, collections_identity),
    ne!("Ljava/util/Collections;", "synchronizedList", "(Ljava/util/List;)Ljava/util/List;", false, collections_identity),
    ne!("Ljava/util/Collections;", "synchronizedSet", "(Ljava/util/Set;)Ljava/util/Set;", false, collections_identity),
    ne!("Ljava/util/Collections;", "synchronizedMap", "(Ljava/util/Map;)Ljava/util/Map;", false, collections_identity),
    ne!("Ljava/util/Collections;", "sort", "(Ljava/util/List;)V", false, collections_sort),
    ne!("Ljava/util/Collections;", "sort", "(Ljava/util/List;Ljava/util/Comparator;)V", false, list_sort_cmp),
    ne!("Ljava/util/Collections;", "reverse", "(Ljava/util/List;)V", false, collections_reverse),
    ne!("Ljava/util/Collections;", "addAll", "(Ljava/util/Collection;[Ljava/lang/Object;)Z", false, collections_add_all),
    // ---- java.util.Arrays (statics) ----
    ne!("Ljava/util/Arrays;", "asList", "([Ljava/lang/Object;)Ljava/util/List;", false, arrays_as_list),
    ne!("Ljava/util/Arrays;", "copyOf", "([BI)[B", false, arrays_copy_of),
    ne!("Ljava/util/Arrays;", "copyOf", "([CI)[C", false, arrays_copy_of),
    ne!("Ljava/util/Arrays;", "copyOf", "([SI)[S", false, arrays_copy_of),
    ne!("Ljava/util/Arrays;", "copyOf", "([II)[I", false, arrays_copy_of),
    ne!("Ljava/util/Arrays;", "copyOf", "([JI)[J", false, arrays_copy_of),
    ne!("Ljava/util/Arrays;", "copyOf", "([FI)[F", false, arrays_copy_of),
    ne!("Ljava/util/Arrays;", "copyOf", "([DI)[D", false, arrays_copy_of),
    ne!("Ljava/util/Arrays;", "copyOf", "([ZI)[Z", false, arrays_copy_of),
    ne!("Ljava/util/Arrays;", "copyOf", "([Ljava/lang/Object;I)[Ljava/lang/Object;", false, arrays_copy_of),
    ne!("Ljava/util/Arrays;", "copyOfRange", "([BII)[B", false, arrays_copy_of_range),
    ne!("Ljava/util/Arrays;", "copyOfRange", "([CII)[C", false, arrays_copy_of_range),
    ne!("Ljava/util/Arrays;", "copyOfRange", "([SII)[S", false, arrays_copy_of_range),
    ne!("Ljava/util/Arrays;", "copyOfRange", "([III)[I", false, arrays_copy_of_range),
    ne!("Ljava/util/Arrays;", "copyOfRange", "([JII)[J", false, arrays_copy_of_range),
    ne!("Ljava/util/Arrays;", "copyOfRange", "([FII)[F", false, arrays_copy_of_range),
    ne!("Ljava/util/Arrays;", "copyOfRange", "([DII)[D", false, arrays_copy_of_range),
    ne!("Ljava/util/Arrays;", "copyOfRange", "([ZII)[Z", false, arrays_copy_of_range),
    ne!("Ljava/util/Arrays;", "copyOfRange", "([Ljava/lang/Object;II)[Ljava/lang/Object;", false, arrays_copy_of_range),
    ne!("Ljava/util/Arrays;", "sort", "([I)V", false, arrays_sort_prim),
    ne!("Ljava/util/Arrays;", "sort", "([J)V", false, arrays_sort_prim),
    ne!("Ljava/util/Arrays;", "sort", "([B)V", false, arrays_sort_prim),
    ne!("Ljava/util/Arrays;", "sort", "([C)V", false, arrays_sort_prim),
    ne!("Ljava/util/Arrays;", "sort", "([S)V", false, arrays_sort_prim),
    ne!("Ljava/util/Arrays;", "sort", "([F)V", false, arrays_sort_prim),
    ne!("Ljava/util/Arrays;", "sort", "([D)V", false, arrays_sort_prim),
    ne!("Ljava/util/Arrays;", "sort", "([Z)V", false, arrays_sort_prim),
    ne!("Ljava/util/Arrays;", "sort", "([Ljava/lang/Object;)V", false, arrays_sort_obj),
    ne!("Ljava/util/Arrays;", "sort", "([Ljava/lang/Object;Ljava/util/Comparator;)V", false, arrays_sort_obj_cmp),
    ne!("Ljava/util/Arrays;", "toString", "([B)Ljava/lang/String;", false, arrays_to_string),
    ne!("Ljava/util/Arrays;", "toString", "([C)Ljava/lang/String;", false, arrays_to_string),
    ne!("Ljava/util/Arrays;", "toString", "([S)Ljava/lang/String;", false, arrays_to_string),
    ne!("Ljava/util/Arrays;", "toString", "([I)Ljava/lang/String;", false, arrays_to_string),
    ne!("Ljava/util/Arrays;", "toString", "([J)Ljava/lang/String;", false, arrays_to_string),
    ne!("Ljava/util/Arrays;", "toString", "([F)Ljava/lang/String;", false, arrays_to_string),
    ne!("Ljava/util/Arrays;", "toString", "([D)Ljava/lang/String;", false, arrays_to_string),
    ne!("Ljava/util/Arrays;", "toString", "([Z)Ljava/lang/String;", false, arrays_to_string),
    ne!("Ljava/util/Arrays;", "toString", "([Ljava/lang/Object;)Ljava/lang/String;", false, arrays_to_string),
    ne!("Ljava/util/Arrays;", "fill", "([II)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([JI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([BI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([CI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([SI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([FI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([DI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([ZI)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "fill", "([Ljava/lang/Object;Ljava/lang/Object;)V", false, arrays_fill),
    ne!("Ljava/util/Arrays;", "equals", "([B[B)Z", false, arrays_equals),
    ne!("Ljava/util/Arrays;", "equals", "([C[C)Z", false, arrays_equals),
    ne!("Ljava/util/Arrays;", "equals", "([S[S)Z", false, arrays_equals),
    ne!("Ljava/util/Arrays;", "equals", "([I[I)Z", false, arrays_equals),
    ne!("Ljava/util/Arrays;", "equals", "([J[J)Z", false, arrays_equals),
    ne!("Ljava/util/Arrays;", "equals", "([F[F)Z", false, arrays_equals),
    ne!("Ljava/util/Arrays;", "equals", "([D[D)Z", false, arrays_equals),
    ne!("Ljava/util/Arrays;", "equals", "([Z[Z)Z", false, arrays_equals),
    ne!("Ljava/util/Arrays;", "equals", "([Ljava/lang/Object;[Ljava/lang/Object;)Z", false, arrays_equals),
    // ---- java.util.Objects (statics) ----
    ne!("Ljava/util/Objects;", "equals", "(Ljava/lang/Object;Ljava/lang/Object;)Z", false, objects_equals),
    ne!("Ljava/util/Objects;", "hashCode", "(Ljava/lang/Object;)I", false, objects_hash_code),
    ne!("Ljava/util/Objects;", "hash", "([Ljava/lang/Object;)I", false, objects_hash),
    ne!("Ljava/util/Objects;", "requireNonNull", "(Ljava/lang/Object;)Ljava/lang/Object;", false, objects_require_non_null),
    ne!("Ljava/util/Objects;", "requireNonNull", "(Ljava/lang/Object;Ljava/lang/String;)Ljava/lang/Object;", false, objects_require_non_null),
    ne!("Ljava/util/Objects;", "requireNonNullElse", "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;", false, objects_require_non_null_else),
    ne!("Ljava/util/Objects;", "toString", "(Ljava/lang/Object;)Ljava/lang/String;", false, objects_to_string),
    ne!("Ljava/util/Objects;", "toString", "(Ljava/lang/Object;Ljava/lang/String;)Ljava/lang/String;", false, objects_to_string_def),
    ne!("Ljava/util/Objects;", "isNull", "(Ljava/lang/Object;)Z", false, objects_is_null),
    ne!("Ljava/util/Objects;", "nonNull", "(Ljava/lang/Object;)Z", false, objects_non_null),
    // ---- java.util.Locale ----
    ne!("Ljava/util/Locale;", "<init>", "(Ljava/lang/String;)V", true, locale_init),
    ne!("Ljava/util/Locale;", "<init>", "(Ljava/lang/String;Ljava/lang/String;)V", true, locale_init),
    ne!("Ljava/util/Locale;", "<init>", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V", true, locale_init),
    ne!("Ljava/util/Locale;", "getDefault", "()Ljava/util/Locale;", false, locale_get_default),
    ne!("Ljava/util/Locale;", "toString", "()Ljava/lang/String;", true, locale_to_string),
    ne!("Ljava/util/Locale;", "getLanguage", "()Ljava/lang/String;", true, locale_get_language),
    ne!("Ljava/util/Locale;", "getCountry", "()Ljava/lang/String;", true, locale_get_country),
    ne!("Ljava/util/Locale;", "forLanguageTag", "(Ljava/lang/String;)Ljava/util/Locale;", false, locale_for_language_tag),
    // ---- java.util.regex.Pattern / Matcher ----
    ne!("Ljava/util/regex/Pattern;", "<init>", "(Ljava/lang/String;)V", true, pattern_init),
    ne!("Ljava/util/regex/Pattern;", "<init>", "(Ljava/lang/String;I)V", true, pattern_init_flags),
    ne!("Ljava/util/regex/Pattern;", "matcher", "(Ljava/lang/CharSequence;)Ljava/util/regex/Matcher;", true, pattern_matcher),
    ne!("Ljava/util/regex/Pattern;", "matches", "(Ljava/lang/String;Ljava/lang/CharSequence;)Z", false, pattern_matches_static),
    ne!("Ljava/util/regex/Pattern;", "compile", "(Ljava/lang/String;)Ljava/util/regex/Pattern;", false, pattern_compile),
    ne!("Ljava/util/regex/Pattern;", "compile", "(Ljava/lang/String;I)Ljava/util/regex/Pattern;", false, pattern_compile_flags),
    ne!("Ljava/util/regex/Pattern;", "pattern", "()Ljava/lang/String;", true, pattern_source),
    ne!("Ljava/util/regex/Pattern;", "toString", "()Ljava/lang/String;", true, pattern_source),
    ne!("Ljava/util/regex/Pattern;", "flags", "()I", true, pattern_flags),
    ne!("Ljava/util/regex/Pattern;", "split", "(Ljava/lang/CharSequence;)[Ljava/lang/String;", true, pattern_split_seq),
    ne!("Ljava/util/regex/Pattern;", "split", "(Ljava/lang/CharSequence;I)[Ljava/lang/String;", true, pattern_split_seq_limit),
    ne!("Ljava/util/regex/Pattern;", "quote", "(Ljava/lang/String;)Ljava/lang/String;", false, pattern_quote),
    ne!("Ljava/util/regex/Matcher;", "matches", "()Z", true, matcher_matches),
    ne!("Ljava/util/regex/Matcher;", "find", "()Z", true, matcher_find),
    ne!("Ljava/util/regex/Matcher;", "find", "(I)Z", true, matcher_find_from),
    ne!("Ljava/util/regex/Matcher;", "lookingAt", "()Z", true, matcher_looking_at),
    ne!("Ljava/util/regex/Matcher;", "group", "()Ljava/lang/String;", true, matcher_group),
    ne!("Ljava/util/regex/Matcher;", "group", "(I)Ljava/lang/String;", true, matcher_group_n),
    ne!("Ljava/util/regex/Matcher;", "groupCount", "()I", true, matcher_group_count),
    ne!("Ljava/util/regex/Matcher;", "start", "()I", true, matcher_start),
    ne!("Ljava/util/regex/Matcher;", "start", "(I)I", true, matcher_start_n),
    ne!("Ljava/util/regex/Matcher;", "end", "()I", true, matcher_end),
    ne!("Ljava/util/regex/Matcher;", "end", "(I)I", true, matcher_end_n),
    ne!("Ljava/util/regex/Matcher;", "replaceAll", "(Ljava/lang/String;)Ljava/lang/String;", true, matcher_replace_all),
    ne!("Ljava/util/regex/Matcher;", "replaceFirst", "(Ljava/lang/String;)Ljava/lang/String;", true, matcher_replace_first),
    ne!("Ljava/util/regex/Matcher;", "reset", "()Ljava/util/regex/Matcher;", true, matcher_reset),
    ne!("Ljava/util/regex/Matcher;", "reset", "(Ljava/lang/CharSequence;)Ljava/util/regex/Matcher;", true, matcher_reset_seq),
    ne!("Ljava/util/regex/Matcher;", "region", "(II)Ljava/util/regex/Matcher;", true, matcher_region),
    ne!("Ljava/util/regex/Matcher;", "pattern", "()Ljava/util/regex/Pattern;", true, matcher_pattern),
    ne!("Ljava/util/regex/Matcher;", "toString", "()Ljava/lang/String;", true, matcher_to_string),
    // ---- java.util.Random ----
    ne!("Ljava/util/Random;", "<init>", "()V", true, random_init),
    ne!("Ljava/util/Random;", "<init>", "(J)V", true, random_init_seed),
    ne!("Ljava/util/Random;", "nextInt", "()I", true, random_next_int),
    ne!("Ljava/util/Random;", "nextInt", "(I)I", true, random_next_int_bound),
    ne!("Ljava/util/Random;", "nextLong", "()J", true, random_next_long),
    ne!("Ljava/util/Random;", "nextDouble", "()D", true, random_next_double),
    ne!("Ljava/util/Random;", "nextFloat", "()F", true, random_next_float),
    ne!("Ljava/util/Random;", "nextBoolean", "()Z", true, random_next_boolean),
    ne!("Ljava/util/Random;", "nextBytes", "([B)V", true, random_next_bytes),
    ne!("Ljava/util/Random;", "setSeed", "(J)V", true, random_set_seed),
    // ---- java.util.Date ----
    ne!("Ljava/util/Date;", "<init>", "()V", true, date_init),
    ne!("Ljava/util/Date;", "<init>", "(J)V", true, date_init_ms),
    ne!("Ljava/util/Date;", "getTime", "()J", true, date_get_time),
    ne!("Ljava/util/Date;", "setTime", "(J)V", true, date_set_time),
    ne!("Ljava/util/Date;", "toString", "()Ljava/lang/String;", true, date_to_string),
    ne!("Ljava/util/Date;", "after", "(Ljava/util/Date;)Z", true, date_after),
    ne!("Ljava/util/Date;", "before", "(Ljava/util/Date;)Z", true, date_before),
    ne!("Ljava/util/Date;", "equals", "(Ljava/lang/Object;)Z", true, date_equals),
    ne!("Ljava/util/Date;", "compareTo", "(Ljava/util/Date;)I", true, date_compare_to),
    ne!("Ljava/util/Date;", "compareTo", "(Ljava/lang/Object;)I", true, date_compare_to),
    // ---- java.text.SimpleDateFormat / DateFormat / ParsePosition / TimeZone ----
    ne!("Ljava/text/DateFormat;", "setTimeZone", "(Ljava/util/TimeZone;)V", true, date_format_set_time_zone),
    ne!("Ljava/text/SimpleDateFormat;", "<init>", "(Ljava/lang/String;Ljava/util/Locale;)V", true, simple_date_format_init),
    ne!("Ljava/text/SimpleDateFormat;", "<init>", "(Ljava/lang/String;)V", true, simple_date_format_init),
    ne!("Ljava/text/SimpleDateFormat;", "parse", "(Ljava/lang/String;Ljava/text/ParsePosition;)Ljava/util/Date;", true, simple_date_format_parse),
    ne!("Ljava/text/SimpleDateFormat;", "toString", "()Ljava/lang/String;", true, simple_date_format_to_string),
    ne!("Ljava/text/ParsePosition;", "<init>", "(I)V", true, parse_position_init),
    ne!("Ljava/text/ParsePosition;", "getIndex", "()I", true, parse_position_get_index),
    ne!("Ljava/text/ParsePosition;", "setIndex", "(I)V", true, parse_position_set_index),
    ne!("Ljava/util/TimeZone;", "getTimeZone", "(Ljava/lang/String;)Ljava/util/TimeZone;", false, time_zone_get_time_zone),
    // ---- java.util.ArrayDeque ----
    ne!("Ljava/util/ArrayDeque;", "<init>", "()V", true, deque_init),
    ne!("Ljava/util/ArrayDeque;", "<init>", "(I)V", true, deque_init),
    ne!("Ljava/util/ArrayDeque;", "addLast", "(Ljava/lang/Object;)V", true, deque_add_last),
    ne!("Ljava/util/ArrayDeque;", "addFirst", "(Ljava/lang/Object;)V", true, deque_add_first),
    ne!("Ljava/util/ArrayDeque;", "removeFirst", "()Ljava/lang/Object;", true, deque_remove_first),
    ne!("Ljava/util/ArrayDeque;", "removeLast", "()Ljava/lang/Object;", true, deque_remove_last),
    ne!("Ljava/util/ArrayDeque;", "size", "()I", true, deque_size),
    ne!("Ljava/util/ArrayDeque;", "isEmpty", "()Z", true, deque_is_empty),
    ne!("Ljava/util/ArrayDeque;", "peekFirst", "()Ljava/lang/Object;", true, deque_peek_first),
    // ---- java.util.concurrent.locks ----
    ne!("Ljava/util/concurrent/locks/ReentrantLock;", "<init>", "()V", true, reentrant_lock_init),
    ne!("Ljava/util/concurrent/locks/ReentrantLock;", "<init>", "(Z)V", true, reentrant_lock_init),
    ne!("Ljava/util/concurrent/locks/ReentrantLock;", "lock", "()V", true, reentrant_lock_lock),
    ne!("Ljava/util/concurrent/locks/ReentrantLock;", "unlock", "()V", true, reentrant_lock_unlock),
    ne!("Ljava/util/concurrent/locks/ReentrantLock;", "newCondition", "()Ljava/util/concurrent/locks/Condition;", true, reentrant_lock_new_condition),
    ne!("Ljava/util/concurrent/locks/Condition;", "awaitNanos", "(J)J", true, condition_await_nanos),
    ne!("Ljava/util/concurrent/locks/Condition;", "await", "()V", true, condition_await),
    ne!("Ljava/util/concurrent/locks/Condition;", "signal", "()V", true, condition_signal),
    ne!("Ljava/util/concurrent/locks/Condition;", "signalAll", "()V", true, condition_signal_all),
    // ---- kotlin.stdlib ----
    ne!("Lkotlin/Lazy;", "getValue", "()Ljava/lang/Object;", true, lazy_get_value),
    ne!("Lkotlin/LazyKt;", "lazy", "(Lkotlin/jvm/functions/Function0;)Lkotlin/Lazy;", false, lazy_kt_lazy),
    ne!("Luy/kohesive/injekt/InjektKt;", "getInjekt", "()Luy/kohesive/injekt/api/InjektScope;", false, injekt_get_injekt),
    ne!("Luy/kohesive/injekt/api/InjektFactory;", "getInstance", "(Ljava/lang/reflect/Type;)Ljava/lang/Object;", true, injekt_get_instance),
    ne!("Luy/kohesive/injekt/api/FullTypeReference;", "<init>", "()V", true, injekt_full_type_init),
    ne!("Luy/kohesive/injekt/api/FullTypeReference;", "getType", "()Ljava/lang/reflect/Type;", true, injekt_full_type_get),
    ne!("Lkotlin/time/Duration$Companion;", "getZERO-UwyO8pc", "()J", true, duration_get_zero),
    ne!("Lkotlin/time/DurationKt;", "toDuration", "(ILkotlin/time/DurationUnit;)J", false, duration_to_duration_int),
    ne!("Lkotlin/time/DurationKt;", "toDuration", "(JLkotlin/time/DurationUnit;)J", false, duration_to_duration_long),
    ne!("Lkotlin/text/Regex;", "<init>", "(Ljava/lang/String;)V", true, regex_init),
    ne!("Lkotlin/text/Regex;", "replace", "(Ljava/lang/CharSequence;Ljava/lang/String;)Ljava/lang/String;", true, regex_replace),
    ne!("Lkotlin/text/Regex;", "matches", "(Ljava/lang/CharSequence;)Z", true, regex_matches),
    ne!("Lkotlin/text/Regex;", "toString", "()Ljava/lang/String;", true, regex_to_string),
    ne!("Lkotlin/collections/CollectionsKt;", "listOf", "([Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_array),
    ne!("Lkotlin/collections/CollectionsKt;", "listOf", "(Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_single),
    ne!("Lkotlin/collections/CollectionsKt;", "mutableListOf", "([Ljava/lang/Object;)Ljava/util/List;", false, collections_list_of_array),
    ne!("Lkotlin/collections/CollectionsKt;", "emptyList", "()Ljava/util/List;", false, kotlin_empty_list),
    ne!("Lkotlin/collections/CollectionsKt;", "plus", "(Ljava/util/Collection;Ljava/lang/Iterable;)Ljava/util/List;", false, collections_plus_iterable),
    ne!("Lkotlin/collections/CollectionsKt;", "plus", "(Ljava/util/Collection;Ljava/lang/Object;)Ljava/util/List;", false, collections_plus_obj),
    ne!("Lkotlin/collections/CollectionsKt;", "contains", "(Ljava/lang/Iterable;Ljava/lang/Object;)Z", false, collections_contains),
    ne!("Lkotlin/collections/CollectionsKt;", "first", "(Ljava/lang/Iterable;)Ljava/lang/Object;", false, collections_first),
    ne!("Lkotlin/collections/CollectionsKt;", "collectionSizeOrDefault", "(Ljava/lang/Iterable;I)I", false, collections_size_or_default),
    ne!("Lkotlin/jvm/internal/Intrinsics;", "areEqual", "(Ljava/lang/Object;Ljava/lang/Object;)Z", false, intrinsics_are_equal),
    ne!("Lkotlin/Pair;", "getFirst", "()Ljava/lang/Object;", true, pair_get_first),
    ne!("Lkotlin/Pair;", "getSecond", "()Ljava/lang/Object;", true, pair_get_second),
    ne!("Lkotlin/TuplesKt;", "to", "(Ljava/lang/Object;Ljava/lang/Object;)Lkotlin/Pair;", false, tupled_to),
    ne!("Lkotlin/ranges/IntRange;", "<init>", "(II)V", true, int_range_init),
    ne!("Lkotlin/ranges/IntRange;", "getFirst", "()I", true, int_range_get_first),
    ne!("Lkotlin/ranges/IntRange;", "getLast", "()I", true, int_range_get_last),
    ne!("Lkotlin/collections/IntIterator;", "<init>", "()V", true, int_iterator_init),
    ne!("Lkotlin/collections/IntIterator;", "nextInt", "()I", true, int_iterator_next_int),
    ne!("Lkotlin/collections/IntIterator;", "hasNext", "()Z", true, int_iterator_has_next),
    ne!("Lkotlin/comparisons/ComparisonsKt;", "maxOf", "(Ljava/lang/Comparable;Ljava/lang/Comparable;)Ljava/lang/Comparable;", false, comparisons_max_of),
    // ---- java.nio.charset.Charset / StandardCharsets ----
    ne!("Ljava/nio/charset/Charset;", "forName", "(Ljava/lang/String;)Ljava/nio/charset/Charset;", false, charset_for_name),
    ne!("Ljava/nio/charset/Charset;", "name", "()Ljava/lang/String;", true, charset_name),
    ne!("Ljava/nio/charset/Charset;", "toString", "()Ljava/lang/String;", true, charset_name),
    ne!("Ljava/nio/charset/Charset;", "displayName", "()Ljava/lang/String;", true, charset_name),
    ne!("Ljava/nio/charset/Charset;", "displayName", "(Ljava/util/Locale;)Ljava/lang/String;", true, charset_name),
    ne!("Ljava/nio/charset/Charset;", "canEncode", "()Z", true, charset_can_encode),
    ne!("Ljava/nio/charset/Charset;", "defaultCharset", "()Ljava/nio/charset/Charset;", false, charset_default_charset),
    ne!("Ljava/nio/charset/Charset;", "isSupported", "(Ljava/lang/String;)Z", false, charset_is_supported),
    // ---- kotlin.jvm.internal.DefaultConstructorMarker ----
    ne!("Lkotlin/jvm/internal/DefaultConstructorMarker;", "<init>", "()V", true, object_noop),
];
