//! java.util.regex.Pattern host shims.

use crate::vm::native::*;


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
    Ok(JValue::Int(i32::from(re.is_match(&input).unwrap_or(false))))
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
    Ok(new_str(vm, &fancy_regex::escape(&s)))
}


/// Native methods for Ljava/util/regex/Pattern;
pub(crate) const TABLE: &[NativeEntry] = &[
    ne!("Ljava/util/regex/Pattern;", "<init>", "(Ljava/lang/String;)V", true, pattern_init),
    ne!("Ljava/util/regex/Pattern;", "<init>", "(Ljava/lang/String;I)V", true, pattern_init_flags),
    ne!("Ljava/util/regex/Pattern;", "matcher", "(Ljava/lang/CharSequence;)Ljava/util/regex/Matcher;", true, pattern_matcher),
    ne!("Ljava/util/regex/Pattern;", "matches", "(Ljava/lang/String;Ljava/lang/CharSequence;)Z", false, pattern_matches_static),
    ne!("Ljava/util/regex/Pattern;", "compile", "(Ljava/lang/String;)Ljava/util/regex/Pattern;", false, pattern_compile),
    ne!("Ljava/util/regex/Pattern;", "compile", "(Ljava/lang/String;I)Ljava/util/regex/Pattern;", false, pattern_compile_flags),
    ne!("Ljava/util/regex/Pattern;", "pattern", "()Ljava/lang/String;", true, pattern_source),
    ne!("Ljava/util/regex/Pattern;", "toString", "()Ljava/lang/String;", true, pattern_source),
    ne!("Ljava/util/regex/Pattern;", "flags", "()I", true, pattern_flags),
    ne!("Ljava/util/regex/Pattern;", "split", "(Ljava/lang/CharSequence;)[Ljava/lang/String;", true, pattern_split_seq),
    ne!("Ljava/util/regex/Pattern;", "split", "(Ljava/lang/CharSequence;I)[Ljava/lang/String;", true, pattern_split_seq_limit),
    ne!("Ljava/util/regex/Pattern;", "quote", "(Ljava/lang/String;)Ljava/lang/String;", false, pattern_quote),
];
