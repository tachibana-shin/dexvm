//! Kotlin unsigned inline-class constructors.
use crate::vm::native::*;

fn identity(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Lkotlin/UInt;", "constructor-impl", "(I)I", false, identity),
    ne!(
        "Lkotlin/UByte;",
        "constructor-impl",
        "(B)B",
        false,
        identity
    ),
];
