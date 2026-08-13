//! Typed bridge for running mihon (keiyoushi) extension dex files.
//!
//! Usage:
//! ```no_run
//! # use dexvm::keiyoushi::*;
//! let mut ext = Keiyoushi::open("fixtures/tachiyomi-all.akuma-v1.4.10.apk").unwrap();
//! ext.set_http(move |r| HttpResp::ok("<html></html>"));
//! let srcs = ext.sources().unwrap();
//! let pages = ext.popular(&srcs[0], 1).unwrap();
//! ```
//!
//! The bridge follows the classic mihon request/parse contract: build a
//! request with `popularMangaRequest`-style methods, execute it through the
//! registered HTTP callback, then hand the response to the matching
//! `*Parse` method. Host-side defaults (`mangaDetailsRequest` etc.) are
//! implemented as natives on `HttpSource`.

use std::rc::Rc;

use crate::context::{Context, ContextError, SandboxOptions, SettingDefinition, SettingValue};
use crate::vm::error::JvmError;
use crate::vm::native::keiyoushi::{FILTER, FILTER_LIST, SCHAPTER, SMANGA};
use crate::vm::object::Native;
use crate::vm::value::JValue;
use crate::vm::Vm;

pub use crate::vm::native::keiyoushi::{HttpData, HttpResp};

/// A live engine: one extension dex plus the host bridge.
pub struct Keiyoushi {
    ctx: Context,
}

/// A source instance (arena id stable for the context lifetime).
#[derive(Debug, Clone, Copy)]
pub struct Source {
    inst: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Manga {
    pub title: String,
    pub author: String,
    pub artist: String,
    pub description: String,
    pub genre: String,
    pub status: i32,
    pub thumbnail_url: String,
    pub url: String,
}

#[derive(Debug, Clone, Default)]
pub struct Chapter {
    pub name: String,
    pub url: String,
    pub date_upload: i64,
    pub scanlator: String,
}

#[derive(Debug, Clone)]
pub struct PageRef {
    pub index: i32,
    pub name: String,
    pub url: String,
    pub image_url: String,
}

#[derive(Debug, Clone)]
pub struct MangaPages {
    pub mangas: Vec<Manga>,
    pub has_next: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    Plain,
    Header,
    Separator,
    Text,
    Select,
    TriState,
    Group,
}

#[derive(Debug, Clone)]
pub struct FilterDef {
    pub kind: FilterKind,
    pub name: String,
    pub state: i32,
    pub options: Vec<String>,
}

impl Keiyoushi {
    pub fn new(data: &[u8]) -> Result<Self, ContextError> {
        Ok(Keiyoushi {
            ctx: Context::new_with(data, SandboxOptions::allow_all())?,
        })
    }

    /// Loads an extension together with additional DEX/APK libraries that
    /// form its boot classpath. Extension classes take precedence over
    /// library classes.
    pub fn new_with_libraries(data: &[u8], libraries: &[&[u8]]) -> Result<Self, ContextError> {
        Ok(Keiyoushi {
            ctx: Context::new_with_libraries(data, libraries, SandboxOptions::allow_all())?,
        })
    }

    pub fn open(path: &str) -> Result<Self, ContextError> {
        let data = std::fs::read(path).map_err(ContextError::Io)?;
        Keiyoushi::new(&data)
    }

    pub fn set_http<F>(&mut self, f: F)
    where
        F: Fn(&HttpData) -> HttpResp + 'static,
    {
        self.ctx.set_http(f);
    }

    pub fn set_http_rc(&mut self, f: Rc<dyn Fn(&HttpData) -> HttpResp>) {
        self.ctx.set_http(move |r| f(r));
    }

    /// Host-owned per-host header resolver (User-Agent + Cookie). The
    /// callback gets the lowercase request host (like
    /// `reqwest::Url::host_str()`); returned values are injected when the
    /// request does not set them itself.
    pub fn set_host_headers<F>(&mut self, f: F)
    where
        F: Fn(&str) -> (Option<String>, Option<String>) + 'static,
    {
        self.ctx.set_host_headers(f);
    }

