use crate::vm::native::*;

mod collator;
mod dateformat;
mod decimalformat;
mod parseposition;
mod simpledateformat;

pub(crate) use simpledateformat::*;

/// All java.text native tables, grouped for `register`.
pub(crate) const TEXT_TABLE: &[&[NativeEntry]] = &[
    collator::TABLE,
    dateformat::TABLE,
    decimalformat::TABLE,
    parseposition::TABLE,
    simpledateformat::TABLE,
];
