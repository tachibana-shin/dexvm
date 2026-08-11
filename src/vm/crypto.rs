//! AES-256-GCM with JCE-compatible IV handling (NIST SP 800-38D).
//!
//! `javax.crypto` (Android/OpenJDK) accepts GCM IVs of any length; a non-12-byte
//! IV is hashed through GHASH into the initial counter block `J0`. The `aes-gcm`
//! crate only supports the fixed 12-byte nonce, so the full GHASH/CTR machinery
//! is implemented here on top of the raw AES block cipher.

use aes::cipher::{BlockEncrypt, KeyInit};
use aes::Aes256;

/// AES-256-GCM context.
pub struct Gcm {
    h: u128,
    cipher: Aes256,
}

fn mul(x: u128, y: u128) -> u128 {
    // GF(2^128) multiplication with the GCM polynomial
    // x^128 + x^7 + x^2 + x + 1.
    let mut z: u128 = 0;
    let mut v = y;
    for i in (0..128).rev() {
        if (x >> i) & 1 == 1 {
            z ^= v;
        }
        let lsb = v & 1;
        v >>= 1;
        if lsb == 1 {
            v ^= 0xe1_u128 << 120;
        }
    }
    z
}

/// GHASH over arbitrary-length input (the final partial block is zero-padded).
fn ghash(h: u128, data: &[u8]) -> u128 {
    let mut y: u128 = 0;
    for chunk in data.chunks(16) {
        let mut b = [0u8; 16];
        b[..chunk.len()].copy_from_slice(chunk);
        y = mul(y ^ u128::from_be_bytes(b), h);
    }
    y
}

fn len_bits(x: usize) -> [u8; 8] {
    (x as u64 * 8).to_be_bytes()
}

/// Increment the low 32 bits (SP 800-38D: `inc32`).
fn inc32(j0: u128) -> u128 {
    let lo = (j0 & 0xffff_ffff).wrapping_add(1) & 0xffff_ffff;
    (j0 & !0xffff_ffff) | lo
}

impl Gcm {
    pub fn new(secret: &[u8; 32]) -> Self {
        let cipher = Aes256::new_from_slice(secret).expect("32-byte AES key");
        let mut zero = [0u8; 16];
        cipher.encrypt_block((&mut zero).into());
        Gcm {
            h: u128::from_be_bytes(zero),
            cipher,
        }
    }

    /// The initial counter block `J0` (SP 800-38D, NIST SP 800-38D 7.1).
    fn j0(&self, iv: &[u8]) -> u128 {
        if iv.len() == 12 {
            let mut j = [0u8; 16];
            j[..12].copy_from_slice(iv);
            j[15] = 1;
            u128::from_be_bytes(j)
        } else {
            let pad = (16 - (iv.len() % 16)) % 16;
            let mut buf = Vec::with_capacity(iv.len() + pad + 8);
            buf.extend_from_slice(iv);
            buf.extend_from_slice(&[0u8; 16][..pad]);
            buf.extend_from_slice(&len_bits(iv.len()));
            ghash(self.h, &buf)
        }
    }

    fn block(&self, counter: u128) -> [u8; 16] {
        let mut c = counter.to_be_bytes();
        self.cipher.encrypt_block((&mut c).into());
        c
    }

    /// GCM tag over `aad` and `ct`, keyed by `iv` (`S ^ E_K(J0)`).
    fn tag(&self, iv: &[u8], aad: &[u8], ct: &[u8]) -> [u8; 16] {
        let aad_pad = (16 - (aad.len() % 16)) % 16;
        let ct_pad = (16 - (ct.len() % 16)) % 16;
        let mut buf = Vec::with_capacity(aad.len() + aad_pad + ct.len() + ct_pad + 16);
        buf.extend_from_slice(aad);
        buf.extend_from_slice(&[0u8; 16][..aad_pad]);
        buf.extend_from_slice(ct);
        buf.extend_from_slice(&[0u8; 16][..ct_pad]);
        buf.extend_from_slice(&len_bits(aad.len()));
        buf.extend_from_slice(&len_bits(ct.len()));
        let s = ghash(self.h, &buf);
        (s ^ u128::from_be_bytes(self.block(self.j0(iv)))).to_be_bytes()
    }

