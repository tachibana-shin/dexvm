//! Kotlin stdlib host shims. Duration raw encoding is milliseconds so that
//! both `getInWholeSeconds` (raw / 1000) and `getInWholeMilliseconds` (raw)
//! round-trip through `toDuration`.
#![allow(dead_code)]

use crate::vm::native::*;

#[cfg(test)]
use super::intrinsics::throw_uninitialized as intrinsics_throw_uninitialized;
#[cfg(test)]
use super::{collections::*, ranges::*, sequences::*, support::*, time::*, tuples::*};

// lazy static materializers
// ---------------------------------------------------------------------------

// kotlin.text.Regex (payload reuses Native::Pattern)
// ---------------------------------------------------------------------------

fn regex_init(vm: &mut Vm, args: &[JValue]) -> R {
    let src = jstr(vm, args[1])?;
    let re =
        ::fancy_regex::Regex::new(&src).map_err(|e| iae(vm, format!("bad regex {src}: {e}")))?;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Pattern { re: dst, source } => {
            *dst = re;
            *source = src;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

fn regex_init_option(vm: &mut Vm, args: &[JValue]) -> R {
    let mut src = jstr(vm, args[1])?;
    let option = match payload(vm, args[2]) {
        Some(Native::Enum { name, .. }) => name.as_str(),
        _ => "",
    };
    src = match option {
        "IGNORE_CASE" => format!("(?i:{src})"),
        "MULTILINE" => format!("(?m:{src})"),
        "DOT_MATCHES_ALL" => format!("(?s:{src})"),
        "LITERAL" => fancy_regex::escape(&src).into_owned(),
        _ => src,
    };
    let pattern = new_str(vm, &src);
    regex_init(vm, &[args[0], pattern])
}

fn regex_replace(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let repl = jstr(vm, args[2])?;
    Ok(new_str(vm, &re.replace_all(&text, repl.as_str())))
}

fn regex_replace_function(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let callback = args[2];
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for matched in re.find_iter(&text).flatten() {
        out.push_str(&text[cursor..matched.start()]);
        let match_obj = alloc(
            vm,
            "Lkotlin/text/MatcherMatchResult;",
            Native::Matcher(MatcherState {
                pattern: re.clone(),
                text: text.clone(),
                pos: matched.end(),
                last: Some((matched.start(), matched.end())),
            }),
        )?;
        let replacement = inv_virt(
            vm,
            callback,
            "invoke",
            "(Ljava/lang/Object;)Ljava/lang/Object;",
            &[match_obj],
        )?;
        out.push_str(&charseq_of(vm, replacement)?);
        cursor = matched.end();
    }
    out.push_str(&text[cursor..]);
    Ok(new_str(vm, &out))
}

fn regex_matches(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let full = re
        .find(&text)
        .ok()
        .flatten()
        .is_some_and(|m| m.start() == 0 && m.end() == text.len());
    Ok(JValue::Int(i32::from(full)))
}

fn regex_match_entire(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let Some(m) = re.find(&text).ok().flatten() else {
        return Ok(JValue::Null);
    };
    if m.start() != 0 || m.end() != text.len() {
        return Ok(JValue::Null);
    }
    let end = m.end();
    alloc(
        vm,
        "Lkotlin/text/MatcherMatchResult;",
        Native::Matcher(MatcherState {
            pattern: re,
            text,
            pos: end,
            last: Some((0, end)),
        }),
    )
}

fn regex_contains_match_in(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    Ok(JValue::Int(i32::from(re.is_match(&text).unwrap_or(false))))
}

fn regex_find_default(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let start = if int_of(vm, args[3]) & 2 != 0 {
        0
    } else {
        int_of(vm, args[2]).max(0) as usize
    };
    let hit = re
        .find_iter(&text)
        .flatten()
        .find(|matched| matched.start() >= start)
        .map(|matched| (matched.start(), matched.end()));
    let Some((match_start, match_end)) = hit else {
        return Ok(JValue::Null);
    };
    alloc(
        vm,
        "Lkotlin/text/MatcherMatchResult;",
        Native::Matcher(MatcherState {
            pattern: re,
            text,
            pos: match_end,
            last: Some((match_start, match_end)),
        }),
    )
}

fn regex_split(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let limit = int_of(vm, args[2]);
    let raw_parts = if limit > 0 {
        re.splitn(&text, limit as usize)
            .collect::<Result<Vec<_>, _>>()
    } else {
        re.split(&text).collect::<Result<Vec<_>, _>>()
    }
    .map_err(|error| iae(vm, format!("regex split failed: {error}")))?;
    let parts = raw_parts
        .into_iter()
        .map(|part| new_str(vm, part))
        .collect();
    list_alloc(vm, parts)
}

fn regex_to_string(vm: &mut Vm, args: &[JValue]) -> R {
    let source = match payload(vm, args[0]) {
        Some(Native::Pattern { source, .. }) => source.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &source))
}

// kotlin.collections.CollectionsKt (statics)
// ---------------------------------------------------------------------------

/// `CollectionsKt.build(list)` — the builder is already the final list.

fn stringskt_starts_with_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    let prefix = charseq_of(vm, args[1])?;
    let ignore = args[2].as_int() != 0;
    let ignore_case = if args[3].as_int() & 4 != 0 {
        false
    } else {
        ignore
    };
    let r = if ignore_case {
        s.to_lowercase().starts_with(&prefix.to_lowercase())
    } else {
        s.starts_with(&prefix)
    };
    Ok(JValue::Int(r as i32))
}

