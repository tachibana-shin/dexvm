use crate::vm::native::*;

mod io;
mod lang;
mod nio;
mod security;
mod text;
mod util;

mod javax_crypto;

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
    out.push(security::SECURITY_TABLE);
    out.push(javax_crypto::JAVAX_CRYPTO_TABLE);
    out.extend(text::TEXT_TABLE.iter().copied());
    out.extend(util::UTIL_TABLE.iter().copied());
}
