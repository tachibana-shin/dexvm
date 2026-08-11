//! Minimal java.text.Collator comparator bridge.

use crate::vm::native::*;

fn collator_get_instance(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Ljava/text/Collator;", Native::Opaque)
}

fn collator_compare(vm: &mut Vm, args: &[JValue]) -> R {
    let left = charseq_of(vm, args[1])?;
    let right = charseq_of(vm, args[2])?;
    Ok(JValue::Int(left.cmp(&right) as i32))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/text/Collator;",
        "getInstance",
        "(Ljava/util/Locale;)Ljava/text/Collator;",
        false,
        collator_get_instance
    ),
    ne!(
        "Ljava/text/Collator;",
        "getInstance",
        "()Ljava/text/Collator;",
        false,
        collator_get_instance
    ),
    ne!(
        "Ljava/text/Collator;",
        "compare",
        "(Ljava/lang/String;Ljava/lang/String;)I",
        true,
        collator_compare
    ),
    ne!(
        "Ljava/text/Collator;",
        "compare",
        "(Ljava/lang/Object;Ljava/lang/Object;)I",
        true,
        collator_compare
    ),
];
