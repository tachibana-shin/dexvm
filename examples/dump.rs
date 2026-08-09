use dexvm::dex::DexFile;

fn main() {
    let data = std::fs::read("fixtures/classes.dex").unwrap();
    let dex = DexFile::parse(&data).unwrap();
    for cd in &dex.classes {
        let Some(d) = &cd.class_data else { continue };
        let desc = dex.type_descriptor(cd.class_idx).to_string();
        let mk = |m: &dexvm::dex::EncodedMethod| dex.method_key(m.method_idx).unwrap();
        let has = d
            .direct_methods
            .iter()
            .chain(d.virtual_methods.iter())
            .any(|m| dex.strings[mk(m).2 as usize].as_ref() == "searchMangaParse");
        if !has {
            continue;
        }
        println!("CLASS {desc}");
        for m in d.direct_methods.iter().chain(d.virtual_methods.iter()) {
            let (_, _, nm) = mk(m);
            let name = dex.strings[nm as usize].to_string();
            if name != "chapterListRequest" {
                continue;
            }
            let code = m.code.as_ref().unwrap();
            println!(
                "  searchMangaRequest: code_units={} regs={} ins={} outs={}",
                code.insns.len(),
                code.registers_size,
                code.ins_size,
                code.outs_size
            );
            let decoded = dexvm::dex::insn::decode_all(&code.insns).unwrap();
            for i in 0..decoded.insns.len() {
                let pc = decoded.units[i];
                match &decoded.insns[i] {
                    dexvm::dex::insn::Insn::Invoke(k, mi, _) => {
                        let (cl, pr, nm) = dex.method_key(*mi).unwrap();
                        println!(
                            "  {pc:04x} INVOKE {k:?} {}.{}{}",
                            dex.type_descriptor(cl),
                            dex.strings[nm as usize],
                            dex.strings[pr as usize]
                        );
                    }
                    dexvm::dex::insn::Insn::CheckCast(_, ty) => {
                        println!("  {pc:04x} CHECKCAST {}", dex.type_descriptor(*ty));
                    }
                    dexvm::dex::insn::Insn::ConstString(d, si) => {
                        println!(
                            "  {pc:04x} CONST-STRING r{d} {:?}",
                            dex.strings[*si as usize]
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}
