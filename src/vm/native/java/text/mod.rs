use crate::vm::native::*;

mod collator;
mod dateformat;
mod parseposition;
mod simpledateformat;

pub(crate) use simpledateformat::*;

/// All java.text native tables, grouped for `register`.
pub(crate) const TEXT_TABLE: &[&[NativeEntry]] = &[
    collator::TABLE,
    dateformat::TABLE,
    parseposition::TABLE,
    simpledateformat::TABLE,
];
