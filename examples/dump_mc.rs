use dexvm::dex::DexFile;

fn main() {
    let data = std::fs::read("/tmp/mt.dex").unwrap();
    let dex = DexFile::parse(&data).unwrap();
    for cd in &dex.classes {
        let Some(d) = &cd.class_data else { continue };
        let desc = dex.type_descriptor(cd.class_idx).to_string();
        if desc != "Leu/kanade/tachiyomi/extension/vi/moetruyen/ExtensionGenerated;" {
            continue;
        }
        for m in d.direct_methods.iter().chain(d.virtual_methods.iter()) {
            let (_, _, nm) = dex.method_key(m.method_idx).unwrap();
            let name = (*dex.strings)[nm as usize].to_string();
            let Some(code) = &m.code else { continue };
            let decoded = dexvm::dex::insn::decode_all(&code.insns).unwrap();
            println!("=== Lm;.{name}");
            for i in 0..decoded.insns.len() {
                let pc = decoded.units[i];
                print!("{pc:04x} ");
                match &decoded.insns[i] {
                    dexvm::dex::insn::Insn::Invoke(k, mi, a) => {
                        let (cl, pr, nm2) = dex.method_key(*mi).unwrap();
                        let ps: Vec<String> = dex.protos[pr as usize]
                            .params
                            .iter()
                            .map(|&t| dex.type_descriptor(t).to_string())
                            .collect();
                        println!(
                            "INVOKE {k:?} {a:?} {}.{}({}){}",
                            dex.type_descriptor(cl),
                            (*dex.strings)[nm2 as usize],
                            ps.join(","),
                            dex.type_descriptor(dex.protos[pr as usize].return_type)
                        );
                    }
                    dexvm::dex::insn::Insn::ConstString(dr, si) => {
                        println!("CONST-STRING r{dr} {:?}", (*dex.strings)[*si as usize]);
                    }
                    dexvm::dex::insn::Insn::Const16(dr, v) => println!("Const16 r{dr} {v}"),
                    dexvm::dex::insn::Insn::Const4(dr, v) => println!("Const4 r{dr} {v}"),
                    dexvm::dex::insn::Insn::ConstHigh16(dr, v) => println!("ConstHigh16 r{dr} {v}"),
                    dexvm::dex::insn::Insn::NewInstance(_, t) => {
                        println!("NEW {}", dex.type_descriptor(*t));
                    }
                    dexvm::dex::insn::Insn::NewArray(dr, sz, ty) => {
                        println!("NewArray r{dr} r{sz} {}", dex.type_descriptor(*ty));
                    }
                    dexvm::dex::insn::Insn::FillArrayData(..) => println!("FillArrayData"),
                    dexvm::dex::insn::Insn::IGetObj(_, r, fi) => {
                        let f = &dex.fields[*fi as usize];
                        println!(
                            "IGetObj r{r} {}.{} {}",
                            dex.type_descriptor(f.class),
                            (*dex.strings)[f.name as usize],
                            dex.type_descriptor(f.ty)
                        );
                    }
                    dexvm::dex::insn::Insn::SGetObj(r, fi) => {
                        let f = &dex.fields[*fi as usize];
                        println!(
                            "SGetObj r{r} {}.{} {}",
                            dex.type_descriptor(f.class),
                            (*dex.strings)[f.name as usize],
                            dex.type_descriptor(f.ty)
                        );
                    }
                    dexvm::dex::insn::Insn::Throw(_) => println!("Throw"),
                    dexvm::dex::insn::Insn::MoveResult(dr) => println!("MoveResult r{dr}"),
                    other => println!("{other:?}"),
                }
            }
        }
    }
}