    /// Encrypt `plain` with `iv`/`aad`; returns ciphertext || GCM tag.
    pub fn seal(&self, iv: &[u8], aad: &[u8], plain: &[u8]) -> Vec<u8> {
        let mut counter = self.j0(iv);
        let mut outp = Vec::with_capacity(plain.len() + 16);
        for chunk in plain.chunks(16) {
            counter = inc32(counter);
            let mut ks = self.block(counter);
            for (o, p) in chunk.iter().enumerate() {
                ks[o] ^= p;
            }
            outp.extend_from_slice(&ks[..chunk.len()]);
        }
        outp.extend_from_slice(&self.tag(iv, aad, &outp));
        outp
    }

    /// Decrypt `data` (ciphertext || tag); returns the plaintext on tag match.
    pub fn open(&self, iv: &[u8], aad: &[u8], data: &[u8]) -> Result<Vec<u8>, GcmError> {
        if data.len() < 16 {
            return Err(GcmError::TooShort);
        }
        let (ct, tag) = data.split_at(data.len() - 16);
        let mut counter = self.j0(iv);
        let mut outp = Vec::with_capacity(ct.len());
        for chunk in ct.chunks(16) {
            counter = inc32(counter);
            let mut ks = self.block(counter);
            for (o, p) in chunk.iter().enumerate() {
                ks[o] ^= p;
            }
            outp.extend_from_slice(&ks[..chunk.len()]);
        }
        if self.tag(iv, aad, ct) == tag {
            Ok(outp)
        } else {
            Err(GcmError::TagMismatch)
        }
    }
}

/// GCM authentication/input errors.
#[derive(Debug)]
pub enum GcmError {
    /// Ciphertext shorter than the 16-byte tag.
    TooShort,
    /// Authentication tag did not match.
    TagMismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_byte_iv_roundtrip() {
        let g = Gcm::new(&[7u8; 32]);
        let iv = [3u8; 12];
        let ct = g.seal(&iv, b"aad", b"hello gcm");
        assert_eq!(g.open(&iv, b"aad", &ct).unwrap(), b"hello gcm");
    }

    #[test]
    fn long_iv_roundtrip() {
        let g = Gcm::new(&[9u8; 32]);
        let iv = [5u8; 25];
        assert_eq!(g.open(&iv, b"", &g.seal(&iv, b"", b"x")).unwrap(), b"x");
    }

    #[test]
    fn interop_with_aes_gcm_crate() {
        use aes_gcm::aead::{Aead, Payload};
        use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
        let secret = [0x42u8; 32];
        let iv = [0x11u8; 12];
        let plain = b"reference interop check payload";
        let aad = b"aad-data";
        let g = Gcm::new(&secret);
        let mine = g.seal(&iv, aad, plain);
        let reference = Aes256Gcm::new_from_slice(&secret).unwrap();
        let theirs = reference
            .encrypt(
                Nonce::from_slice(&iv),
                Payload {
                    msg: plain,
                    aad: aad.as_ref(),
                },
            )
            .unwrap();
        assert_eq!(mine, theirs);
        assert_eq!(g.open(&iv, aad, &theirs).unwrap(), plain);
        assert_eq!(
            reference
                .decrypt(
                    Nonce::from_slice(&iv),
                    Payload {
                        msg: &mine,
                        aad: aad.as_ref()
                    }
                )
                .unwrap(),
            plain
        );
    }

    #[test]
    fn tag_mismatch_fails() {
        let g = Gcm::new(&[1u8; 32]);
        let iv = [2u8; 12];
        let mut ct = g.seal(&iv, b"a", b"payload");
        let n = ct.len();
        ct[n - 1] ^= 1;
        assert!(g.open(&iv, b"a", &ct).is_err());
    }
}
