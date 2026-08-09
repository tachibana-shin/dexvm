use crate::vm::native::*;

mod object;
mod string;
mod stringbuilder;
mod class;
mod r#enum;
mod throwable;
mod system;
mod thread;
mod math;
mod integer;
mod long;
mod short;
mod byte;
mod character;
mod boolean;
mod float;
mod double;

pub(crate) use object::*;
pub(crate) use string::*;
pub(crate) use class::*;
pub(crate) use throwable::*;
pub(crate) use integer::*;
pub(crate) use boolean::*;

/// All java.lang native tables, grouped for `register`.
pub(crate) const LANG_TABLE: &[&[NativeEntry]] = &[
    object::TABLE,
    string::TABLE,
    stringbuilder::TABLE,
    class::TABLE,
    r#enum::TABLE,
    throwable::TABLE,
    system::TABLE,
    thread::TABLE,
    math::TABLE,
    integer::TABLE,
    long::TABLE,
    short::TABLE,
    byte::TABLE,
    character::TABLE,
    boolean::TABLE,
    float::TABLE,
    double::TABLE,
];
