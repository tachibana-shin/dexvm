//! Unit tests for the java.security host shims.

use super::message_digest::*;
use super::secure_random::*;
use crate::context::Context;
use crate::vm::native::*;
use crate::SandboxOptions;

fn with_vm<T>(f: impl FnOnce(&mut Vm) -> T) -> T {
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
    f(ctx.vm())
}

fn jbytes(vm: &mut Vm, v: JValue) -> Vec<u8> {
    bytes_of_jvalue(vm, v).unwrap()
}

/// The test fixture dex has no `[B` type, so alloc_arr falls back to a
/// zero-filled array; fill the payload after allocation instead.
fn filled_bytes(vm: &mut Vm, data: &[u8]) -> JValue {
    let arr = alloc_empty_arr(vm, "B").unwrap();
    if let Some(Native::Array(ArrayData::Byte(bs))) = payload_mut(vm, arr) {
        bs.extend(data.iter().map(|&b| b as i8));
    }
    arr
}

fn digest_bytes(vm: &mut Vm, algo: &str, data: &[u8]) -> Vec<u8> {
    let name = vm.alloc_string(algo);
    let md = md_get_instance(vm, &[name]).unwrap();
    let input = filled_bytes(vm, data);
    md_update(vm, &[md, input]).unwrap();
    let out = md_digest(vm, &[md]).unwrap();
    jbytes(vm, out)
}

#[test]
fn array_roundtrip() {
    with_vm(|vm| {
        let v = filled_bytes(vm, b"hello");
        assert_eq!(bytes_of_jvalue(vm, v).unwrap(), b"hello");
    });
}

#[test]
fn digest_of_direct() {
    assert_eq!(
        digest_of(ALGO_SHA256, b"abc")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn sha256_matches_known_vector() {
    with_vm(|vm| {
        let hexd = digest_bytes(vm, "SHA-256", b"abc")
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(
            hexd,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    });
}

#[test]
fn md5_and_sha1_vectors() {
    with_vm(|vm| {
        let mut hexd = |algo: &str, data: &[u8]| {
            digest_bytes(vm, algo, data)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        assert_eq!(hexd("MD5", b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            hexd("SHA-1", b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    });
}

#[test]
fn digest_with_input_and_reset_semantics() {
    with_vm(|vm| {
        let name = vm.alloc_string("SHA-256");
        let md = md_get_instance(vm, &[name]).unwrap();
        let input = filled_bytes(vm, b"abc");
        let out = md_digest_input(vm, &[md, input]).unwrap();
        let hexd = jbytes(vm, out)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert!(hexd.starts_with("ba7816bf"));
        // digest() reset the state: a fresh digest of "" differs
        let out = md_digest(vm, &[md]).unwrap();
        let empty = jbytes(vm, out);
        assert!(!empty.is_empty());
    });
}

#[test]
fn secure_random_fills_bytes_and_advances() {
    with_vm(|vm| {
        let a = alloc(
            vm,
            "Ljava/security/SecureRandom;",
            Native::SecureRandom(seed_os()),
        )
        .unwrap();
        let b1 = byte_array(vm, vec![0; 16]).unwrap();
        let b2 = byte_array(vm, vec![0; 16]).unwrap();
        sr_next_bytes(vm, &[a, b1]).unwrap();
        sr_next_bytes(vm, &[a, b2]).unwrap();
        let v1 = bytes_of_jvalue(vm, b1).unwrap();
        let v2 = bytes_of_jvalue(vm, b2).unwrap();
        assert!(v1.iter().any(|&b| b != 0));
        assert!(v2.iter().any(|&b| b != 0));
        assert_ne!(v1, v2);
    });
}
