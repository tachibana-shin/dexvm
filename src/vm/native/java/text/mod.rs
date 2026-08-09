use crate::vm::native::*;

mod dateformat;
mod parseposition;
mod simpledateformat;

pub(crate) use simpledateformat::*;

/// All java.text native tables, grouped for `register`.
pub(crate) const TEXT_TABLE: &[&[NativeEntry]] = &[
    dateformat::TABLE,
    parseposition::TABLE,
    simpledateformat::TABLE,
];