    /// Selects the host-owned file used to persist Android
    /// `SharedPreferences`. Without this, preferences remain in memory only.
    pub fn set_shared_preferences_path(&mut self, path: impl AsRef<std::path::Path>) {
        self.ctx.set_shared_preferences_path(path);
    }

    /// Materializes the source's AndroidX preference screen and returns the
    /// captured definitions. Both mihon-era shapes are supported:
    /// `ConfigurableSource.setupPreferenceScreen(screen)` and, when the
    /// extension declares it, the newer `createPreferenceScreen(context)`.
    /// Needs no network.
    pub fn preference_definitions(
        &mut self,
        src: &Source,
    ) -> Result<Vec<SettingDefinition>, JvmError> {
        let ctx_inst = {
            let vm = self.vm();
            let cid = vm.ensure_class_by_desc("Landroid/content/Context;")?;
            vm.alloc_instance(cid)?
        };
        if let Ok(_screen) = self.ctx.invoke_on(
            src.inst,
            "createPreferenceScreen",
            "(Landroid/content/Context;)Landroidx/preference/PreferenceScreen;",
            &[JValue::Obj(ctx_inst)],
        ) {
            return Ok(self.get_all_setting_definitions());
        }
        let screen = {
            let vm = self.vm();
            let cid = vm.ensure_class_by_desc("Landroidx/preference/PreferenceScreen;")?;
            vm.arena.alloc(
                cid,
                Vec::new(),
                Some(Native::PreferenceScreen {
                    children: Vec::new(),
                    title: None,
                }),
            )
        };
        self.ctx.invoke_on(
            src.inst,
            "setupPreferenceScreen",
            "(Landroidx/preference/PreferenceScreen;)V",
            &[JValue::Obj(screen)],
        )?;
        Ok(self.get_all_setting_definitions())
    }

    pub fn get_all_setting_definitions(&self) -> Vec<SettingDefinition> {
        self.ctx.get_all_setting_definitions()
    }

    /// Resolves an arena object to its string payload, if it is one
    /// (e.g. the `default_value` of a [`SettingDefinition`]).
    pub fn string_of(&mut self, id: u32) -> Option<String> {
        self.ctx.string_of(id)
    }

    pub fn get_settings(
        &mut self,
        preference_file: &str,
    ) -> std::collections::HashMap<String, SettingValue> {
        self.ctx.get_settings(preference_file)
    }

    pub fn on_update_settings<F>(&mut self, callback: F)
    where
        F: Fn(&str, &SettingValue) + 'static,
    {
        self.ctx.on_update_settings(callback);
    }

    pub fn update_setting(
        &mut self,
        preference_file: &str,
        key: &str,
        value: SettingValue,
    ) -> std::io::Result<()> {
        self.ctx.update_setting(preference_file, key, value)
    }

    fn vm(&mut self) -> &mut Vm {
        self.ctx.vm()
    }

    /// Instantiates the extension's sources. Two shapes are supported,
    /// matching what mihon ships across extension generations:
    /// - modern factories: a class declaring `createSources()` (returning a
    ///   list, one source per bundled site variation);
    /// - legacy single-source apks where `ExtensionGenerated` inherits from
    ///   `HttpSource`/`Source` directly.
    pub fn sources(&mut self) -> Result<Vec<Source>, JvmError> {
        let factory = self.vm().find_factory_class("createSources");
        if let Ok(desc) = factory {
            self.ctx.call(&desc, "<init>", &[])?;
            let list = self.ctx.invoke("createSources", &[])?;
            let items = match list {
                JValue::Obj(id) => match &self.vm().arena.objects[id as usize].native {
                    Some(Native::List(items)) => items.clone(),
                    _ => return Err(JvmError::Resolution("createSources: bad result".into())),
                },
                _ => return Err(JvmError::Resolution("createSources: bad result".into())),
            };
            let mut out = Vec::new();
            for item in items {
                if let JValue::Obj(o) = item {
                    out.push(Source { inst: o });
                }
            }
            return Ok(out);
        }
        let desc = self.vm().find_http_source_subclass()?;
        self.ctx.call(&desc, "<init>", &[])?;
        let inst = self
            .ctx
            .last_instance()
            .ok_or_else(|| JvmError::Resolution(format!("{desc}: no instance after <init>")))?;
        Ok(vec![Source { inst }])
    }

