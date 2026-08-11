//! Resolved classes, methods, and the shim-class registry.
//!
//! Shim classes supply the host-provided JVM surface (`java.lang.*`,
//! `java.util.*`, ...) that keiyoushi APKs reference but do not bundle.
//! A shim class has no bytecode: every method resolves to a native function.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use crate::dex::insn::Decoded;
use crate::dex::CodeItem;
use crate::vm::native;
use crate::vm::value::JValue;

pub const ACC_PUBLIC: u32 = 0x0001;
pub const ACC_PRIVATE: u32 = 0x0002;
pub const ACC_PROTECTED: u32 = 0x0004;
pub const ACC_STATIC: u32 = 0x0008;
pub const ACC_FINAL: u32 = 0x0010;
pub const ACC_SYNCHRONIZED: u32 = 0x0020;
pub const ACC_VOLATILE: u32 = 0x0040;
pub const ACC_TRANSIENT: u32 = 0x0080;
pub const ACC_NATIVE: u32 = 0x0100;
pub const ACC_INTERFACE: u32 = 0x0200;
pub const ACC_ABSTRACT: u32 = 0x0400;
pub const ACC_SYNTHETIC: u32 = 0x1000;
pub const ACC_ENUM: u32 = 0x4000;

#[derive(Debug, Clone)]
pub struct Method {
    /// Index into `Class::methods`.
    pub slot: u32,
    /// Owning class id.
    pub class: u32,
    /// Interned method name.
    pub name: u32,
    /// Interned full signature `(args)ret`, e.g. `(II)Ljava/lang/String;`.
    pub sig: u32,
    /// Interned return descriptor.
    pub ret: u32,
    /// Interned argument descriptors (excluding `this`).
    pub args: Vec<u32>,
    pub access_flags: u32,
    pub static_method: bool,
    /// Dex file (index into `Vm::dexes`) this method's bytecode/ids came from.
    pub dex_idx: u32,
    /// Present when this method is implemented natively.
    pub native_key: Option<(u32, u32, u32)>,
    /// Declared `native` in the dex (no code item, no JNI bridge).
    ///
    /// JNI is unsupported; invoking such a method raises a resolution error
    /// unless the host registered a shim for it first.
    pub native_decl: bool,
    pub code: Option<Arc<CodeItem>>,
    /// Decoded instructions (bytecode methods only).
    pub insns: OnceLock<Arc<Decoded>>,
}

impl Method {
    pub fn is_native(&self) -> bool {
        self.native_key.is_some()
    }
}

/// Lazily materialized static field value (e.g. `System.out`).
pub type ShimLazy = fn(&mut crate::vm::Vm) -> JValue;

#[derive(Debug, Clone, Copy)]
pub enum ShimValue {
    Const(JValue),
    Lazy(ShimLazy),
}

#[derive(Debug, Clone, Copy)]
pub struct ShimStaticDef {
    pub name: &'static str,
    pub ty: &'static str,
    pub value: ShimValue,
}

#[derive(Debug, Clone, Copy)]
pub struct ShimDef {
    pub desc: &'static str,
    pub super_desc: Option<&'static str>,
    pub interfaces: &'static [&'static str],
    pub flags: u32,
    pub statics: &'static [ShimStaticDef],
}

macro_rules! shim {
    ($desc:expr, $super_desc:expr, $interfaces:expr, $flags:expr) => {
        ShimDef {
            desc: $desc,
            super_desc: $super_desc,
            interfaces: $interfaces,
            flags: $flags,
            statics: &[],
        }
    };
    ($desc:expr, $super_desc:expr, $interfaces:expr, $flags:expr, [$($s:expr),* $(,)?]) => {
        ShimDef {
            desc: $desc,
            super_desc: $super_desc,
            interfaces: $interfaces,
            flags: $flags,
            statics: &[$($s),*],
        }
    };
}

macro_rules! sdef {
    ($name:expr, $ty:expr, $value:expr) => {
        ShimStaticDef {
            name: $name,
            ty: $ty,
            value: $value,
        }
    };
}

