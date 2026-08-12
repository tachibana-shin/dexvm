//! java.net.InetAddress host shim: no real DNS resolution (network host
//! lookups already happen inside the okhttp/okio layer), just a hostname
//! holder good enough for code that stores/logs/compares it.

use crate::vm::native::*;

fn get_by_name(vm: &mut Vm, args: &[JValue]) -> R {
    let host = jstr(vm, args[0])?;
    alloc(vm, "Ljava/net/InetAddress;", Native::Str(host))
}

fn get_host_name(vm: &mut Vm, args: &[JValue]) -> R {
    let host = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &host))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/net/InetAddress;",
        "getByName",
        "(Ljava/lang/String;)Ljava/net/InetAddress;",
        false,
        get_by_name
    ),
    ne!(
        "Ljava/net/InetAddress;",
        "getHostName",
        "()Ljava/lang/String;",
        true,
        get_host_name
    ),
    ne!(
        "Ljava/net/InetAddress;",
        "getHostAddress",
        "()Ljava/lang/String;",
        true,
        get_host_name
    ),
];
