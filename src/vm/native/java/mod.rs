use crate::vm::native::*;

mod lang;
mod io;
mod nio;
mod text;
mod util;

pub(crate) use self::io::*;
pub(crate) use self::nio::*;
pub(crate) use lang::*;
pub(crate) use text::*;
pub(crate) use util::*;

/// Collect every java.* native table for `register`.
pub(crate) fn java_tables(out: &mut Vec<&'static [NativeEntry]>) {
    out.extend(lang::LANG_TABLE.iter().copied());
    out.push(io::TABLE);
    out.push(nio::TABLE);
    out.extend(text::TEXT_TABLE.iter().copied());
    out.extend(util::UTIL_TABLE.iter().copied());
}
