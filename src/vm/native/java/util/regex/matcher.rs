//! java.util.regex.Matcher host shims.

use crate::vm::native::*;

/// fancy_regex has no `find_at`; emulate it with find_iter.
fn find_at(re: &fancy_regex::Regex, text: &str, pos: usize) -> Option<(usize, usize)> {
    for m in re.find_iter(text).flatten() {
        if m.start() >= pos {
            return Some((m.start(), m.end()));
        }
    }
    None
}

/// fancy_regex has no `captures_at`; emulate it with captures_iter. Returns
/// every group as `(start, end)`, avoiding the version-specific
/// `Captures<'t, S>` type from the fancy-regex crate.
fn captures_at(
    re: &fancy_regex::Regex,
    text: &str,
    pos: usize,
) -> Option<Vec<Option<(usize, usize)>>> {
    for c in re.captures_iter(text).flatten() {
        if c.get(0).map(|m| m.start()) == Some(pos) {
            return Some(
                (0..c.len())
                    .map(|i| c.get(i).map(|m| (m.start(), m.end())))
                    .collect(),
            );
        }
    }
    None
}

pub(crate) fn matcher_matches(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Matcher(ms) => {
            let hit =
                find_at(&ms.pattern, &ms.text, 0).filter(|(s, e)| *s == 0 && *e == ms.text.len());
            match hit {
                Some((s, e)) => {
                    ms.last = Some((s, e));
                    ms.pos = e;
                    Ok(JValue::Int(1))
                }
                None => {
                    ms.last = None;
                    Ok(JValue::Int(0))
                }
            }
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn matcher_find(vm: &mut Vm, args: &[JValue]) -> R {
    matcher_find_at(vm, args, None)
}

pub(crate) fn matcher_find_from(vm: &mut Vm, args: &[JValue]) -> R {
    matcher_find_at(vm, args, Some(int_of(vm, args[1]) as usize))
}

pub(crate) fn matcher_find_at(vm: &mut Vm, args: &[JValue], from: Option<usize>) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Matcher(ms) => {
            if let Some(p) = from {
                ms.pos = p;
            }
            let hit = find_at(&ms.pattern, &ms.text, ms.pos);
            match hit {
                Some((s, e)) => {
                    ms.last = Some((s, e));
                    ms.pos = if e == s { e.saturating_add(1) } else { e };
                    Ok(JValue::Int(1))
                }
                None => {
                    ms.last = None;
                    Ok(JValue::Int(0))
                }
            }
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn matcher_looking_at(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Matcher(ms) => {
            let pos = ms.pos;
            let hit = find_at(&ms.pattern, &ms.text, pos).filter(|(s, _)| *s == pos);
            match hit {
                Some((s, e)) => {
                    ms.last = Some((s, e));
                    Ok(JValue::Int(1))
                }
                None => {
                    ms.last = None;
                    Ok(JValue::Int(0))
                }
            }
        }
        _ => Err(npe(vm)),
    }
}

pub(crate) fn matcher_group(vm: &mut Vm, args: &[JValue]) -> R {
    let sub = match payload(vm, args[0]) {
        Some(Native::Matcher(ms)) => ms.last.map(|(s, e)| ms.text[s..e].to_string()),
        _ => return Err(npe(vm)),
    };
    match sub {
        Some(s) => Ok(new_str(vm, &s)),
        None => Err(iae(vm, "No match found")),
    }
}

pub(crate) fn matcher_group_n(vm: &mut Vm, args: &[JValue]) -> R {
    let idx = int_of(vm, args[1]);
    let mut out: Option<String> = None;
    let mut no_match = false;
    match payload(vm, args[0]) {
        Some(Native::Matcher(ms)) => match ms.last {
            Some((s, _)) => match captures_at(&ms.pattern, &ms.text, s) {
                Some(c) => match c.get(idx as usize).and_then(|m| *m) {
                    Some((gs, ge)) => out = Some(ms.text[gs..ge].to_string()),
                    None => no_match = true,
                },
                None => no_match = true,
            },
            None => no_match = true,
        },
        _ => return Err(npe(vm)),
    }
    match out {
        Some(s) => Ok(new_str(vm, &s)),
        None => Err(iae(
            vm,
            if no_match {
                "No match found"
            } else {
                "No group"
            },
        )),
    }
}

pub(crate) fn matcher_group_count(vm: &mut Vm, args: &[JValue]) -> R {
    let c = match payload(vm, args[0]) {
        Some(Native::Matcher(ms)) => ms.pattern.captures_len().saturating_sub(1) as i32,
        _ => return Err(npe(vm)),
    };
    Ok(JValue::Int(c))
}

pub(crate) fn matcher_start(vm: &mut Vm, args: &[JValue]) -> R {
    let b = matcher_bound_pair(vm, args[0], 0);
    match b {
        Some((s, _)) => Ok(JValue::Int(s as i32)),
        None => Err(iae(vm, "No match found")),
    }
}

pub(crate) fn matcher_end(vm: &mut Vm, args: &[JValue]) -> R {
    let b = matcher_bound_pair(vm, args[0], 0);
    match b {
        Some((_, e)) => Ok(JValue::Int(e as i32)),
        None => Err(iae(vm, "No match found")),
    }
}

pub(crate) fn matcher_start_n(vm: &mut Vm, args: &[JValue]) -> R {
    let b = matcher_bound_pair(vm, args[0], int_of(vm, args[1]));
    match b {
        Some((s, _)) => Ok(JValue::Int(s as i32)),
        None => Err(iae(vm, "No match found")),
    }
}

pub(crate) fn matcher_end_n(vm: &mut Vm, args: &[JValue]) -> R {
    let b = matcher_bound_pair(vm, args[0], int_of(vm, args[1]));
    match b {
        Some((_, e)) => Ok(JValue::Int(e as i32)),
        None => Err(iae(vm, "No match found")),
    }
}

pub(crate) fn matcher_bound_pair(vm: &mut Vm, v: JValue, idx: i32) -> Option<(usize, usize)> {
    match payload(vm, v) {
        Some(Native::Matcher(ms)) => match ms.last {
            Some((s, e)) if idx <= 0 => Some((s, e)),
            Some((s, _)) => match captures_at(&ms.pattern, &ms.text, s) {
                Some(c) => c.get(idx as usize).and_then(|m| *m),
                None => None,
            },
            None => None,
        },
        _ => None,
    }
}

pub(crate) fn matcher_replace_all(vm: &mut Vm, args: &[JValue]) -> R {
    let repl = jstr(vm, args[1])?;
    let out = match payload(vm, args[0]) {
        Some(Native::Matcher(ms)) => Some(ms.pattern.replace_all(&ms.text, &repl).into_owned()),
        _ => None,
    };
    match out {
        Some(s) => Ok(new_str(vm, &s)),
        None => Err(npe(vm)),
    }
}

pub(crate) fn matcher_replace_first(vm: &mut Vm, args: &[JValue]) -> R {
    let repl = jstr(vm, args[1])?;
    let out = match payload(vm, args[0]) {
        Some(Native::Matcher(ms)) => Some(ms.pattern.replace(&ms.text, &repl).into_owned()),
        _ => None,
    };
    match out {
        Some(s) => Ok(new_str(vm, &s)),
        None => Err(npe(vm)),
    }
}

pub(crate) fn matcher_reset(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Matcher(ms) => {
            ms.pos = 0;
            ms.last = None;
        }
        _ => return Err(npe(vm)),
    }
    Ok(args[0])
}

pub(crate) fn matcher_reset_seq(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[1])?;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Matcher(ms) => {
            ms.text = text;
            ms.pos = 0;
            ms.last = None;
        }
        _ => return Err(npe(vm)),
    }
    Ok(args[0])
}

