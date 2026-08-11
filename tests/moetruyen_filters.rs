//! Moetruyen `getFilterList` over a seeded filter cache: writes a real
//! `filters.json.zst` (plain JSON — the zstd natives are identity) into the
//! extension's cache dir, then drives the whole REAL dex pipeline:
//! `Okio.source -> OkioZstd.zstdDecompress -> buffer ->
//! OkioStreamsKt.decodeFromBufferedSource(Json, JsonElementSerializer) ->
//! ExtensionGenerated.g -> Json.decodeFromJsonElement(ArrayListSerializer) ->
//! Lg.deserialize (PluginGeneratedSerialDescriptor) -> FilterList`.

mod common;
use common::*;

use dexvm::vm::object::Native;

/// Two genre options serialized exactly like the extension's own
/// GenreOption serializer (`{"name","id"}` element names from Lg.<clinit>).
const GENRES_JSON: &str = r#"[{"name":"Action","id":"1"},{"name":"Adventure","id":"2"}]"#;

#[test]
fn get_filter_list_decodes_seeded_cache() {
    init_logger();
    let mut ctx = open();

    let eg = init(&mut ctx, EG);
    ctx.invoke_on(eg.as_obj(), "<init>", "()V", &[])
        .map_err(|e| err_str(&mut ctx, e))
        .expect("eg ctor");

    // The extension's filter cache file — <cacheDir>/source_<id>/
    // filters.json.zst — built by the real `f` Lazy (Ls.invoke).
    let lazy = ctx
        .vm()
        .instance_field(eg.as_obj(), "f")
        .expect("filter lazy field f");
    let file = ctx
        .invoke_on(lazy.as_obj(), "getValue", "()Ljava/lang/Object;", &[])
        .map_err(|e| err_str(&mut ctx, e))
        .expect("filter lazy getValue");
    let path = match ctx.vm().payload_of(file) {
        Some(Native::File { path }) => path.clone(),
        other => {
            panic!(
                "filter lazy must yield a File, got {:?}",
                std::mem::discriminant(&other)
            )
        }
    };
    let parent = std::path::Path::new(&path)
        .parent()
        .expect("cache file parent");
    std::fs::create_dir_all(parent).expect("create cache dir");
    std::fs::write(&path, GENRES_JSON).expect("seed cache file");

    let flist = ctx
        .invoke_on(
            eg.as_obj(),
            "getFilterList",
            "()Leu/kanade/tachiyomi/source/model/FilterList;",
            &[],
        )
        .map_err(|e| err_str(&mut ctx, e))
        .expect("getFilterList");

    let filters = match ctx.vm().payload_of(flist) {
        Some(Native::SFilterList(items)) => items.clone(),
        other => {
            panic!(
                "getFilterList must return SFilterList, got {:?}",
                std::mem::discriminant(&other)
            )
        }
    };
    assert_eq!(filters.len(), 2, "status Select + genre Group");

    let Native::SFilter { name, options, .. } = ctx.vm().payload_of(filters[0]).unwrap() else {
        panic!("status filter payload");
    };
    assert_eq!(name, "Trạng thái");
    let opts: Vec<String> = options
        .iter()
        .map(|o| match ctx.vm().payload_of(*o) {
            Some(Native::Str(s)) => s.clone(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(opts, ["Tất cả", "Còn tiếp", "Hoàn thành", "Tạm dừng"]);

    let Native::SFilter { name, children, .. } = ctx.vm().payload_of(filters[1]).unwrap() else {
        panic!("genre group payload");
    };
    assert_eq!(name, "Thể loại");
    let genres: Vec<String> = children
        .iter()
        .map(|c| match ctx.vm().payload_of(*c) {
            Some(Native::SFilter { name, .. }) => name.clone(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(genres, ["Action", "Adventure"]);
}

#[test]
fn get_filter_list_missing_cache_returns_static_filters() {
    init_logger();
    let mut ctx = open();

    let eg = init(&mut ctx, EG);
    ctx.invoke_on(eg.as_obj(), "<init>", "()V", &[])
        .map_err(|e| err_str(&mut ctx, e))
        .expect("eg ctor");

    let flist = ctx
        .invoke_on(
            eg.as_obj(),
            "getFilterList",
            "()Leu/kanade/tachiyomi/source/model/FilterList;",
            &[],
        )
        .map_err(|e| err_str(&mut ctx, e))
        .expect("getFilterList");

    // No cache file: the extension builds the static fallback filter list
    // (network fetch is not part of this offline test).
    let filters = match ctx.vm().payload_of(flist) {
        Some(Native::SFilterList(items)) => items,
        other => {
            panic!(
                "getFilterList must return SFilterList, got {:?}",
                std::mem::discriminant(&other)
            )
        }
    };
    assert!(!filters.is_empty(), "static fallback filters must exist");
}
