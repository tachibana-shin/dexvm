//! Android application metadata: the binary `AndroidManifest.xml` (AXML)
//! and the resource table (`resources.arsc`) parsers.
//!
//! APK packaging is lossy for host-side consumers: the manifest is a binary
//! XML stream (string pools + typed attributes) and the launcher icon lives
//! behind a resource id (`@mipmap/ic_launcher` → `0x7f010000`) that only the
//! resource table can map to an actual file entry — Tachiyomi/keiyoushi
//! builds additionally obfuscate every resource path (`res/9w.png`), so the
//! file name is only discoverable through `resources.arsc`.
//!
//! Everything here is pure parsing over the raw container entries already
//! exposed by [`crate::Context`] (via the VM resource map); no Android
//! runtime is involved.

use std::collections::HashMap;

/// Parsed application-level metadata extracted from `AndroidManifest.xml`.
#[derive(Debug, Clone)]
pub struct AppManifest {
    /// The `package` attribute, e.g. `eu.kanade.tachiyomi.extension.vi.cuutruyenmoe`.
    pub package_id: String,
    /// The `application` `android:label`. A literal string in most
    /// keiyoushi builds; a `@string/...` reference is resolved through the
    /// resource table when present.
    pub app_name: String,
    /// The `manifest` `android:versionName`, when declared.
    pub version_name: Option<String>,
    /// The `uses-sdk` `android:minSdkVersion`, when declared.
    pub min_sdk: Option<u32>,
    /// The `uses-sdk` `android:targetSdkVersion`, when declared.
    pub target_sdk: Option<u32>,
    /// The `application` `android:icon` resource id (e.g. `0x7f010000`),
    /// when declared. Resolve it to a file path with
    /// [`ResourceTable::path`] and to the icon file with
    /// [`crate::Context::icon_bytes`].
    pub icon_resource_id: Option<u32>,
}

/// Errors produced while parsing an APK's manifest or resource table.
#[derive(Debug)]
pub enum ManifestError {
    /// The container holds no `AndroidManifest.xml` (plain dex input).
    Missing(String),
    /// The binary stream is malformed.
    Parse(String),
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::Missing(what) => write!(f, "no {what} in the container"),
            ManifestError::Parse(what) => write!(f, "malformed resource stream: {what}"),
        }
    }
}

impl std::error::Error for ManifestError {}

// ---------------------------------------------------------------------------
// Binary resource stream (`resources.arsc`)
// ---------------------------------------------------------------------------

/// What an entry in the resource table ultimately resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResEntry {
    /// A string value: either a `@string/...` label or, for raw resources
    /// (icons, drawables), the APK entry path (e.g. `res/9w.png`) — aapt2
    /// encodes both as `TYPE_STRING`.
    Str(String),
    /// An integer value.
    Int(i64),
    /// A reference to another resource id (`@0x7f010000`); the outer
    /// [`ResourceTable::resolve`] follows the chain.
    Ref(u32),
}

/// The parts of a `resources.arsc` table needed to resolve resource ids.
#[derive(Debug, Default)]
pub struct ResourceTable {
    /// The table-level string pool (file paths, string values).
    values: Vec<String>,
    /// `type id -> entry index -> resolved entry`.
    types: HashMap<u8, HashMap<u32, ResEntry>>,
}

/// Android `Res_value` data types (the ones relevant here).
mod res_type {
    pub const REFERENCE: u8 = 0x01;
    pub const STRING: u8 = 0x03;
    pub const INT_DEC: u8 = 0x10;
    pub const INT_HEX: u8 = 0x11;
    pub const INT_BOOLEAN: u8 = 0x12;
}

