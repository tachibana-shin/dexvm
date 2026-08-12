//! java.io host shims.

use crate::vm::native::*;

mod buffered_reader;
mod byte_array_input_stream;
mod byte_array_output_stream;
#[cfg(feature = "android")]
mod file;
mod gzip;
mod input_stream_reader;
mod output_stream;
mod print_stream;

#[cfg(test)]
pub(crate) use byte_array_input_stream::*;
pub(crate) use output_stream::*;
pub(crate) use print_stream::*;

/// All java.io native tables, grouped for `register`.
pub(crate) const IO_TABLE: &[&[NativeEntry]] = &[
    buffered_reader::TABLE,
    byte_array_input_stream::TABLE,
    byte_array_output_stream::TABLE,
    gzip::TABLE,
    input_stream_reader::TABLE,
    output_stream::TABLE,
    print_stream::TABLE,
];
#[cfg(feature = "android")]
pub(crate) const FILE_TABLE: &[&[NativeEntry]] = &[file::TABLE];
