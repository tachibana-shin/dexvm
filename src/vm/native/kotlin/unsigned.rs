//! Kotlin unsigned inline-class constructors.
use crate::vm::native::*;

fn identity(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[0])
}
fn uint_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let v = int_of(vm, args[0]) as u32;
    Ok(new_str(vm, &v.to_string()))
}
fn uint_hash_code(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(int_of(vm, args[0])))
}
fn uint_to_double(vm: &mut Vm, args: &[JValue]) -> R {
    let v = int_of(vm, args[0]) as u32;
    Ok(JValue::Double(f64::from(v)))
}
fn uint_array_get(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Array(ArrayData::Int(v))) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    let i = int_of(vm, args[1]);
    match v.get(i as usize) {
        Some(&x) => Ok(JValue::Int(x)),
        None => Err(ioobe(vm, i)),
    }
}
fn uint_array_size(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Array(ArrayData::Int(v))) = payload(vm, args[0]) else {
        return Err(npe(vm));
    };
    Ok(JValue::Int(v.len() as i32))
}
fn next_ubytes(vm: &mut Vm, args: &[JValue]) -> R {
    let n = int_of(vm, args[1]).max(0) as usize;
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let mut h = RandomState::new().build_hasher();
        h.write_usize(out.len());
        h.write_u64(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
        );
        out.extend_from_slice(&h.finish().to_le_bytes());
    }
    out.truncate(n);
    let data: Vec<i8> = out.iter().map(|&b| b as i8).collect();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Lkotlin/UInt;", "constructor-impl", "(I)I", false, identity),
    ne!("Lkotlin/UInt;", "box-impl", "(I)Lkotlin/UInt;", false, identity),
    ne!("Lkotlin/UInt;", "unbox-impl", "()I", true, identity),
    ne!("Lkotlin/UInt;", "toString-impl", "(I)Ljava/lang/String;", false, uint_to_string),
    ne!("Lkotlin/UInt;", "hashCode-impl", "(I)I", false, uint_hash_code),
    ne!("Lkotlin/ULong;", "constructor-impl", "(J)J", false, identity),
    ne!("Lkotlin/UnsignedKt;", "uintToDouble", "(I)D", false, uint_to_double),
    ne!(
        "Lkotlin/UByte;",
        "constructor-impl",
        "(B)B",
        false,
        identity
    ),
    ne!("Lkotlin/UByte;", "unbox-impl", "()B", true, identity),
    ne!("Lkotlin/UByteArray;", "box-impl", "([B)Lkotlin/UByteArray;", false, identity),
    ne!("Lkotlin/UIntArray;", "get-pVg5ArA", "([II)I", false, uint_array_get),
    ne!("Lkotlin/UIntArray;", "getSize-impl", "([I)I", false, uint_array_size),
    ne!("Lkotlin/random/URandomKt;", "nextUBytes", "(Lkotlin/random/Random;I)[B", false, next_ubytes),
];
