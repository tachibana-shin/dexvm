use crate::vm::native::*;

mod boolean;
mod byte;
mod character;
mod class;
mod double;
mod r#enum;
mod float;
mod integer;
mod long;
mod math;
mod object;
mod reflect;
mod short;
mod string;
mod stringbuilder;
mod system;
mod thread;
mod throwable;

pub(crate) use boolean::*;
pub(crate) use class::*;
pub(crate) use integer::*;
pub(crate) use object::*;
pub(crate) use string::*;
pub(crate) use throwable::*;

/// All java.lang native tables, grouped for `register`.
pub(crate) const LANG_TABLE: &[&[NativeEntry]] = &[
    object::TABLE,
    string::TABLE,
    stringbuilder::TABLE,
    class::TABLE,
    reflect::TABLE,
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