    pub fn source_name(&mut self, src: &Source) -> Result<String, JvmError> {
        self.call_str(src, "getName", "()Ljava/lang/String;", &[])
    }

    pub fn source_lang(&mut self, src: &Source) -> Result<String, JvmError> {
        self.call_str(src, "getLang", "()Ljava/lang/String;", &[])
    }

    pub fn supports_latest(&mut self, src: &Source) -> Result<bool, JvmError> {
        let v = self
            .ctx
            .invoke_on(src.inst, "getSupportsLatest", "()Z", &[])?;
        Ok(!v.is_zero())
    }

    pub fn popular(&mut self, src: &Source, page: i32) -> Result<MangaPages, JvmError> {
        let req = self.ctx.invoke_on(
            src.inst,
            "popularMangaRequest",
            "(I)Lokhttp3/Request;",
            &[JValue::Int(page)],
        )?;
        let resp = self.execute(req)?;
        let mangas = self.ctx.invoke_on(
            src.inst,
            "popularMangaParse",
            "(Lokhttp3/Response;)Leu/kanade/tachiyomi/source/model/MangasPage;",
            &[resp],
        )?;
        self.manga_pages(mangas)
    }

    pub fn latest(&mut self, src: &Source, page: i32) -> Result<MangaPages, JvmError> {
        let req = self.ctx.invoke_on(
            src.inst,
            "latestUpdatesRequest",
            "(I)Lokhttp3/Request;",
            &[JValue::Int(page)],
        )?;
        let resp = self.execute(req)?;
        let mangas = self.ctx.invoke_on(
            src.inst,
            "latestUpdatesParse",
            "(Lokhttp3/Response;)Leu/kanade/tachiyomi/source/model/MangasPage;",
            &[resp],
        )?;
        self.manga_pages(mangas)
    }

    /// Executes a search: builds the request from the query plus the given
    /// filter states and parses the result.
    pub fn search(
        &mut self,
        src: &Source,
        page: i32,
        query: &str,
        filters: &[FilterState],
    ) -> Result<MangaPages, JvmError> {
        let flist = self.build_filter_list(filters)?;
        let query_obj = self.ctx.vm().alloc_string(query);
        let req = self.ctx.invoke_on(
            src.inst,
            "searchMangaRequest",
            "(ILjava/lang/String;Leu/kanade/tachiyomi/source/model/FilterList;)Lokhttp3/Request;",
            &[JValue::Int(page), query_obj, flist],
        )?;
        let resp = self.execute(req)?;
        let mangas = self.ctx.invoke_on(
            src.inst,
            "searchMangaParse",
            "(Lokhttp3/Response;)Leu/kanade/tachiyomi/source/model/MangasPage;",
            &[resp],
        )?;
        self.manga_pages(mangas)
    }

    pub fn manga_details(&mut self, src: &Source, manga: &Manga) -> Result<Manga, JvmError> {
        let m = self.alloc_manga(manga)?;
        let req = self.ctx.invoke_on(
            src.inst,
            "mangaDetailsRequest",
            "(Leu/kanade/tachiyomi/source/model/SManga;)Lokhttp3/Request;",
            &[m],
        )?;
        let resp = self.execute(req)?;
        let out = self.ctx.invoke_on(
            src.inst,
            "mangaDetailsParse",
            "(Lokhttp3/Response;)Leu/kanade/tachiyomi/source/model/SManga;",
            &[resp],
        )?;
        self.read_manga(out)?
            .ok_or_else(|| JvmError::Resolution("mangaDetailsParse: not a SManga".into()))
    }

