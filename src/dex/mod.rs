//! DEX (Dalvik Executable) format parser.
//!
//! Parses `classes.dex` (and multi-dex parts) into table-oriented structures
//! that the VM can resolve lazily. Follows the DEX format specification
//! (<https://source.android.com/docs/core/runtime/dex-format>).

pub mod insn;
pub mod read;

use std::sync::Arc;

use read::{decode_mutf8, Cursor, DexError};

/// Raw encoded-value entries from `static_values` / annotation members.
#[derive(Debug, Clone, PartialEq)]
pub enum EncodedValue {
    Byte(i8),
    Short(i16),
    Char(u16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    MethodType(u32),
    MethodHandle(u32),
    String(u32),
    Type(u32),
    Field(u32),
    Method(u32),
    Enum(u32),
    Array(Vec<EncodedValue>),
    Annotation(Annotation),
    Null,
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub type_idx: u32,
    pub elements: Vec<(u32, EncodedValue)>,
}

impl EncodedValue {
    /// Decode one encoded value. The header's low five bits are `value_type`;
    /// the high three bits are `value_arg` (payload byte count minus one for
    /// byte-width values, or the boolean value for `VALUE_BOOLEAN`).
    fn decode(c: &mut Cursor) -> Result<EncodedValue, DexError> {
        let b = c.u8()?;
        let value_type = b & 0x1f;
        let value_arg = (b >> 5) as usize;
        let size = value_arg + 1;
        let read_signed = |c: &mut Cursor, n: usize| -> Result<i64, DexError> {
            let mut v: u64 = 0;
            for i in 0..n {
                v |= u64::from(c.u8()?) << (8 * i);
            }
            // sign extend
            let shift = 64 - 8 * n;
            Ok(((v << shift) as i64) >> shift)
        };
        let read_unsigned = |c: &mut Cursor, n: usize| -> Result<u32, DexError> {
            let mut v = 0u32;
            for i in 0..n {
                v |= u32::from(c.u8()?) << (8 * i);
            }
            Ok(v)
        };
        let invalid_arg = |c: &Cursor| {
            DexError::new(
                c.pos - 1,
                format!("invalid value_arg {value_arg} for encoded value type {value_type:#x}"),
            )
        };
        match value_type {
            0x00 if value_arg == 0 => Ok(EncodedValue::Byte(read_signed(c, size)? as i8)),
            0x02 if value_arg <= 1 => Ok(EncodedValue::Short(read_signed(c, size)? as i16)),
            0x03 => Ok(EncodedValue::Char({
                if value_arg > 1 {
                    return Err(invalid_arg(c));
                }
                let mut v: u64 = 0;
                for i in 0..size {
                    v |= u64::from(c.u8()?) << (8 * i);
                }
                v as u16
            })),
            0x04 if value_arg <= 3 => Ok(EncodedValue::Int(read_signed(c, size)? as i32)),
            0x06 if value_arg <= 7 => Ok(EncodedValue::Long(read_signed(c, size)?)),
            0x10 if value_arg <= 3 => {
                // float: value_arg size <= 4, stored shifted right
                let mut v: u64 = 0;
                for i in 0..size {
                    v |= u64::from(c.u8()?) << (8 * i);
                }
                Ok(EncodedValue::Float(f32::from_bits(
                    (v << (32 - 8 * size)) as u32,
                )))
            }
            0x11 if value_arg <= 7 => {
                let mut v: u64 = 0;
                for i in 0..size {
                    v |= u64::from(c.u8()?) << (8 * i);
                }
                Ok(EncodedValue::Double(f64::from_bits(v << (64 - 8 * size))))
            }
            0x15..=0x1b if value_arg <= 3 => {
                let index = read_unsigned(c, size)?;
                Ok(match value_type {
                    0x15 => EncodedValue::MethodType(index),
                    0x16 => EncodedValue::MethodHandle(index),
                    0x17 => EncodedValue::String(index),
                    0x18 => EncodedValue::Type(index),
                    0x19 => EncodedValue::Field(index),
                    0x1a => EncodedValue::Method(index),
                    0x1b => EncodedValue::Enum(index),
                    _ => unreachable!(),
                })
            }
            0x1c if value_arg == 0 => {
                let n = c.uleb128()? as usize;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(EncodedValue::decode(c)?);
                }
                Ok(EncodedValue::Array(items))
            }
            0x1d if value_arg == 0 => {
                let type_idx = c.uleb128()?;
                let n = c.uleb128()? as usize;
                let mut elements = Vec::with_capacity(n);
                for _ in 0..n {
                    let name_idx = c.uleb128()?;
                    let v = EncodedValue::decode(c)?;
                    elements.push((name_idx, v));
                }
                Ok(EncodedValue::Annotation(Annotation { type_idx, elements }))
            }
            0x1e if value_arg == 0 => Ok(EncodedValue::Null),
            0x1f if value_arg <= 1 => Ok(EncodedValue::Bool(value_arg != 0)),
            0x00 | 0x02..=0x04 | 0x06 | 0x10..=0x11 | 0x15..=0x1f => Err(invalid_arg(c)),
            other => Err(DexError::new(
                c.pos - 1,
                format!("unknown encoded value type {other:#x}"),
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProtoId {
    pub shorty: u32,
    pub return_type: u32,
    pub params: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct FieldId {
    pub class: u32,
    pub ty: u32,
    pub name: u32,
}

#[derive(Debug, Clone)]
pub struct MethodId {
    pub class: u32,
    pub proto: u32,
    pub name: u32,
}

#[derive(Debug, Clone)]
pub struct TryItem {
    pub start_addr: u32,
    pub insn_count: u16,
    /// Handler list: (type_idx, handler addr in code units). `catch_all` address if present.
    pub handlers: Vec<(u32, u32)>,
    pub catch_all: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct CodeItem {
    pub registers_size: u16,
    pub ins_size: u16,
    pub outs_size: u16,
    /// Raw code units (u16), for decoding and debugging.
    pub insns: Arc<[u16]>,
    pub tries: Vec<TryItem>,
}

impl CodeItem {
    /// Byte offset of code-unit `pc` inside the original dex (for payload refs).
    pub fn is_wide_pair(&self, pc: usize) -> bool {
        pc + 1 < self.insns.len()
    }
}

#[derive(Debug, Clone)]
pub struct EncodedField {
    pub field_idx: u32,
    pub access_flags: u32,
}

#[derive(Debug, Clone)]
pub struct EncodedMethod {
    pub method_idx: u32,
    pub access_flags: u32,
    pub code: Option<Arc<CodeItem>>,
}

#[derive(Debug, Clone, Default)]
pub struct ClassData {
    pub static_fields: Vec<EncodedField>,
    pub instance_fields: Vec<EncodedField>,
    pub direct_methods: Vec<EncodedMethod>,
    pub virtual_methods: Vec<EncodedMethod>,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub class_idx: u32,
    pub access_flags: u32,
    /// u32::MAX when the class has no superclass (java.lang.Object).
    pub superclass_idx: u32,
    pub interfaces: Vec<u32>,
    pub source_file_idx: u32,
    pub class_data: Option<Arc<ClassData>>,
    pub static_values: Vec<EncodedValue>,
    /// Runtime generic signature from the `Ldalvik/annotation/Signature;`
    /// class annotation (e.g. a `FullTypeReference<Lkotlinx/.../Json;>`
    /// subclass). `None` when absent.
    pub generic_signature: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DexFile {
    /// Raw bytes (kept for payload access / debugging).
    pub data: Arc<[u8]>,
    pub strings: Vec<Arc<str>>,
    pub types: Vec<u32>,
    pub protos: Vec<ProtoId>,
    pub fields: Vec<FieldId>,
    pub methods: Vec<MethodId>,
    pub classes: Vec<ClassDef>,
}

impl DexFile {
    pub fn parse(data: &[u8]) -> Result<DexFile, DexError> {
        let mut c = Cursor::new(data);
        let magic = c.bytes(8)?;
        if !magic.starts_with(b"dex\n") {
            return Err(DexError::new(0, "not a dex file (bad magic)"));
        }
        // version: dex/035, 036, 037, 038, 039, 040, 041
        let _version = std::str::from_utf8(&magic[4..7]).unwrap_or("???");
        let _checksum = c.u32()?;
        let _signature = c.bytes(20)?;
        let file_size = c.u32()? as usize;
        if file_size != data.len() {
            return Err(DexError::new(
                12,
                format!("file_size mismatch: {file_size} != {}", data.len()),
            ));
        }
        let header_size = c.u32()?;
        if header_size != 0x70 {
            return Err(DexError::new(
                16,
                format!("unexpected header size {header_size:#x}"),
            ));
        }
        let endian = c.u32()?;
        if endian != 0x1234_5678 {
            return Err(DexError::new(
                20,
                "unexpected endian tag (reversed-endian dex not supported)",
            ));
        }
        let _link_size = c.u32()?;
        let _link_off = c.u32()?;
        let map_off = c.u32()?;
        let string_ids_size = c.u32()? as usize;
        let string_ids_off = c.u32()? as usize;
        let type_ids_size = c.u32()? as usize;
        let type_ids_off = c.u32()? as usize;
        let proto_ids_size = c.u32()? as usize;
        let proto_ids_off = c.u32()? as usize;
        let field_ids_size = c.u32()? as usize;
        let field_ids_off = c.u32()? as usize;
        let method_ids_size = c.u32()? as usize;
        let method_ids_off = c.u32()? as usize;
        let class_defs_size = c.u32()? as usize;
        let class_defs_off = c.u32()? as usize;
        let _data_size = c.u32()?;
        let _data_off = c.u32()?;

        // --- string ids ---
        let mut strings = Vec::with_capacity(string_ids_size);
        for i in 0..string_ids_size {
            let off = c.u32_at(string_ids_off + i * 4)? as usize;
            let sc = &mut Cursor::new(data);
            sc.seek(off)?;
            sc.uleb128()?; // utf16 length (not needed; we scan for NUL)
                           // find the NUL-terminated MUTF-8 payload
            let mut end = sc.pos;
            while end < data.len() && data[end] != 0 {
                end += 1;
            }
            if end >= data.len() {
                return Err(DexError::new(off, "unterminated string"));
            }
            strings.push(Arc::from(decode_mutf8(&data[sc.pos..end])?));
        }

        // --- type ids ---
        let mut types = Vec::with_capacity(type_ids_size);
        for i in 0..type_ids_size {
            types.push(c.u32_at(type_ids_off + i * 4)?);
        }

        // --- proto ids ---
        let mut protos = Vec::with_capacity(proto_ids_size);
        for i in 0..proto_ids_size {
            let off = proto_ids_off + i * 12;
            let shorty = c.u32_at(off)?;
            let return_type = c.u32_at(off + 4)?;
            let parameters_off = c.u32_at(off + 8)? as usize;
            let params = if parameters_off == 0 {
                Vec::new()
            } else {
                let n = c.u32_at(parameters_off)? as usize;
                let mut v = Vec::with_capacity(n);
                for j in 0..n {
                    v.push(c.u16_at(parameters_off + 4 + j * 2)? as u32);
                }
                v
            };
            protos.push(ProtoId {
                shorty,
                return_type,
                params,
            });
        }

        // --- field ids ---
        let mut fields = Vec::with_capacity(field_ids_size);
        for i in 0..field_ids_size {
            let off = field_ids_off + i * 8;
            let class = c.u16_at(off)? as u32;
            let ty = c.u16_at(off + 2)? as u32;
            let name = c.u32_at(off + 4)?;
            fields.push(FieldId { class, ty, name });
        }

        // --- method ids ---
        let mut methods = Vec::with_capacity(method_ids_size);
        for i in 0..method_ids_size {
            let off = method_ids_off + i * 8;
            let class = c.u16_at(off)? as u32;
            let proto = c.u16_at(off + 2)? as u32;
            let name = c.u32_at(off + 4)?;
            methods.push(MethodId { class, proto, name });
        }

        // --- class defs ---
        let signature_type_idx = strings
            .iter()
            .position(|s: &Arc<str>| s.as_ref() == "Ldalvik/annotation/Signature;")
            .map(|i| i as u32);
        let mut classes = Vec::with_capacity(class_defs_size);
        for i in 0..class_defs_size {
            let off = class_defs_off + i * 32;
            let class_idx = c.u32_at(off)?;
            let access_flags = c.u32_at(off + 4)?;
            let superclass_idx = c.u32_at(off + 8)?;
            let interfaces_off = c.u32_at(off + 12)? as usize;
            let source_file_idx = c.u32_at(off + 16)?;
            let annotations_off = c.u32_at(off + 20)? as usize;
            let class_data_off = c.u32_at(off + 24)? as usize;
            let static_values_off = c.u32_at(off + 28)? as usize;

            let generic_signature = if annotations_off == 0 {
                None
            } else {
                parse_runtime_signature(data, annotations_off, &strings, signature_type_idx)?
            };

            let interfaces = if interfaces_off == 0 {
                Vec::new()
            } else {
                let n = c.u32_at(interfaces_off)? as usize;
                let mut v = Vec::with_capacity(n);
                for j in 0..n {
                    v.push(c.u16_at(interfaces_off + 4 + j * 2)? as u32);
                }
                v
            };

            let class_data = if class_data_off == 0 {
                None
            } else {
                Some(Arc::new(parse_class_data(data, class_data_off)?))
            };

            let static_values = if static_values_off == 0 {
                Vec::new()
            } else {
                let sc = &mut Cursor::new(data);
                sc.seek(static_values_off)?;
                let n = sc.uleb128()? as usize;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push(EncodedValue::decode(sc)?);
                }
                v
            };

            classes.push(ClassDef {
                class_idx,
                access_flags,
                superclass_idx,
                interfaces,
                source_file_idx,
                class_data,
                static_values,
                generic_signature,
            });
        }

        // Sanity: map_list must exist and point at a valid location (we don't
        // require its content, but a malformed dex should fail early).
        if map_off == 0 {
            return Err(DexError::new(52, "missing map_list"));
        }
        let map_size = c.u32_at(map_off as usize)? as usize;
        if map_size > 128 || map_off as usize + 4 + map_size * 12 > data.len() {
            return Err(DexError::new(map_off as usize, "invalid map_list"));
        }

        Ok(DexFile {
            data: Arc::from(data),
            strings,
            types,
            protos,
            fields,
            methods,
            classes,
        })
    }

    pub fn type_descriptor(&self, type_idx: u32) -> &str {
        let s = self.types.get(type_idx as usize).copied().unwrap_or(0) as usize;
        self.strings
            .get(s)
            .map(|s| s.as_ref())
            .unwrap_or("<invalid-type>")
    }

    pub fn method_key(&self, method_idx: u32) -> Option<(u32, u32, u32)> {
        let m = self.methods.get(method_idx as usize)?;
        Some((m.class, m.proto, m.name))
    }

    /// Find the type id whose descriptor equals `desc`.
    pub fn type_id_of(&self, desc: &str) -> Option<u32> {
        let sid = self.strings.iter().position(|s| s.as_ref() == desc)? as u32;
        self.types.iter().position(|t| *t == sid).map(|i| i as u32)
    }

    /// Find a class def by its descriptor string.
    pub fn class_by_descriptor(&self, desc: &str) -> Option<usize> {
        // string_id of descriptor, then type_id, then class_def index
        let sid = self.strings.iter().position(|s| s.as_ref() == desc)? as u32;
        let tid = self.types.iter().position(|t| *t == sid)? as u32;
        self.classes.iter().position(|cd| cd.class_idx == tid)
    }
}

/// Parses the class annotations of the class whose `annotations_off`
/// (class_def field) points at an `annotations_directory_item` and returns
/// the runtime generic signature string from any
/// `Ldalvik/annotation/Signature;` class annotation (value = array of
/// strings).
pub(crate) fn parse_runtime_signature(
    data: &[u8],
    annotations_off: usize,
    strings: &[Arc<str>],
    signature_type_idx: Option<u32>,
) -> Result<Option<String>, DexError> {
    let Some(sig_type_idx) = signature_type_idx else {
        return Ok(None);
    };
    let c = &mut Cursor::new(data);
    c.seek(annotations_off)?;
    // annotations_directory_item: class_annotations_off + section sizes.
    let class_annotations_off = c.u32()? as usize;
    if class_annotations_off == 0 {
        return Ok(None);
    }
    c.seek(class_annotations_off)?;
    let _visibility = c.u8()?;
    let n = c.uleb128()? as usize;
    for _ in 0..n {
        let ann_off = c.uleb128()? as usize;
        let ac = &mut Cursor::new(data);
        ac.seek(ann_off)?;
        let type_idx = ac.uleb128()?;
        let sz = ac.uleb128()? as usize;
        let mut elements = Vec::with_capacity(sz);
        for _ in 0..sz {
            let name_idx = ac.uleb128()?;
            let v = EncodedValue::decode(ac)?;
            elements.push((name_idx, v));
        }
        if type_idx == sig_type_idx {
            for (_, v) in elements {
                if let EncodedValue::Array(items) = v {
                    let mut sig = String::new();
                    for it in items {
                        if let EncodedValue::String(s) = it {
                            sig.push_str(&strings[s as usize]);
                        }
                    }
                    if !sig.is_empty() {
                        return Ok(Some(sig));
                    }
                }
            }
        }
    }
    Ok(None)
}

fn parse_class_data(data: &[u8], off: usize) -> Result<ClassData, DexError> {
    let c = &mut Cursor::new(data);
    c.seek(off)?;
    let static_fields_size = c.uleb128()? as usize;
    let instance_fields_size = c.uleb128()? as usize;
    let direct_methods_size = c.uleb128()? as usize;
    let virtual_methods_size = c.uleb128()? as usize;

    let mut static_fields = Vec::with_capacity(static_fields_size);
    let mut idx = 0u32;
    for _ in 0..static_fields_size {
        idx += c.uleb128()?;
        let access_flags = c.uleb128()?;
        static_fields.push(EncodedField {
            field_idx: idx,
            access_flags,
        });
    }
    let mut instance_fields = Vec::with_capacity(instance_fields_size);
    let mut idx = 0u32;
    for _ in 0..instance_fields_size {
        idx += c.uleb128()?;
        let access_flags = c.uleb128()?;
        instance_fields.push(EncodedField {
            field_idx: idx,
            access_flags,
        });
    }
    let mut direct_methods = Vec::with_capacity(direct_methods_size);
    let mut idx = 0u32;
    for _ in 0..direct_methods_size {
        idx += c.uleb128()?;
        let access_flags = c.uleb128()?;
        let code_off = c.uleb128()?;
        let code = if code_off == 0 {
            None
        } else {
            Some(Arc::new(parse_code_item(data, code_off as usize)?))
        };
        direct_methods.push(EncodedMethod {
            method_idx: idx,
            access_flags,
            code,
        });
    }
    let mut virtual_methods = Vec::with_capacity(virtual_methods_size);
    let mut idx = 0u32;
    for _ in 0..virtual_methods_size {
        idx += c.uleb128()?;
        let access_flags = c.uleb128()?;
        let code_off = c.uleb128()?;
        let code = if code_off == 0 {
            None
        } else {
            Some(Arc::new(parse_code_item(data, code_off as usize)?))
        };
        virtual_methods.push(EncodedMethod {
            method_idx: idx,
            access_flags,
            code,
        });
    }
    Ok(ClassData {
        static_fields,
        instance_fields,
        direct_methods,
        virtual_methods,
    })
}

fn parse_code_item(data: &[u8], off: usize) -> Result<CodeItem, DexError> {
    let c = &mut Cursor::new(data);
    c.seek(off)?;
    let registers_size = c.u16()?;
    let ins_size = c.u16()?;
    let outs_size = c.u16()?;
    let tries_size = c.u16()?;
    let _debug_info_off = c.u32()?;
    let insns_size = c.u32()? as usize;
    let insns_bytes = c.bytes(insns_size * 2)?;
    let mut insns = Vec::with_capacity(insns_size);
    for i in 0..insns_size {
        insns.push(u16::from_le_bytes([
            insns_bytes[i * 2],
            insns_bytes[i * 2 + 1],
        ]));
    }
    // align to 4 bytes for the try list
    if (tries_size > 0) && (insns_size % 2 == 1) {
        c.u16()?; // padding
    }
    let mut tries = Vec::with_capacity(tries_size as usize);
    // handler_off values are relative to the start of the
    // encoded_catch_handler_list, which begins right after the tries array
    // (each try item is 8 bytes: start_addr u32, insn_count u16, handler_off u16).
    let tries_start = c.pos;
    for _ in 0..tries_size {
        let start_addr = c.u32()?;
        let insn_count = c.u16()?;
        let handler_off = c.u16()? as usize;
        // handler list starts at handler_off relative to the encoded_catch_handler_list
        let list_pos = tries_start + tries_size as usize * 8 + handler_off;
        let saved = c.pos;
        c.seek(list_pos)?;
        let n = c.sleb128()?;
        let mut handlers = Vec::new();
        let mut catch_all = None;
        if n > 0 {
            for _ in 0..n {
                let type_idx = c.uleb128()?;
                let addr = c.uleb128()?;
                handlers.push((type_idx, addr));
            }
        } else {
            for _ in 0..-n {
                let type_idx = c.uleb128()?;
                let addr = c.uleb128()?;
                handlers.push((type_idx, addr));
            }
            catch_all = Some(c.uleb128()?);
        }
        c.seek(saved)?;
        tries.push(TryItem {
            start_addr,
            insn_count,
            handlers,
            catch_all,
        });
    }
    Ok(CodeItem {
        registers_size,
        ins_size,
        outs_size,
        insns: Arc::from(insns),
        tries,
    })
}

/// Iterate over a type_list (shared helper used for interface lists etc.).
pub fn read_type_list(c: &mut Cursor, off: usize) -> Result<Vec<u32>, DexError> {
    if off == 0 {
        return Ok(Vec::new());
    }
    let n = c.u32_at(off)? as usize;
    let mut v = Vec::with_capacity(n);
    for j in 0..n {
        v.push(c.u16_at(off + 4 + j * 2)? as u32);
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal hand-assembled dex: header + one string "hi" + empty tables.
    fn minimal_dex() -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(b"dex\n035\0");
        d.extend_from_slice(&[0u8; 4]); // checksum
        d.extend_from_slice(&[0u8; 20]); // signature
        let file_size = 0x88; // header(0x70) + string_id(4) + string data(4) + map(16)
        d.extend_from_slice(&(file_size as u32).to_le_bytes());
        d.extend_from_slice(&0x70u32.to_le_bytes()); // header_size
        d.extend_from_slice(&0x1234_5678u32.to_le_bytes()); // endian
        d.extend_from_slice(&0u32.to_le_bytes()); // link_size
        d.extend_from_slice(&0u32.to_le_bytes()); // link_off
        d.extend_from_slice(&0x78u32.to_le_bytes()); // map_off
        d.extend_from_slice(&1u32.to_le_bytes()); // string_ids_size
        d.extend_from_slice(&0x70u32.to_le_bytes()); // string_ids_off
        d.extend_from_slice(&0u32.to_le_bytes()); // type_ids_size
        d.extend_from_slice(&0u32.to_le_bytes()); // type_ids_off
        d.extend_from_slice(&0u32.to_le_bytes()); // proto_ids_size
        d.extend_from_slice(&0u32.to_le_bytes()); // proto_ids_off
        d.extend_from_slice(&0u32.to_le_bytes()); // field_ids_size
        d.extend_from_slice(&0u32.to_le_bytes()); // field_ids_off
        d.extend_from_slice(&0u32.to_le_bytes()); // method_ids_size
        d.extend_from_slice(&0u32.to_le_bytes()); // method_ids_off
        d.extend_from_slice(&0u32.to_le_bytes()); // class_defs_size
        d.extend_from_slice(&0u32.to_le_bytes()); // class_defs_off
        d.extend_from_slice(&0u32.to_le_bytes()); // data_size
        d.extend_from_slice(&0u32.to_le_bytes()); // data_off
                                                  // string_ids[0] = offset of string_data at 0x74
        d.extend_from_slice(&0x74u32.to_le_bytes());
        // string_data "hi": uleb utf16_len=2, bytes 'h','i', NUL
        d.push(2);
        d.extend_from_slice(b"hi\0");
        // map_list: one entry: type 0x0000 (header), size 1, off 0
        d.extend_from_slice(&1u32.to_le_bytes());
        d.extend_from_slice(&0x0000u32.to_le_bytes());
        d.extend_from_slice(&1u32.to_le_bytes());
        d.extend_from_slice(&0u32.to_le_bytes());
        d
    }

    #[test]
    fn parses_minimal_dex() {
        let dex = DexFile::parse(&minimal_dex()).expect("parse");
        assert_eq!(dex.strings.len(), 1);
        assert_eq!(dex.strings[0].as_ref(), "hi");
        assert!(dex.types.is_empty());
    }

    #[test]
    fn rejects_bad_magic() {
        assert!(DexFile::parse(b"notadex........").is_err());
    }

    #[test]
    fn rejects_reversed_endian() {
        let mut d = minimal_dex();
        d[40..44].copy_from_slice(&0x7856_3412u32.to_le_bytes());
        assert!(DexFile::parse(&d).is_err());
    }

    #[test]
    fn decode_mutf8_with_nul() {
        assert_eq!(decode_mutf8(b"a\xc0\x80b\0").unwrap(), "a\u{0}b");
        assert_eq!(decode_mutf8(b"h\xc3\xa9llo\0").unwrap(), "héllo");
    }

    #[test]
    fn sleb128_roundtrip() {
        let mut c = Cursor::new(&[0xc0, 0xbb, 0x78]); // -123456 (encoded)
        assert_eq!(c.sleb128().unwrap(), -123456);
    }

    #[test]
    fn decodes_encoded_value_header_and_fixed_width_indices() {
        let mut byte = Cursor::new(&[0x00, 0xfe]);
        assert_eq!(
            EncodedValue::decode(&mut byte).unwrap(),
            EncodedValue::Byte(-2)
        );

        let mut int = Cursor::new(&[0x24, 0x34, 0x12]);
        assert_eq!(
            EncodedValue::decode(&mut int).unwrap(),
            EncodedValue::Int(0x1234)
        );

        let mut string = Cursor::new(&[0x37, 0x34, 0x12]);
        assert_eq!(
            EncodedValue::decode(&mut string).unwrap(),
            EncodedValue::String(0x1234)
        );

        let mut false_value = Cursor::new(&[0x1f]);
        assert_eq!(
            EncodedValue::decode(&mut false_value).unwrap(),
            EncodedValue::Bool(false)
        );
        let mut true_value = Cursor::new(&[0x3f]);
        assert_eq!(
            EncodedValue::decode(&mut true_value).unwrap(),
            EncodedValue::Bool(true)
        );
    }
}