/// Host-provided classes not present in extension dex files.
pub static SHIM_CLASSES: &[ShimDef] = &[
    shim!("Ljava/lang/Object;", None, &[], 0),
    shim!(
        "Ljava/io/Serializable;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/lang/Cloneable;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/lang/Comparable;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/lang/CharSequence;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/lang/Iterable;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/lang/reflect/AccessibleObject;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_PUBLIC | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/lang/reflect/Field;",
        Some("Ljava/lang/reflect/AccessibleObject;"),
        &[],
        ACC_PUBLIC
    ),
    // mihon extension host API (keiyoushi/Tachiyomi extension entry points)
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/SourceFactory;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/Source;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/ConfigurableSource;",
        Some("Leu/kanade/tachiyomi/source/Source;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/online/HttpSource;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    // mihon extension network API (eu.kanade.tachiyomi.network.*)
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/network/NetworkHelper;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    // mihon source model classes (eu.kanade.tachiyomi.source.model.*)
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/SManga;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [sdef!(
            "Companion",
            "Leu/kanade/tachiyomi/source/model/SManga$Companion;",
            ShimValue::Lazy(native::lazy_smanga_companion)
        ),]
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/SManga$Companion;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/SChapter;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [sdef!(
            "Companion",
            "Leu/kanade/tachiyomi/source/model/SChapter$Companion;",
            ShimValue::Lazy(native::lazy_schapter_companion)
        ),]
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/SChapter$Companion;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/Page;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/MangasPage;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/FilterList;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/Filter;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/Filter$Header;",
        Some("Leu/kanade/tachiyomi/source/model/Filter;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/Filter$Separator;",
        Some("Leu/kanade/tachiyomi/source/model/Filter;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/Filter$Select;",
        Some("Leu/kanade/tachiyomi/source/model/Filter;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/Filter$CheckBox;",
        Some("Leu/kanade/tachiyomi/source/model/Filter;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/Filter$Sort;",
        Some("Leu/kanade/tachiyomi/source/model/Filter;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/Filter$Text;",
        Some("Leu/kanade/tachiyomi/source/model/Filter;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/Filter$TriState;",
        Some("Leu/kanade/tachiyomi/source/model/Filter;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/Filter$Group;",
        Some("Leu/kanade/tachiyomi/source/model/Filter;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Leu/kanade/tachiyomi/source/model/UpdateStrategy;",
        Some("Ljava/lang/Enum;"),
        &[],
        0,
        [sdef!(
            "ONLY_FETCH_ONCE",
            "Leu/kanade/tachiyomi/source/model/UpdateStrategy;",
            ShimValue::Lazy(native::lazy_update_strategy_once)
        ),]
    ),
    // org.jsoup host shims (parsed via dom_query)
    #[cfg(feature = "jsoup")]
    shim!(
        "Leu/kanade/tachiyomi/util/JsoupExtensionsKt;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    // Kotlin stdlib host shims (default-arg synthetic methods)
    #[cfg(feature = "kotlin")]
    shim!(
        "Lkotlin/text/StringsKt;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "kotlin")]
    shim!(
        "Lkotlin/text/Charsets;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [
            sdef!(
                "UTF_8",
                "Ljava/nio/charset/Charset;",
                ShimValue::Lazy(native::lazy_charset_utf8)
            ),
            sdef!(
                "ISO_8859_1",
                "Ljava/nio/charset/Charset;",
                ShimValue::Lazy(native::lazy_charset_iso)
            ),
            sdef!(
                "US_ASCII",
                "Ljava/nio/charset/Charset;",
                ShimValue::Lazy(native::lazy_charset_ascii)
            ),
        ]
    ),
    #[cfg(feature = "jsoup")]
    shim!("Lorg/jsoup/Jsoup;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "jsoup")]
    shim!(
        "Lorg/jsoup/nodes/Document;",
        Some("Lorg/jsoup/nodes/Element;"),
        &[],
        0
    ),
    #[cfg(feature = "jsoup")]
    shim!(
        "Lorg/jsoup/nodes/Element;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "jsoup")]
    shim!(
        "Lorg/jsoup/select/Elements;",
        Some("Ljava/util/AbstractCollection;"),
        &[],
        0
    ),
    // okhttp host shims
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/Interceptor;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/Interceptor$Chain;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/OkHttpClient;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Closeable;", "Ljava/lang/Cloneable;"],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/OkHttpClient$Builder;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Leu/kanade/tachiyomi/network/interceptor/UncaughtExceptionInterceptor;",
        Some("Lokhttp3/Interceptor;"),
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Leu/kanade/tachiyomi/network/interceptor/UserAgentInterceptor;",
        Some("Lokhttp3/Interceptor;"),
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Leu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor;",
        Some("Lokhttp3/Interceptor;"),
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/CompressionInterceptor;",
        Some("Lokhttp3/Interceptor;"),
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/CompressionInterceptor$DecompressionAlgorithm;",
        None,
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/brotli/BrotliInterceptor;",
        Some("Lokhttp3/Interceptor;"),
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/brotli/Brotli;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [sdef!(
            "INSTANCE",
            "Lokhttp3/brotli/Brotli;",
            ShimValue::Lazy(native::lazy_brotli_inst)
        )]
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/Gzip;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [sdef!(
            "INSTANCE",
            "Lokhttp3/Gzip;",
            ShimValue::Lazy(native::lazy_gzip_inst)
        )]
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/zstd/Zstd;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [sdef!(
            "INSTANCE",
            "Lokhttp3/zstd/Zstd;",
            ShimValue::Lazy(native::lazy_zstd_inst)
        )]
    ),
    #[cfg(feature = "okhttp")]
    shim!("Lokhttp3/FormBody;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!("Lokio/Okio;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokio/Source;",
        None,
        &["Ljava/io/Closeable;"],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokio/BufferedSource;",
        None,
        &["Lokio/Source;", "Ljava/io/Closeable;"],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokio/Buffer;",
        Some("Ljava/lang/Object;"),
        &["Lokio/BufferedSource;", "Ljava/io/Closeable;"],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!("Lokio/ByteString;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!("Lokio/ByteStreams;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!("Lokio/ByteStreamsKt;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/FormBody$Builder;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/HttpUrl;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [sdef!(
            "Companion",
            "Lokhttp3/HttpUrl$Companion;",
            ShimValue::Lazy(native::lazy_http_url_companion)
        ),]
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/HttpUrl$Companion;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/HttpUrl$Builder;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/MediaType;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [sdef!(
            "Companion",
            "Lokhttp3/MediaType$Companion;",
            ShimValue::Lazy(native::lazy_media_type_companion)
        ),]
    ),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/MediaType$Companion;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!("Lokhttp3/Headers;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!("Lokhttp3/RequestBody;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!("Lokhttp3/CacheControl;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!(
        "Lokhttp3/Request$Builder;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "okhttp")]
    shim!("Lokhttp3/Request;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!("Lokhttp3/Call;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!("Lokhttp3/Response;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!("Lokhttp3/ResponseBody;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "okhttp")]
    shim!(
        "Leu/kanade/tachiyomi/network/RequestsKt;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    // android framework shims
    #[cfg(feature = "android")]
    shim!(
        "Landroid/content/Context;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "android")]
    shim!(
        "Landroid/content/ContextWrapper;",
        Some("Landroid/content/Context;"),
        &[],
        0
    ),
    #[cfg(feature = "android")]
    shim!(
        "Landroid/app/Activity;",
        Some("Landroid/content/ContextWrapper;"),
        &[],
        0
    ),
    #[cfg(feature = "android")]
    shim!(
        "Landroid/app/Application;",
        Some("Landroid/content/ContextWrapper;"),
        &[],
        0
    ),
    #[cfg(feature = "android")]
    shim!(
        "Landroid/content/ActivityNotFoundException;",
        Some("Ljava/lang/RuntimeException;"),
        &[],
        0
    ),
    #[cfg(feature = "android")]
    shim!(
        "Landroid/content/Intent;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "android")]
    shim!(
        "Landroid/content/SharedPreferences;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "android")]
    shim!("Landroid/net/Uri;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "android")]
    shim!("Landroid/os/Bundle;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "android")]
    shim!(
        "Landroid/os/SystemClock;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "android")]
    shim!("Landroid/util/Log;", Some("Ljava/lang/Object;"), &[], 0),
    #[cfg(feature = "android")]
    shim!(
        "Landroid/webkit/CookieManager;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "android")]
    shim!(
        "Landroidx/preference/Preference;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "android")]
    shim!(
        "Landroidx/preference/PreferenceScreen;",
        Some("Landroidx/preference/Preference;"),
        &[],
        0
    ),
    #[cfg(feature = "android")]
    shim!(
        "Landroidx/preference/SwitchPreferenceCompat;",
        Some("Landroidx/preference/Preference;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/String;",
        Some("Ljava/lang/Object;"),
        &[
            "Ljava/io/Serializable;",
            "Ljava/lang/Comparable;",
            "Ljava/lang/CharSequence;"
        ],
        0
    ),
    shim!(
        "Ljava/lang/StringBuilder;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;", "Ljava/lang/CharSequence;"],
        0
    ),
    shim!(
        "Ljava/lang/Class;",
        Some("Ljava/lang/Object;"),
        &[
            "Ljava/io/Serializable;",
            "Ljava/lang/reflect/GenericDeclaration;",
            "Ljava/lang/reflect/Type;",
            "Ljava/lang/reflect/AnnotatedElement;"
        ],
        0
    ),
    shim!(
        "Ljava/lang/Throwable;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;"],
        0
    ),
    shim!(
        "Ljava/lang/Exception;",
        Some("Ljava/lang/Throwable;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/RuntimeException;",
        Some("Ljava/lang/Exception;"),
        &[],
        0
    ),
    shim!("Ljava/lang/Error;", Some("Ljava/lang/Throwable;"), &[], 0),
    shim!(
        "Ljava/lang/AssertionError;",
        Some("Ljava/lang/Error;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/StackOverflowError;",
        Some("Ljava/lang/Error;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/OutOfMemoryError;",
        Some("Ljava/lang/Error;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/NullPointerException;",
        Some("Ljava/lang/RuntimeException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/ArithmeticException;",
        Some("Ljava/lang/RuntimeException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/IllegalArgumentException;",
        Some("Ljava/lang/RuntimeException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/IllegalStateException;",
        Some("Ljava/lang/RuntimeException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/NumberFormatException;",
        Some("Ljava/lang/IllegalArgumentException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/UnsupportedOperationException;",
        Some("Ljava/lang/RuntimeException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/IndexOutOfBoundsException;",
        Some("Ljava/lang/RuntimeException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/ArrayIndexOutOfBoundsException;",
        Some("Ljava/lang/IndexOutOfBoundsException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/StringIndexOutOfBoundsException;",
        Some("Ljava/lang/IndexOutOfBoundsException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/ClassCastException;",
        Some("Ljava/lang/RuntimeException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/NegativeArraySizeException;",
        Some("Ljava/lang/RuntimeException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/NoSuchElementException;",
        Some("Ljava/lang/RuntimeException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/NoSuchMethodError;",
        Some("Ljava/lang/Error;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/NoClassDefFoundError;",
        Some("Ljava/lang/Error;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/ClassNotFoundException;",
        Some("Ljava/lang/Exception;"),
        &[],
        0
    ),
    shim!(
        "Ljava/io/IOException;",
        Some("Ljava/lang/Exception;"),
        &[],
        0
    ),
    shim!(
        "Ljava/net/MalformedURLException;",
        Some("Ljava/io/IOException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/InterruptedException;",
        Some("Ljava/lang/Exception;"),
        &[],
        0
    ),
    shim!(
        "Ljava/io/InputStream;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Closeable;"],
        0
    ),
    shim!(
        "Ljava/io/ByteArrayInputStream;",
        Some("Ljava/io/InputStream;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/SecurityException;",
        Some("Ljava/lang/RuntimeException;"),
        &[],
        0
    ),
    shim!(
        "Ljava/lang/Enum;",
        Some("Ljava/lang/Object;"),
        &["Ljava/lang/Comparable;", "Ljava/io/Serializable;"],
        0
    ),
    shim!(
        "Ljava/lang/System;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [
            sdef!(
                "out",
                "Ljava/io/PrintStream;",
                ShimValue::Lazy(native::lazy_print_stream)
            ),
            sdef!(
                "err",
                "Ljava/io/PrintStream;",
                ShimValue::Lazy(native::lazy_print_stream)
            ),
        ]
    ),
    shim!(
        "Ljava/io/PrintStream;",
        Some("Ljava/lang/Object;"),
        &["Ljava/lang/Appendable;", "Ljava/io/Closeable;"],
        0
    ),
    shim!("Ljava/lang/Math;", Some("Ljava/lang/Object;"), &[], 0),
    shim!(
        "Ljava/lang/Integer;",
        Some("Ljava/lang/Number;"),
        &["Ljava/lang/Comparable;"],
        0,
        [
            sdef!(
                "TYPE",
                "Ljava/lang/Class;",
                ShimValue::Lazy(native::lazy_int_type)
            ),
            sdef!("MIN_VALUE", "I", ShimValue::Const(JValue::Int(i32::MIN))),
            sdef!("MAX_VALUE", "I", ShimValue::Const(JValue::Int(i32::MAX))),
        ]
    ),
    shim!(
        "Ljava/lang/Long;",
        Some("Ljava/lang/Number;"),
        &["Ljava/lang/Comparable;"],
        0,
        [
            sdef!(
                "TYPE",
                "Ljava/lang/Class;",
                ShimValue::Lazy(native::lazy_long_type)
            ),
            sdef!("MIN_VALUE", "J", ShimValue::Const(JValue::Long(i64::MIN))),
            sdef!("MAX_VALUE", "J", ShimValue::Const(JValue::Long(i64::MAX))),
        ]
    ),
    shim!(
        "Ljava/lang/Short;",
        Some("Ljava/lang/Number;"),
        &["Ljava/lang/Comparable;"],
        0,
        [
            sdef!(
                "TYPE",
                "Ljava/lang/Class;",
                ShimValue::Lazy(native::lazy_short_type)
            ),
            sdef!(
                "MIN_VALUE",
                "S",
                ShimValue::Const(JValue::Int(i16::MIN as i32))
            ),
            sdef!(
                "MAX_VALUE",
                "S",
                ShimValue::Const(JValue::Int(i16::MAX as i32))
            ),
        ]
    ),
    shim!(
        "Ljava/lang/Byte;",
        Some("Ljava/lang/Number;"),
        &["Ljava/lang/Comparable;"],
        0,
        [
            sdef!(
                "TYPE",
                "Ljava/lang/Class;",
                ShimValue::Lazy(native::lazy_byte_type)
            ),
            sdef!(
                "MIN_VALUE",
                "B",
                ShimValue::Const(JValue::Int(i8::MIN as i32))
            ),
            sdef!(
                "MAX_VALUE",
                "B",
                ShimValue::Const(JValue::Int(i8::MAX as i32))
            ),
        ]
    ),
    shim!(
        "Ljava/lang/Character;",
        Some("Ljava/lang/Object;"),
        &["Ljava/lang/Comparable;"],
        0,
        [
            sdef!(
                "TYPE",
                "Ljava/lang/Class;",
                ShimValue::Lazy(native::lazy_char_type)
            ),
            sdef!("MIN_VALUE", "C", ShimValue::Const(JValue::Int(0))),
            sdef!("MAX_VALUE", "C", ShimValue::Const(JValue::Int(0xFFFF))),
        ]
    ),
    shim!(
        "Ljava/lang/Boolean;",
        Some("Ljava/lang/Object;"),
        &["Ljava/lang/Comparable;"],
        0,
        [
            sdef!(
                "TYPE",
                "Ljava/lang/Class;",
                ShimValue::Lazy(native::lazy_bool_type)
            ),
            sdef!(
                "TRUE",
                "Ljava/lang/Boolean;",
                ShimValue::Lazy(native::lazy_bool_true)
            ),
            sdef!(
                "FALSE",
                "Ljava/lang/Boolean;",
                ShimValue::Lazy(native::lazy_bool_false)
            ),
        ]
    ),
    shim!(
        "Ljava/lang/Float;",
        Some("Ljava/lang/Number;"),
        &["Ljava/lang/Comparable;"],
        0,
        [
            sdef!(
                "TYPE",
                "Ljava/lang/Class;",
                ShimValue::Lazy(native::lazy_float_type)
            ),
            sdef!(
                "MIN_VALUE",
                "F",
                ShimValue::Const(JValue::Float(f32::MIN_POSITIVE))
            ),
            sdef!("MAX_VALUE", "F", ShimValue::Const(JValue::Float(f32::MAX))),
            sdef!(
                "POSITIVE_INFINITY",
                "F",
                ShimValue::Const(JValue::Float(f32::INFINITY))
            ),
            sdef!(
                "NEGATIVE_INFINITY",
                "F",
                ShimValue::Const(JValue::Float(f32::NEG_INFINITY))
            ),
            sdef!("NaN", "F", ShimValue::Const(JValue::Float(f32::NAN))),
        ]
    ),
    shim!(
        "Ljava/lang/Double;",
        Some("Ljava/lang/Number;"),
        &["Ljava/lang/Comparable;"],
        0,
        [
            sdef!(
                "TYPE",
                "Ljava/lang/Class;",
                ShimValue::Lazy(native::lazy_double_type)
            ),
            sdef!(
                "MIN_VALUE",
                "D",
                ShimValue::Const(JValue::Double(f64::MIN_POSITIVE))
            ),
            sdef!("MAX_VALUE", "D", ShimValue::Const(JValue::Double(f64::MAX))),
            sdef!(
                "POSITIVE_INFINITY",
                "D",
                ShimValue::Const(JValue::Double(f64::INFINITY))
            ),
            sdef!(
                "NEGATIVE_INFINITY",
                "D",
                ShimValue::Const(JValue::Double(f64::NEG_INFINITY))
            ),
            sdef!("NaN", "D", ShimValue::Const(JValue::Double(f64::NAN))),
        ]
    ),
    shim!(
        "Ljava/lang/Number;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;"],
        0
    ),
    shim!(
        "Ljava/lang/StackTraceElement;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;"],
        0
    ),
    shim!("Ljava/lang/Thread;", Some("Ljava/lang/Object;"), &[], 0),
    shim!("Ljava/lang/Thread$State;", Some("Ljava/lang/Enum;"), &[], 0),
    shim!(
        "Ljava/security/MessageDigest;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Ljava/security/SecureRandom;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Ljava/security/Key;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/security/spec/AlgorithmParameterSpec;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!("Ljavax/crypto/Cipher;", Some("Ljava/lang/Object;"), &[], 0),
    shim!(
        "Ljavax/crypto/spec/SecretKeySpec;",
        Some("Ljava/lang/Object;"),
        &[
            "Ljava/security/Key;",
            "Ljava/security/spec/AlgorithmParameterSpec;"
        ],
        0
    ),
    shim!(
        "Ljavax/crypto/spec/GCMParameterSpec;",
        Some("Ljava/lang/Object;"),
        &["Ljava/security/spec/AlgorithmParameterSpec;"],
        0
    ),
    shim!(
        "Ljava/util/Collection;",
        Some("Ljava/lang/Iterable;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/List;",
        Some("Ljava/util/Collection;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/Set;",
        Some("Ljava/util/Collection;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/SortedSet;",
        Some("Ljava/util/Set;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/NavigableSet;",
        Some("Ljava/util/SortedSet;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/Map;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/SortedMap;",
        Some("Ljava/util/Map;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/NavigableMap;",
        Some("Ljava/util/SortedMap;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/Map$Entry;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/Iterator;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/ListIterator;",
        Some("Ljava/util/Iterator;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/Comparator;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/RandomAccess;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/ArrayList;",
        Some("Ljava/util/AbstractList;"),
        &[
            "Ljava/util/List;",
            "Ljava/util/RandomAccess;",
            "Ljava/lang/Cloneable;",
            "Ljava/io/Serializable;"
        ],
        0
    ),
    shim!(
        "Ljava/util/AbstractList;",
        Some("Ljava/util/AbstractCollection;"),
        &["Ljava/util/List;"],
        0
    ),
    shim!(
        "Ljava/util/AbstractCollection;",
        Some("Ljava/util/Collection;"),
        &[],
        0
    ),
    shim!("Ljava/util/AbstractMap;", Some("Ljava/util/Map;"), &[], 0),
    shim!(
        "Ljava/util/AbstractSet;",
        Some("Ljava/util/AbstractCollection;"),
        &["Ljava/util/Set;"],
        0
    ),
    shim!(
        "Ljava/util/HashMap;",
        Some("Ljava/util/AbstractMap;"),
        &[
            "Ljava/util/Map;",
            "Ljava/lang/Cloneable;",
            "Ljava/io/Serializable;"
        ],
        0
    ),
    shim!(
        "Ljava/util/LinkedHashMap;",
        Some("Ljava/util/HashMap;"),
        &["Ljava/util/Map;"],
        0
    ),
    shim!(
        "Ljava/util/HashSet;",
        Some("Ljava/util/AbstractSet;"),
        &[
            "Ljava/util/Set;",
            "Ljava/lang/Cloneable;",
            "Ljava/io/Serializable;"
        ],
        0
    ),
    shim!(
        "Ljava/util/LinkedHashSet;",
        Some("Ljava/util/HashSet;"),
        &[
            "Ljava/util/Set;",
            "Ljava/lang/Cloneable;",
            "Ljava/io/Serializable;"
        ],
        0
    ),
    shim!(
        "Ljava/util/Collections;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!("Ljava/util/Arrays;", Some("Ljava/lang/Object;"), &[], 0),
    shim!("Ljava/util/Objects;", Some("Ljava/lang/Object;"), &[], 0),
    shim!(
        "Ljava/util/Locale;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;", "Ljava/lang/Cloneable;"],
        0,
        [
            sdef!(
                "ROOT",
                "Ljava/util/Locale;",
                ShimValue::Lazy(native::lazy_opaque_locale)
            ),
            sdef!(
                "ENGLISH",
                "Ljava/util/Locale;",
                ShimValue::Lazy(native::lazy_opaque_locale)
            ),
            sdef!(
                "US",
                "Ljava/util/Locale;",
                ShimValue::Lazy(native::lazy_locale_us)
            ),
            sdef!(
                "UK",
                "Ljava/util/Locale;",
                ShimValue::Lazy(native::lazy_locale_uk)
            ),
            sdef!(
                "CANADA",
                "Ljava/util/Locale;",
                ShimValue::Lazy(native::lazy_locale_canada)
            ),
            sdef!(
                "JAPAN",
                "Ljava/util/Locale;",
                ShimValue::Lazy(native::lazy_locale_japan)
            ),
            sdef!(
                "KOREA",
                "Ljava/util/Locale;",
                ShimValue::Lazy(native::lazy_locale_korea)
            ),
            sdef!(
                "CHINA",
                "Ljava/util/Locale;",
                ShimValue::Lazy(native::lazy_locale_china)
            ),
            sdef!(
                "FRANCE",
                "Ljava/util/Locale;",
                ShimValue::Lazy(native::lazy_locale_france)
            ),
            sdef!(
                "GERMANY",
                "Ljava/util/Locale;",
                ShimValue::Lazy(native::lazy_locale_germany)
            ),
            sdef!(
                "ITALY",
                "Ljava/util/Locale;",
                ShimValue::Lazy(native::lazy_locale_italy)
            ),
        ]
    ),
    shim!(
        "Ljava/util/TimeZone;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;", "Ljava/lang/Cloneable;"],
        0
    ),
    shim!(
        "Ljava/util/ArrayDeque;",
        Some("Ljava/util/AbstractCollection;"),
        &[
            "Ljava/util/Deque;",
            "Ljava/lang/Cloneable;",
            "Ljava/io/Serializable;"
        ],
        0
    ),
    shim!(
        "Ljava/util/Deque;",
        Some("Ljava/util/Collection;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!("Ljava/text/DateFormat;", Some("Ljava/lang/Object;"), &[], 0),
    shim!(
        "Ljava/text/SimpleDateFormat;",
        Some("Ljava/text/DateFormat;"),
        &[],
        0
    ),
    shim!(
        "Ljava/text/ParsePosition;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Ljava/util/concurrent/locks/Lock;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/util/concurrent/locks/ReentrantLock;",
        Some("Ljava/lang/Object;"),
        &[
            "Ljava/util/concurrent/locks/Lock;",
            "Ljava/io/Serializable;"
        ],
        0
    ),
    shim!(
        "Ljava/util/concurrent/locks/Condition;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/nio/charset/Charset;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;", "Ljava/lang/Comparable;"],
        0
    ),
    shim!(
        "Ljava/nio/charset/StandardCharsets;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [
            sdef!(
                "UTF_8",
                "Ljava/nio/charset/Charset;",
                ShimValue::Lazy(native::lazy_charset_utf8)
            ),
            sdef!(
                "ISO_8859_1",
                "Ljava/nio/charset/Charset;",
                ShimValue::Lazy(native::lazy_charset_iso)
            ),
            sdef!(
                "US_ASCII",
                "Ljava/nio/charset/Charset;",
                ShimValue::Lazy(native::lazy_charset_ascii)
            ),
        ]
    ),
    shim!(
        "Ljava/util/regex/Pattern;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;"],
        0
    ),
    shim!(
        "Ljava/util/regex/Matcher;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Ljava/util/Random;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;"],
        0
    ),
    shim!(
        "Ljava/util/Date;",
        Some("Ljava/lang/Object;"),
        &[
            "Ljava/io/Serializable;",
            "Ljava/lang/Cloneable;",
            "Ljava/lang/Comparable;"
        ],
        0
    ),
    shim!(
        "Ljava/lang/reflect/GenericDeclaration;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/lang/reflect/Type;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/lang/reflect/AnnotatedElement;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/lang/Appendable;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/io/Closeable;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Ljava/io/Flushable;",
        Some("Ljava/lang/Object;"),
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    // kotlin stdlib host shims
    shim!(
        "Lkotlin/Lazy;",
        None,
        &["Ljava/io/Serializable;"],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Lkotlin/SynchronizedLazyImpl;",
        Some("Ljava/lang/Object;"),
        &["Lkotlin/Lazy;", "Ljava/io/Serializable;"],
        0
    ),
    shim!(
        "Lkotlin/jvm/functions/Function0;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Lkotlin/jvm/functions/Function1;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!("Lkotlin/LazyKt;", Some("Ljava/lang/Object;"), &[], 0),
    shim!(
        "Lkotlin/time/Duration;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [sdef!(
            "Companion",
            "Lkotlin/time/Duration$Companion;",
            ShimValue::Lazy(native::lazy_duration_companion)
        ),]
    ),
    shim!(
        "Lkotlin/time/Duration$Companion;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/time/DurationUnit;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [
            sdef!(
                "SECONDS",
                "Lkotlin/time/DurationUnit;",
                ShimValue::Lazy(native::lazy_duration_unit_seconds)
            ),
            sdef!(
                "MINUTES",
                "Lkotlin/time/DurationUnit;",
                ShimValue::Lazy(native::lazy_duration_unit_millis)
            ),
            sdef!(
                "MILLISECONDS",
                "Lkotlin/time/DurationUnit;",
                ShimValue::Lazy(native::lazy_duration_unit_millis)
            ),
            sdef!(
                "DAYS",
                "Lkotlin/time/DurationUnit;",
                ShimValue::Lazy(native::lazy_duration_unit_days)
            ),
        ]
    ),
    shim!(
        "Lkotlin/time/DurationKt;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/text/Regex;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;"],
        0
    ),
    shim!(
        "Lkotlin/collections/CollectionsKt;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/collections/IntIterator;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/ranges/IntRange;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/comparisons/ComparisonsKt;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/jvm/internal/Intrinsics;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/Unit;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [sdef!(
            "INSTANCE",
            "Lkotlin/Unit;",
            ShimValue::Lazy(native::lazy_unit_instance)
        ),]
    ),
    shim!(
        "Lkotlinx/coroutines/GlobalScope;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [sdef!(
            "INSTANCE",
            "Lkotlinx/coroutines/GlobalScope;",
            ShimValue::Lazy(native::lazy_global_scope)
        ),]
    ),
    shim!(
        "Lkotlin/coroutines/jvm/internal/SuspendLambda;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/coroutines/jvm/internal/ContinuationImpl;",
        Some("Ljava/lang/Object;"),
        &["Lkotlin/coroutines/Continuation;"],
        0
    ),
    shim!(
        "Lkotlin/coroutines/Continuation;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Lkotlin/coroutines/intrinsics/IntrinsicsKt;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!("Lkotlin/ResultKt;", Some("Ljava/lang/Object;"), &[], 0),
    shim!(
        "Lkotlin/jvm/functions/Function2;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Lokhttp3/CacheControl$Builder;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/coroutines/CoroutineDispatcher;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/Pair;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;"],
        0
    ),
    shim!(
        "Lkotlin/Result$Failure;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/Result;",
        Some("Ljava/lang/Object;"),
        &[],
        0,
        [sdef!(
            "Companion",
            "Lkotlin/Result$Companion;",
            ShimValue::Lazy(native::lazy_result_companion)
        ),]
    ),
    shim!(
        "Lkotlin/Result$Companion;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/text/MatchResult;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!("Lkotlin/TuplesKt;", Some("Ljava/lang/Object;"), &[], 0),
    shim!(
        "Lkotlin/ranges/RangesKt;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Lkotlin/jvm/internal/DefaultConstructorMarker;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    // kotlinx.serialization JSON pipeline (cached filter lists, moetruyen)
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/json/Json;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;"],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/json/JsonElement;",
        Some("Ljava/lang/Object;"),
        &["Ljava/io/Serializable;"],
        ACC_ABSTRACT,
        [sdef!(
            "Companion",
            "Lkotlinx/serialization/json/JsonElement$Companion;",
            ShimValue::Lazy(native::lazy_json_element_companion)
        )]
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/json/JsonElement$Companion;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/json/JsonObject;",
        Some("Lkotlinx/serialization/json/JsonElement;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/json/JsonArray;",
        Some("Lkotlinx/serialization/json/JsonElement;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/json/JsonPrimitive;",
        Some("Lkotlinx/serialization/json/JsonElement;"),
        &[],
        ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/json/JsonNull;",
        Some("Lkotlinx/serialization/json/JsonPrimitive;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/json/JsonLiteral;",
        Some("Lkotlinx/serialization/json/JsonPrimitive;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/SerializationStrategy;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/DeserializationStrategy;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/KSerializer;",
        None,
        &[
            "Lkotlinx/serialization/SerializationStrategy;",
            "Lkotlinx/serialization/DeserializationStrategy;",
        ],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/encoding/Decoder;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/encoding/CompositeDecoder;",
        None,
        &["Lkotlinx/serialization/encoding/Decoder;"],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/encoding/Encoder;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/encoding/CompositeEncoder;",
        None,
        &["Lkotlinx/serialization/encoding/Encoder;"],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/json/JsonDecoder;",
        None,
        &["Lkotlinx/serialization/encoding/Decoder;"],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/descriptors/SerialDescriptor;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/internal/GeneratedSerializer;",
        None,
        &["Lkotlinx/serialization/KSerializer;"],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/internal/PluginGeneratedSerialDescriptor;",
        Some("Ljava/lang/Object;"),
        &["Lkotlinx/serialization/descriptors/SerialDescriptor;"],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/internal/ArrayListSerializer;",
        Some("Ljava/lang/Object;"),
        &["Lkotlinx/serialization/KSerializer;"],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/json/okio/OkioStreamsKt;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lcom/squareup/zstd/okio/OkioZstd;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/json/internal/StreamingJsonDecoder;",
        Some("Ljava/lang/Object;"),
        &[
            "Lkotlinx/serialization/json/JsonDecoder;",
            "Lkotlinx/serialization/encoding/CompositeDecoder;",
        ],
        0
    ),
    #[cfg(feature = "tachiyomi")]
    shim!(
        "Lkotlinx/serialization/UnknownFieldException;",
        Some("Ljava/lang/IllegalArgumentException;"),
        &[],
        0
    ),
    // injekt DI
    shim!(
        "Luy/kohesive/injekt/InjektKt;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
    shim!(
        "Luy/kohesive/injekt/api/InjektScope;",
        None,
        &["Luy/kohesive/injekt/api/InjektFactory;"],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Luy/kohesive/injekt/api/InjektFactory;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Luy/kohesive/injekt/api/InjektRegister;",
        None,
        &[],
        ACC_INTERFACE | ACC_ABSTRACT
    ),
    shim!(
        "Luy/kohesive/injekt/api/FullTypeReference;",
        Some("Ljava/lang/Object;"),
        &[],
        0
    ),
];

#[derive(Debug, Clone, Default)]
pub struct Class {
    pub id: u32,
    /// Interned full descriptor (e.g. `Lcom/foo/Bar;` or `[I`).
    pub descriptor: u32,
    pub superclass: Option<u32>,
    pub interfaces: Vec<u32>,
    pub access_flags: u32,
    pub is_interface: bool,
    pub is_abstract: bool,
    /// For array classes: (dex index, element type id).
    pub array_elem: Option<(u32, u32)>,
    /// Instance fields in declaration order (walk order == offset).
    pub instance_fields: Vec<(u32, u32, u32)>, // (name, ty_desc, access)
    /// (name, ty_desc) -> instance offset. Inherited fields are copied in.
    pub field_offsets: HashMap<(u32, u32), u32>,
    /// (name, ty_desc) -> (owning class id, offset in that class's `statics`).
    pub static_fields: HashMap<(u32, u32), (u32, u32)>,
    /// Own static field values (offset == index).
    pub statics: Vec<JValue>,
    /// Lazy static materializers for shim classes.
    pub statics_lazy: Vec<Option<ShimLazy>>,
    /// 0 = uninitialized, 1 = initializing, 2 = initialized.
    pub clinit_state: u8,
    /// Cached `java.lang.Class` instance for this class (identity-stable).
    pub class_obj: Option<u32>,
    pub methods: Vec<Method>,
    /// (name, sig) -> slot into `methods`.
    pub dispatch: HashMap<(u32, u32), u32>,
}

impl Class {
    /// Find the `<clinit>` static initializer slot (if any). `clinit_name`
    /// must be the VM-interned id of `"<clinit>"`.
    pub fn clinit_slot(&self, clinit_name: u32) -> Option<u32> {
        self.methods
            .iter()
            .find(|m| m.static_method && m.name == clinit_name)
            .map(|m| m.slot)
    }
}