/// kotlin.collections.joinToString with the compiler-generated `$default`
/// marker: (iterable, separator, prefix, postfix, limit, truncated,
/// transform, mask, marker).

// kotlin.text
// ---------------------------------------------------------------------------

/// `StringsKt.isBlank(CharSequence)`.
fn stringskt_is_blank(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    Ok(JValue::Int(i32::from(s.trim().is_empty())))
}

/// `StringsKt.toIntOrNull(String)` — boxed Integer or null.
fn stringskt_to_int_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    match s.trim().parse::<i32>() {
        Ok(n) => boxed(vm, "Ljava/lang/Integer;", Native::IntBox(n)),
        Err(_) => Ok(JValue::Null),
    }
}

fn stringskt_to_float_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    match value.trim().parse::<f32>() {
        Ok(value) => boxed(vm, "Ljava/lang/Float;", Native::FloatBox(value)),
        Err(_) => Ok(JValue::Null),
    }
}

fn stringskt_to_int_radix_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let radix = int_of(vm, args[1]);
    if !(2..=36).contains(&radix) {
        return Err(iae(vm, format!("radix {radix} was not in 2..36")));
    }
    match i32::from_str_radix(value.trim(), radix as u32) {
        Ok(value) => boxed(vm, "Ljava/lang/Integer;", Native::IntBox(value)),
        Err(_) => Ok(JValue::Null),
    }
}

fn stringskt_to_long_or_null(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    match value.trim().parse::<i64>() {
        Ok(value) => boxed(vm, "Ljava/lang/Long;", Native::LongBox(value)),
        Err(_) => Ok(JValue::Null),
    }
}

fn chars_from_array(vm: &mut Vm, value: JValue) -> Result<Vec<char>, NatErr> {
    match payload(vm, value) {
        Some(Native::Array(ArrayData::Char(chars))) => Ok(chars
            .iter()
            .map(|value| char::from_u32(u32::from(*value)).unwrap_or('\u{fffd}'))
            .collect()),
        _ => Err(npe(vm)),
    }
}

fn stringskt_trim_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let chars = chars_from_array(vm, args[1])?;
    Ok(new_str(vm, value.trim_matches(|ch| chars.contains(&ch))))
}

fn stringskt_trim_end_chars(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let chars = chars_from_array(vm, args[1])?;
    Ok(new_str(
        vm,
        value.trim_end_matches(|ch| chars.contains(&ch)),
    ))
}

fn stringskt_remove_surrounding(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = charseq_of(vm, args[1])?;
    let stripped = value
        .strip_prefix(&delimiter)
        .and_then(|value| value.strip_suffix(&delimiter))
        .unwrap_or(&value);
    Ok(new_str(vm, stripped))
}

fn stringskt_contains(vm: &mut Vm, args: &[JValue]) -> R {
    let haystack = charseq_of(vm, args[0])?;
    let needle = charseq_of(vm, args[1])?;
    let found = if args[2].as_int() != 0 {
        haystack.to_lowercase().contains(&needle.to_lowercase())
    } else {
        haystack.contains(&needle)
    };
    Ok(JValue::Int(i32::from(found)))
}

fn stringskt_starts_with(vm: &mut Vm, args: &[JValue]) -> R {
    let value = charseq_of(vm, args[0])?;
    let prefix = charseq_of(vm, args[1])?;
    let result = if args[2].as_int() != 0 {
        value.to_lowercase().starts_with(&prefix.to_lowercase())
    } else {
        value.starts_with(&prefix)
    };
    Ok(JValue::Int(i32::from(result)))
}

fn stringskt_ends_with(vm: &mut Vm, args: &[JValue]) -> R {
    let value = charseq_of(vm, args[0])?;
    let suffix = charseq_of(vm, args[1])?;
    let result = if args[2].as_int() != 0 {
        value.to_lowercase().ends_with(&suffix.to_lowercase())
    } else {
        value.ends_with(&suffix)
    };
    Ok(JValue::Int(i32::from(result)))
}

