//! Kotlin sequence bridge registrations.
use crate::vm::native::*;

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "map",
        "(Lkotlin/sequences/Sequence;Lkotlin/jvm/functions/Function1;)Lkotlin/sequences/Sequence;",
        false,
        super::sequence_map
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "filter",
        "(Lkotlin/sequences/Sequence;Lkotlin/jvm/functions/Function1;)Lkotlin/sequences/Sequence;",
        false,
        super::sequence_filter
    ),
    ne!(
        "Lkotlin/sequences/SequencesKt;",
        "toList",
        "(Lkotlin/sequences/Sequence;)Ljava/util/List;",
        false,
        super::sequence_to_list
    ),
];
