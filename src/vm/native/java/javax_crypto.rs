//! javax.crypto host shims: AES-GCM/CBC/ECB plus key and IV parameter specs.

use super::*;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::{Aes128, Aes192, Aes256};
use hmac::digest::KeyInit as HmacKeyInit;
use hmac::{Hmac, Mac as HmacMac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

fn byte_array(vm: &mut Vm, bytes: Vec<u8>) -> Result<JValue, NatErr> {
    let data = bytes.into_iter().map(|b| b as i8).collect::<Vec<_>>();
    alloc_arr(vm, "B", data.len(), move || ArrayData::Byte(data))
}

/// `char[]` -> UTF-8 bytes, matching how JCE password-based key derivation
/// treats a `PBEKeySpec` password.
fn chars_to_utf8(vm: &Vm, v: JValue) -> Option<Vec<u8>> {
    match payload(vm, v) {
        Some(Native::Array(ArrayData::Char(units))) => {
            Some(String::from_utf16_lossy(units).into_bytes())
        }
        _ => None,
    }
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

// ---- javax.crypto.spec.PBEKeySpec / SecretKeyFactory ----

fn pbe_key_spec_init(vm: &mut Vm, args: &[JValue]) -> R {
    let password = chars_to_utf8(vm, args[1]).unwrap_or_default();
    let salt = bytes_of(vm, args[2]).unwrap_or_default();
    let iterations = int_of(vm, args[3]);
    let key_len_bits = int_of(vm, args[4]);
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::PbeKeySpec {
        password,
        salt,
        iterations,
        key_len_bits,
    });
    Ok(JValue::Null)
}

fn secret_key_factory_get_instance(vm: &mut Vm, args: &[JValue]) -> R {
    let algo = jstr(vm, args[0])?;
    alloc(vm, "Ljavax/crypto/SecretKeyFactory;", Native::Str(algo))
}

fn secret_key_factory_generate_secret(vm: &mut Vm, args: &[JValue]) -> R {
    let algo = match payload(vm, args[0]) {
        Some(Native::Str(s)) => s.clone(),
        _ => return Err(npe(vm)),
    };
    let (password, salt, iterations, key_len_bits) = match payload(vm, args[1]) {
        Some(Native::PbeKeySpec {
            password,
            salt,
            iterations,
            key_len_bits,
        }) => (password.clone(), salt.clone(), *iterations, *key_len_bits),
        _ => return Err(iae(vm, "not a PBEKeySpec")),
    };
    let rounds = iterations.max(1) as u32;
    let out_len = (key_len_bits.max(8) as usize).div_ceil(8);
    let mut out = vec![0u8; out_len];
    let algo_upper = algo.to_ascii_uppercase();
    if algo_upper.contains("SHA256") {
        pbkdf2::pbkdf2_hmac::<Sha256>(&password, &salt, rounds, &mut out);
    } else if algo_upper.contains("SHA512") {
        pbkdf2::pbkdf2_hmac::<Sha512>(&password, &salt, rounds, &mut out);
    } else {
        pbkdf2::pbkdf2_hmac::<Sha1>(&password, &salt, rounds, &mut out);
    }
    alloc(vm, "Ljavax/crypto/spec/SecretKeySpec;", Native::Key(out))
}

// ---- javax.crypto.Mac ----

fn mac_get_instance(vm: &mut Vm, args: &[JValue]) -> R {
    let algorithm = jstr(vm, args[0])?;
    alloc(
        vm,
        "Ljavax/crypto/Mac;",
        Native::Mac {
            algorithm,
            key: Vec::new(),
        },
    )
}

fn mac_init(vm: &mut Vm, args: &[JValue]) -> R {
    let key = match payload(vm, args[1]) {
        Some(Native::Key(k)) => k.clone(),
        _ => return Err(npe(vm)),
    };
    let Some(Native::Mac { key: dst, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    *dst = key;
    Ok(JValue::Null)
}

fn mac_get_algorithm(vm: &mut Vm, args: &[JValue]) -> R {
    let algorithm = match payload(vm, args[0]) {
        Some(Native::Mac { algorithm, .. }) => algorithm.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &algorithm))
}

fn mac_do_final(vm: &mut Vm, args: &[JValue]) -> R {
    let input = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    let (algorithm, key) = match payload(vm, args[0]) {
        Some(Native::Mac { algorithm, key }) => (algorithm.clone(), key.clone()),
        _ => return Err(npe(vm)),
    };
    let algo_upper = algorithm.to_ascii_uppercase();
    let out = if algo_upper.contains("SHA256") {
        let mut mac = <Hmac<Sha256> as HmacKeyInit>::new_from_slice(&key)
            .map_err(|_| iae(vm, "invalid HMAC key"))?;
        mac.update(&input);
        mac.finalize().into_bytes().to_vec()
    } else if algo_upper.contains("SHA512") {
        let mut mac = <Hmac<Sha512> as HmacKeyInit>::new_from_slice(&key)
            .map_err(|_| iae(vm, "invalid HMAC key"))?;
        mac.update(&input);
        mac.finalize().into_bytes().to_vec()
    } else if algo_upper.contains("SHA1") {
        let mut mac = <Hmac<Sha1> as HmacKeyInit>::new_from_slice(&key)
            .map_err(|_| iae(vm, "invalid HMAC key"))?;
        mac.update(&input);
        mac.finalize().into_bytes().to_vec()
    } else if algo_upper.contains("MD5") {
        let mut mac = <Hmac<md5::Md5> as HmacKeyInit>::new_from_slice(&key)
            .map_err(|_| iae(vm, "invalid HMAC key"))?;
        mac.update(&input);
        mac.finalize().into_bytes().to_vec()
    } else {
        return Err(iae(vm, format!("unsupported Mac algorithm {algorithm}")));
    };
    byte_array(vm, out)
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
    ne!(
        "Ljavax/crypto/spec/PBEKeySpec;",
        "<init>",
        "([C[BII)V",
        true,
        pbe_key_spec_init
    ),
    ne!(
        "Ljavax/crypto/SecretKeyFactory;",
        "getInstance",
        "(Ljava/lang/String;)Ljavax/crypto/SecretKeyFactory;",
        false,
        secret_key_factory_get_instance
    ),
    ne!(
        "Ljavax/crypto/SecretKeyFactory;",
        "generateSecret",
        "(Ljava/security/spec/KeySpec;)Ljavax/crypto/SecretKey;",
        true,
        secret_key_factory_generate_secret
    ),
    ne!(
        "Ljavax/crypto/Mac;",
        "getInstance",
        "(Ljava/lang/String;)Ljavax/crypto/Mac;",
        false,
        mac_get_instance
    ),
    ne!(
        "Ljavax/crypto/Mac;",
        "init",
        "(Ljava/security/Key;)V",
        true,
        mac_init
    ),
    ne!(
        "Ljavax/crypto/Mac;",
        "getAlgorithm",
        "()Ljava/lang/String;",
        true,
        mac_get_algorithm
    ),
    ne!(
        "Ljavax/crypto/Mac;",
        "doFinal",
        "([B)[B",
        true,
        mac_do_final
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

    /// RFC 6070 PBKDF2-HMAC-SHA1 test vector 1.
    #[test]
    fn pbkdf2_hmac_sha1_matches_rfc6070_vector() {
        let mut out = [0u8; 20];
        pbkdf2::pbkdf2_hmac::<Sha1>(b"password", b"salt", 1, &mut out);
        assert_eq!(
            out,
            [
                0x0c, 0x60, 0xc8, 0x0f, 0x96, 0x1f, 0x0e, 0x71, 0xf3, 0xa9, 0xb5, 0x24, 0xaf, 0x60,
                0x12, 0x06, 0x2f, 0xe0, 0x37, 0xa6,
            ]
        );
    }

    /// RFC 4231 HMAC-SHA256 test case 1.
    #[test]
    fn hmac_sha256_matches_rfc4231_vector() {
        let key = [0x0bu8; 20];
        let mut mac = <Hmac<Sha256> as HmacKeyInit>::new_from_slice(&key).unwrap();
        mac.update(b"Hi There");
        let result = mac.finalize().into_bytes();
        assert_eq!(
            result.as_slice(),
            [
                0xb0, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b,
                0xf1, 0x2b, 0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c,
                0x2e, 0x32, 0xcf, 0xf7,
            ]
        );
    }
}