fn stringskt_ends_with_default(vm: &mut Vm, args: &[JValue]) -> R {
    let ignore_case = if int_of(vm, args[3]) & 2 != 0 {
        JValue::Int(0)
    } else {
        args[2]
    };
    stringskt_ends_with(vm, &[args[0], args[1], ignore_case])
}

fn stringskt_remove_prefix(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let prefix = charseq_of(vm, args[1])?;
    Ok(new_str(vm, value.strip_prefix(&prefix).unwrap_or(&value)))
}

fn stringskt_remove_suffix(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let suffix = charseq_of(vm, args[1])?;
    Ok(new_str(vm, value.strip_suffix(&suffix).unwrap_or(&value)))
}

fn stringskt_substring_before_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = jstr(vm, args[1])?;
    let missing = if int_of(vm, args[3]) & 2 != 0 {
        value.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        value
            .find(&delimiter)
            .map(|index| &value[..index])
            .unwrap_or(&missing),
    ))
}

fn split_literal(value: &str, delimiters: &[String], ignore_case: bool, limit: i32) -> Vec<String> {
    let mut output = Vec::new();
    let mut offset = 0;
    while offset <= value.len() && (limit <= 0 || output.len() + 1 < limit as usize) {
        let rest = &value[offset..];
        let folded = ignore_case.then(|| rest.to_lowercase());
        let hit = delimiters
            .iter()
            .filter(|delimiter| !delimiter.is_empty())
            .filter_map(|delimiter| {
                let index = if let Some(folded) = &folded {
                    folded.find(&delimiter.to_lowercase())
                } else {
                    rest.find(delimiter)
                }?;
                Some((index, delimiter.len()))
            })
            .min_by_key(|(index, _)| *index);
        let Some((index, delimiter_len)) = hit else {
            break;
        };
        output.push(rest[..index].to_string());
        offset += index + delimiter_len;
    }
    output.push(value[offset..].to_string());
    output
}

fn stringskt_split_strings_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = charseq_of(vm, args[0])?;
    let delimiters = coll_elems(vm, args[1])?
        .into_iter()
        .map(|value| jstr(vm, value))
        .collect::<Result<Vec<_>, _>>()?;
    let mask = int_of(vm, args[4]);
    let ignore_case = mask & 2 == 0 && args[2].as_int() != 0;
    let limit = if mask & 4 != 0 {
        0
    } else {
        int_of(vm, args[3])
    };
    let parts = split_literal(&value, &delimiters, ignore_case, limit)
        .into_iter()
        .map(|part| new_str(vm, &part))
        .collect();
    list_alloc(vm, parts)
}

fn stringskt_split_chars_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = charseq_of(vm, args[0])?;
    let delimiters = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Char(chars))) => chars
            .iter()
            .map(|value| {
                char::from_u32(u32::from(*value))
                    .unwrap_or('\u{fffd}')
                    .to_string()
            })
            .collect::<Vec<_>>(),
        _ => return Err(npe(vm)),
    };
    let mask = int_of(vm, args[4]);
    let ignore_case = mask & 2 == 0 && args[2].as_int() != 0;
    let limit = if mask & 4 != 0 {
        0
    } else {
        int_of(vm, args[3])
    };
    let parts = split_literal(&value, &delimiters, ignore_case, limit)
        .into_iter()
        .map(|part| new_str(vm, &part))
        .collect();
    list_alloc(vm, parts)
}

fn charskt_is_whitespace(_vm: &mut Vm, args: &[JValue]) -> R {
    let value = char::from_u32(args[0].as_int() as u32).unwrap_or('\u{fffd}');
    Ok(JValue::Int(i32::from(value.is_whitespace())))
}

fn charskt_titlecase(vm: &mut Vm, args: &[JValue]) -> R {
    let value = char::from_u32(args[0].as_int() as u32).unwrap_or('\u{fffd}');
    Ok(new_str(vm, &value.to_uppercase().collect::<String>()))
}

fn stringskt_substring_before_last_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = jstr(vm, args[1])?;
    let missing = if int_of(vm, args[3]) & 2 != 0 {
        value.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        value
            .rfind(&delimiter)
            .map(|index| &value[..index])
            .unwrap_or(&missing),
    ))
}

fn stringskt_substring_after_last_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = jstr(vm, args[1])?;
    let missing = if int_of(vm, args[3]) & 2 != 0 {
        value.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        value
            .rfind(&delimiter)
            .map(|index| &value[index + delimiter.len()..])
            .unwrap_or(&missing),
    ))
}