pub(crate) fn matcher_region(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(_args[0])
}

pub(crate) fn matcher_pattern(vm: &mut Vm, args: &[JValue]) -> R {
    let (re, src) = match payload(vm, args[0]) {
        Some(Native::Matcher(ms)) => (ms.pattern.clone(), ms.pattern.to_string()),
        _ => return Err(npe(vm)),
    };
    alloc(
        vm,
        "Ljava/util/regex/Pattern;",
        Native::Pattern { re, source: src },
    )
}

pub(crate) fn matcher_to_string(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(new_str(vm, "java.util.regex.Matcher[...]"))
}

// ---------------------------------------------------------------------------

/// Native methods for Ljava/util/regex/Matcher;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/regex/Matcher;",
        "matches",
        "()Z",
        true,
        matcher_matches
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "find",
        "()Z",
        true,
        matcher_find
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "find",
        "(I)Z",
        true,
        matcher_find_from
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "lookingAt",
        "()Z",
        true,
        matcher_looking_at
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "group",
        "()Ljava/lang/String;",
        true,
        matcher_group
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "group",
        "(I)Ljava/lang/String;",
        true,
        matcher_group_n
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "groupCount",
        "()I",
        true,
        matcher_group_count
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "start",
        "()I",
        true,
        matcher_start
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "start",
        "(I)I",
        true,
        matcher_start_n
    ),
    ne!("Ljava/util/regex/Matcher;", "end", "()I", true, matcher_end),
    ne!(
        "Ljava/util/regex/Matcher;",
        "end",
        "(I)I",
        true,
        matcher_end_n
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "replaceAll",
        "(Ljava/lang/String;)Ljava/lang/String;",
        true,
        matcher_replace_all
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "replaceFirst",
        "(Ljava/lang/String;)Ljava/lang/String;",
        true,
        matcher_replace_first
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "reset",
        "()Ljava/util/regex/Matcher;",
        true,
        matcher_reset
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "reset",
        "(Ljava/lang/CharSequence;)Ljava/util/regex/Matcher;",
        true,
        matcher_reset_seq
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "region",
        "(II)Ljava/util/regex/Matcher;",
        true,
        matcher_region
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "pattern",
        "()Ljava/util/regex/Pattern;",
        true,
        matcher_pattern
    ),
    ne!(
        "Ljava/util/regex/Matcher;",
        "toString",
        "()Ljava/lang/String;",
        true,
        matcher_to_string
    ),
];
