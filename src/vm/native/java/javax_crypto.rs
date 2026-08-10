//! javax.crypto host shims: AES-256-GCM [`Cipher`] plus the argument holder
//! specs ([`SecretKeySpec`], [`GCMParameterSpec`]). Used by encrypted-image
//! formats such as MoeTruyen's IMGX payloads.

use super::*;

fn byte_array(vm: &mut Vm, bytes: Vec<u8>) -> Result<JValue, NatErr> {
    let data = bytes.into_iter().map(|b| b as i8).collect::<Vec<_>>();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

pub(crate) fn cipher_get_instance(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[0])?;
    if !name.eq_ignore_ascii_case("AES/GCM/NoPadding") {
        return Err(iae(
            vm,
            format!("Invalid transformation {name}, expected AES/GCM/NoPadding"),
        ));
    }
    alloc(
        vm,
        "Ljavax/crypto/Cipher;",
        Native::AesGcm {
            mode: 0,
            secret: [0u8; 32],
            iv: Vec::new(),
            tag_bits: 0,
            aad: Vec::new(),
        },
    )
}

pub(crate) fn secret_key_spec_init(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| iae(vm, "secret key bytes missing"))?;
    let _algo = jstr(vm, args[2])?;
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Key(bytes));
    Ok(JValue::Null)
}

pub(crate) fn gcm_parameter_spec_init(vm: &mut Vm, args: &[JValue]) -> R {
    let tag_bits = int_of(vm, args[1]);
    let iv = bytes_of(vm, args[2]).ok_or_else(|| iae(vm, "GCM IV bytes missing"))?;
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::GcmSpec { tag_bits, iv });
    Ok(JValue::Null)
}

pub(crate) fn cipher_init(vm: &mut Vm, args: &[JValue]) -> R {
    let mode = int_of(vm, args[1]);
    let key = match payload(vm, args[2]) {
        Some(Native::Key(k)) => k.clone(),
        _ => return Err(npe(vm)),
    };
    let (tag_bits, iv) = match payload(vm, args[3]) {
        Some(Native::GcmSpec { tag_bits, iv }) => (*tag_bits, iv.clone()),
        _ => return Err(npe(vm)),
    };
    let secret: [u8; 32] = key
        .try_into()
        .map_err(|_| iae(vm, "AES key must be 32 bytes"))?;
    let Some(Native::AesGcm {
        mode: m,
        secret: s,
        iv: i,
        tag_bits: t,
        aad,
    }) = payload_mut(vm, args[0])
    else {
        return Err(npe(vm));
    };
    *m = mode as u8;
    *s = secret;
    *i = iv;
    *t = tag_bits as usize;
    aad.clear();
    Ok(JValue::Null)
}

pub(crate) fn cipher_update_aad(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let Some(Native::AesGcm { aad, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    aad.extend_from_slice(&bytes);
    Ok(JValue::Null)
}

pub(crate) fn cipher_do_final(vm: &mut Vm, args: &[JValue]) -> R {
    let input = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let off = int_of(vm, args[2]).max(0) as usize;
    let len = int_of(vm, args[3]).max(0) as usize;
    let end = off.saturating_add(len).min(input.len());
    let (mode, secret, iv, _tag_bits, aad) = match payload(vm, args[0]) {
        Some(Native::AesGcm {
            mode,
            secret,
            iv,
            tag_bits,
            aad,
        }) => (*mode, *secret, iv.clone(), *tag_bits, aad.clone()),
        _ => return Err(npe(vm)),
    };
    if mode == 0 {
        return Err(iae(vm, "Cipher not initialized"));
    }
    let gcm = crate::vm::crypto::Gcm::new(&secret);
    let out = match mode {
        2 => gcm
            .open(&iv, &aad, &input[off..end])
            .map_err(|_| iae(vm, "GCM operation failed (bad key, tag or AAD)"))?,
        1 => gcm.seal(&iv, &aad, &input[off..end]),
        _ => return Err(iae(vm, "unsupported cipher mode")),
    };
    byte_array(vm, out)
}

pub(crate) const JAVAX_CRYPTO_TABLE: &[NativeEntry] = &[
    ne!("Ljavax/crypto/Cipher;", "getInstance", "(Ljava/lang/String;)Ljavax/crypto/Cipher;", true, cipher_get_instance),
    ne!("Ljavax/crypto/Cipher;", "init", "(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V", true, cipher_init),
    ne!("Ljavax/crypto/Cipher;", "updateAAD", "([B)V", true, cipher_update_aad),
    ne!("Ljavax/crypto/Cipher;", "doFinal", "([BII)[B", true, cipher_do_final),
    ne!("Ljavax/crypto/spec/SecretKeySpec;", "<init>", "([BLjava/lang/String;)V", true, secret_key_spec_init),
    ne!("Ljavax/crypto/spec/GCMParameterSpec;", "<init>", "(I[B)V", true, gcm_parameter_spec_init),
];