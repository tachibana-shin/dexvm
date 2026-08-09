//! java.util.ListIterator host shims.

use crate::vm::native::*;

// ListIterator shares the iter_* impls with Iterator (see iterator.rs).

/// Native methods for Ljava/util/ListIterator;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/ListIterator;",
        "hasNext",
        "()Z",
        true,
        iter_has_next
    ),
    ne!(
        "Ljava/util/ListIterator;",
        "next",
        "()Ljava/lang/Object;",
        true,
        iter_next
    ),
    ne!(
        "Ljava/util/ListIterator;",
        "remove",
        "()V",
        true,
        iter_remove
    ),
];
