//! Unit tests for java.util.regex.Pattern / Matcher host shims.

use crate::context::{Context, SandboxOptions};
use crate::vm::native::java::util::regex::matcher::*;
use crate::vm::native::java::util::regex::pattern::*;
use crate::vm::native::*;

fn with_vm<T>(f: impl FnOnce(&mut Vm) -> T) -> T {
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let mut ctx = Context::new_with(&data, SandboxOptions::allow_all()).unwrap();
    f(ctx.vm())
}

fn s(vm: &mut Vm, x: &str) -> JValue {
    vm.alloc_string(x)
}

/// Decode a string result (native call runs first, then vm is reborrowed).
macro_rules! s_of {
    ($vm:expr, $call:expr) => {{
        let r = $call.unwrap();
        jstr($vm, r).unwrap()
    }};
}

/// Decode a Java String[] payload into Vec<String>.
fn str_arr_of(vm: &mut Vm, v: JValue) -> Vec<String> {
    let items: Vec<JValue> = match payload(vm, v) {
        Some(Native::Array(ArrayData::Obj(items))) => items.clone(),
        _ => panic!("expected Obj array payload"),
    };
    items.into_iter().map(|v| jstr(vm, v).unwrap()).collect()
}

fn bool_of(v: JValue) -> bool {
    match v {
        JValue::Int(i) => i != 0,
        other => panic!("expected int-bool, got {other:?}"),
    }
}

fn int_of(v: JValue) -> i32 {
    match v {
        JValue::Int(i) => i,
        other => panic!("expected int, got {other:?}"),
    }
}

fn pattern_of(vm: &mut Vm, src: &str) -> JValue {
    let re = Regex::new(src).unwrap();
    alloc(
        vm,
        "Ljava/util/regex/Pattern;",
        Native::Pattern {
            re,
            source: src.to_string(),
        },
    )
    .unwrap()
}

fn matcher_of(vm: &mut Vm, pat: JValue, text: &str) -> JValue {
    let src = s(vm, text);
    pattern_matcher(vm, &[pat, src]).unwrap()
}

#[test]
fn compile_matches_static() {
    with_vm(|vm| {
        // static Pattern.matches(regex, input): full-string match.
        let rx = s(vm, r"\d{2,4}");
        let input = s(vm, "2024");
        let yes = pattern_matches_static(vm, &[rx, input]).unwrap();
        assert!(bool_of(yes));

        let rx = s(vm, r"\d{2,4}");
        let input = s(vm, "20x24");
        let no = pattern_matches_static(vm, &[rx, input]).unwrap();
        assert!(!bool_of(no));
    });
}

#[test]
fn source_and_flags() {
    with_vm(|vm| {
        let p = pattern_of(vm, "[a-z]+");
        assert_eq!(s_of!(vm, pattern_source(vm, &[p])), "[a-z]+");
        assert_eq!(int_of(pattern_flags(vm, &[p]).unwrap()), 0);
    });
}

#[test]
fn matcher_matches_find_group() {
    with_vm(|vm| {
        let p = pattern_of(vm, r"(\d+)-(\d+)");
        let m = matcher_of(vm, p, "12-34");
        assert!(bool_of(matcher_matches(vm, &[m]).unwrap()));
        assert_eq!(int_of(matcher_group_count(vm, &[m]).unwrap()), 2);
        assert_eq!(s_of!(vm, matcher_group(vm, &[m])), "12-34");
        assert_eq!(
            s_of!(vm, matcher_group_n(vm, &[m, JValue::Int(1)])),
            "12"
        );
        assert_eq!(s_of!(vm, matcher_group_n(vm, &[m, JValue::Int(2)])), "34");
    });
}

#[test]
fn matcher_find_iterates() {
    with_vm(|vm| {
        let p = pattern_of(vm, r"\d+");
        let m = matcher_of(vm, p, "a 12 b 345 c");
        assert!(bool_of(matcher_find(vm, &[m]).unwrap()));
        assert_eq!(s_of!(vm, matcher_group(vm, &[m])), "12");
        assert!(bool_of(matcher_find(vm, &[m]).unwrap()));
        assert_eq!(s_of!(vm, matcher_group(vm, &[m])), "345");
        assert!(!bool_of(matcher_find(vm, &[m]).unwrap()));

        // start()/end() are byte offsets into the input.
        let p2 = pattern_of(vm, r"\d+");
        let m2 = matcher_of(vm, p2, "a 12 b 345 c");
        assert!(bool_of(matcher_find(vm, &[m2]).unwrap()));
        assert_eq!(int_of(matcher_start(vm, &[m2]).unwrap()), 2);
        assert_eq!(int_of(matcher_end(vm, &[m2]).unwrap()), 4);
    });
}

#[test]
fn matcher_replace() {
    with_vm(|vm| {
        let p = pattern_of(vm, r"\d+");
        let m = matcher_of(vm, p, "a1b22c333");
        let repl = s(vm, "#");
        assert_eq!(
            s_of!(vm, matcher_replace_all(vm, &[m, repl])),
            "a#b#c#"
        );

        let m2 = matcher_of(vm, p, "a1b22c333");
        let repl = s(vm, "#");
        assert_eq!(
            s_of!(vm, matcher_replace_first(vm, &[m2, repl])),
            "a#b22c333"
        );
    });
}

#[test]
fn matcher_reset_reuses_state() {
    with_vm(|vm| {
        let p = pattern_of(vm, r"x+");
        let m = matcher_of(vm, p, "xx");
        assert!(bool_of(matcher_find(vm, &[m]).unwrap()));

        // reset(seq) rebinds the input; find starts over from the beginning.
        let src = s(vm, "y y");
        matcher_reset_seq(vm, &[m, src]).unwrap();
        assert!(!bool_of(matcher_find(vm, &[m]).unwrap()));
    });
}

#[test]
fn split_and_quote() {
    with_vm(|vm| {
        let p = pattern_of(vm, r",\s*");
        let text = s(vm, "a, b,c, d");
        let arr = pattern_split_seq(vm, &[p, text]).unwrap();
        assert_eq!(str_arr_of(vm, arr), ["a", "b", "c", "d"]);

        // limit < 0 means "apply as many as possible" (Java semantics).
        let p2 = pattern_of(vm, ",");
        let text2 = s(vm, "a,b,c");
        let arr2 = pattern_split_seq_limit(vm, &[p2, text2, JValue::Int(-1)]).unwrap();
        assert_eq!(str_arr_of(vm, arr2), ["a", "b", "c"]);
        // limit = 2 keeps the remainder as the last element.
        let text3 = s(vm, "a,b,c");
        let arr3 = pattern_split_seq_limit(vm, &[p2, text3, JValue::Int(2)]).unwrap();
        assert_eq!(str_arr_of(vm, arr3), ["a", "b,c"]);

        // quote() escapes regex metacharacters.
        let src = s(vm, "a.b(c)");
        assert_eq!(s_of!(vm, pattern_quote(vm, &[src])), r"a\.b\(c\)");
    });
}

#[test]
fn bad_pattern_reports_error() {
    with_vm(|vm| {
        let rx = s(vm, "(");
        assert!(pattern_compile(vm, &[rx]).is_err());
    });
}