    pub fn chapters(&mut self, src: &Source, manga: &Manga) -> Result<Vec<Chapter>, JvmError> {
        let m = self.alloc_manga(manga)?;
        let req = self.ctx.invoke_on(
            src.inst,
            "chapterListRequest",
            "(Leu/kanade/tachiyomi/source/model/SManga;)Lokhttp3/Request;",
            &[m],
        )?;
        let resp = self.execute(req)?;
        let list = self.ctx.invoke_on(
            src.inst,
            "chapterListParse",
            "(Lokhttp3/Response;)Ljava/util/List;",
            &[resp],
        )?;
        self.read_chapter_list(list)
    }

    pub fn pages(&mut self, src: &Source, chapter: &Chapter) -> Result<Vec<PageRef>, JvmError> {
        let (url, name) = (chapter.url.clone(), chapter.name.clone());
        let cid = self.ctx.vm().ensure_class_by_desc(SCHAPTER)?;
        let c = JValue::Obj(self.ctx.vm().arena.alloc(
            cid,
            Vec::new(),
            Some(empty_chapter(url, name)),
        ));
        let req = self.ctx.invoke_on(
            src.inst,
            "pageListRequest",
            "(Leu/kanade/tachiyomi/source/model/SChapter;)Lokhttp3/Request;",
            &[c],
        )?;
        let resp = self.execute(req)?;
        let list = self.ctx.invoke_on(
            src.inst,
            "pageListParse",
            "(Lokhttp3/Response;)Ljava/util/List;",
            &[resp],
        )?;
        self.read_page_list(list)
    }

    pub fn filters(&mut self, src: &Source) -> Result<Vec<FilterDef>, JvmError> {
        let flist = self.ctx.invoke_on(
            src.inst,
            "getFilterList",
            "()Leu/kanade/tachiyomi/source/model/FilterList;",
            &[],
        )?;
        self.read_filters(flist)
    }
}

/// Per-filter state used when building a [`Keiyoushi::search`] call.
#[derive(Debug, Clone, Default)]
pub struct FilterState {
    pub name: String,
    pub state: i32,
}

impl Keiyoushi {
    /// Executes a request through the registered HTTP callback.
    fn execute(&mut self, req: JValue) -> Result<JValue, JvmError> {
        let vm = self.ctx.vm();
        let key = (
            vm.intern("Leu/kanade/tachiyomi/network/RequestsKt;"),
            vm.intern("__host_execute"),
            vm.intern("(Lokhttp3/Request;)Lokhttp3/Response;"),
        );
        let f = *vm.natives.get(&key).ok_or_else(|| {
            JvmError::Resolution("keiyoushi bridge: __host_execute not registered".into())
        })?;
        match f(vm, &[req]) {
            Ok(v) => Ok(v),
            Err(crate::vm::NatErr::Throw(ex)) => Err(JvmError::Uncaught(ex)),
            Err(crate::vm::NatErr::Fatal(e)) => Err(e),
        }
    }

    /// Human-readable description of an error: for `Uncaught` includes the
    /// exception class and message from the VM heap.
    pub fn describe_error(&mut self, e: &JvmError) -> String {
        match e {
            JvmError::Uncaught(id) => {
                let vm = self.ctx.vm();
                let class = vm
                    .arena
                    .objects
                    .get(*id as usize)
                    .map(|o| vm.class_desc_str(o.class))
                    .unwrap_or_default();
                let msg = vm
                    .payload_of(JValue::Obj(*id))
                    .and_then(|p| match p {
                        crate::vm::object::Native::Throwable { message, .. } => message.clone(),
                        _ => None,
                    })
                    .unwrap_or_default();
                format!("uncaught {class}: {msg}")
            }
            other => other.to_string(),
        }
    }

