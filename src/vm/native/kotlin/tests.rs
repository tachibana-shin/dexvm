//! Unit tests for the Kotlin stdlib host shims (kotlin.rs).

use super::*;
use crate::context::Context;
use crate::SandboxOptions;

fn with_vm<T>(f: impl FnOnce(&mut Vm) -> T) -> T {
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
    f(ctx.vm())
}

fn s(vm: &mut Vm, x: &str) -> JValue {
    vm.alloc_string(x)
}

/// Decode a string result (native runs first, then vm is reborrowed).
macro_rules! s_of {
    ($vm:expr, $call:expr) => {{
        let r = $call.unwrap();
        jstr($vm, r).unwrap()
    }};
}

fn int_of(v: JValue) -> i32 {
    match v {
        JValue::Int(i) => i,
        other => panic!("expected int, got {other:?}"),
    }
}

fn long_of(v: JValue) -> i64 {
    match v {
        JValue::Long(i) => i,
        other => panic!("expected long, got {other:?}"),
    }
}

fn bool_of(v: JValue) -> bool {
    match v {
        JValue::Int(i) => i != 0,
        other => panic!("expected int-bool, got {other:?}"),
    }
}

/// Decode a List payload into strings.
macro_rules! list_of {
    ($vm:expr, $call:expr) => {{
        let r = $call.unwrap();
        let items: Vec<JValue> = match payload($vm, r) {
            Some(Native::List(items)) => items.clone(),
            _ => panic!("expected List payload"),
        };
        items
            .into_iter()
            .map(|v| jstr($vm, v).unwrap())
            .collect::<Vec<String>>()
    }};
}

#[test]
fn pair_roundtrip() {
    with_vm(|vm| {
        let a = s(vm, "x");
        let b = s(vm, "y");
        let p = alloc(vm, "Lkotlin/Pair;", Native::Pair(a, b)).unwrap();
        assert_eq!(s_of!(vm, pair_get_first(vm, &[p])), "x");
        assert_eq!(s_of!(vm, pair_get_second(vm, &[p])), "y");
        assert_eq!(s_of!(vm, pair_get_first(vm, &[p])), "x");
    });
}

#[test]
fn tupled_to_makes_pair() {
    with_vm(|vm| {
        let a = s(vm, "left");
        let b = JValue::Int(42);
        let p = tupled_to(vm, &[a, b]).unwrap();
        assert_eq!(s_of!(vm, pair_get_first(vm, &[p])), "left");
        assert_eq!(int_of(pair_get_second(vm, &[p]).unwrap()), 42);
    });
}

#[test]
fn collections_basics() {
    with_vm(|vm| {
        // listOf() from a single element.
        let one = s(vm, "only");
        let l1 = collections_list_of_single(vm, &[one]).unwrap();
        assert_eq!(
            int_of(collections_size_or_default(vm, &[l1, JValue::Int(99)]).unwrap()),
            1
        );
        let hit = s(vm, "only");
        assert!(bool_of(
            collections_contains(vm, &[l1, hit]).unwrap()
        ));
        let miss = s(vm, "other");
        assert!(!bool_of(collections_contains(vm, &[l1, miss]).unwrap()));

        // emptyList().
        let e = kotlin_empty_list(vm, &[]).unwrap();
        match payload(vm, e) {
            Some(Native::List(items)) => assert!(items.is_empty()),
            _ => panic!("expected empty List payload"),
        }
    });
}

#[test]
fn collections_reversed_and_plus() {
    with_vm(|vm| {
        let a = s(vm, "a");
        let b = s(vm, "b");
        let c = s(vm, "c");
        let l = collections_list_of_single(vm, &[a]).unwrap();
        let l = collections_plus_obj(vm, &[l, b]).unwrap();
        let l = collections_plus_obj(vm, &[l, c]).unwrap();
        assert_eq!(
            list_of!(vm, collections_reversed(vm, &[l])),
            ["c", "b", "a"]
        );

        // plus(element) appends to a copy; plus(iterable) extends.
        let x = s(vm, "x");
        let l2 = collections_plus_obj(vm, &[l, x]).unwrap();
        let l2b = collections_plus_iterable(vm, &[l, l2]).unwrap();
        let items = match payload(vm, l2b) {
            Some(Native::List(items)) => items.clone(),
            _ => panic!("expected List payload"),
        };
        let strs: Vec<String> = items.into_iter().map(|v| jstr(vm, v).unwrap()).collect();
        assert_eq!(strs, ["a", "b", "c", "a", "b", "c", "x"]);
    });
}

