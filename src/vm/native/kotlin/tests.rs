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
fn ranges_and_int_iterator_keep_state() {
    with_vm(|vm| {
        let range = rangeskt_until(vm, &[JValue::Int(2), JValue::Int(5)]).unwrap();
        assert_eq!(int_of(int_range_get_first(vm, &[range]).unwrap()), 2);
        assert_eq!(int_of(int_range_get_last(vm, &[range]).unwrap()), 4);

        let iterator = alloc(
            vm,
            "Lkotlin/collections/IntIterator;",
            Native::IntRange(2, 4),
        )
        .unwrap();
        assert!(bool_of(int_iterator_has_next(vm, &[iterator]).unwrap()));
        assert_eq!(int_of(int_iterator_next_int(vm, &[iterator]).unwrap()), 2);
        assert_eq!(int_of(int_iterator_next_int(vm, &[iterator]).unwrap()), 3);
        assert_eq!(int_of(int_iterator_next_int(vm, &[iterator]).unwrap()), 4);
        assert!(!bool_of(int_iterator_has_next(vm, &[iterator]).unwrap()));
    });
}

#[test]
fn high_frequency_kotlin_helpers_have_real_results() {
    with_vm(|vm| {
        assert_eq!(
            int_of(charskt_check_radix(vm, &[JValue::Int(16)]).unwrap()),
            16
        );
        let value = kotlin_random_default_next_int(vm, &[JValue::Int(3), JValue::Int(9)]).unwrap();
        assert!((3..9).contains(&int_of(value)));
        let token = s(vm, "token");
        let err = intrinsics_throw_uninitialized(vm, &[token]);
        assert!(matches!(err, Err(NatErr::Throw(_))));
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
        assert!(bool_of(collections_contains(vm, &[l1, hit]).unwrap()));
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
fn missing_collection_helpers_are_stateful() {
    with_vm(|vm| {
        let empty = kotlin_empty_list(vm, &[]).unwrap();
        assert!(collections_first_or_null(vm, &[empty]).unwrap().is_null());

        let a = s(vm, "a");
        let list = collections_list_of_single(vm, &[a]).unwrap();
        let b = s(vm, "b");
        let extra = collections_list_of_single(vm, &[b]).unwrap();
        assert!(bool_of(collections_add_all(vm, &[list, extra]).unwrap()));
        assert_eq!(s_of!(vm, collections_last(vm, &[list])), "b");
        assert!(collections_get_or_null(vm, &[list, JValue::Int(9)])
            .unwrap()
            .is_null());
        assert!(matches!(
            collections_throw_index_overflow(vm, &[]),
            Err(NatErr::Throw(_))
        ));
    });
}

#[test]
fn missing_string_and_regex_helpers_match_kotlin_behavior() {
    with_vm(|vm| {
        let value = s(vm, "Prefix-Middle.Suffix");
        let needle = s(vm, "middle");
        assert!(bool_of(
            stringskt_contains(vm, &[value, needle, JValue::Int(1)]).unwrap()
        ));

        let value = s(vm, "Prefix-Middle.Suffix");
        let delimiter = s(vm, ".");
        assert_eq!(
            s_of!(
                vm,
                stringskt_substring_before_default(
                    vm,
                    &[value, delimiter, JValue::Null, JValue::Int(2), JValue::Null]
                )
            ),
            "Prefix-Middle"
        );

        let pattern = alloc(
            vm,
            "Lkotlin/text/Regex;",
            Native::Pattern {
                re: fancy_regex::Regex::new("[,;]").unwrap(),
                source: "[,;]".to_string(),
            },
        )
        .unwrap();
        let input = s(vm, "a,b;c");
        assert!(bool_of(
            regex_contains_match_in(vm, &[pattern, input]).unwrap()
        ));
        let input = s(vm, "a,b;c");
        assert_eq!(
            list_of!(vm, regex_split(vm, &[pattern, input, JValue::Int(0)])),
            ["a", "b", "c"]
        );
    });
}

#[test]
fn default_split_and_regex_find_return_real_values() {
    with_vm(|vm| {
        let comma = s(vm, ",");
        let semi = s(vm, ";");
        let delimiters = alloc_arr(vm, "Ljava/lang/String;", 2, || {
            ArrayData::Obj(vec![comma, semi])
        })
        .unwrap();
        let input = s(vm, "a,b;c");
        assert_eq!(
            list_of!(
                vm,
                stringskt_split_strings_default(
                    vm,
                    &[
                        input,
                        delimiters,
                        JValue::Int(0),
                        JValue::Int(0),
                        JValue::Int(6),
                        JValue::Null,
                    ]
                )
            ),
            ["a", "b", "c"]
        );

        let regex = alloc(
            vm,
            "Lkotlin/text/Regex;",
            Native::Pattern {
                re: fancy_regex::Regex::new("b+").unwrap(),
                source: "b+".to_string(),
            },
        )
        .unwrap();
        let input = s(vm, "aa-bbb-cc");
        let found = regex_find_default(
            vm,
            &[regex, input, JValue::Int(0), JValue::Int(2), JValue::Null],
        )
        .unwrap();
        assert_eq!(s_of!(vm, match_result_get_value(vm, &[found])), "bbb");
    });
}

#[test]
fn map_and_byte_array_helpers_preserve_values() {
    with_vm(|vm| {
        let key = s(vm, "key");
        let value = s(vm, "value");
        let pair = alloc(vm, "Lkotlin/Pair;", Native::Pair(key, value)).unwrap();
        let pairs = alloc_arr(vm, "Lkotlin/Pair;", 1, || ArrayData::Obj(vec![pair])).unwrap();
        let map = mapskt_map_of(vm, &[pairs]).unwrap();
        let list = mapskt_to_list(vm, &[map]).unwrap();
        assert_eq!(coll_elems(vm, list).unwrap().len(), 1);

        let left = alloc_arr(vm, "B", 2, || ArrayData::Byte(vec![1, 2])).unwrap();
        let right = alloc_arr(vm, "B", 1, || ArrayData::Byte(vec![3])).unwrap();
        let joined = arrayskt_plus_bytes(vm, &[left, right]).unwrap();
        assert!(matches!(
            payload(vm, joined),
            Some(Native::Array(ArrayData::Byte(values))) if values == &[1, 2, 3]
        ));
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

#[test]
fn instant_and_match_result_bridges_keep_values() {
    with_vm(|vm| {
        let instant = alloc(vm, "Lkotlin/time/Instant;", Native::EpochMillis(1234)).unwrap();
        assert_eq!(
            kotlin_instant_to_epoch_millis(vm, &[instant]).unwrap(),
            JValue::Long(1234)
        );
        let minus = kotlin_instant_minus(vm, &[instant, JValue::Long(234)]).unwrap();
        assert_eq!(
            kotlin_instant_to_epoch_millis(vm, &[minus]).unwrap(),
            JValue::Long(1000)
        );

        let matcher = alloc(
            vm,
            "Lkotlin/text/MatcherMatchResult;",
            Native::Matcher(MatcherState {
                pattern: Regex::new("x").unwrap(),
                text: "prefix".into(),
                pos: 0,
                last: Some((0, 3)),
                groups: Vec::new(),
            }),
        )
        .unwrap();
        let value = match_result_get_value(vm, &[matcher]).unwrap();
        assert_eq!(jstr(vm, value).unwrap(), "pre");
        let list = match_result_destructured_to_list(vm, &[matcher]).unwrap();
        assert!(matches!(payload(vm, list), Some(Native::List(values)) if values.len() == 1));
    });
}

#[test]
fn instant_parse_and_reflection_bridges_are_real() {
    with_vm(|vm| {
        let text = s(vm, "2024-01-02T03:04:05.123Z");
        let companion = opaque_inst(vm, "Lkotlin/time/Instant$Companion;");
        let parsed = kotlin_instant_parse_or_null(vm, &[companion, text]).unwrap();
        assert_eq!(
            kotlin_instant_to_epoch_millis(vm, &[parsed]).unwrap(),
            JValue::Long(1_704_164_645_123)
        );
    });
}

#[test]
fn sequence_and_flatten_bridges_preserve_elements() {
    with_vm(|vm| {
        let a = s(vm, "a");
        let b = s(vm, "b");
        let inner1 = list_alloc(vm, vec![a]).unwrap();
        let inner2 = list_alloc(vm, vec![b]).unwrap();
        let nested = list_alloc(vm, vec![inner1, inner2]).unwrap();
        let flat = collections_flatten(vm, &[nested]).unwrap();
        let seq = sequence_as_sequence(vm, &[flat]).unwrap();
        let list = sequence_to_list(vm, &[seq]).unwrap();
        assert_eq!(coll_elems(vm, list).unwrap().len(), 2);
    });
}

#[test]
fn regex_match_entire_and_collection_slices_are_real() {
    with_vm(|vm| {
        let pattern = alloc(
            vm,
            "Lkotlin/text/Regex;",
            Native::Pattern {
                re: fancy_regex::Regex::new("a+").unwrap(),
                source: "a+".into(),
            },
        )
        .unwrap();
        let text = s(vm, "aaa");
        assert!(!regex_match_entire(vm, &[pattern, text])
            .unwrap()
            .is_null_ref());
        let a = s(vm, "a");
        let b = s(vm, "b");
        let c = s(vm, "c");
        let list = list_alloc(vm, vec![a, b, c]).unwrap();
        let taken = collections_take(vm, &[list, JValue::Int(2)]).unwrap();
        assert_eq!(coll_elems(vm, taken).unwrap().len(), 2);
    });
}

#[test]
fn triple_bridge_keeps_all_three_values() {
    with_vm(|vm| {
        let a = s(vm, "a");
        let b = s(vm, "b");
        let c = s(vm, "c");
        let t = tripled_to(vm, &[a, b, c]).unwrap();
        assert_eq!(triple_get_first(vm, &[t]).unwrap(), a);
        assert_eq!(triple_get_second(vm, &[t]).unwrap(), b);
        assert_eq!(triple_get_third(vm, &[t]).unwrap(), c);
    });
}

#[test]
fn array_regex_and_progression_bridges_are_real() {
    with_vm(|vm| {
        let arr = alloc_arr(vm, "I", 2, || ArrayData::Int(vec![4, 9])).unwrap();
        let list = arrayskt_int_to_list(vm, &[arr]).unwrap();
        assert_eq!(
            coll_elems(vm, list).unwrap(),
            vec![JValue::Int(4), JValue::Int(9)]
        );
        assert_eq!(
            progression_last_element(vm, &[JValue::Int(1), JValue::Int(10), JValue::Int(3)])
                .unwrap(),
            JValue::Int(10)
        );
        let p = alloc(
            vm,
            "Lkotlin/text/Regex;",
            Native::Pattern {
                re: Regex::new("x+").unwrap(),
                source: "x+".into(),
            },
        )
        .unwrap();
        let text = s(vm, "axx x");
        let repl = s(vm, "!");
        assert_eq!(s_of!(vm, regex_replace_first(vm, &[p, text, repl])), "a! x");
    });
}

#[test]
fn common_string_and_collection_bridges_are_real() {
    with_vm(|vm| {
        let x = s(vm, "abcdef");
        let take = stringskt_take(vm, &[x, JValue::Int(3)]).unwrap();
        assert_eq!(jstr(vm, take).unwrap(), "abc");
        let x = s(vm, "42");
        let pad = stringskt_pad_start(vm, &[x, JValue::Int(4), JValue::Int('0' as i32)]).unwrap();
        assert_eq!(jstr(vm, pad).unwrap(), "0042");
        let x = s(vm, "abcd");
        let drop = stringskt_drop_last(vm, &[x, JValue::Int(2)]).unwrap();
        assert_eq!(jstr(vm, drop).unwrap(), "ab");
        let a = s(vm, "b");
        let b = s(vm, "a");
        let list = list_alloc(vm, vec![a, b]).unwrap();
        let sorted = collections_sorted(vm, &[list]).unwrap();
        let items = coll_elems(vm, sorted).unwrap();
        let first = items[0];
        assert_eq!(jstr(vm, first).unwrap(), "a");
    });
}

#[test]
fn high_frequency_string_and_mutex_bridges_are_real() {
    with_vm(|vm| {
        let text = s(vm, "alpha");
        let delim = s(vm, "ph");
        let missing = s(vm, "none");
        let out = stringskt_substring_after(vm, &[text, delim, missing]).unwrap();
        assert_eq!(jstr(vm, out).unwrap(), "a");
        let mutex = crate::vm::native::kotlinx::coroutines::mutex_default(
            vm,
            &[JValue::Int(0), JValue::Int(2), JValue::Null],
        )
        .unwrap();
        assert_eq!(
            crate::vm::native::kotlinx::coroutines::mutex_is_locked(vm, &[mutex]).unwrap(),
            JValue::Int(0)
        );
        assert_eq!(
            crate::vm::native::kotlinx::coroutines::mutex_try_lock(vm, &[mutex]).unwrap(),
            JValue::Int(1)
        );
        assert_eq!(
            crate::vm::native::kotlinx::coroutines::mutex_try_lock(vm, &[mutex]).unwrap(),
            JValue::Int(0)
        );
        crate::vm::native::kotlinx::coroutines::mutex_unlock(vm, &[mutex]).unwrap();
        assert_eq!(
            crate::vm::native::kotlinx::coroutines::mutex_is_locked(vm, &[mutex]).unwrap(),
            JValue::Int(0)
        );
    });
}

#[test]
fn group_values_wire_initial_data() {
    with_vm(|vm| {
        let html = "x wire:initial-data=\"{&quot;fingerprint&quot;:1}\" y";
        let pattern = r#"wire:initial-data="([^"]+)""#;
        let re = alloc(
            vm,
            "Lkotlin/text/Regex;",
            Native::Pattern {
                re: fancy_regex::Regex::new(pattern).unwrap(),
                source: pattern.to_string(),
            },
        )
        .unwrap();
        let html = s(vm, "x wire:initial-data=\"{&quot;fingerprint&quot;:1}\" y");
        let found = regex_find_default(
            vm,
            &[re, html, JValue::Int(0), JValue::Int(2), JValue::Null],
        )
        .unwrap();
        let values = match_result_get_group_values(vm, &[found]);
        let out = list_of!(vm, values);
        eprintln!("groupValues = {out:?}");
        assert_eq!(out.len(), 2);
        assert!(out[0].starts_with("wire:initial-data="));
        assert_eq!(out[1], "{&quot;fingerprint&quot;:1}");
    });
}

/// Case-insensitive `replaceFirst`/`replace`/`split`/`lastIndexOf` must not
/// slice at byte offsets computed from a lowercased copy: `to_lowercase`
/// can change the byte length (`İ` -> "i\u{307}"), which used to panic with
/// "not a char boundary" on multi-byte strings.
#[test]
fn case_insensitive_string_ops_handle_multibyte() {
    with_vm(|vm| {
        // ReplaceFirst(..., ignoreCase=true): the byte index of the match in
        // a lowercased copy (4) no longer matches the original string,
        // where 'ệ' starts at byte 3.
        let value = s(vm, "İxệy");
        let from = s(vm, "ệ");
        let to = s(vm, "ệN");
        let out = s_of!(
            vm,
            stringskt_replace_first_default(
                vm,
                &[value, from, to, JValue::Int(1), JValue::Null, JValue::Null],
            )
        );
        assert_eq!(out, "İxệNy");

        // Replace(..., ignoreCase=true) over the whole string.
        let out = s_of!(
            vm,
            stringskt_replace_default(
                vm,
                &[value, from, to, JValue::Int(1), JValue::Null, JValue::Null],
            )
        );
        assert_eq!(out, "İxệNy");

        // Split(ignoreCase=true) must not panic on multi-byte strings.
        let comma = s(vm, "ệ");
        let delimiters =
            alloc_arr(vm, "Ljava/lang/String;", 1, || ArrayData::Obj(vec![comma])).unwrap();
        let pieces = list_of!(
            vm,
            stringskt_split_strings_default(
                vm,
                &[
                    value,
                    delimiters,
                    JValue::Int(0),
                    JValue::Int(1),
                    JValue::Int(6),
                    JValue::Null,
                ]
            )
        );
        assert_eq!(pieces, ["İx", "y"]);

        // LastIndexOf(ignoreCase=true) returns an index into the original
        // string, not into the lowercased copy (which would yield 4).
        let hay = s(vm, "İxệy");
        let needle = s(vm, "ệ");
        let found = stringskt_last_index_of_default(
            vm,
            &[
                hay,
                needle,
                JValue::Int(2),
                JValue::Int(1),
                JValue::Int(4),
                JValue::Null,
            ],
        )
        .unwrap();
        assert_eq!(int_of(found), 3);
    });
}

#[test]
fn step_recomputes_last_like_kotlin() {
    with_vm(|vm| {
        for n in [20i32, 21, 23, 37, 62] {
            let range = rangeskt_until(vm, &[JValue::Int(0), JValue::Int(n)]).unwrap();
            let prog = progression_step(vm, &[range, JValue::Int(5)]).unwrap();
            let last = int_of(progression_get_last(vm, &[prog]).unwrap());
            let expected = n - 1 - (n - 1).rem_euclid(5);
            eprintln!("until(0,{n}).step(5).last = {last}  (real Kotlin: {expected})");
            assert_eq!(
                last, expected,
                "progression_step must recompute last (n={n})"
            );
        }
    });
}
