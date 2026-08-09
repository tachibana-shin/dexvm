//! java.util.LinkedHashSet host shims.

use crate::vm::native::*;

// LinkedHashSet shares the set_* impls with HashSet (see hashset.rs).

/// Native methods for Ljava/util/LinkedHashSet;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/util/LinkedHashSet;", "<init>", "()V", true, set_init),
    ne!(
        "Ljava/util/LinkedHashSet;",
        "<init>",
        "(I)V",
        true,
        set_init
    ),
    ne!(
        "Ljava/util/LinkedHashSet;",
        "<init>",
        "(Ljava/util/Collection;)V",
        true,
        set_init
    ),
    ne!("Ljava/util/LinkedHashSet;", "size", "()I", true, set_size),
    ne!(
        "Ljava/util/LinkedHashSet;",
        "isEmpty",
        "()Z",
        true,
        set_is_empty
    ),
    ne!(
        "Ljava/util/LinkedHashSet;",
        "contains",
        "(Ljava/lang/Object;)Z",
        true,
        set_contains
    ),
    ne!(
        "Ljava/util/LinkedHashSet;",
        "add",
        "(Ljava/lang/Object;)Z",
        true,
        set_add
    ),
    ne!(
        "Ljava/util/LinkedHashSet;",
        "remove",
        "(Ljava/lang/Object;)Z",
        true,
        set_remove
    ),
    ne!("Ljava/util/LinkedHashSet;", "clear", "()V", true, set_clear),
    ne!(
        "Ljava/util/LinkedHashSet;",
        "iterator",
        "()Ljava/util/Iterator;",
        true,
        set_iterator
    ),
    ne!(
        "Ljava/util/LinkedHashSet;",
        "addAll",
        "(Ljava/util/Collection;)Z",
        true,
        set_add_all
    ),
    ne!(
        "Ljava/util/LinkedHashSet;",
        "toString",
        "()Ljava/lang/String;",
        true,
        set_to_string
    ),
];