    fn call_str(
        &mut self,
        src: &Source,
        method: &str,
        sig: &str,
        args: &[JValue],
    ) -> Result<String, JvmError> {
        let v = self.ctx.invoke_on(src.inst, method, sig, args)?;
        self.ctx
            .vm()
            .str_of_jvalue(v)
            .ok_or_else(|| JvmError::Resolution(format!("{method}: not a String")))
    }

    fn manga_pages(&mut self, v: JValue) -> Result<MangaPages, JvmError> {
        let (mangas, has_next) = match self.ctx.vm().payload_of(v) {
            Some(Native::SMangasPage { mangas, has_next }) => (mangas, has_next),
            _ => return Err(JvmError::Resolution("not a MangasPage".into())),
        };
        let mut out = Vec::with_capacity(mangas.len());
        for (i, m) in mangas.iter().enumerate() {
            let desc = if let JValue::Obj(o) = m {
                let vm = self.ctx.vm();
                let class = vm.arena.objects[*o as usize].class;
                vm.class_desc_str(class)
            } else {
                format!("{m:?}")
            };
            eprintln!("DEXTRACE manga[{i}] = {m:?} class={desc}");
            #[allow(clippy::manual_let_else)]
            let value = match m {
                JValue::Obj(_) => *m,
                _ => continue,
            };
            if let Some(manga) = self.read_manga(value)? {
                out.push(manga);
            }
        }
        Ok(MangaPages {
            mangas: out,
            has_next,
        })
    }

    fn read_manga(&mut self, v: JValue) -> Result<Option<Manga>, JvmError> {
        match self.ctx.vm().payload_of(v) {
            Some(Native::SManga {
                title,
                author,
                artist,
                description,
                genre,
                status,
                thumbnail_url,
                url,
                ..
            }) => Ok(Some(Manga {
                title,
                author: author.unwrap_or_default(),
                artist: artist.unwrap_or_default(),
                description: description.unwrap_or_default(),
                genre: genre.unwrap_or_default(),
                status,
                thumbnail_url,
                url,
            })),
            _ => Ok(None),
        }
    }

    fn read_chapter_list(&mut self, list: JValue) -> Result<Vec<Chapter>, JvmError> {
        let items = match self.ctx.vm().payload_of(list) {
            Some(Native::List(items)) => items,
            _ => return Err(JvmError::Resolution("not a List".into())),
        };
        let mut out = Vec::with_capacity(items.len());
        for c in items {
            if let Some(Native::SChapter {
                name,
                url,
                date_upload,
                scanlator,
                ..
            }) = self.ctx.vm().payload_of(c)
            {
                out.push(Chapter {
                    name,
                    url,
                    date_upload,
                    scanlator,
                });
            }
        }
        Ok(out)
    }

    fn read_page_list(&mut self, list: JValue) -> Result<Vec<PageRef>, JvmError> {
        let items = match self.ctx.vm().payload_of(list) {
            Some(Native::List(items)) => items,
            _ => {
                if std::env::var("DEXVM_TRACE").is_ok() {
                    eprintln!(
                        "DEXVM_TRACE read_page_list: not a List (value={list:?}, native={})",
                        self.ctx
                            .vm()
                            .payload_of(list)
                            .map(|n| {
                                match n {
                                    Native::Json(_) => "Json",
                                    Native::Str(_) => "Str",
                                    Native::SPPage { .. } => "SPPage",
                                    Native::SChapter { .. } => "SChapter",
                                    Native::SManga { .. } => "SManga",
                                    Native::Opaque => "Opaque",
                                    _ => "other",
                                }
                            })
                            .unwrap_or("none")
                    );
                }
                return Err(JvmError::Resolution("not a List".into()));
            }
        };
        if std::env::var("DEXVM_TRACE").is_ok() {
            eprintln!("DEXVM_TRACE read_page_list: items={}", items.len());
            if let Some(first) = items.first() {
                eprintln!(
                    "DEXVM_TRACE read_page_list: first item raw={first:?} class={}",
                    match first {
                        JValue::Obj(o) => {
                            let cls = self.ctx.vm().arena.objects[*o as usize].class;
                            format!("{} (idx {o})", self.ctx.vm().class_desc_str(cls))
                        }
                        _ => "-".to_string(),
                    }
                );
                let n = self.ctx.vm().payload_of(*first);
                eprintln!(
                    "DEXVM_TRACE read_page_list: first item payload={}",
                    match n {
                        Some(Native::SPPage { .. }) => "SPPage",
                        Some(Native::Str(_)) => "Str",
                        Some(Native::Json(_)) => "Json",
                        Some(Native::Opaque) => "Opaque",
                        Some(_) => "other",
                        None => "none",
                    }
                );
            }
        }
        let mut out = Vec::with_capacity(items.len());
        for p in items {
            if let Some(Native::SPPage {
                index,
                name,
                url,
                image_url,
            }) = self.ctx.vm().payload_of(p)
            {
                out.push(PageRef {
                    index,
                    name,
                    url,
                    image_url,
                });
            }
        }
        Ok(out)
    }

