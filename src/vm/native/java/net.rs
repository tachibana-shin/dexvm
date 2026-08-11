//! java.net host shims.

use crate::vm::native::*;

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/net/URI;",
        "<init>",
        "(Ljava/lang/String;)V",
        true,
        crate::vm::native::kotlin::uri_init
    ),
    ne!(
        "Ljava/net/URI;",
        "getHost",
        "()Ljava/lang/String;",
        true,
        crate::vm::native::kotlin::uri_get_host
    ),
];
