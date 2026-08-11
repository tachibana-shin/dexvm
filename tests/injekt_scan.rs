//! Probe: verify the bytecode-derived injekt type registry against the real
//! moetruyen APK (the one with the JSON cache pipeline). Loads every class,
//! then queries the registry for every loaded descriptor.

use dexvm::Context;

#[test]
fn scan_injekt_registry() {
    let mut ctx = Context::open("fixtures/tachiyomi-vi.moetruyen-v1.6.8.apk").unwrap();
    let vm = ctx.vm();

    let defs: Vec<(usize, u32)> = vm
        .dexes
        .iter()
        .enumerate()
        .flat_map(|(d, dex)| dex.classes.iter().map(move |def| (d, def.class_idx)))
        .collect();
    for (dex_idx, type_idx) in defs {
        let desc = vm.dexes[dex_idx].type_descriptor(type_idx).to_string();
        if desc.starts_with('L') {
            let _ = vm.ensure_class_by_desc(&desc);
        }
    }

    let class_descs: Vec<u32> = vm.classes.iter().map(|c| c.descriptor).collect();
    let mut any = false;
    for desc_id in class_descs {
        let desc = vm.str_of(desc_id).to_string();
        if let Some(ty) = vm.injekt_type_of(desc_id) {
            eprintln!("injekt: {desc} -> {}", vm.str_of(ty));
            any = true;
        }
    }
    assert!(any, "no injekt type registry entries found");
    let q = vm.intern("Lq;");
    let json = vm.intern("Lkotlinx/serialization/json/Json;");
    assert_eq!(vm.injekt_type_of(q), Some(json), "expected Lq; -> Json");
    let a = vm.intern("La;");
    let app = vm.intern("Landroid/app/Application;");
    assert_eq!(
        vm.injekt_type_of(a),
        Some(app),
        "expected La; -> Application"
    );
}