    fn read_filters(&mut self, flist: JValue) -> Result<Vec<FilterDef>, JvmError> {
        let items = match self.ctx.vm().payload_of(flist) {
            Some(Native::SFilterList(items)) => items,
            _ => return Err(JvmError::Resolution("not a FilterList".into())),
        };
        let mut out = Vec::with_capacity(items.len());
        for f in items {
            let id = f.as_obj();
            let kind = self.filter_kind(id);
            if let Some(Native::SFilter {
                name,
                state,
                options,
                children,
                ..
            }) = self.ctx.vm().payload_of(f)
            {
                if kind == FilterKind::Group {
                    out.push(FilterDef {
                        kind,
                        name,
                        state,
                        options: self.str_options(options)?,
                    });
                    for c in children {
                        let cid = c.as_obj();
                        let k = self.filter_kind(cid);
                        if let Some(Native::SFilter {
                            name,
                            state,
                            options,
                            ..
                        }) = self.ctx.vm().payload_of(c)
                        {
                            out.push(FilterDef {
                                kind: k,
                                name,
                                state,
                                options: self.str_options(options)?,
                            });
                        }
                    }
                } else {
                    out.push(FilterDef {
                        kind,
                        name,
                        state,
                        options: self.str_options(options)?,
                    });
                }
            }
        }
        Ok(out)
    }

    fn str_options(&mut self, options: Vec<JValue>) -> Result<Vec<String>, JvmError> {
        let mut out = Vec::with_capacity(options.len());
        for o in options {
            if let Some(s) = self.ctx.vm().str_of_jvalue(o) {
                out.push(s);
            }
        }
        Ok(out)
    }

    fn filter_kind(&mut self, obj: u32) -> FilterKind {
        let mut cid = self.ctx.vm().arena.objects[obj as usize].class;
        loop {
            let cid_now = cid;
            let (desc_id, sup) = {
                let Some(c) = self.ctx.vm().classes.get(cid_now as usize) else {
                    break;
                };
                (c.descriptor, c.superclass)
            };
            let desc = self.ctx.vm().str_of(desc_id);
            match desc {
                s if s == FILTER_LIST => return FilterKind::Plain,
                s if s.contains("Text") => return FilterKind::Text,
                s if s.contains("Select") => return FilterKind::Select,
                s if s.contains("TriState") => return FilterKind::TriState,
                s if s.contains("Group") => return FilterKind::Group,
                s if s.contains("Header") => return FilterKind::Plain,
                s if s.contains("Separator") => return FilterKind::Separator,
                s if s.ends_with("Filter;") => return FilterKind::Plain,
                _ => {}
            }
            match sup {
                Some(s) if s != cid_now => cid = s,
                _ => break,
            }
        }
        FilterKind::Plain
    }