impl ResourceTable {
    /// Parses the binary resource table. The container-relative layout is:
    /// a table-level string pool, then one package chunk holding the type
    /// and key string pools plus the per-type entry chunks.
    pub fn parse(data: &[u8]) -> Result<ResourceTable, ManifestError> {
        let mut table = ResourceTable::default();
        let mut off = 0usize;
        let mut first = true;
        while off + 8 <= data.len() {
            let (ctype, _, size) = chunk_header(data, off)?;
            if std::env::var("DEXVM_TRACE").is_ok() {
                eprintln!("arsc walk off={off} ctype={ctype:#06x} size={size}");
            }
            let end = off
                .checked_add(size as usize)
                .filter(|e| *e <= data.len())
                .ok_or_else(|| ManifestError::Parse(format!("chunk @{off} overruns")))?;
            match ctype {
                0x0002 => {
                    // The table header chunk's size field only covers the
                    // leading header; the value pool and package chunks are
                    // siblings right after it, so advance by the 12-byte
                    // ResTable_header instead.
                    off = 12;
                    continue;
                }
                0x0001 if first => {
                    // Table-level (value) string pool.
                    table.values = parse_string_pool(data, off)?.0;
                    first = false;
                }
                0x0001 => {
                    // Type or key pool: consumed by the package chunk.
                }
                0x0200 => parse_package(data, off, end, &mut table)?,
                0x0201 => parse_type_chunk(data, off, end, &mut table)?,
                _ => {}
            }
            off = end;
        }
        Ok(table)
    }

    /// Resolves a resource id (`0x7fpptttt` = package `pp`, type `tt`,
    /// entry `tttt`) to its entry, following reference chains.
    pub fn resolve(&self, resource_id: u32) -> Option<&ResEntry> {
        let mut current = resource_id;
        for _ in 0..8 {
            let type_id = ((current >> 16) & 0xff) as u8;
            let entry = current & 0xffff;
            let value = self.types.get(&type_id)?.get(&entry)?;
            if let ResEntry::Ref(next) = value {
                current = *next;
                continue;
            }
            return Some(value);
        }
        None
    }

    /// The APK entry path behind a resource id (e.g. `res/9w.png`). Raw
    /// resources are `TYPE_STRING` values whose string *is* the entry path.
    pub fn path(&self, resource_id: u32) -> Option<String> {
        self.string(resource_id)
    }

    /// The string value behind a resource id (a `@string/...` label or a
    /// raw-resource file path).
    pub fn string(&self, resource_id: u32) -> Option<String> {
        match self.resolve(resource_id) {
            Some(ResEntry::Str(s)) => Some(s.clone()),
            _ => None,
        }
    }
}

/// Parses the package chunk: package id, the type/key string pools (their
/// offsets are relative to the package chunk start), and the per-type
/// entry chunks that follow (parsed by the outer walk).
fn parse_package(
    data: &[u8],
    off: usize,
    end: usize,
    table: &mut ResourceTable,
) -> Result<(), ManifestError> {
    if off + 280 > end {
        return Err(ManifestError::Parse("truncated package chunk".into()));
    }
    // ResTable_package: id u8 + 3 pad, name u16[128], typeStrings u32,
    // lastPublicType u32, keyStrings u32, lastPublicKey u32.
    let type_strings = u32_at(data, off + 268) as usize;
    let key_strings = u32_at(data, off + 276) as usize;
    if off + type_strings + 28 > end || off + key_strings + 28 > end {
        return Err(ManifestError::Parse(
            "package pool offset out of range".into(),
        ));
    }
    // Both pools are only needed structurally: type chunk keys resolve
    // through the key pool during type-chunk parsing, which is not
    // validated further here.
    let mut off2 = off + key_strings + pool_size(data, off + key_strings)?;
    while off2 + 8 <= end {
        let (ctype, _, size) = chunk_header(data, off2)?;
        if std::env::var("DEXVM_TRACE").is_ok() {
            eprintln!("arsc pkg walk off={off2} ctype={ctype:#06x} size={size}");
        }
        let cend = off2 + size as usize;
        if cend > end {
            break;
        }
        if ctype == 0x0201 {
            parse_type_chunk(data, off2, cend, table)?;
        }
        off2 = cend;
    }
    Ok(())
}