fn stringskt_last_index_of_default(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let needle = charseq_of(vm, args[1])?;
    let start = if int_of(vm, args[4]) & 4 != 0 {
        text.len()
    } else {
        int_of(vm, args[2]).max(0) as usize
    };
    let hay = &text[..start.min(text.len())];
    let found = if int_of(vm, args[3]) != 0 {
        hay.to_lowercase().rfind(&needle.to_lowercase())
    } else {
        hay.rfind(&needle)
    };
    Ok(JValue::Int(found.map_or(-1, |i| i as i32)))
}

fn stringskt_trim_indent(vm: &mut Vm, args: &[JValue]) -> R {
    let text = jstr(vm, args[0])?;
    let lines: Vec<&str> = text.lines().collect();
    let nonblank: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| !l.trim().is_empty())
        .collect();
    let indent = nonblank
        .iter()
        .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
        .min()
        .unwrap_or(0);
    let out = lines
        .into_iter()
        .map(|l| {
            l.chars()
                .skip(indent)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(new_str(vm, &out))
}

fn stringskt_substring_after_last_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = char::from_u32(int_of(vm, args[1]) as u32).unwrap_or('\0');
    let missing = if int_of(vm, args[3]) & 2 != 0 {
        value.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        value
            .rfind(delimiter)
            .map(|index| &value[index + delimiter.len_utf8()..])
            .unwrap_or(&missing),
    ))
}

fn stringskt_substring_before_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let value = jstr(vm, args[0])?;
    let delimiter = char::from_u32(int_of(vm, args[1]) as u32).unwrap_or('\0');
    let missing = if int_of(vm, args[3]) & 2 != 0 {
        value.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        value
            .find(delimiter)
            .map(|index| &value[..index])
            .unwrap_or(&missing),
    ))
}

fn stringskt_equals(vm: &mut Vm, args: &[JValue]) -> R {
    let left = jstr(vm, args[0])?;
    let right = jstr(vm, args[1])?;
    let equals = if int_of(vm, args[2]) != 0 {
        left.to_lowercase() == right.to_lowercase()
    } else {
        left == right
    };
    Ok(JValue::Int(i32::from(equals)))
}

fn stringskt_index_of_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let needle = char::from_u32(int_of(vm, args[1]) as u32).unwrap_or('\0');
    let start = if int_of(vm, args[4]) & 2 != 0 {
        0
    } else {
        int_of(vm, args[2]).max(0) as usize
    };
    let ignore_case = int_of(vm, args[4]) & 4 == 0 && int_of(vm, args[3]) != 0;
    let suffix = text.get(start..).unwrap_or("");
    let found = if ignore_case {
        suffix
            .char_indices()
            .find(|(_, ch)| ch.to_lowercase().to_string() == needle.to_lowercase().to_string())
            .map(|(index, _)| start + index)
    } else {
        suffix.find(needle).map(|index| start + index)
    };
    Ok(JValue::Int(found.map_or(-1, |index| index as i32)))
}

fn stringskt_index_of_string_default(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let needle = jstr(vm, args[1])?;
    let start = if int_of(vm, args[4]) & 2 != 0 {
        0
    } else {
        int_of(vm, args[2]).max(0) as usize
    };
    let ignore_case = int_of(vm, args[4]) & 4 == 0 && int_of(vm, args[3]) != 0;
    let suffix = text.get(start..).unwrap_or("");
    let found = if ignore_case {
        suffix
            .to_lowercase()
            .find(&needle.to_lowercase())
            .map(|index| start + index)
    } else {
        suffix.find(&needle).map(|index| start + index)
    };
    Ok(JValue::Int(found.map_or(-1, |index| index as i32)))
}

fn charskt_check_radix(vm: &mut Vm, args: &[JValue]) -> R {
    let radix = int_of(vm, args[0]);
    if !(2..=36).contains(&radix) {
        return Err(NatErr::Throw(vm.throwable_of(
            "Ljava/lang/IllegalArgumentException;",
            format!("radix {radix} was not in range 2..36"),
        )));
    }
    Ok(JValue::Int(radix))
}

/// `MatchResult.getValue` — the whole matched text of the last match on a
/// regex-backed value.
fn match_result_get_value(vm: &mut Vm, args: &[JValue]) -> R {
    let s = match payload(vm, args[0]) {
        Some(Native::Matcher(ms)) => {
            let Some((start, end)) = ms.last else {
                return Ok(new_str(vm, ""));
            };
            ms.text.get(start..end).unwrap_or("").to_string()
        }
        _ => return Ok(new_str(vm, "")),
    };
    Ok(new_str(vm, &s))
}

fn match_result_destructured_to_list(vm: &mut Vm, args: &[JValue]) -> R {
    let value = match_result_get_value(vm, args)?;
    list_alloc(vm, vec![value])
}

fn match_group_get_value(vm: &mut Vm, args: &[JValue]) -> R {
    let value = match payload(vm, args[0]) {
        Some(Native::Str(value)) => value.clone(),
        _ => String::new(),
    };
    Ok(new_str(vm, &value))
}

