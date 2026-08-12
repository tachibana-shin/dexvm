use crate::vm::native::*;

mod collator;
mod dateformat;
mod decimalformat;
mod normalizer;
mod parseposition;
mod simpledateformat;

pub(crate) use normalizer::{
    lazy_form_nfc, lazy_form_nfd, lazy_form_nfkc, lazy_form_nfkd,
};
pub(crate) use simpledateformat::*;

/// All java.text native tables, grouped for `register`.
pub(crate) const TEXT_TABLE: &[&[NativeEntry]] = &[
    collator::TABLE,
    dateformat::TABLE,
    decimalformat::TABLE,
    normalizer::TABLE,
    parseposition::TABLE,
    simpledateformat::TABLE,
];