    fn alloc_manga(&mut self, m: &Manga) -> Result<JValue, JvmError> {
        let cid = self.ctx.vm().ensure_class_by_desc(SMANGA)?;
        let payload = Native::SManga {
            title: m.title.clone(),
            author: Some(m.author.clone()).filter(|s| !s.is_empty()),
            artist: Some(m.artist.clone()).filter(|s| !s.is_empty()),
            description: Some(m.description.clone()).filter(|s| !s.is_empty()),
            genre: Some(m.genre.clone()).filter(|s| !s.is_empty()),
            status: m.status,
            thumbnail_url: m.thumbnail_url.clone(),
            url: m.url.clone(),
            update_strategy: JValue::Null,
            memo: JValue::Null,
        };
        Ok(JValue::Obj(self.ctx.vm().arena.alloc(
            cid,
            Vec::new(),
            Some(payload),
        )))
    }

    /// Builds a FilterList object carrying the given per-filter states.
    fn build_filter_list(&mut self, states: &[FilterState]) -> Result<JValue, JvmError> {
        let cid = self.ctx.vm().ensure_class_by_desc(FILTER_LIST)?;
        let mut children = Vec::new();
        for s in states {
            let fc = self.ctx.vm().ensure_class_by_desc(FILTER)?;
            let payload = Native::SFilter {
                name: s.name.clone(),
                state: s.state,
                is_checked: false,
                children: Vec::new(),
                options: Vec::new(),
                text_value: String::new(),
            };
            children.push(JValue::Obj(self.ctx.vm().arena.alloc(
                fc,
                Vec::new(),
                Some(payload),
            )));
        }
        Ok(JValue::Obj(self.ctx.vm().arena.alloc(
            cid,
            Vec::new(),
            Some(Native::SFilterList(children)),
        )))
    }

    /// Allocates a shim continuation so suspend-style dex functions
    /// (`getPopularManga`, `getSearchManga`, ...) run their coroutine state
    /// machine to completion synchronously: every network native resolves
    /// inline, the frame never actually suspends, and the finished value is
    /// returned directly instead of `COROUTINE_SUSPENDED`.
    fn suspend_cont(&mut self) -> Result<JValue, JvmError> {
        let cid = self
            .ctx
            .vm()
            .ensure_class_by_desc("Lkotlin/coroutines/jvm/internal/ContinuationImpl;")?;
        Ok(JValue::Obj(self.ctx.vm().alloc_instance(cid)?))
    }

    /// `getPopularManga` (suspend) — the coroutine entry point used by
    /// modern keiyoushi sources (request/parse pairs are stubbed there).
    pub fn popular_coro(&mut self, src: &Source, page: i32) -> Result<MangaPages, JvmError> {
        let cont = self.suspend_cont()?;
        let out = self.ctx.invoke_on(
            src.inst,
            "getPopularManga",
            "(ILkotlin/coroutines/Continuation;)Ljava/lang/Object;",
            &[JValue::Int(page), cont],
        )?;
        self.manga_pages(out)
    }

    /// `getLatestUpdates` (suspend).
    pub fn latest_coro(&mut self, src: &Source, page: i32) -> Result<MangaPages, JvmError> {
        let cont = self.suspend_cont()?;
        let out = self.ctx.invoke_on(
            src.inst,
            "getLatestUpdates",
            "(ILkotlin/coroutines/Continuation;)Ljava/lang/Object;",
            &[JValue::Int(page), cont],
        )?;
        self.manga_pages(out)
    }

    /// `getSearchManga` (suspend), with the query plus per-filter states.
    pub fn search_coro(
        &mut self,
        src: &Source,
        page: i32,
        query: &str,
        filters: &[FilterState],
    ) -> Result<MangaPages, JvmError> {
        let flist = self.build_filter_list(filters)?;
        let query_obj = self.ctx.vm().alloc_string(query);
        let cont = self.suspend_cont()?;
        let out = self.ctx.invoke_on(
            src.inst,
            "getSearchManga",
            "(ILjava/lang/String;Leu/kanade/tachiyomi/source/model/FilterList;Lkotlin/coroutines/Continuation;)Ljava/lang/Object;",
            &[JValue::Int(page), query_obj, flist, cont],
        )?;
        self.manga_pages(out)
    }