/// Kotlin's ISO-8601 parser used by extension date filters.  This accepts the
/// common UTC form (`YYYY-MM-DDTHH:MM:SS[.fraction]Z`) and returns null for
/// malformed/unsupported values, matching `parseOrNull`.

// java.net.URI
// ---------------------------------------------------------------------------

// kotlin.comparisons.ComparisonsKt
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
fn strings_append_array(vm: &mut Vm, args: &[JValue]) -> R {
    let items = match payload(vm, args[1]) {
        Some(Native::Array(data)) => {
            let mut v = Vec::new();
            for i in 0..data.len() {
                v.push(data.get(i));
            }
            v
        }
        _ => return Err(npe(vm)),
    };
    let mut s = String::new();
    for item in items {
        if let Ok(t) = jstr(vm, item) {
            s.push_str(&t);
        }
    }
    let Some(Native::StringBuilder(dst)) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    dst.push_str(&s);
    Ok(args[0])
}

// kotlin.text.StringsKt synthetic default-arg shims (mask bit 2 = ignoreCase default false)
// ---------------------------------------------------------------------------

// kotlin.text.StringsKt synthetic default-arg shims (mask bit 2 = ignoreCase default false)
fn stringskt_contains_default(vm: &mut Vm, args: &[JValue]) -> R {
    let haystack = charseq_of(vm, args[0])?;
    let needle = charseq_of(vm, args[1])?;
    let ignore = args[2].as_int() != 0;
    let ignore_case = if args[3].as_int() & 2 != 0 {
        false
    } else {
        ignore
    };
    let found = if ignore_case {
        haystack.to_lowercase().contains(&needle.to_lowercase())
    } else {
        haystack.contains(&needle)
    };
    Ok(JValue::Int(found as i32))
}

fn stringskt_contains_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let ch = char::from_u32(args[1].as_int() as u32).unwrap_or('\u{fffd}');
    let ignore = args[2].as_int() != 0 && args[3].as_int() & 4 == 0;
    let found = if ignore {
        text.to_lowercase().contains(ch.to_ascii_lowercase())
    } else {
        text.contains(ch)
    };
    Ok(JValue::Int(found as i32))
}

fn stringskt_take(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let n = int_of(vm, args[1]).max(0) as usize;
    Ok(new_str(vm, &s.chars().take(n).collect::<String>()))
}
fn stringskt_pad_start(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let n = int_of(vm, args[1]).max(0) as usize;
    let pad = char::from_u32(int_of(vm, args[2]) as u32).unwrap_or(' ');
    let len = s.chars().count();
    let mut out = std::iter::repeat_n(pad, n.saturating_sub(len)).collect::<String>();
    out.push_str(&s);
    Ok(new_str(vm, &out))
}
fn stringskt_drop_last(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let n = int_of(vm, args[1]).max(0) as usize;
    Ok(new_str(
        vm,
        &s.chars()
            .take(s.chars().count().saturating_sub(n))
            .collect::<String>(),
    ))
}
fn stringskt_replace_first_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    let from = charseq_of(vm, args[1])?;
    let to = charseq_of(vm, args[2])?;
    let ignore = args[3].as_int() != 0 && args[4].as_int() & 4 == 0;
    let pos = if ignore {
        s.to_lowercase().find(&from.to_lowercase())
    } else {
        s.find(&from)
    };
    let out = pos
        .map(|i| format!("{}{}{}", &s[..i], to, &s[i + from.len()..]))
        .unwrap_or(s);
    Ok(new_str(vm, &out))
}

fn stringskt_replace_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    let from = charseq_of(vm, args[1])?;
    let to = charseq_of(vm, args[2])?;
    let ignore = args[3].as_int() != 0;
    let ignore_case = if args[4].as_int() & 4 != 0 {
        false
    } else {
        ignore
    };
    let r = if ignore_case {
        regex_replace_case_insensitive(&s, &from, &to)
    } else {
        s.replace(&from, &to)
    };
    alloc(vm, "Ljava/lang/String;", Native::Str(r))
}

// Char/Char variant: String.replace(oldChar, newChar, ignoreCase) — the
// compiler emits the `$default` synthetic for the trailing `ignoreCase`.
fn stringskt_replace_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let old = char::from_u32(int_of(vm, args[1]) as u32).unwrap_or('\u{FFFD}');
    let new = char::from_u32(int_of(vm, args[2]) as u32).unwrap_or('\u{FFFD}');
    let r = s.replace(old, &new.to_string());
    alloc(vm, "Ljava/lang/String;", Native::Str(r))
}

