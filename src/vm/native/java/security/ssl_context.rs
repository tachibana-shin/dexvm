//! javax.net.ssl.SSLContext host shims.

use crate::vm::native::*;

fn ssl_context_get_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljavax/net/ssl/SSLContext;", Native::Opaque)
}

fn ssl_context_get_socket_factory(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljavax/net/ssl/SSLSocketFactory;", Native::Opaque)
}

fn ssl_context_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

/// Native methods for Ljavax/net/ssl/SSLContext;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljavax/net/ssl/SSLContext;",
        "getInstance",
        "(Ljava/lang/String;)Ljavax/net/ssl/SSLContext;",
        false,
        ssl_context_get_instance
    ),
    ne!(
        "Ljavax/net/ssl/SSLContext;",
        "getSocketFactory",
        "()Ljavax/net/ssl/SSLSocketFactory;",
        true,
        ssl_context_get_socket_factory
    ),
    ne!(
        "Ljavax/net/ssl/SSLContext;",
        "init",
        "([Ljavax/net/ssl/KeyManager;[Ljavax/net/ssl/TrustManager;Ljava/security/SecureRandom;)V",
        true,
        ssl_context_init
    ),
];