fn parse_type_chunk(
    data: &[u8],
    off: usize,
    end: usize,
    table: &mut ResourceTable,
) -> Result<(), ManifestError> {
    // ResTable_type: id u8, res0 u8, res1 u16, entryCount u32,
    // entriesStart u32, config (size field + payload), entry offsets,
    // entries. The offset table directly follows the config block, while
    // the offsets are relative to entriesStart (some builders write an
    // entriesStart that includes a trailing flags word after the config).
    if off + 24 > end {
        return Err(ManifestError::Parse("truncated type chunk".into()));
    }
    let type_id = data[off + 8];
    let entry_count = u32_at(data, off + 12) as usize;
    let entries_start = u32_at(data, off + 16) as usize;
    let config_size = u32_at(data, off + 20) as usize;
    if entry_count == 0 || type_id == 0 {
        return Ok(());
    }
    let offsets_at = off + 20 + config_size;
    let entries = table.types.entry(type_id).or_default();
    for i in 0..entry_count {
        if offsets_at + 4 * i + 4 > end {
            return Err(ManifestError::Parse("entry offsets overrun".into()));
        }
        let rel = u32_at(data, offsets_at + 4 * i) as usize;
        let entry_at = off + entries_start + rel;
        if entry_at + 16 > end {
            continue;
        }
        // ResTable_entry: size u16, flags u16, key u32, then Res_value.
        let (vsize, _vres0, vtype, vdata) = {
            let v = entry_at + 8;
            (
                u16_at(data, v) as usize,
                data[v + 2],
                data[v + 3],
                u32_at(data, v + 4),
            )
        };
        if vsize < 8 {
            continue;
        }
        let entry = match vtype {
            res_type::REFERENCE => ResEntry::Ref(vdata),
            res_type::STRING => {
                let idx = vdata as usize;
                let value = table
                    .values
                    .get(idx)
                    .ok_or_else(|| ManifestError::Parse(format!("value string {idx} missing")))?
                    .clone();
                ResEntry::Str(value)
            }
            res_type::INT_DEC | res_type::INT_HEX | res_type::INT_BOOLEAN => {
                ResEntry::Int(i64::from(vdata as i32))
            }
            _ => continue,
        };
        if std::env::var("DEXVM_TRACE").is_ok() {
            eprintln!("arsc type {type_id} entry {i}: {entry:?} (vtype {vtype:#04x})");
        }
        // Several type chunks may describe the same (type, entry) across
        // density configs; the first chunk is the default config, so
        // first-wins keeps the canonical value.
        entries.entry(i as u32).or_insert(entry);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Binary XML (`AndroidManifest.xml`)
// ---------------------------------------------------------------------------

/// Parses `AndroidManifest.xml`, resolving `@string/...` labels through the
/// resource table when one is supplied.
pub fn parse_manifest(
    data: &[u8],
    resources: Option<&ResourceTable>,
) -> Result<AppManifest, ManifestError> {
    let mut strings = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut manifest = AppManifest {
        package_id: String::new(),
        app_name: String::new(),
        version_name: None,
        min_sdk: None,
        target_sdk: None,
        icon_resource_id: None,
    };
    let mut off = 8usize;
    while off + 8 <= data.len() {
        let (ctype, _, size) = chunk_header(data, off)?;
        let end = off + size as usize;
        match ctype {
            0x0001 => strings = parse_string_pool(data, off)?.0,
            0x0102 => {
                // ResXMLTree_attrExt: ns u32, name u32, attrStart u16,
                // attrSize u16, attrCount u16, id u16, class u16, style u16.
                if off + 36 > end {
                    return Err(ManifestError::Parse("truncated element".into()));
                }
                let name_idx = u32_at(data, off + 20) as usize;
                let attr_start = u16_at(data, off + 24) as usize;
                let attr_size = u16_at(data, off + 26) as usize;
                let attr_count = u16_at(data, off + 28) as usize;
                let el_name = strings.get(name_idx).cloned().ok_or_else(|| {
                    ManifestError::Parse(format!("element name {name_idx} missing"))
                })?;
                stack.push(el_name.clone());
                let mut label: Option<AttrValue> = None;
                let mut icon: Option<AttrValue> = None;
                for i in 0..attr_count {
                    let a = off + 16 + attr_start + i * attr_size;
                    if a + 20 > end {
                        return Err(ManifestError::Parse("attribute overrun".into()));
                    }
                    let name = attr_name(&strings, u32_at(data, a + 4));
                    let raw = attr_raw(&strings, u32_at(data, a + 8));
                    let (_vsize, _vres0, vtype, vdata) = {
                        let v = a + 12;
                        (
                            u16_at(data, v),
                            data[v + 2],
                            data[v + 3],
                            u32_at(data, v + 4),
                        )
                    };
                    let value = AttrValue::from_parts(vtype, vdata, raw);
                    match el_name.as_str() {
                        "manifest" => match name.as_deref() {
                            Some("package") => {
                                if let AttrValue::Str(s) = value {
                                    manifest.package_id = s;
                                }
                            }
                            Some("versionName") => {
                                if let AttrValue::Str(s) = value {
                                    manifest.version_name = Some(s);
                                }
                            }
                            _ => {}
                        },
                        "application" => match name.as_deref() {
                            Some("label") => label = Some(value),
                            Some("icon") => icon = Some(value),
                            _ => {}
                        },
                        "uses-sdk" => match name.as_deref() {
                            Some("minSdkVersion") => {
                                if let AttrValue::Int(v) = value {
                                    manifest.min_sdk = Some(v as u32);
                                }
                            }
                            Some("targetSdkVersion") => {
                                if let AttrValue::Int(v) = value {
                                    manifest.target_sdk = Some(v as u32);
                                }
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                }
                if let Some(AttrValue::Ref(id)) = icon {
                    manifest.icon_resource_id = Some(id);
                }
                if let Some(v) = label {
                    manifest.app_name = match v {
                        AttrValue::Str(s) => s,
                        AttrValue::Ref(id) => resources
                            .and_then(|r| r.string(id))
                            .unwrap_or_else(|| manifest.package_id.clone()),
                        _ => manifest.package_id.clone(),
                    };
                }
            }
            0x0103 => {
                stack.pop();
            }
            _ => {}
        }
        off = end;
    }
    if manifest.package_id.is_empty() {
        return Err(ManifestError::Parse(
            "manifest without a package attribute".into(),
        ));
    }
    if manifest.app_name.is_empty() {
        manifest.app_name = manifest.package_id.clone();
    }
    Ok(manifest)
}

/// A typed manifest attribute value.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AttrValue {
    /// `android:type="string"` — either the raw value or a pool index.
    Str(String),
    /// `@0x7f010000` reference.
    Ref(u32),
    /// Typed integer (sdk levels, flags).
    Int(i32),
    /// Anything else (dimensions, colors, booleans-as-objects, ...).
    Other,
}

impl AttrValue {
    fn from_parts(vtype: u8, vdata: u32, raw: Option<String>) -> AttrValue {
        match (vtype, raw) {
            (res_type::STRING, Some(s)) => AttrValue::Str(s),
            (res_type::STRING, None) => AttrValue::Str(vdata.to_string()),
            (res_type::REFERENCE, _) => AttrValue::Ref(vdata),
            (res_type::INT_DEC | res_type::INT_HEX | res_type::INT_BOOLEAN, _) => {
                AttrValue::Int(vdata as i32)
            }
            _ => AttrValue::Other,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared primitives
// ---------------------------------------------------------------------------

fn chunk_header(data: &[u8], off: usize) -> Result<(u16, u16, u32), ManifestError> {
    if off + 8 > data.len() {
        return Err(ManifestError::Parse("chunk header overruns".into()));
    }
    Ok((
        u16_at(data, off),
        u16_at(data, off + 2),
        u32_at(data, off + 4),
    ))
}

fn u16_at(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn u32_at(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

/// The size field of the string pool at `off` (its chunk header).
fn pool_size(data: &[u8], off: usize) -> Result<usize, ManifestError> {
    Ok(chunk_header(data, off)?.2 as usize)
}

/// Parses a `ResStringPool` at `off`, returning its strings. Both UTF-16
/// (the `0x100` flag clear) and UTF-8 (flag set) layouts are handled; the
/// UTF-8 form carries the utf-16 length and the utf-8 byte length.
fn parse_string_pool(data: &[u8], off: usize) -> Result<(Vec<String>, usize), ManifestError> {
    let (_, _, size) = chunk_header(data, off)?;
    let end = off + size as usize;
    if off + 28 > end {
        return Err(ManifestError::Parse("truncated string pool".into()));
    }
    let count = u32_at(data, off + 8) as usize;
    let flags = u32_at(data, off + 16);
    let strings_start = u32_at(data, off + 20) as usize;
    let utf8 = flags & 0x100 != 0;
    let mut strings = Vec::with_capacity(count);
    for i in 0..count {
        let rel = u32_at(data, off + 28 + 4 * i) as usize;
        let s = off + strings_start + rel;
        if s >= end {
            return Err(ManifestError::Parse("string offset out of range".into()));
        }
        let string = if utf8 {
            let mut p = s;
            let _utf16_len = read_utf8_len(data, &mut p, end)?;
            let utf8_len = read_utf8_len(data, &mut p, end)?;
            let bytes = data
                .get(p..p + utf8_len)
                .ok_or_else(|| ManifestError::Parse("utf8 string overruns".into()))?;
            String::from_utf8_lossy(bytes).into_owned()
        } else {
            let len = u16_at(data, s) as usize;
            let end2 = s + 2 + len * 2;
            if end2 > end {
                return Err(ManifestError::Parse("utf16 string overruns".into()));
            }
            data[s + 2..end2]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect::<Vec<u16>>()
                .into_iter()
                .map(|u| char::from_u32(u32::from(u)).unwrap_or('\u{fffd}'))
                .collect()
        };
        strings.push(string);
    }
    Ok((strings, size as usize))
}

/// A UTF-8 string-pool length prefix: one byte when < 0x80, two otherwise.
/// Advances the cursor past the prefix.
fn read_utf8_len(data: &[u8], p: &mut usize, end: usize) -> Result<usize, ManifestError> {
    let b0 = *data
        .get(*p)
        .ok_or_else(|| ManifestError::Parse("utf8 length overruns".into()))?;
    *p += 1;
    if b0 & 0x80 == 0 {
        Ok(b0 as usize)
    } else {
        if *p >= end {
            return Err(ManifestError::Parse("utf8 length overruns".into()));
        }
        let b1 = data[*p];
        *p += 1;
        Ok((((b0 & 0x7f) as usize) << 8) | b1 as usize)
    }
}

/// An attribute name: either an index into the string pool or, for aapt1
/// output, an inline resource id (e.g. `0x01010001` = `android:label`).
fn attr_name(strings: &[String], raw: u32) -> Option<String> {
    if raw >= 0x0100_0000 {
        return match raw {
            0x0101_0001 => Some("label".into()),
            0x0101_0002 => Some("icon".into()),
            0x0101_0003 => Some("name".into()),
            0x0101_021b => Some("versionName".into()),
            0x0101_020c => Some("minSdkVersion".into()),
            0x0101_0270 => Some("targetSdkVersion".into()),
            _ => None,
        };
    }
    strings.get(raw as usize).cloned()
}

fn attr_raw(strings: &[String], raw: u32) -> Option<String> {
    if raw == u32::MAX {
        None
    } else {
        strings.get(raw as usize).cloned()
    }
}