// kotlin.text.StringsKt.trimStart(String, charArray) — strips every leading
// char present in the (sparse) trim character array.
fn stringskt_trim_start(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let trim = match payload(vm, args[1]) {
        Some(Native::Array(ArrayData::Char(chars))) => chars.clone(),
        _ => return Err(npe(vm)),
    };
    let r = s.trim_start_matches(|c: char| trim.contains(&(c as u16)));
    alloc(vm, "Ljava/lang/String;", Native::Str(r.to_string()))
}

fn stringskt_trim_start_charseq(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    Ok(new_str(vm, s.trim_start()))
}

// kotlin.collections.ArraysKt.copyOfRange(byte[], from, to)

fn regex_replace_first(vm: &mut Vm, args: &[JValue]) -> R {
    let re = match payload(vm, args[0]) {
        Some(Native::Pattern { re, .. }) => re.clone(),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let repl = jstr(vm, args[2])?;
    let out = if let Some(m) = re.find(&text).ok().flatten() {
        format!("{}{}{}", &text[..m.start()], repl, &text[m.end()..])
    } else {
        text
    };
    Ok(new_str(vm, &out))
}

fn strings_encode_bytes(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let bytes = s.into_bytes();
    alloc_arr(vm, "B", bytes.len(), move || {
        ArrayData::Byte(bytes.into_iter().map(|b| b as i8).collect())
    })
}

// kotlin.UInt / UByte `constructor-impl`: identity (already raw ints).
// Static: the value arrives as the only argument.
// kotlin.io.TextStreamsKt.readText(Reader) — drains the Reader through
// repeated virtual `read([CII)I` calls (any Reader the VM can invoke),
// assembling the chars into one String. A null reader is raised as an
// IllegalStateException (kotlin.UninitializedPropertyAccessException is
// not a registered shim, so this is its closest registered sibling).

// kotlin.io.CloseableKt.closeFinally(source, cause). With a primary failure,
// a close failure is suppressed; otherwise it propagates.

fn stringskt_trim(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    alloc(vm, "Ljava/lang/String;", Native::Str(s.trim().to_string()))
}

fn stringskt_substring_after_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    let delim = charseq_of(vm, args[1])?;
    let missing = if args[3].as_int() & 2 != 0 {
        s.clone()
    } else {
        charseq_of(vm, args[2])?
    };
    let r = if delim.is_empty() {
        missing.to_string()
    } else {
        match s.find(&delim) {
            Some(i) => s[i + delim.len()..].to_string(),
            None => missing.to_string(),
        }
    };
    alloc(vm, "Ljava/lang/String;", Native::Str(r))
}

fn stringskt_substring_after_char_default(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    let delim = char::from_u32(int_of(vm, args[1]) as u32).unwrap_or('\u{FFFD}');
    let missing = if args[3].as_int() & 2 != 0 {
        s.clone()
    } else {
        jstr(vm, args[2])?
    };
    Ok(new_str(
        vm,
        s.find(delim)
            .map_or(missing, |i| s[i + delim.len_utf8()..].to_string())
            .as_str(),
    ))
}

fn stringskt_substring_after(vm: &mut Vm, args: &[JValue]) -> R {
    let s = charseq_of(vm, args[0])?;
    let delim = charseq_of(vm, args[1])?;
    let missing = charseq_of(vm, args[2])?;
    let out = s
        .find(&delim)
        .map(|i| s[i + delim.len()..].to_string())
        .unwrap_or(missing);
    Ok(new_str(vm, &out))
}