#[test]
fn collections_first_empty_errors() {
    with_vm(|vm| {
        let head = s(vm, "head");
        let l = collections_list_of_single(vm, &[head]).unwrap();
        assert_eq!(s_of!(vm, collections_first(vm, &[l])), "head");
    });
}

#[test]
fn starts_with_default_mask() {
    with_vm(|vm| {
        // (str, prefix, ignoreCase, mask, marker).
        // mask=0: args[2] is honored; mask bit 2 (value 4): ignoreCase defaulted.
        let s1 = s(vm, "Hello");
        let p1 = s(vm, "he");
        let no = stringskt_starts_with_default(
            vm,
            &[s1, p1, JValue::Int(0), JValue::Int(0), JValue::Null],
        )
        .unwrap();
        assert!(
            !bool_of(no),
            "ignoreCase=false: case-sensitive startsWith is false"
        );

        let s2 = s(vm, "Hello");
        let p2 = s(vm, "he");
        let yes = stringskt_starts_with_default(
            vm,
            &[s2, p2, JValue::Int(1), JValue::Int(0), JValue::Null],
        )
        .unwrap();
        assert!(
            bool_of(yes),
            "ignoreCase=true: case-insensitive startsWith is true"
        );

        // ignoreCase provided but mask says it was defaulted -> false.
        let s3 = s(vm, "Hello");
        let p3 = s(vm, "he");
        let overridden = stringskt_starts_with_default(
            vm,
            &[s3, p3, JValue::Int(1), JValue::Int(4), JValue::Null],
        )
        .unwrap();
        assert!(
            !bool_of(overridden),
            "mask bit set: ignoreCase defaults to false"
        );
    });
}

#[test]
fn regex_matches_full_string() {
    with_vm(|vm| {
        // Kotlin Regex.matches requires the whole string to match.
        let p = alloc(
            vm,
            "Lkotlin/text/Regex;",
            Native::Pattern {
                re: Regex::new("").unwrap(),
                source: String::new(),
            },
        )
        .unwrap();
        let src = s(vm, r"\d{2,4}");
        regex_init(vm, &[p, src]).unwrap();

        let full = s(vm, "2024");
        assert!(bool_of(regex_matches(vm, &[p, full]).unwrap()));

        let partial = s(vm, "20x24");
        assert!(!bool_of(regex_matches(vm, &[p, partial]).unwrap()));

        assert_eq!(s_of!(vm, regex_to_string(vm, &[p])), r"\d{2,4}");
    });
}

#[test]
fn regex_kotlin_replace_all() {
    with_vm(|vm| {
        let p = alloc(
            vm,
            "Lkotlin/text/Regex;",
            Native::Pattern {
                re: Regex::new("").unwrap(),
                source: String::new(),
            },
        )
        .unwrap();
        let src = s(vm, r"o+");
        regex_init(vm, &[p, src]).unwrap();
        let text = s(vm, "foo booo");
        let repl = s(vm, "O");
        assert_eq!(s_of!(vm, regex_replace(vm, &[p, text, repl])), "fO bO");
    });
}

#[test]
fn duration_conversions() {
    with_vm(|vm| {
        // toDuration(5, MILLISECONDS) -> raw 5000; unit null -> default ms.
        let d = duration_to_duration_int(vm, &[JValue::Int(5), JValue::Null]).unwrap();
        assert_eq!(long_of(d), 5000);
        let d = duration_to_duration_long(vm, &[JValue::Long(7), JValue::Null]).unwrap();
        assert_eq!(long_of(d), 7000);
        assert_eq!(long_of(duration_get_zero(vm, &[]).unwrap()), 0);
    });
}
