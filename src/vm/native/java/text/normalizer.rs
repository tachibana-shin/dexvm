//! java.text.Normalizer host shim, backed by `unicode-normalization`.
//! `Normalizer$Form` constants are plain `Native::Str` tags read back here.

use crate::vm::native::*;
use unicode_normalization::UnicodeNormalization;

fn form_const(vm: &mut Vm, tag: &str) -> JValue {
    let Ok(class) = vm.ensure_class_by_desc("Ljava/text/Normalizer$Form;") else {
        return JValue::Null;
    };
    JValue::Obj(
        vm.arena
            .alloc(class, Vec::new(), Some(Native::Str(tag.to_string()))),
    )
}
pub fn lazy_form_nfc(vm: &mut Vm) -> JValue {
    form_const(vm, "NFC")
}
pub fn lazy_form_nfd(vm: &mut Vm) -> JValue {
    form_const(vm, "NFD")
}
pub fn lazy_form_nfkc(vm: &mut Vm) -> JValue {
    form_const(vm, "NFKC")
}
pub fn lazy_form_nfkd(vm: &mut Vm) -> JValue {
    form_const(vm, "NFKD")
}

fn normalize(vm: &mut Vm, args: &[JValue]) -> R {
    let text = charseq_of(vm, args[0])?;
    let form = match payload(vm, args[1]) {
        Some(Native::Str(s)) => s.clone(),
        _ => "NFC".to_string(),
    };
    let out: String = match form.as_str() {
        "NFD" => text.nfd().collect(),
        "NFKC" => text.nfkc().collect(),
        "NFKD" => text.nfkd().collect(),
        _ => text.nfc().collect(),
    };
    Ok(new_str(vm, &out))
}

pub(crate) const TABLE: &[NativeEntry] = &[ne!(
    "Ljava/text/Normalizer;",
    "normalize",
    "(Ljava/lang/CharSequence;Ljava/text/Normalizer$Form;)Ljava/lang/String;",
    false,
    normalize
)];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfd_decomposes_accents_and_nfc_recomposes() {
        let decomposed: String = "café".nfd().collect();
        assert_eq!(decomposed.chars().count(), 5); // e + combining acute
        let recomposed: String = decomposed.nfc().collect();
        assert_eq!(recomposed, "café");
    }
}