fn regex_replace_case_insensitive(s: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(idx) = rest.to_lowercase().find(&from.to_lowercase()) {
        out.push_str(&rest[..idx]);
        out.push_str(to);
        rest = &rest[idx + from.len()..];
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------
// kotlin.time.Duration value-class methods (host stdlib)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// kotlin.time.Duration value-class methods (host stdlib)
// ---------------------------------------------------------------------------

/// `Duration.getInWholeMilliseconds-impl(J)J`; raw unit is milliseconds.

// ---------------------------------------------------------------------------
// kotlin stdlib native table
// ---------------------------------------------------------------------------

pub(crate) const KOTLIN_TABLE: &[NativeEntry] = &[
    ne!("Lkotlin/text/Regex;", "<init>", "(Ljava/lang/String;)V", true, regex_init),
    ne!("Lkotlin/text/Regex;", "<init>", "(Ljava/lang/String;Lkotlin/text/RegexOption;)V", true, regex_init_option),
    ne!("Lkotlin/text/Regex;", "replace", "(Ljava/lang/CharSequence;Ljava/lang/String;)Ljava/lang/String;", true, regex_replace),
    ne!("Lkotlin/text/Regex;", "replace", "(Ljava/lang/CharSequence;Lkotlin/jvm/functions/Function1;)Ljava/lang/String;", true, regex_replace_function),
    ne!("Lkotlin/text/Regex;", "matches", "(Ljava/lang/CharSequence;)Z", true, regex_matches),
    ne!("Lkotlin/text/Regex;", "matchEntire", "(Ljava/lang/CharSequence;)Lkotlin/text/MatchResult;", true, regex_match_entire),
    ne!("Lkotlin/text/Regex;", "containsMatchIn", "(Ljava/lang/CharSequence;)Z", true, regex_contains_match_in),
    ne!("Lkotlin/text/Regex;", "find$default", "(Lkotlin/text/Regex;Ljava/lang/CharSequence;IILjava/lang/Object;)Lkotlin/text/MatchResult;", false, regex_find_default),
    ne!("Lkotlin/text/Regex;", "findAll$default", "(Lkotlin/text/Regex;Ljava/lang/CharSequence;IILjava/lang/Object;)Lkotlin/sequences/Sequence;", false, regex_find_default),
    ne!("Lkotlin/text/Regex;", "split", "(Ljava/lang/CharSequence;I)Ljava/util/List;", true, regex_split),
    ne!("Lkotlin/text/Regex;", "toString", "()Ljava/lang/String;", true, regex_to_string),
    ne!("Lkotlin/text/StringsKt;", "append", "(Ljava/lang/StringBuilder;[Ljava/lang/String;)Ljava/lang/StringBuilder;", false, strings_append_array),
    ne!("Lkotlin/text/StringsKt;", "startsWith$default", "(Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Z", false, stringskt_starts_with_default),
    ne!("Lkotlin/text/StringsKt;", "isBlank", "(Ljava/lang/CharSequence;)Z", false, stringskt_is_blank),
    ne!("Lkotlin/text/StringsKt;", "toIntOrNull", "(Ljava/lang/String;)Ljava/lang/Integer;", false, stringskt_to_int_or_null),
    ne!("Lkotlin/text/StringsKt;", "toFloatOrNull", "(Ljava/lang/String;)Ljava/lang/Float;", false, stringskt_to_float_or_null),
    ne!("Lkotlin/text/StringsKt;", "toIntOrNull", "(Ljava/lang/String;I)Ljava/lang/Integer;", false, stringskt_to_int_radix_or_null),
    ne!("Lkotlin/text/StringsKt;", "toLongOrNull", "(Ljava/lang/String;)Ljava/lang/Long;", false, stringskt_to_long_or_null),
    ne!("Lkotlin/text/StringsKt;", "trim", "(Ljava/lang/String;[C)Ljava/lang/String;", false, stringskt_trim_chars),
    ne!("Lkotlin/text/StringsKt;", "trimEnd", "(Ljava/lang/String;[C)Ljava/lang/String;", false, stringskt_trim_end_chars),
    ne!("Lkotlin/text/StringsKt;", "removeSurrounding", "(Ljava/lang/String;Ljava/lang/CharSequence;)Ljava/lang/String;", false, stringskt_remove_surrounding),
    ne!("Lkotlin/text/StringsKt;", "contains", "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;Z)Z", false, stringskt_contains),
    ne!("Lkotlin/text/StringsKt;", "startsWith", "(Ljava/lang/String;Ljava/lang/String;Z)Z", false, stringskt_starts_with),
    ne!("Lkotlin/text/StringsKt;", "endsWith", "(Ljava/lang/String;Ljava/lang/String;Z)Z", false, stringskt_ends_with),
    ne!("Lkotlin/text/StringsKt;", "endsWith$default", "(Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Z", false, stringskt_ends_with_default),
    ne!("Lkotlin/text/StringsKt;", "removePrefix", "(Ljava/lang/String;Ljava/lang/CharSequence;)Ljava/lang/String;", false, stringskt_remove_prefix),
    ne!("Lkotlin/text/StringsKt;", "removeSuffix", "(Ljava/lang/String;Ljava/lang/CharSequence;)Ljava/lang/String;", false, stringskt_remove_suffix),
    ne!("Lkotlin/text/StringsKt;", "substringBefore$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_before_default),
    ne!("Lkotlin/text/StringsKt;", "split$default", "(Ljava/lang/CharSequence;[Ljava/lang/String;ZIILjava/lang/Object;)Ljava/util/List;", false, stringskt_split_strings_default),
    ne!("Lkotlin/text/StringsKt;", "split$default", "(Ljava/lang/CharSequence;[CZIILjava/lang/Object;)Ljava/util/List;", false, stringskt_split_chars_default),
    ne!("Lkotlin/text/StringsKt;", "substringBeforeLast$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_before_last_default),
    ne!("Lkotlin/text/StringsKt;", "substringAfterLast$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_after_last_default),
    ne!("Lkotlin/text/StringsKt;", "substringAfterLast$default", "(Ljava/lang/String;CLjava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_after_last_char_default),
    ne!("Lkotlin/text/StringsKt;", "lastIndexOf$default", "(Ljava/lang/CharSequence;Ljava/lang/String;IZILjava/lang/Object;)I", false, stringskt_last_index_of_default),
    ne!("Lkotlin/text/StringsKt;", "trimIndent", "(Ljava/lang/String;)Ljava/lang/String;", false, stringskt_trim_indent),
    ne!("Lkotlin/text/StringsKt;", "substringBefore$default", "(Ljava/lang/String;CLjava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_before_char_default),
    ne!("Lkotlin/text/StringsKt;", "equals", "(Ljava/lang/String;Ljava/lang/String;Z)Z", false, stringskt_equals),
    ne!("Lkotlin/text/StringsKt;", "indexOf$default", "(Ljava/lang/CharSequence;CIZILjava/lang/Object;)I", false, stringskt_index_of_char_default),
    ne!("Lkotlin/text/StringsKt;", "indexOf$default", "(Ljava/lang/CharSequence;Ljava/lang/String;IZILjava/lang/Object;)I", false, stringskt_index_of_string_default),
    ne!("Lkotlin/text/CharsKt;", "isWhitespace", "(C)Z", false, charskt_is_whitespace),
    ne!("Lkotlin/text/CharsKt;", "checkRadix", "(I)I", false, charskt_check_radix),
    ne!("Lkotlin/text/CharsKt;", "titlecase", "(CLjava/util/Locale;)Ljava/lang/String;", false, charskt_titlecase),
    ne!("Lkotlin/text/MatchResult;", "getValue", "()Ljava/lang/String;", true, match_result_get_value),
    ne!("Lkotlin/text/MatchResult$Destructured;", "toList", "()Ljava/util/List;", true, match_result_destructured_to_list),
    ne!("Lkotlin/text/MatchGroup;", "getValue", "()Ljava/lang/String;", true, match_group_get_value),
    ne!("Lkotlin/text/MatcherMatchResult;", "getValue", "()Ljava/lang/String;", true, match_result_get_value),
    ne!("Lkotlin/text/StringsKt;", "contains$default", "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;ZILjava/lang/Object;)Z", false, stringskt_contains_default),
    ne!("Lkotlin/text/StringsKt;", "contains$default", "(Ljava/lang/CharSequence;CZILjava/lang/Object;)Z", false, stringskt_contains_char_default),
    ne!("Lkotlin/text/StringsKt;", "substringAfter", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;", false, stringskt_substring_after),
    ne!("Lkotlin/text/StringsKt;", "substringAfter$default", "(Ljava/lang/String;CLjava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_after_char_default),
    ne!("Lkotlin/text/StringsKt;", "take", "(Ljava/lang/String;I)Ljava/lang/String;", false, stringskt_take),
    ne!("Lkotlin/text/StringsKt;", "padStart", "(Ljava/lang/String;IC)Ljava/lang/String;", false, stringskt_pad_start),
    ne!("Lkotlin/text/StringsKt;", "dropLast", "(Ljava/lang/String;I)Ljava/lang/String;", false, stringskt_drop_last),
    ne!("Lkotlin/text/StringsKt;", "replaceFirst$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Ljava/lang/String;", false, stringskt_replace_first_default),
    ne!("Lkotlin/text/StringsKt;", "replace$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ZILjava/lang/Object;)Ljava/lang/String;", false, stringskt_replace_default),
    ne!("Lkotlin/text/StringsKt;", "replace$default", "(Ljava/lang/String;CCZILjava/lang/Object;)Ljava/lang/String;", false, stringskt_replace_char_default),
    ne!("Lkotlin/text/StringsKt;", "trimStart", "(Ljava/lang/String;[C)Ljava/lang/String;", false, stringskt_trim_start),
    ne!("Lkotlin/text/StringsKt;", "trimStart", "(Ljava/lang/CharSequence;)Ljava/lang/CharSequence;", false, stringskt_trim_start_charseq),
    ne!("Lkotlin/text/Regex;", "replaceFirst", "(Ljava/lang/CharSequence;Ljava/lang/String;)Ljava/lang/String;", true, regex_replace_first),
    ne!("Lkotlin/text/StringsKt;", "encodeToByteArray", "(Ljava/lang/String;)[B", false, strings_encode_bytes),
    ne!("Lkotlin/text/StringsKt;", "substringAfter$default", "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;ILjava/lang/Object;)Ljava/lang/String;", false, stringskt_substring_after_default),
    ne!("Lkotlin/text/StringsKt;", "trim", "(Ljava/lang/CharSequence;)Ljava/lang/CharSequence;", false, stringskt_trim),
];

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