    /// `getPageList` (suspend) against a synthetic chapter ref.
    pub fn pages_coro(
        &mut self,
        src: &Source,
        chapter: &Chapter,
    ) -> Result<Vec<PageRef>, JvmError> {
        let (url, name) = (chapter.url.clone(), chapter.name.clone());
        let cid = self.ctx.vm().ensure_class_by_desc(SCHAPTER)?;
        let c = JValue::Obj(self.ctx.vm().arena.alloc(
            cid,
            Vec::new(),
            Some(empty_chapter(url, name)),
        ));
        let cont = self.suspend_cont()?;
        let out = self.ctx.invoke_on(
            src.inst,
            "getPageList",
            "(Leu/kanade/tachiyomi/source/model/SChapter;Lkotlin/coroutines/Continuation;)Ljava/lang/Object;",
            &[c, cont],
        )?;
        self.read_page_list(out)
    }

    /// `getMangaUpdate` (suspend) — the combined details+chapters entry point
    /// of the tachiyomix 1.6 era, which is what mihon 0.20.1+ calls instead
    /// of `getMangaDetails`/`getChapterList`. Returns the `SMangaUpdate`
    /// object; the caller reads the manga or chapter half off it.
    pub fn manga_update_coro(
        &mut self,
        src: &Source,
        manga: &Manga,
        fetch_details: bool,
        fetch_chapters: bool,
    ) -> Result<JValue, JvmError> {
        let m = self.alloc_manga(manga)?;
        let cid = self
            .ctx
            .vm()
            .ensure_class_by_desc("Ljava/util/ArrayList;")?;
        let empty_list = JValue::Obj(self.ctx.vm().arena.alloc(
            cid,
            Vec::new(),
            Some(Native::List(Vec::new())),
        ));
        let cont = self.suspend_cont()?;
        self.ctx.invoke_on(
            src.inst,
            "getMangaUpdate",
            "(Leu/kanade/tachiyomi/source/model/SManga;Ljava/util/List;ZZLkotlin/coroutines/Continuation;)Ljava/lang/Object;",
            &[
                m,
                empty_list,
                JValue::Int(i32::from(fetch_details)),
                JValue::Int(i32::from(fetch_chapters)),
                cont,
            ],
        )
    }

    /// `getMangaUpdate(..., true, false)` read as a manga.
    pub fn manga_update_details(&mut self, src: &Source, manga: &Manga) -> Result<Manga, JvmError> {
        let out = self.manga_update_coro(src, manga, true, false)?;
        let JValue::Obj(out_id) = out else {
            return Err(JvmError::Resolution("getMangaUpdate: not an object".into()));
        };
        let smanga = self.ctx.invoke_on(
            out_id,
            "getManga",
            "()Leu/kanade/tachiyomi/source/model/SManga;",
            &[],
        )?;
        self.read_manga(smanga)?
            .ok_or_else(|| JvmError::Resolution("getMangaUpdate: not a SManga".into()))
    }

    /// `getMangaUpdate(..., false, true)` read as a chapter list.
    pub fn manga_update_chapters(
        &mut self,
        src: &Source,
        manga: &Manga,
    ) -> Result<Vec<Chapter>, JvmError> {
        let out = self.manga_update_coro(src, manga, false, true)?;
        let JValue::Obj(out_id) = out else {
            return Err(JvmError::Resolution("getMangaUpdate: not an object".into()));
        };
        let list = self
            .ctx
            .invoke_on(out_id, "getChapters", "()Ljava/util/List;", &[])?;
        self.read_chapter_list(list)
    }
}

fn empty_chapter(url: String, name: String) -> Native {
    Native::SChapter {
        name,
        url,
        date_upload: 0,
        scanlator: String::new(),
        chapter_number: 0.0,
        memo: JValue::Null,
    }
}
