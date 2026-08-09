use dexvm::dex::DexFile;
use dexvm::vm::error::JvmError;
use dexvm::vm::value::JValue;
use dexvm::vm::Vm;
use dexvm::{Context, SandboxOptions};

fn proto_sig(dex: &DexFile, proto_id: u32) -> String {
    let p = &dex.protos[proto_id as usize];
    let mut s = String::from("(");
    for &t in &p.params {
        s.push_str(dex.type_descriptor(t));
    }
    s.push(')');
    s.push_str(dex.type_descriptor(p.return_type));
    s
}

fn disassemble(dex: &DexFile, class: &str, method: &str) {
    use dexvm::dex::insn::{decode_all, Insn};
    let def_idx = dex
        .class_by_descriptor(class)
        .unwrap_or_else(|| panic!("class not found: {class}"));
    let c = &dex.classes[def_idx];
    let mut em: Option<(String, &dexvm::dex::EncodedMethod)> = None;
    if let Some(cd) = &c.class_data {
        for (owner, list) in [
            ("direct", &cd.direct_methods),
            ("virtual", &cd.virtual_methods),
        ] {
            for m in list {
                if dex.strings[dex.methods[m.method_idx as usize].name as usize].as_ref() == method
                {
                    em = Some((owner.to_string(), m));
                }
            }
        }
    }
    let (owner, em) = em.unwrap_or_else(|| panic!("no method {method} in {class}"));
    let mid = &dex.methods[em.method_idx as usize];
    println!("-- {owner} {}{} :", proto_sig(dex, mid.proto), method);
    let code = em.code.as_ref().expect("no code");
    let dec = decode_all(&code.insns).expect("decode");
    for (i, pc) in dec.units.iter().enumerate() {
        let insn = &dec.insns[i];
        let mut line = format!("  {pc:04x}: ");
        for w in code.insns[*pc as usize..].iter().take(5) {
            line.push_str(&format!("{w:04x} "));
        }
        line.push_str("| ");
        eprintln!("{line}");
        match insn {
            Insn::Invoke(kind, idx, args) => {
                let m = &dex.methods[*idx as usize];
                let klass = dex.type_descriptor(m.class).to_string();
                let name = dex.strings[m.name as usize].to_string();
                let sig = proto_sig(dex, m.proto);
                let regs: Vec<String> = (0..args.count)
                    .map(|a| format!("v{}", args.reg_at(a)))
                    .collect();
                line.push_str(&format!(
                    "invoke-{kind:?}/{{{}}} {klass}->{name}{sig}",
                    regs.join(", ")
                ));
            }
            Insn::ConstString(r, s) => {
                line.push_str(&format!(
                    "const-string v{r}, \"{}\"",
                    dex.strings[*s as usize]
                ));
            }
            Insn::ConstStringJumbo(r, s) => {
                line.push_str(&format!(
                    "const-string/jumbo v{r}, \"{}\"",
                    dex.strings[*s as usize]
                ));
            }
            Insn::ConstClass(r, t) => {
                line.push_str(&format!("const-class v{r}, {}", dex.type_descriptor(*t)));
            }
            Insn::NewInstance(r, t) => {
                line.push_str(&format!("new-instance v{r}, {}", dex.type_descriptor(*t)));
            }
            Insn::NewArray(a, b, t) => {
                line.push_str(&format!(
                    "new-array v{a}, v{b}, {}",
                    dex.type_descriptor(*t)
                ));
            }
            Insn::FilledNewArray(args, t) => {
                let regs: Vec<String> = (0..args.count)
                    .map(|a| format!("v{}", args.reg_at(a)))
                    .collect();
                line.push_str(&format!(
                    "filled-new-array {{{}}}, {}",
                    regs.join(", "),
                    dex.type_descriptor(*t)
                ));
            }
            Insn::CheckCast(r, t) => {
                line.push_str(&format!("check-cast v{r}, {}", dex.type_descriptor(*t)));
            }
            Insn::InstanceOf(a, b, t) => {
                line.push_str(&format!(
                    "instance-of v{a}, v{b}, {}",
                    dex.type_descriptor(*t)
                ));
            }
            Insn::IGet(a, b, f) | Insn::IGetWide(a, b, f) | Insn::IGetObj(a, b, f) => {
                let fd = &dex.fields[*f as usize];
                let op = match insn {
                    Insn::IGet(..) => "iget",
                    Insn::IGetWide(..) => "iget-wide",
                    _ => "iget-object",
                };
                line.push_str(&format!(
                    "{op} v{a}, v{b}, {}->{}.{}",
                    dex.type_descriptor(fd.class),
                    dex.type_descriptor(fd.ty),
                    dex.strings[fd.name as usize]
                ));
            }
            Insn::IPut(a, b, f) | Insn::IPutWide(a, b, f) | Insn::IPutObj(a, b, f) => {
                let fd = &dex.fields[*f as usize];
                let op = match insn {
                    Insn::IPut(..) => "iput",
                    Insn::IPutWide(..) => "iput-wide",
                    _ => "iput-object",
                };
                line.push_str(&format!(
                    "{op} v{a}, v{b}, {}->{}.{}",
                    dex.type_descriptor(fd.class),
                    dex.type_descriptor(fd.ty),
                    dex.strings[fd.name as usize]
                ));
            }
            Insn::SGet(r, f) | Insn::SGetWide(r, f) | Insn::SGetObj(r, f) => {
                let fd = &dex.fields[*f as usize];
                let op = match insn {
                    Insn::SGet(..) => "sget",
                    Insn::SGetWide(..) => "sget-wide",
                    _ => "sget-object",
                };
                line.push_str(&format!(
                    "{op} v{r}, {}->{}.{}",
                    dex.type_descriptor(fd.class),
                    dex.type_descriptor(fd.ty),
                    dex.strings[fd.name as usize]
                ));
            }
            Insn::SPut(r, f) | Insn::SPutWide(r, f) | Insn::SPutObj(r, f) => {
                let fd = &dex.fields[*f as usize];
                let op = match insn {
                    Insn::SPut(..) => "sput",
                    Insn::SPutWide(..) => "sput-wide",
                    _ => "sput-object",
                };
                line.push_str(&format!(
                    "{op} v{r}, {}->{}.{}",
                    dex.type_descriptor(fd.class),
                    dex.type_descriptor(fd.ty),
                    dex.strings[fd.name as usize]
                ));
            }
            Insn::Goto(t) => line.push_str(&format!("goto ->{t:04x}")),
            Insn::If(op, a, b, t) => line.push_str(&format!("if-{op:?} v{a}, v{b} ->{t:04x}")),
            Insn::IfZ(op, r, t) => line.push_str(&format!("if-{op:?}z v{r} ->{t:04x}")),
            Insn::PackedSwitch(r, _, base, targets) => {
                line.push_str(&format!(
                    "packed-switch v{r} ({}..{}): {}",
                    base,
                    base + targets.len() as i32 - 1,
                    targets
                        .iter()
                        .map(|t| format!("{t:04x}"))
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
            Insn::SparseSwitch(r, _, _keys, targets) => {
                line.push_str(&format!(
                    "sparse-switch v{r}: {:?}",
                    targets
                        .iter()
                        .map(|t| format!("{t:04x}"))
                        .collect::<Vec<_>>()
                ));
            }
            other => line.push_str(&format!("{other:?}")),
        }
        println!("{line}");
    }
}

fn list_classes(dex: &DexFile) {
    for (def_idx, c) in dex.classes.iter().enumerate() {
        let mut flags: Vec<&str> = Vec::new();
        if c.access_flags & 0x1 != 0 {
            flags.push("public");
        }
        if c.access_flags & 0x200 != 0 {
            flags.push("interface");
        }
        if c.access_flags & 0x400 != 0 {
            flags.push("abstract");
        }
        if c.access_flags & 0x1000 != 0 {
            flags.push("synthetic");
        }
        let kind = if c.access_flags & 0x200 != 0 {
            "interface"
        } else {
            "class"
        };
        let cls = dex.type_descriptor(c.class_idx);
        let mut line = format!("{def_idx:>3}: {kind} {cls}");
        if !flags.is_empty() {
            line.push_str(&format!(" ({})", flags.join(" ")));
        }
        if c.superclass_idx != u32::MAX {
            line.push_str(&format!(
                " extends {}",
                dex.type_descriptor(c.superclass_idx)
            ));
        }
        println!("{line}");
        if let Some(cd) = &c.class_data {
            let print_m = |label: &str, list: &Vec<dexvm::dex::EncodedMethod>| {
                for em in list {
                    let m = &dex.methods[em.method_idx as usize];
                    println!(
                        "      {label} {}{}{}",
                        proto_sig(dex, m.proto),
                        dex.strings[m.name as usize],
                        if em.code.is_some() { "" } else { "  // native" }
                    );
                }
            };
            print_m("direct ", &cd.direct_methods);
            print_m("virtual", &cd.virtual_methods);
        }
    }
}

fn refs_of(dex: &DexFile, desc: &str) {
    use dexvm::dex::insn::{decode_all, Insn};
    use std::collections::BTreeMap;
    let target = desc.to_string();
    let mut methods: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut fields: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut creates: usize = 0;
    let mut casts: usize = 0;
    for c in &dex.classes {
        if let Some(cd) = &c.class_data {
            for em in cd.direct_methods.iter().chain(&cd.virtual_methods) {
                let Some(code) = &em.code else { continue };
                let Ok(dec) = decode_all(&code.insns) else {
                    continue;
                };
                for (i, _pc) in dec.units.iter().enumerate() {
                    let insn = &dec.insns[i];
                    match insn {
                        Insn::Invoke(_, idx, _) => {
                            let m = &dex.methods[*idx as usize];
                            if dex.type_descriptor(m.class) == target {
                                let name = dex.strings[m.name as usize].to_string();
                                let sig = proto_sig(dex, m.proto);
                                *methods.entry((name, sig)).or_default() += 1;
                            }
                        }
                        Insn::IGet(_, _, f)
                        | Insn::IGetWide(_, _, f)
                        | Insn::IGetObj(_, _, f)
                        | Insn::IPut(_, _, f)
                        | Insn::IPutWide(_, _, f)
                        | Insn::IPutObj(_, _, f)
                        | Insn::SGet(_, f)
                        | Insn::SGetWide(_, f)
                        | Insn::SGetObj(_, f)
                        | Insn::SPut(_, f)
                        | Insn::SPutWide(_, f)
                        | Insn::SPutObj(_, f) => {
                            let fd = &dex.fields[*f as usize];
                            if dex.type_descriptor(fd.class) == target {
                                let name = dex.strings[fd.name as usize].to_string();
                                let ty = dex.type_descriptor(fd.ty).to_string();
                                *fields.entry((name, ty)).or_default() += 1;
                            }
                        }
                        Insn::NewInstance(_, t) => {
                            if dex.type_descriptor(*t) == target {
                                creates += 1;
                            }
                        }
                        Insn::CheckCast(_, t) => {
                            if dex.type_descriptor(*t) == target {
                                casts += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    println!("== refs on {target} ==");
    if creates > 0 {
        println!("  new-instance x{creates}");
    }
    if casts > 0 {
        println!("  check-cast x{casts}");
    }
    println!("-- methods:");
    for ((name, sig), n) in &methods {
        println!("  x{n:>3} {name}{sig}");
    }
    println!("-- fields:");
    for ((name, ty), n) in &fields {
        println!("  x{n:>3} {name} {ty}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut path: Option<String> = None;
    let mut show_types = false;
    let mut show_classes = false;
    let mut show_methods: Option<String> = None;
    let mut refs_target: Option<String> = None;
    let mut show_code: Option<(String, String)> = None;
    let mut run: Option<(String, String, Vec<i32>)> = None;
    let mut calls: Vec<(String, String, Vec<i32>)> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--types" => show_types = true,
            "--classes" => show_classes = true,
            "--code" => {
                i += 1;
                let class = args.get(i).cloned().unwrap_or_default();
                i += 1;
                let method = args.get(i).cloned().unwrap_or_default();
                show_code = Some((class, method));
            }
            "--methods" => {
                i += 1;
                show_methods = args.get(i).cloned();
            }
            "--refs" => {
                i += 1;
                let cls = args.get(i).cloned().unwrap_or_default();
                refs_target = Some(cls);
            }
            "--run" => {
                i += 1;
                let class = args.get(i).cloned().unwrap_or_default();
                i += 1;
                let method = args.get(i).cloned().unwrap_or_default();
                let mut ints = Vec::new();
                while args.get(i + 1).is_some() && args[i + 1].parse::<i32>().is_ok() {
                    i += 1;
                    ints.push(args[i].parse().unwrap());
                }
                run = Some((class, method, ints));
            }
            "--call" => {
                i += 1;
                let class = args.get(i).cloned().unwrap_or_default();
                i += 1;
                let method = args.get(i).cloned().unwrap_or_default();
                let mut ints = Vec::new();
                while args.get(i + 1).is_some() && args[i + 1].parse::<i32>().is_ok() {
                    i += 1;
                    ints.push(args[i].parse().unwrap());
                }
                calls.push((class, method, ints));
            }
            a => path = Some(a.to_string()),
        }
        i += 1;
    }
    let path = path.unwrap_or_else(|| {
        eprintln!("usage: dexcli [--types] [--classes] [--methods <class>] [--code <class> <method>] [--run <class> <method> [ints...]] [--call ...] <file.dex|file.apk>");
        std::process::exit(2);
    });
    let data = std::fs::read(&path).unwrap_or_else(|e| {
        eprintln!("open {path}: {e}");
        std::process::exit(1);
    });
    let mut ctx = match Context::new_with(&data, SandboxOptions::allow_all()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
    };
    let dex = &*ctx.dex();
    println!("dex: {} bytes", dex.data.len());
    println!(
        "strings={} types={} protos={} fields={} methods={} classes={}",
        dex.strings.len(),
        dex.types.len(),
        dex.protos.len(),
        dex.fields.len(),
        dex.methods.len(),
        dex.classes.len(),
    );
    if show_types {
        let defined: std::collections::HashSet<u32> =
            dex.classes.iter().map(|c| c.class_idx).collect();
        println!("-- external (host API) types referenced:");
        for (i, _t) in dex.types.iter().enumerate() {
            if !defined.contains(&(i as u32)) {
                println!("  {i}: {}", dex.type_descriptor(i as u32));
            }
        }
    }
    if show_classes {
        println!("-- classes:");
        list_classes(dex);
    }
    if let Some(desc) = refs_target {
        let mut desc = desc;
        if !desc.starts_with('L') && !desc.starts_with('[') {
            desc = format!("L{desc}");
        }
        if !desc.ends_with(';') {
            desc.push(';');
        }
        refs_of(dex, &desc);
    }
    if let Some(desc) = show_methods {
        let desc = if desc.ends_with(';') {
            desc
        } else {
            format!("{desc};")
        };
        if let Some(def_idx) = dex.class_by_descriptor(&desc) {
            let c = &dex.classes[def_idx];
            println!("-- class {desc}:");
            let print_m = |label: &str, list: &Vec<dexvm::dex::EncodedMethod>| {
                for em in list {
                    let m = &dex.methods[em.method_idx as usize];
                    let sig = proto_sig(dex, m.proto);
                    println!(
                        "  {label} {} {}{}",
                        sig,
                        dex.strings[m.name as usize],
                        if em.code.is_some() { "" } else { " (native)" }
                    );
                }
            };
            if let Some(cd) = &c.class_data {
                print_m("direct  ", &cd.direct_methods);
                print_m("virtual ", &cd.virtual_methods);
            }
        } else {
            eprintln!("class not found: {desc}");
            std::process::exit(1);
        }
    }
    if let Some((class, method)) = show_code {
        let class = if class.ends_with(';') {
            class
        } else {
            format!("{class};")
        };
        disassemble(dex, &class, &method);
    }
    let mut sequence: Vec<(String, String, Vec<i32>)> = Vec::new();
    if let Some((class, method, ints)) = run {
        let class = if class.ends_with(';') {
            class
        } else {
            format!("{class};")
        };
        sequence.push((class, method, ints));
    }
    for (class, method, ints) in calls {
        let class = if class.ends_with(';') {
            class
        } else {
            format!("{class};")
        };
        sequence.push((class, method, ints));
    }
    for (class, method, ints) in sequence {
        let args: Vec<JValue> = ints.into_iter().map(JValue::Int).collect();
        let v = match ctx.call(&class, &method, &args) {
            Ok(v) => v,
            Err(e) => {
                print_call_error(&mut ctx, e);
                continue;
            }
        };
        let shown = display_value(ctx.vm(), v);
        println!("{method} => {shown}");
    }
}

fn display_value(vm: &mut Vm, v: JValue) -> String {
    use dexvm::vm::object::Native;
    match v {
        JValue::Obj(o) => match &vm.arena.objects[o as usize].native {
            Some(Native::Str(s)) => format!("Obj(\"{s}\")"),
            Some(Native::Request {
                url,
                method,
                headers,
                body,
            }) => {
                format!("Request(method={method}, url={url}, headers={headers:?}, body={body:?})")
            }
            Some(Native::HttpUrl(url)) => format!("HttpUrl(\"{url}\")"),
            Some(Native::FormBody(fields)) => format!("FormBody({fields:?})"),
            _ => format!("Obj({o})"),
        },
        other => format!("{other:?}"),
    }
}

fn print_call_error(ctx: &mut Context, e: JvmError) {
    use dexvm::vm::object::Native;
    match e {
        JvmError::Uncaught(o) => {
            let vm = ctx.vm();
            let obj = &vm.arena.objects[o as usize];
            let class = vm.class_desc_str(obj.class);
            let msg = match &obj.native {
                Some(Native::Throwable { message, .. }) => message.clone(),
                _ => None,
            };
            eprintln!(
                "uncaught {class}{}",
                msg.map(|m| format!(": {m}")).unwrap_or_default()
            );
            std::process::exit(1);
        }
        e => {
            eprintln!("runtime error: {e}");
            std::process::exit(1);
        }
    }
}
