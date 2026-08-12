//! java.util.zip.ZipInputStream / ZipEntry host shims: this VM has no real
//! streaming, so the whole archive is decoded eagerly into (name, content)
//! pairs on construction, and `getNextEntry` just walks that list.

use crate::vm::native::*;
use std::io::{Cursor, Read as _};

fn zip_input_stream_init(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = match payload(vm, args[1]) {
        Some(Native::ByteArrayInputStream { bytes, pos }) => bytes[*pos..].to_vec(),
        _ => return Err(npe(vm)),
    };
    let entries = match zip::ZipArchive::new(Cursor::new(bytes)) {
        Ok(mut archive) => {
            let mut out = Vec::new();
            for i in 0..archive.len() {
                if let Ok(mut file) = archive.by_index(i) {
                    let name = file.name().to_string();
                    let mut content = Vec::new();
                    let _ = file.read_to_end(&mut content);
                    out.push((name, content));
                }
            }
            out
        }
        Err(_) => Vec::new(),
    };
    let JValue::Obj(id) = args[0] else {
        return Err(npe(vm));
    };
    vm.arena.objects[id as usize].native = Some(Native::ZipReader { entries, idx: -1 });
    Ok(JValue::Null)
}

fn zip_input_stream_get_next_entry(vm: &mut Vm, args: &[JValue]) -> R {
    let (name, done) = match payload_mut(vm, args[0]) {
        Some(Native::ZipReader { entries, idx }) => {
            *idx += 1;
            match entries.get(*idx as usize) {
                Some((name, _)) => (name.clone(), false),
                None => (String::new(), true),
            }
        }
        _ => return Err(npe(vm)),
    };
    if done {
        return Ok(JValue::Null);
    }
    alloc(vm, "Ljava/util/zip/ZipEntry;", Native::ZipEntryName(name))
}

fn zip_input_stream_close_entry(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

fn zip_input_stream_close(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

fn zip_entry_get_name(vm: &mut Vm, args: &[JValue]) -> R {
    let name = match payload(vm, args[0]) {
        Some(Native::ZipEntryName(name)) => name.clone(),
        _ => return Err(npe(vm)),
    };
    Ok(new_str(vm, &name))
}

pub(crate) const TABLE: &[NativeEntry] = &[
    ne!(
        "Ljava/util/zip/ZipInputStream;",
        "<init>",
        "(Ljava/io/InputStream;)V",
        true,
        zip_input_stream_init
    ),
    ne!(
        "Ljava/util/zip/ZipInputStream;",
        "getNextEntry",
        "()Ljava/util/zip/ZipEntry;",
        true,
        zip_input_stream_get_next_entry
    ),
    ne!(
        "Ljava/util/zip/ZipInputStream;",
        "closeEntry",
        "()V",
        true,
        zip_input_stream_close_entry
    ),
    ne!(
        "Ljava/util/zip/ZipInputStream;",
        "close",
        "()V",
        true,
        zip_input_stream_close
    ),
    ne!(
        "Ljava/util/zip/ZipEntry;",
        "getName",
        "()Ljava/lang/String;",
        true,
        zip_entry_get_name
    ),
];
