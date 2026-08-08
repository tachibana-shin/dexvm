use super::*;
use ::regex as regex;

// java.util.regex.Pattern / Matcher
// ---------------------------------------------------------------------------

pub(crate) fn pattern_init(vm: &mut Vm, args: &[JValue]) -> R {
    let src = jstr(vm, args[1])?;
    let re = Regex::new(&src).map_err(|e| iae(vm, format!("PatternSyntaxException: {e}")))?;
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Pattern {
            re: dst,
            source: dst_src,
        } => {
            *dst = re;
            *dst_src = src;
        }
        _ => return Err(npe(vm)),
    }
    Ok(JValue::Null)
}

pub(crate) fn pattern_init_flags(vm: &mut Vm, args: &[JValue]) -> R {
    pattern_init(vm, args)
}

pub(crate) fn pattern_matcher(vm: &mut Vm, args: &[JValue]) -> R {
    let (re, _src) = match payload(vm, args[0]) {
        Some(Native::Pattern { re, source }) => (re.clone(), source.clone()),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    alloc(
        vm,
        "Ljava/util/regex/Matcher;",
        Native::Matcher(MatcherState {
            pattern: re,
            text,
            pos: 0,
            last: None,
        }),
    )
}

pub(crate) fn pattern_matches_static(vm: &mut Vm, args: &[JValue]) -> R {
    let re_str = jstr(vm, args[0])?;
    let input = charseq_of(vm, args[1])?;
    let re = Regex::new(&re_str).map_err(|e| iae(vm, format!("PatternSyntaxException: {e}")))?;
    Ok(JValue::Int(i32::from(re.is_match(&input))))
}

pub(crate) fn pattern_compile(vm: &mut Vm, args: &[JValue]) -> R {
    let src = jstr(vm, args[0])?;
    let re = Regex::new(&src).map_err(|e| iae(vm, format!("PatternSyntaxException: {e}")))?;
    alloc(vm, "Ljava/util/regex/Pattern;", Native::Pattern { re, source: src })
}

pub(crate) fn pattern_compile_flags(vm: &mut Vm, args: &[JValue]) -> R {
    pattern_compile(vm, args)
}

pub(crate) fn pattern_source(vm: &mut Vm, args: &[JValue]) -> R {
    let src = match payload(vm, args[0]) {
        Some(Native::Pattern { source, .. }) => source.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &src))
}

pub(crate) fn pattern_flags(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

pub(crate) fn pattern_split_seq(vm: &mut Vm, args: &[JValue]) -> R {
    let (re, _src) = match payload(vm, args[0]) {
        Some(Native::Pattern { re, source }) => (re.clone(), source.clone()),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    str_array(vm, split_java(&re, &text, 0))
}

pub(crate) fn pattern_split_seq_limit(vm: &mut Vm, args: &[JValue]) -> R {
    let (re, _src) = match payload(vm, args[0]) {
        Some(Native::Pattern { re, source }) => (re.clone(), source.clone()),
        _ => return Err(npe(vm)),
    };
    let text = charseq_of(vm, args[1])?;
    let limit = int_of(vm, args[2]);
    str_array(vm, split_java(&re, &text, limit))
}

pub(crate) fn pattern_quote(vm: &mut Vm, args: &[JValue]) -> R {
    let s = jstr(vm, args[0])?;
    Ok(new_str(vm, &regex::escape(&s)))
}

pub(crate) fn matcher_matches(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(n) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    match n {
        Native::Matcher(ms) => {
            let hit = ms
                .pattern
                .find(&ms.text)
                .filter(|m| m.start() == 0 && m.end() == ms.text.len())
                .map(|m| (m.start(), m.end()));
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
            let hit = ms
                .pattern
                .find_at(&ms.text, ms.pos)
                .map(|m| (m.start(), m.end()));
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
            let hit = ms
                .pattern
                .find_at(&ms.text, pos)
                .filter(|m| m.start() == pos)
                .map(|m| (m.start(), m.end()));
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
        Some(Native::Matcher(ms)) => match ms.last {
            Some((s, e)) => Some(ms.text[s..e].to_string()),
            None => None,
        },
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
            Some((s, _)) => match ms.pattern.captures_at(&ms.text, s) {
                Some(c) => match c.get(idx as usize) {
                    Some(m) => out = Some(ms.text[m.start()..m.end()].to_string()),
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
        None => Err(iae(vm, if no_match { "No match found" } else { "No group" })),
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
            Some((s, _)) if idx <= 0 => Some((s, s)),
            Some((s, _)) => match ms.pattern.captures_at(&ms.text, s) {
                Some(c) => c.get(idx as usize).map(|m| (m.start(), m.end())),
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
    alloc(vm, "Ljava/util/regex/Pattern;", Native::Pattern { re, source: src })
}

pub(crate) fn matcher_to_string(vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(new_str(vm, "java.util.regex.Matcher[...]"))
}

// ---------------------------------------------------------------------------
