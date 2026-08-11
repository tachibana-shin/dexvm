use std::cell::RefCell;
use std::rc::Rc;

use dexvm::keiyoushi::{HttpData, HttpResp, Keiyoushi};

fn mock(_req: &HttpData) -> HttpResp {
    HttpResp {
        code: 200,
        message: "OK".into(),
        headers: vec![("content-type".into(), "text/html".into())],
        body: Some(
            "<ol class=\"homepage-ranking-list\" data-ranking-period=\"total\"><li>\
             <a class=\"homepage-ranking-item__link\" href=\"/truyen/a\"><div class=\"homepage-ranking-item__title\">A</div>\
             <img src=\"/img.png\"/></a></li></ol>"
                .into(),
        ),
    }
}

#[test]
fn probe_coro_popular() {
    let mut ext = Keiyoushi::open("fixtures/tachiyomi-vi.moetruyen-v1.6.8.apk").expect("open");
    ext.set_http_rc(Rc::new(mock));
    let srcs = ext.sources().expect("sources");
    let src = &srcs[0];
    let data = std::fs::read("fixtures/tachiyomi-vi.moetruyen-v1.6.8.apk").unwrap();
    let mut ctx =
        dexvm::context::Context::new_with(&data, dexvm::context::SandboxOptions::default())
            .unwrap();
    {
        let vm = ctx.vm();
        let m0id = vm.ensure_class_by_desc("Lm0;").unwrap();
        let lhm = vm
            .ensure_class_by_desc("Ljava/util/LinkedHashMap;")
            .unwrap();
        let c = &vm.classes[m0id as usize];
        eprintln!(
            "PROBE m0 super={:?} is-lhm={}",
            c.superclass
                .map(|s| vm.str_of(vm.classes[s as usize].descriptor).to_string()),
            c.superclass == Some(lhm)
        );
        let lc = &vm.classes[lhm as usize];
        let mut keys: Vec<String> = lc
            .dispatch
            .keys()
            .map(|(n, s)| format!("{} {}", vm.str_of(*n), vm.str_of(*s)))
            .collect();
        keys.sort();
        eprintln!(
            "PROBE lhm n={} has-remove={} keys={}",
            lc.dispatch.len(),
            keys.iter().any(|k| k.starts_with("remove")),
            keys.join(" | ")
        );
    }
    match ext.popular_coro(src, 1) {
        Ok(mp) => eprintln!(
            "PROBE popular ok: {} mangas has_next={}",
            mp.mangas.len(),
            mp.has_next
        ),
        Err(e) => eprintln!("PROBE popular err: {}", ext.describe_error(&e)),
    }
    let _ = RefCell::new(0);
}
