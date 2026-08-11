//! javax.crypto host shims: AES-GCM/CBC/ECB plus key and IV parameter specs.

use super::*;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::{Aes128, Aes192, Aes256};

fn byte_array(vm: &mut Vm, bytes: Vec<u8>) -> Result<JValue, NatErr> {
    let data = bytes.into_iter().map(|b| b as i8).collect::<Vec<_>>();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

pub(crate) fn cipher_get_instance(vm: &mut Vm, args: &[JValue]) -> R {
    let name = jstr(vm, args[0])?.to_ascii_uppercase();
    let transformation = if name == "AES" {
        "AES/ECB/PKCS5PADDING".to_string()
    } else {
        name
    };
    if !matches!(
        transformation.as_str(),
        "AES/GCM/NOPADDING"
            | "AES/CBC/PKCS5PADDING"
            | "AES/CBC/PKCS7PADDING"
            | "AES/CBC/NOPADDING"
            | "AES/ECB/PKCS5PADDING"
            | "AES/ECB/PKCS7PADDING"
            | "AES/ECB/NOPADDING"
    ) {
        return Err(iae(
            vm,
            format!("unsupported Cipher transformation {transformation}"),
        ));
    }
    alloc(
        vm,
        "Ljavax/crypto/Cipher;",
        Native::CipherState {
            transformation,
            mode: 0,
            key: Vec::new(),
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

fn iv_parameter_spec_init(vm: &mut Vm, args: &[JValue]) -> R {
    let iv = bytes_of(vm, args[1]).ok_or_else(|| iae(vm, "IV bytes missing"))?;
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::GcmSpec { tag_bits: 0, iv });
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
    if !matches!(key.len(), 16 | 24 | 32) {
        return Err(iae(vm, "AES key must be 16, 24 or 32 bytes"));
    }
    let Some(Native::CipherState {
        mode: m,
        key: stored_key,
        iv: i,
        tag_bits: t,
        aad,
        ..
    }) = payload_mut(vm, args[0])
    else {
        return Err(npe(vm));
    };
    *m = mode as u8;
    *stored_key = key;
    *i = iv;
    *t = tag_bits as usize;
    aad.clear();
    Ok(JValue::Null)
}

fn cipher_init_key(vm: &mut Vm, args: &[JValue]) -> R {
    let mode = int_of(vm, args[1]);
    let key = match payload(vm, args[2]) {
        Some(Native::Key(key)) => key.clone(),
        _ => return Err(npe(vm)),
    };
    if !matches!(key.len(), 16 | 24 | 32) {
        return Err(iae(vm, "AES key must be 16, 24 or 32 bytes"));
    }
    match payload_mut(vm, args[0]) {
        Some(Native::CipherState {
            transformation,
            mode: stored_mode,
            key: stored_key,
            iv,
            aad,
            ..
        }) if transformation.contains("/ECB/") => {
            *stored_mode = mode as u8;
            *stored_key = key;
            iv.clear();
            aad.clear();
            Ok(JValue::Null)
        }
        Some(Native::CipherState { .. }) => Err(iae(vm, "Cipher mode requires parameters")),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn cipher_update_aad(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let Some(Native::CipherState { aad, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    aad.extend_from_slice(&bytes);
    Ok(JValue::Null)
}

fn crypt_block(key: &[u8], block: &mut [u8; 16], encrypt: bool) -> Result<(), ()> {
    macro_rules! crypt {
        ($cipher:ty) => {{
            let cipher = <$cipher>::new_from_slice(key).map_err(|_| ())?;
            let block = aes::cipher::generic_array::GenericArray::from_mut_slice(block);
            if encrypt {
                cipher.encrypt_block(block);
            } else {
                cipher.decrypt_block(block);
            }
        }};
    }
    match key.len() {
        16 => crypt!(Aes128),
        24 => crypt!(Aes192),
        32 => crypt!(Aes256),
        _ => return Err(()),
    }
    Ok(())
}

fn aes_block_mode(
    transformation: &str,
    mode: u8,
    key: &[u8],
    iv: &[u8],
    input: &[u8],
) -> Result<Vec<u8>, &'static str> {
    let cbc = transformation.contains("/CBC/");
    let padded =
        transformation.ends_with("PKCS5PADDING") || transformation.ends_with("PKCS7PADDING");
    let mut data = input.to_vec();
    if mode == 1 && padded {
        let padding = 16 - data.len() % 16;
        data.extend(std::iter::repeat_n(padding as u8, padding));
    }
    if !data.len().is_multiple_of(16) {
        return Err("AES input is not block-aligned");
    }
    if cbc && iv.len() != 16 {
        return Err("AES-CBC IV must be 16 bytes");
    }

    let mut out = Vec::with_capacity(data.len());
    let mut chain = if cbc { iv.to_vec() } else { vec![0; 16] };
    for chunk in data.chunks_exact(16) {
        let mut block: [u8; 16] = chunk.try_into().map_err(|_| "bad AES block")?;
        if mode == 1 {
            if cbc {
                for (byte, previous) in block.iter_mut().zip(&chain) {
                    *byte ^= previous;
                }
            }
            crypt_block(key, &mut block, true).map_err(|_| "invalid AES key")?;
            if cbc {
                chain.copy_from_slice(&block);
            }
        } else if mode == 2 {
            let ciphertext = block;
            crypt_block(key, &mut block, false).map_err(|_| "invalid AES key")?;
            if cbc {
                for (byte, previous) in block.iter_mut().zip(&chain) {
                    *byte ^= previous;
                }
                chain.copy_from_slice(&ciphertext);
            }
        } else {
            return Err("unsupported Cipher mode");
        }
        out.extend_from_slice(&block);
    }
    if mode == 2 && padded {
        let padding = out.last().copied().ok_or("invalid PKCS padding")? as usize;
        if padding == 0
            || padding > 16
            || padding > out.len()
            || !out[out.len() - padding..]
                .iter()
                .all(|byte| usize::from(*byte) == padding)
        {
            return Err("invalid PKCS padding");
        }
        out.truncate(out.len() - padding);
    }
    Ok(out)
}

pub(crate) fn cipher_do_final(vm: &mut Vm, args: &[JValue]) -> R {
    let input = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let off = int_of(vm, args[2]).max(0) as usize;
    let len = int_of(vm, args[3]).max(0) as usize;
    let end = off.saturating_add(len).min(input.len());
    let (transformation, mode, key, iv, _tag_bits, aad) = match payload(vm, args[0]) {
        Some(Native::CipherState {
            transformation,
            mode,
            key,
            iv,
            tag_bits,
            aad,
        }) => (
            transformation.clone(),
            *mode,
            key.clone(),
            iv.clone(),
            *tag_bits,
            aad.clone(),
        ),
        _ => return Err(npe(vm)),
    };
    if mode == 0 {
        return Err(iae(vm, "Cipher not initialized"));
    }
    let out = if transformation == "AES/GCM/NOPADDING" {
        let secret: [u8; 32] = key
            .try_into()
            .map_err(|_| iae(vm, "AES-GCM currently requires a 32-byte key"))?;
        let gcm = crate::vm::crypto::Gcm::new(&secret);
        match mode {
            2 => gcm
                .open(&iv, &aad, &input[off..end])
                .map_err(|_| iae(vm, "GCM operation failed (bad key, tag or AAD)"))?,
            1 => gcm.seal(&iv, &aad, &input[off..end]),
            _ => return Err(iae(vm, "unsupported cipher mode")),
        }
    } else {
        aes_block_mode(&transformation, mode, &key, &iv, &input[off..end])
            .map_err(|message| iae(vm, message))?
    };
    byte_array(vm, out)
}

fn cipher_do_final_all(vm: &mut Vm, args: &[JValue]) -> R {
    let len = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?.len() as i32;
    cipher_do_final(vm, &[args[0], args[1], JValue::Int(0), JValue::Int(len)])
}

pub(crate) const JAVAX_CRYPTO_TABLE: &[NativeEntry] = &[
    ne!(
        "Ljavax/crypto/Cipher;",
        "getInstance",
        "(Ljava/lang/String;)Ljavax/crypto/Cipher;",
        false,
        cipher_get_instance
    ),
    ne!(
        "Ljavax/crypto/Cipher;",
        "init",
        "(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V",
        true,
        cipher_init
    ),
    ne!(
        "Ljavax/crypto/Cipher;",
        "init",
        "(ILjava/security/Key;)V",
        true,
        cipher_init_key
    ),
    ne!(
        "Ljavax/crypto/Cipher;",
        "updateAAD",
        "([B)V",
        true,
        cipher_update_aad
    ),
    ne!(
        "Ljavax/crypto/Cipher;",
        "doFinal",
        "([BII)[B",
        true,
        cipher_do_final
    ),
    ne!(
        "Ljavax/crypto/Cipher;",
        "doFinal",
        "([B)[B",
        true,
        cipher_do_final_all
    ),
    ne!(
        "Ljavax/crypto/spec/SecretKeySpec;",
        "<init>",
        "([BLjava/lang/String;)V",
        true,
        secret_key_spec_init
    ),
    ne!(
        "Ljavax/crypto/spec/GCMParameterSpec;",
        "<init>",
        "(I[B)V",
        true,
        gcm_parameter_spec_init
    ),
    ne!(
        "Ljavax/crypto/spec/IvParameterSpec;",
        "<init>",
        "([B)V",
        true,
        iv_parameter_spec_init
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_128_cbc_matches_nist_vector_and_pkcs_roundtrips() {
        let key = [
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let iv = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ];
        let plaintext = [
            0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93,
            0x17, 0x2a,
        ];
        let expected = [
            0x76, 0x49, 0xab, 0xac, 0x81, 0x19, 0xb2, 0x46, 0xce, 0xe9, 0x8e, 0x9b, 0x12, 0xe9,
            0x19, 0x7d,
        ];
        assert_eq!(
            aes_block_mode("AES/CBC/NOPADDING", 1, &key, &iv, &plaintext).unwrap(),
            expected
        );

        let message = b"extension image payload";
        let encrypted = aes_block_mode("AES/CBC/PKCS5PADDING", 1, &key, &iv, message).unwrap();
        assert_eq!(
            aes_block_mode("AES/CBC/PKCS5PADDING", 2, &key, &iv, &encrypted).unwrap(),
            message
        );
    }
}
