//! The VM: class loading/resolution, the object arena, strings, exceptions.

pub mod class;
pub mod crypto;
pub mod error;
pub mod interpret;
pub mod native;
pub mod object;
pub mod value;

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use class::{Class, Method, ShimValue, ACC_NATIVE, ACC_STATIC, SHIM_CLASSES};
use error::JvmError;
use object::{Arena, ClassOrPrim, Native, PreferenceValue};
use value::{default_of, JValue};

use crate::dex::insn::InvokeKind;
use crate::dex::{CodeItem, DexFile, EncodedValue};

/// What a native bridge can produce.
#[derive(Debug)]
pub enum NatErr {
    /// A Java throwable (interned throwable object id) was raised.
    Throw(u32),
    /// VM-level failure, not a Java exception.
    Fatal(JvmError),
}

/// Native bridge signature. `args` includes the receiver for instance methods.
pub type NativeFn = fn(&mut Vm, &[JValue]) -> Result<JValue, NatErr>;

#[derive(Debug, Clone, Copy)]
pub struct NativeEntry {
    pub class: &'static str,
    pub name: &'static str,
    pub sig: &'static str,
    /// true = instance method (receiver in `args[0]`).
    pub instance: bool,
    pub f: NativeFn,
}

/// Pre-interned ids for classes/names the VM checks frequently.
pub struct Hot {
    pub clinit: u32,
    pub init: u32,
    pub object: u32,
    pub string: u32,
    pub throwable: u32,
    pub npe: u32,
    pub arithmetic: u32,
    pub aioobe: u32,
    pub ioobe: u32,
    pub cce: u32,
    pub nas: u32,
    pub uoe: u32,
    pub iae: u32,
    pub string_builder: u32,
    pub class_: u32,
    pub default_ctor_marker: u32,
}

/// A resolved method reference (signature data pulled from the dex tables).
#[derive(Debug, Clone)]
pub struct MethodRef {
    pub name: u32,
    pub sig: u32,
    pub ret: u32,
    pub args: Vec<u32>,
    /// Interned descriptor of the method ref's class.
    pub class_desc: u32,
}

/// Where a call lands.
#[derive(Debug, Clone)]
pub enum Target {
    Native((u32, u32, u32)),
    Bytecode {
        class: u32,
        slot: u32,
        decoded: Arc<crate::dex::insn::Decoded>,
        code: Option<Arc<CodeItem>>,
        ins_size: u16,
        registers: u16,
        ret: u32,
        args: Vec<u32>,
        static_method: bool,
    },
}

/// A resolved field reference.
#[derive(Debug, Clone, Copy)]
pub struct FieldRef {
    pub name: u32,
    pub ty: u32,
    /// Interned descriptor of the field's declaring class.
    pub class_desc: u32,
}

pub struct Vm {
    /// All dex files of the loaded program. Ids in dex tables (strings,
    /// types, methods, fields) are relative to the file that defines the
    /// referencing class; the class namespace is shared across dexes with
    /// earlier files winning on duplicates.
    pub dexes: Vec<DexFile>,
    /// Non-DEX entries retained from the application and library APKs.
    /// These are package resources, not guest filesystem paths.
    pub resources: HashMap<String, Vec<u8>>,
    pub intern: Vec<Arc<str>>,
    pub intern_map: HashMap<String, u32>,
    pub classes: Vec<Class>,
    pub class_by_desc: HashMap<u32, u32>,
    /// (dex index, dex type id) -> loaded class id.
    pub class_by_type: HashMap<(u32, u32), u32>,
    pub arena: Arena,
    /// (dex index, dex string id) -> String object.
    pub string_objs: HashMap<(u32, u32), u32>,
    pub runtime_strings: HashMap<String, u32>,
    pub natives: HashMap<(u32, u32, u32), NativeFn>,
    pub monitors: HashMap<u32, usize>,
    pub method_refs: HashMap<(u32, u32), MethodRef>,
    pub field_refs: HashMap<(u32, u32), FieldRef>,
    pub out: Box<dyn Write>,
    pub budget: i64,
    pub depth_limit: usize,
    /// Nested VM-entry depth (each entry = one Rust stack level via [`interpret::run`]).
    pub recursion_depth: usize,
    /// Ring of recently entered guest methods (for overflow diagnostics).
    pub trace_ring: VecDeque<String>,
    pub hot: Hot,
    pub frames: Vec<crate::vm::interpret::Frame>,
    /// Host-registered natives (see [`Vm::register_native`]).
    pub host_natives: Vec<NativeEntry>,
    /// Sandbox capability grants checked by host natives.
    pub perms: crate::permission::Permissions,
    pub array_classes: HashMap<(u32, u32), u32>,
    #[cfg(feature = "tachiyomi")]
    pub http: Option<native::keiyoushi::HttpCall>,
    /// Host callback resolving per-host request headers (User-Agent and
    /// Cookie) from a host-owned store. Called right before each HTTP
    /// request with the lowercase host of the request URL (the same string
    /// `reqwest::Url::host_str()` yields); the returned values are injected
    /// as headers only when the request does not already set them.
    #[cfg(feature = "tachiyomi")]
    pub host_headers: Option<native::keiyoushi::HostHeaderFn>,
    /// Real host directory backing `Context.getCacheDir()` (created on first
    /// use so extension cache logic runs against a genuine filesystem).
    pub cache_root: Option<String>,
    /// Per-context Android SharedPreferences store.
    pub shared_preferences: HashMap<String, HashMap<String, PreferenceValue>>,
    /// Optional host-owned persistence file for `shared_preferences`.
    pub shared_preferences_path: Option<PathBuf>,
    /// Whether the configured preferences file has been loaded.
    pub shared_preferences_loaded: bool,
    /// FullTypeReference subclass descriptor -> concrete type descriptor,
    /// derived from dex bytecode (`getInstance` result check-casts).
    injekt_type_by_subclass: HashMap<u32, u32>,
    /// class count when the injekt registry was last rebuilt; the scan re-runs
    /// whenever new classes have been loaded since.
    injekt_scanned_classes: usize,
    loading: Vec<(u32, usize)>,
}

impl Vm {
    pub fn new(dexes: Vec<DexFile>, out: Box<dyn Write>) -> Result<Vm, JvmError> {
        let mut vm = Vm {
            dexes,
            resources: HashMap::new(),
            intern: Vec::new(),
            intern_map: HashMap::new(),
            classes: Vec::new(),
            class_by_desc: HashMap::new(),
            class_by_type: HashMap::new(),
            arena: Arena::default(),
            string_objs: HashMap::new(),
            runtime_strings: HashMap::new(),
            natives: HashMap::new(),
            monitors: HashMap::new(),
            method_refs: HashMap::new(),
            field_refs: HashMap::new(),
            out,
            budget: 50_000_000,
            depth_limit: 700,
            recursion_depth: 0,
            trace_ring: VecDeque::with_capacity(24),
            frames: Vec::new(),
            hot: Hot {
                clinit: 0,
                init: 0,
                object: 0,
                string: 0,
                throwable: 0,
                npe: 0,
                arithmetic: 0,
                aioobe: 0,
                ioobe: 0,
                cce: 0,
                nas: 0,
                uoe: 0,
                iae: 0,
                string_builder: 0,
                class_: 0,
                default_ctor_marker: 0,
            },
            array_classes: HashMap::new(),
            loading: Vec::new(),
            injekt_type_by_subclass: HashMap::new(),
            injekt_scanned_classes: 0,
            cache_root: None,
            shared_preferences: HashMap::new(),
            shared_preferences_path: None,
            shared_preferences_loaded: false,
            #[cfg(feature = "tachiyomi")]
            http: None,
            #[cfg(feature = "tachiyomi")]
            host_headers: None,
            host_natives: Vec::new(),
            perms: crate::permission::Permissions::new(),
        };
        vm.hot.clinit = vm.intern("<clinit>");
        vm.hot.init = vm.intern("<init>");
        vm.hot.object = vm.intern("Ljava/lang/Object;");
        let string_desc = vm.intern("Ljava/lang/String;");
        vm.hot.string = vm.ensure_class_by_desc_id(string_desc)?;
        vm.hot.throwable = vm.intern("Ljava/lang/Throwable;");
        vm.hot.npe = vm.intern("Ljava/lang/NullPointerException;");
        vm.hot.arithmetic = vm.intern("Ljava/lang/ArithmeticException;");
        vm.hot.aioobe = vm.intern("Ljava/lang/ArrayIndexOutOfBoundsException;");
        vm.hot.ioobe = vm.intern("Ljava/lang/IndexOutOfBoundsException;");
        vm.hot.cce = vm.intern("Ljava/lang/ClassCastException;");
        vm.hot.nas = vm.intern("Ljava/lang/NegativeArraySizeException;");
        vm.hot.uoe = vm.intern("Ljava/lang/UnsupportedOperationException;");
        vm.hot.iae = vm.intern("Ljava/lang/IllegalArgumentException;");
        vm.hot.string_builder = vm.intern("Ljava/lang/StringBuilder;");
        vm.hot.class_ = vm.intern("Ljava/lang/Class;");
        vm.hot.default_ctor_marker = vm.intern("Lkotlin/jvm/internal/DefaultConstructorMarker;");
        // bootstrap shim classes so that `Object` always exists (class id 0)
        vm.ensure_class_by_desc_id(vm.hot.object)?;
        vm.register_natives();
        Ok(vm)
    }

    // ---- interning ----

    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.intern_map.get(s) {
            return id;
        }
        let id = self.intern.len() as u32;
        let arc: Arc<str> = Arc::from(s);
        self.intern_map.insert(s.to_string(), id);
        self.intern.push(arc);
        id
    }

    pub fn str_of(&self, id: u32) -> &str {
        &self.intern[id as usize]
    }

    /// Dotted class name for a class id, e.g. `a.b.Main`.
    pub fn class_desc_str(&self, class: u32) -> String {
        crate::vm::value::dotted_name(self.str_of(self.classes[class as usize].descriptor))
    }

    /// `Class.methodName(args)ret` for a class id + method slot.
    pub fn method_desc_str(&self, class: u32, slot: u32) -> String {
        let cls = &self.classes[class as usize];
        let m = &cls.methods[slot as usize];
        format!(
            "{}.{}{}",
            crate::vm::value::dotted_name(self.str_of(cls.descriptor)),
            self.str_of(m.name),
            self.str_of(m.sig)
        )
    }

    /// Runtime generic signature of a loaded class (from its dex
    /// `Ldalvik/annotation/Signature;` annotation), if any.
    pub fn generic_signature(&self, class: u32) -> Option<String> {
        let desc = self.str_of(self.classes[class as usize].descriptor);
        let (dex_idx, def_idx) = self.class_location(desc)?;
        self.dex_at(dex_idx).classes[def_idx]
            .generic_signature
            .clone()
    }

    /// Concrete type of an injekt `FullTypeReference` subclass, derived from
    /// bytecode: the subclass is `new`-ed, passed through `getType()` into
    /// `InjektFactory.getInstance(...)`, whose result is `check-cast` to the
    /// concrete type. Used when the dex carries no generic `Signature`
    /// annotation (obfuscated/minified APKs).
    pub fn injekt_type_of(&mut self, subclass_desc: u32) -> Option<u32> {
        if self.injekt_scanned_classes != self.classes.len() {
            self.scan_injekt_types();
        }
        self.injekt_type_by_subclass.get(&subclass_desc).copied()
    }

    fn scan_injekt_types(&mut self) {
        self.injekt_scanned_classes = self.classes.len();
        use crate::dex::insn::{decode_all, Insn};
        // Phase 1 (immutable): collect (subclass desc, type desc) pairs.
        let mut pairs: Vec<(String, String)> = Vec::new();
        let get_type_cls = "Luy/kohesive/injekt/api/FullTypeReference;";
        let factory_cls = "Luy/kohesive/injekt/api/InjektFactory;";
        for class in 0..self.classes.len() as u32 {
            for m in &self.classes[class as usize].methods {
                let Some(code) = &m.code else {
                    continue;
                };
                let Ok(decoded) = decode_all(&code.insns) else {
                    continue;
                };
                let dex = self.dex_at(m.dex_idx);
                // register -> dex type id of the last `new-instance`.
                let mut last_new: Vec<Option<u32>> = vec![None; code.registers_size as usize];
                // register -> subclass descriptor of the last getType result.
                let mut type_reg: Vec<Option<String>> = vec![None; code.registers_size as usize];
                // set by getType; the next move-result carries its value.
                let mut pending_get_type: Option<String> = None;
                // set by getInstance; the next move-result carries its value.
                let mut want_result: Option<String> = None;
                // (result reg, subclass) awaiting the immediate check-cast.
                let mut pending_inst: Option<(u8, String)> = None;
                for insn in decoded.insns.iter() {
                    match insn {
                        Insn::NewInstance(reg, type_idx) => {
                            if let Some(slot) = last_new.get_mut(*reg as usize) {
                                *slot = Some(*type_idx);
                            }
                        }
                        Insn::MoveResult(reg) | Insn::MoveResultWide(reg) => {
                            if let Some(sub) = pending_get_type.take() {
                                if let Some(slot) = type_reg.get_mut(*reg as usize) {
                                    *slot = Some(sub);
                                }
                            }
                            if let Some(sub) = want_result.take() {
                                pending_inst = Some((*reg, sub));
                            }
                            if let Some(slot) = last_new.get_mut(*reg as usize) {
                                *slot = None;
                            }
                        }
                        Insn::Invoke(_, method_idx, args) => {
                            let Some(mref) = dex.methods.get(*method_idx as usize) else {
                                continue;
                            };
                            let name = dex
                                .strings
                                .get(mref.name as usize)
                                .map(|s| s.as_ref())
                                .unwrap_or("");
                            let owner = dex
                                .strings
                                .get(dex.types.get(mref.class as usize).copied().unwrap_or(0)
                                    as usize)
                                .map(|s| s.as_ref())
                                .unwrap_or("");
                            match name {
                                "getType" if owner == get_type_cls => {
                                    let recv = args.reg_at(0) as usize;
                                    if let Some(Some(sid)) = last_new.get(recv) {
                                        pending_get_type =
                                            Some(dex.type_descriptor(*sid).to_string());
                                    }
                                }
                                "getInstance" if owner == factory_cls => {
                                    let targ = args.reg_at(1) as usize;
                                    if let Some(Some(sub)) = type_reg.get(targ) {
                                        want_result = Some(sub.clone());
                                    }
                                }
                                _ => {}
                            }
                        }
                        Insn::CheckCast(reg, type_idx) => {
                            if let Some((r, sub)) = pending_inst.take() {
                                if r == *reg {
                                    let t = dex.type_descriptor(*type_idx);
                                    pairs.push((sub, t.to_string()));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        // Phase 2 (mutable): intern and record.
        for (sub, t) in pairs {
            let sub_id = self.intern(&sub);
            let t_id = self.intern(&t);
            self.injekt_type_by_subclass.insert(sub_id, t_id);
        }
    }

    // ---- class loading ----

    /// The dex file whose id tables the given index refers to.
    pub fn dex_at(&self, dex_idx: u32) -> &DexFile {
        &self.dexes[dex_idx as usize]
    }

    /// Locates a class descriptor across all dex files, earlier dexes win.
    pub fn class_location(&self, desc: &str) -> Option<(u32, usize)> {
        for (i, dex) in self.dexes.iter().enumerate() {
            if let Some(def_idx) = dex.class_by_descriptor(desc) {
                return Some((i as u32, def_idx));
            }
        }
        None
    }

    pub fn ensure_class_by_type(&mut self, dex_idx: u32, type_id: u32) -> Result<u32, JvmError> {
        if let Some(&c) = self.class_by_type.get(&(dex_idx, type_id)) {
            return Ok(c);
        }
        let desc = self.dex_at(dex_idx).type_descriptor(type_id).to_string();
        if desc.starts_with('[') {
            let desc_id = self.intern(&desc);
            if let Some(&c) = self.class_by_desc.get(&desc_id) {
                self.class_by_type.insert((dex_idx, type_id), c);
                return Ok(c);
            }
            if let Some(inner) = desc.strip_prefix('[') {
                if let Some(inner_tid) = self.dex_at(dex_idx).type_id_of(inner) {
                    return self.array_class(dex_idx, inner_tid);
                }
            }
            return self.synth_array_class(desc_id);
        }
        let desc_id = self.intern(&desc);
        if let Some(&c) = self.class_by_desc.get(&desc_id) {
            self.class_by_type.insert((dex_idx, type_id), c);
            return Ok(c);
        }
        if let Some((dx, def_idx)) = self.class_location(&desc) {
            let c = self.load_dex_class(dx, def_idx)?;
            self.class_by_type.insert((dex_idx, type_id), c);
            return Ok(c);
        }
        if self.shim_or_native(&desc) {
            let c = self.load_shim_class(desc_id)?;
            self.class_by_type.insert((dex_idx, type_id), c);
            return Ok(c);
        }
        Err(JvmError::Resolution(format!("class not found: {desc}")))
    }

    pub fn ensure_class_by_desc(&mut self, desc: &str) -> Result<u32, JvmError> {
        let id = self.intern(desc);
        self.ensure_class_by_desc_id(id)
    }

    pub fn ensure_class_by_desc_id(&mut self, desc_id: u32) -> Result<u32, JvmError> {
        if let Some(&c) = self.class_by_desc.get(&desc_id) {
            return Ok(c);
        }
        let desc = self.str_of(desc_id).to_string();
        if desc.starts_with('[') {
            // find the dex type id for the inner descriptor of this array
            // descriptor (any dex), so array classes link to the exact
            // element type and nested-array assignability terminates.
            let inner = &desc[1..];
            for (di, dex) in self.dexes.iter().enumerate() {
                let Some(inner_tid) = dex.type_id_of(inner) else {
                    continue;
                };
                return self.array_class(di as u32, inner_tid);
            }
            // No dex defines the descriptor (test fixtures): synthesize a
            // plain array class. Real dexes always carry array descriptors,
            // so this path only fires for synthetic/programmatic arrays.
            return self.synth_array_class(desc_id);
        }
        if let Some((dx, def_idx)) = self.class_location(&desc) {
            return self.load_dex_class(dx, def_idx);
        }
        if self.shim_or_native(&desc) {
            return self.load_shim_class(desc_id);
        }
        Err(JvmError::Resolution(format!("class not found: {desc}")))
    }

    fn load_dex_class(&mut self, dex_idx: u32, def_idx: usize) -> Result<u32, JvmError> {
        let def = self.dex_at(dex_idx).classes[def_idx].clone();
        if let Some(&c) = self.class_by_type.get(&(dex_idx, def.class_idx)) {
            return Ok(c);
        }
        let desc = self
            .dex_at(dex_idx)
            .type_descriptor(def.class_idx)
            .to_string();
        let desc_id = self.intern(&desc);
        if let Some(&c) = self.class_by_desc.get(&desc_id) {
            self.class_by_type.insert((dex_idx, def.class_idx), c);
            return Ok(c);
        }
        if self.loading.contains(&(dex_idx, def_idx)) {
            return Err(JvmError::Resolution("cyclic class hierarchy".into()));
        }
        self.loading.push((dex_idx, def_idx));

        let is_object = desc_id == self.hot.object;
        let superclass = if def.superclass_idx == u32::MAX {
            if is_object {
                None
            } else {
                Some(self.ensure_class_by_desc_id(self.hot.object)?)
            }
        } else {
            Some(self.ensure_class_by_type(dex_idx, def.superclass_idx)?)
        };
        let mut interfaces = Vec::new();
        for &t in &def.interfaces {
            interfaces.push(self.ensure_class_by_type(dex_idx, t)?);
        }

        let id = self.classes.len() as u32;
        self.classes.push(Class {
            id,
            descriptor: desc_id,
            superclass,
            interfaces: interfaces.clone(),
            access_flags: def.access_flags,
            is_interface: def.access_flags & class::ACC_INTERFACE != 0,
            is_abstract: def.access_flags & class::ACC_ABSTRACT != 0,
            ..Default::default()
        });
        self.class_by_desc.insert(desc_id, id);
        self.class_by_type.insert((dex_idx, def.class_idx), id);

        // instance fields: inherit super's offsets, then assign own
        let mut field_offsets = superclass
            .map(|s| self.classes[s as usize].field_offsets.clone())
            .unwrap_or_default();
        let mut instance_fields = superclass
            .map(|s| self.classes[s as usize].instance_fields.clone())
            .unwrap_or_default();
        let mut static_fields = superclass
            .map(|s| self.classes[s as usize].static_fields.clone())
            .unwrap_or_default();

        let mut statics: Vec<JValue> = Vec::new();
        let mut statics_lazy: Vec<Option<class::ShimLazy>> = Vec::new();
        let mut methods: Vec<Method> = Vec::new();
        let mut dispatch: HashMap<(u32, u32), u32> = HashMap::new();

        if let Some(cd) = &def.class_data {
            for ef in &cd.static_fields {
                let f = self.dex_at(dex_idx).fields[ef.field_idx as usize].clone();
                let f_name = self.dex_at(dex_idx).strings[f.name as usize].clone();
                let name = self.intern(&f_name);
                let ty_desc = self.dex_at(dex_idx).type_descriptor(f.ty).to_owned();
                let ty = self.intern(&ty_desc);
                let off = statics.len() as u32;
                statics.push(JValue::Null);
                statics_lazy.push(None);
                static_fields.insert((name, ty), (id, off));
            }
            for ef in &cd.instance_fields {
                let f = self.dex_at(dex_idx).fields[ef.field_idx as usize].clone();
                let f_name = self.dex_at(dex_idx).strings[f.name as usize].clone();
                let name = self.intern(&f_name);
                let ty_desc = self.dex_at(dex_idx).type_descriptor(f.ty).to_owned();
                let ty = self.intern(&ty_desc);
                let off = instance_fields.len() as u32;
                field_offsets.insert((name, ty), off);
                instance_fields.push((name, ty, ef.access_flags));
            }
            // encoded static values (in declaration order)
            for (i, ev) in def.static_values.iter().enumerate() {
                let ty = self.dex_at(dex_idx).fields[cd.static_fields[i].field_idx as usize].ty;
                let ty_desc = self.dex_at(dex_idx).type_descriptor(ty).to_string();
                let ty_id = self.intern(&ty_desc);
                statics[i] = self.enc_to_value(ev, dex_idx, ty_id)?;
            }
            let push_methods = |vm: &mut Self,
                                dex_idx: u32,
                                list: &[crate::dex::EncodedMethod],
                                methods: &mut Vec<Method>,
                                dispatch: &mut HashMap<(u32, u32), u32>,
                                class_id: u32|
             -> Result<(), JvmError> {
                for em in list {
                    let m = vm.dex_at(dex_idx).methods[em.method_idx as usize].clone();
                    let m_name = vm.dex_at(dex_idx).strings[m.name as usize].clone();
                    let name = vm.intern(&m_name);
                    let sig = vm.intern(&vm.proto_sig(dex_idx, m.proto));
                    let ret_desc = vm
                        .dex_at(dex_idx)
                        .type_descriptor(vm.dex_at(dex_idx).protos[m.proto as usize].return_type)
                        .to_string();
                    let ret = vm.intern(&ret_desc);
                    let arg_descs: Vec<String> = vm.dex_at(dex_idx).protos[m.proto as usize]
                        .params
                        .iter()
                        .map(|&t| vm.dex_at(dex_idx).type_descriptor(t).to_string())
                        .collect();
                    let args: Vec<u32> = arg_descs.iter().map(|d| vm.intern(d)).collect();
                    let static_method = em.access_flags & ACC_STATIC != 0;
                    let native_decl = em.access_flags & ACC_NATIVE != 0;
                    let slot = methods.len() as u32;
                    dispatch.insert((name, sig), slot);
                    methods.push(Method {
                        slot,
                        class: class_id,
                        name,
                        sig,
                        ret,
                        args,
                        access_flags: em.access_flags,
                        static_method,
                        dex_idx,
                        native_key: None,
                        native_decl,
                        code: em.code.clone(),
                        insns: OnceLock::new(),
                    });
                }
                Ok(())
            };
            push_methods(
                self,
                dex_idx,
                &cd.direct_methods,
                &mut methods,
                &mut dispatch,
                id,
            )?;
            push_methods(
                self,
                dex_idx,
                &cd.virtual_methods,
                &mut methods,
                &mut dispatch,
                id,
            )?;
        }

        // A real DEX declaration always wins. Native entries only fill API
        // holes in a partially supplied class, or provide the implementation
        // for a DEX method explicitly declared `native` (JNI itself is not
        // available). Later entries win, preserving per-context host override
        // precedence over built-in/global fallbacks.
        let mut native_fallbacks: Vec<NativeEntry> = Vec::new();
        let mut candidates: Vec<NativeEntry> = native::native_tables()
            .into_iter()
            .flatten()
            .chain(native::global_native_entries())
            .filter(|entry| entry.class == desc)
            .copied()
            .collect();
        candidates.extend(
            self.host_natives
                .iter()
                .filter(|entry| entry.class == desc)
                .copied(),
        );
        for entry in candidates {
            if let Some(existing) = native_fallbacks
                .iter_mut()
                .find(|existing| existing.name == entry.name && existing.sig == entry.sig)
            {
                *existing = entry;
            } else {
                native_fallbacks.push(entry);
            }
        }
        for entry in native_fallbacks {
            let name = self.intern(entry.name);
            let sig = self.intern(entry.sig);
            let key = (name, sig);
            if let Some(&slot) = dispatch.get(&key) {
                let method = &mut methods[slot as usize];
                if method.native_decl && method.static_method == !entry.instance {
                    method.native_key = Some((desc_id, name, sig));
                    method.native_decl = false;
                }
                continue;
            }
            let (args, ret) = parse_sig(entry.sig);
            let static_method = !entry.instance;
            let slot = methods.len() as u32;
            let ret = self.intern(ret);
            let args = args.iter().map(|arg| self.intern(arg)).collect();
            dispatch.insert(key, slot);
            methods.push(Method {
                slot,
                class: id,
                name,
                sig,
                ret,
                args,
                access_flags: class::ACC_PUBLIC | if static_method { ACC_STATIC } else { 0 },
                static_method,
                dex_idx: 0,
                native_key: Some((desc_id, name, sig)),
                native_decl: false,
                code: None,
                insns: OnceLock::new(),
            });
        }

        let cl = &mut self.classes[id as usize];
        cl.instance_fields = instance_fields;
        cl.field_offsets = field_offsets;
        cl.static_fields = static_fields;
        cl.statics = statics;
        cl.statics_lazy = statics_lazy;
        cl.methods = methods;
        cl.dispatch = dispatch;
        self.loading.pop();
        Ok(id)
    }

    fn shim_or_native(&self, desc: &str) -> bool {
        if class::SHIM_CLASSES.iter().any(|d| d.desc == desc) {
            return true;
        }
        if self.host_natives.iter().any(|e| e.class == desc) {
            return true;
        }
        crate::vm::native::native_tables()
            .into_iter()
            .flatten()
            .chain(crate::vm::native::global_native_entries())
            .any(|e| e.class == desc)
    }

    fn load_shim_class(&mut self, desc_id: u32) -> Result<u32, JvmError> {
        let desc = self.str_of(desc_id).to_string();
        let def = SHIM_CLASSES
            .iter()
            .find(|d| d.desc == desc)
            .copied()
            .unwrap_or(class::ShimDef {
                desc: "",
                super_desc: Some("Ljava/lang/Object;"),
                interfaces: &[],
                flags: class::ACC_PUBLIC,
                statics: &[],
            });
        let superclass = match def.super_desc {
            Some(s) => Some(self.ensure_class_by_desc(s)?),
            None => None,
        };
        let mut interfaces = Vec::new();
        for &s in def.interfaces {
            interfaces.push(self.ensure_class_by_desc(s)?);
        }
        let id = self.classes.len() as u32;
        self.classes.push(Class {
            id,
            descriptor: desc_id,
            superclass,
            interfaces,
            access_flags: def.flags,
            is_interface: def.flags & class::ACC_INTERFACE != 0,
            is_abstract: def.flags & class::ACC_ABSTRACT != 0,
            ..Default::default()
        });
        self.class_by_desc.insert(desc_id, id);

        // methods from the native table (statics + host-registered APIs)
        let mut methods: Vec<Method> = Vec::new();
        let mut dispatch: HashMap<(u32, u32), u32> = HashMap::new();
        let mut shim_natives: Vec<&NativeEntry> = native::native_tables()
            .into_iter()
            .flatten()
            .chain(native::global_native_entries())
            .collect();
        let host = self.host_natives.clone();
        shim_natives.extend(host.iter());
        for ne in shim_natives.into_iter().filter(|ne| ne.class == desc) {
            let name = self.intern(ne.name);
            let sig = self.intern(ne.sig);
            let (args, ret) = parse_sig(ne.sig);
            let static_method = !ne.instance;
            let slot = methods.len() as u32;
            dispatch.insert((name, sig), slot);
            methods.push(Method {
                slot,
                class: id,
                name,
                sig,
                ret: self.intern(ret),
                args: args.iter().map(|a| self.intern(a)).collect(),
                access_flags: class::ACC_PUBLIC | if static_method { ACC_STATIC } else { 0 },
                static_method,
                dex_idx: 0,
                native_key: Some((desc_id, name, sig)),
                native_decl: false,
                code: None,
                insns: OnceLock::new(),
            });
        }

        // static fields
        let mut statics = Vec::new();
        let mut statics_lazy: Vec<Option<class::ShimLazy>> = Vec::new();
        let mut static_fields: HashMap<(u32, u32), (u32, u32)> = HashMap::new();
        for sd in def.statics {
            let name = self.intern(sd.name);
            let ty = self.intern(sd.ty);
            let off = statics.len() as u32;
            match sd.value {
                ShimValue::Const(v) => {
                    statics.push(v);
                    statics_lazy.push(None);
                }
                ShimValue::Lazy(f) => {
                    statics.push(JValue::Null);
                    statics_lazy.push(Some(f));
                }
            }
            static_fields.insert((name, ty), (id, off));
        }
        let cl = &mut self.classes[id as usize];
        cl.methods = methods;
        cl.dispatch = dispatch;
        cl.statics = statics;
        cl.statics_lazy = statics_lazy;
        cl.static_fields = static_fields;
        Ok(id)
    }

    pub(crate) fn array_class(&mut self, dex_idx: u32, elem_type: u32) -> Result<u32, JvmError> {
        if let Some(&c) = self.array_classes.get(&(dex_idx, elem_type)) {
            return Ok(c);
        }
        let elem_desc = self.dex_at(dex_idx).type_descriptor(elem_type).to_string();
        let desc = format!("[{elem_desc}");
        let desc_id = self.intern(&desc);
        let object = self.ensure_class_by_desc_id(self.hot.object)?;
        let cloneable = self.ensure_class_by_desc("Ljava/lang/Cloneable;")?;
        let serializable = self.ensure_class_by_desc("Ljava/io/Serializable;")?;
        let id = self.classes.len() as u32;
        self.classes.push(Class {
            id,
            descriptor: desc_id,
            superclass: Some(object),
            interfaces: vec![cloneable, serializable],
            array_elem: Some((dex_idx, elem_type)),
            ..Default::default()
        });
        self.class_by_desc.insert(desc_id, id);
        self.array_classes.insert((dex_idx, elem_type), id);
        Ok(id)
    }

    fn synth_array_class(&mut self, desc_id: u32) -> Result<u32, JvmError> {
        if let Some(&c) = self.class_by_desc.get(&desc_id) {
            return Ok(c);
        }
        let object = self.ensure_class_by_desc_id(self.hot.object)?;
        let cloneable = self.ensure_class_by_desc("Ljava/lang/Cloneable;")?;
        let serializable = self.ensure_class_by_desc("Ljava/io/Serializable;")?;
        let id = self.classes.len() as u32;
        self.classes.push(Class {
            id,
            descriptor: desc_id,
            superclass: Some(object),
            interfaces: vec![cloneable, serializable],
            array_elem: None,
            ..Default::default()
        });
        self.class_by_desc.insert(desc_id, id);
        Ok(id)
    }

    // ---- field resolution ----

    pub fn field_ref(&mut self, dex_idx: u32, field_idx: u32) -> Result<FieldRef, JvmError> {
        if let Some(fr) = self.field_refs.get(&(dex_idx, field_idx)) {
            return Ok(*fr);
        }
        let f = self
            .dex_at(dex_idx)
            .fields
            .get(field_idx as usize)
            .cloned()
            .ok_or_else(|| JvmError::Resolution(format!("bad field idx {field_idx}")))?;
        let f_name = self.dex_at(dex_idx).strings[f.name as usize].clone();
        let name = self.intern(&f_name);
        let ty_desc = self.dex_at(dex_idx).type_descriptor(f.ty).to_owned();
        let ty = self.intern(&ty_desc);
        let class_desc_s = self.dex_at(dex_idx).type_descriptor(f.class).to_owned();
        let class_desc = self.intern(&class_desc_s);
        let fr = FieldRef {
            name,
            ty,
            class_desc,
        };
        self.field_refs.insert((dex_idx, field_idx), fr);
        Ok(fr)
    }

    /// Instance field offset for `(name, ty)` on `class_id` or any superclass.
    pub fn field_offset(&self, class_id: u32, name: u32, ty: u32) -> Option<u32> {
        let mut c = Some(class_id);
        while let Some(cc) = c {
            if let Some(&off) = self.classes[cc as usize].field_offsets.get(&(name, ty)) {
                return Some(off);
            }
            c = self.classes[cc as usize].superclass;
        }
        None
    }

    /// Static field read (triggers `<clinit>`, materializes lazy shim fields).
    pub fn static_field_get(&mut self, fr: FieldRef) -> Result<JValue, JvmError> {
        let start = self.ensure_class_by_desc_id(fr.class_desc)?;
        let (owner, off) = self.static_field_owner(start, fr)?;
        self.ensure_class_initialized(owner)?;
        if let Some(lazy) = self.classes[owner as usize].statics_lazy[off as usize] {
            let v = lazy(self);
            self.classes[owner as usize].statics[off as usize] = v;
            self.classes[owner as usize].statics_lazy[off as usize] = None;
        }
        Ok(self.classes[owner as usize].statics[off as usize])
    }

    pub fn static_field_put(&mut self, fr: FieldRef, v: JValue) -> Result<(), JvmError> {
        let start = self.ensure_class_by_desc_id(fr.class_desc)?;
        let (owner, off) = self.static_field_owner(start, fr)?;
        self.ensure_class_initialized(owner)?;
        if self.classes[owner as usize].statics_lazy[off as usize].is_some() {
            self.classes[owner as usize].statics_lazy[off as usize] = None;
        }
        self.classes[owner as usize].statics[off as usize] = v;
        Ok(())
    }

    fn static_field_owner(&mut self, start: u32, fr: FieldRef) -> Result<(u32, u32), JvmError> {
        let mut c = Some(start);
        while let Some(cc) = c {
            if let Some(&(owner, off)) = self.classes[cc as usize]
                .static_fields
                .get(&(fr.name, fr.ty))
            {
                return Ok((owner, off));
            }
            c = self.classes[cc as usize].superclass;
        }
        Err(JvmError::Resolution(format!(
            "no static field {} {} in {}",
            self.str_of(fr.name),
            self.str_of(fr.ty),
            self.str_of(self.classes[start as usize].descriptor)
        )))
    }

    pub fn ensure_class_initialized(&mut self, class_id: u32) -> Result<(), JvmError> {
        let state = self.classes[class_id as usize].clinit_state;
        if state != 0 {
            return Ok(());
        }
        let clinit = self.classes[class_id as usize].clinit_slot(self.hot.clinit);
        match clinit {
            Some(slot) => {
                self.classes[class_id as usize].clinit_state = 1;
                match interpret::run(self, class_id, slot, Vec::new()) {
                    Ok(_) => {
                        self.classes[class_id as usize].clinit_state = 2;
                        Ok(())
                    }
                    Err(e) => {
                        self.classes[class_id as usize].clinit_state = 0;
                        Err(e)
                    }
                }
            }
            None => {
                self.classes[class_id as usize].clinit_state = 2;
                Ok(())
            }
        }
    }

    // ---- garbage collection ----

    /// Mark-sweep heap reclamation. Called between top-level calls (see
    /// [`Context::call`](crate::Context::call)); the frame stack must be empty.
    ///
    /// Roots: interned/runtime strings, class static fields, monitor keys, and
    /// `extra` (the context's `last_instance`). Objects reachable from those
    /// are kept; everything else is returned to the arena free list and its
    /// slots get reused by later allocations. Handles never move, and any
    /// JValue the host keeps *outside* the roots is a documented use-after-
    /// reuse hazard: do not retain values across top-level calls.
    ///
    /// Returns the number of objects reclaimed.
    pub fn gc(&mut self, extra: &[u32]) -> usize {
        debug_assert!(self.frames.is_empty(), "gc while executing");
        let n = self.arena.objects.len();
        if n == 0 {
            return 0;
        }
        let mut marks = vec![false; n];
        let mut stack: Vec<u32> = Vec::new();
        let seed = |id: u32, marks: &mut Vec<bool>, stack: &mut Vec<u32>| {
            if (id as usize) < marks.len() && !marks[id as usize] {
                marks[id as usize] = true;
                stack.push(id);
            }
        };
        for &s in self.string_objs.values() {
            seed(s, &mut marks, &mut stack);
        }
        for &s in self.runtime_strings.values() {
            seed(s, &mut marks, &mut stack);
        }
        for c in &self.classes {
            for v in &c.statics {
                if let JValue::Obj(o) = v {
                    seed(*o, &mut marks, &mut stack);
                }
            }
        }
        for &m in self.monitors.keys() {
            seed(m, &mut marks, &mut stack);
        }
        for &e in extra {
            seed(e, &mut marks, &mut stack);
        }
        let mut refs: Vec<u32> = Vec::new();
        while let Some(id) = stack.pop() {
            refs.clear();
            {
                let obj = &self.arena.objects[id as usize];
                obj.collect_refs(&mut refs);
            }
            for r in &refs {
                seed(*r, &mut marks, &mut stack);
            }
        }
        let mut freed = 0;
        for (id, &marked) in marks.iter().enumerate().take(n) {
            if !marked {
                self.arena.reclaim(id as u32);
                freed += 1;
            }
        }
        self.monitors.retain(|id, _| marks[*id as usize]);
        freed
    }

    // ---- invocation helpers (used by natives) ----

    pub fn invoke_virtual(
        &mut self,
        receiver: JValue,
        name: &str,
        sig: &str,
    ) -> Result<JValue, JvmError> {
        let mref = MethodRef {
            name: self.intern(name),
            sig: self.intern(sig),
            ret: 0,
            args: Vec::new(),
            class_desc: 0,
        };
        if receiver.is_null() {
            return Err(JvmError::Fatal("invoke_virtual on null".into()));
        }
        let recv = receiver.as_obj();
        let target = self.resolve_target(InvokeKind::Virtual, &mref, Some(recv), 0)?;
        let mut args = Vec::with_capacity(1 + mref.args.len());
        args.push(receiver);
        self.call_target(target, args)
    }

    pub fn invoke_virtual_args(
        &mut self,
        receiver: JValue,
        name: &str,
        sig: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, JvmError> {
        let mref = MethodRef {
            name: self.intern(name),
            sig: self.intern(sig),
            ret: 0,
            args: Vec::new(),
            class_desc: 0,
        };
        if receiver.is_null() {
            return Err(JvmError::Fatal("invoke_virtual on null".into()));
        }
        let recv = receiver.as_obj();
        let target = self.resolve_target(InvokeKind::Virtual, &mref, Some(recv), 0)?;
        let mut all = Vec::with_capacity(1 + args.len());
        all.push(receiver);
        all.extend(args);
        self.call_target(target, all)
    }

    pub fn invoke_static(
        &mut self,
        class_desc: &str,
        name: &str,
        sig: &str,
        args: Vec<JValue>,
    ) -> Result<JValue, JvmError> {
        let mref = MethodRef {
            name: self.intern(name),
            sig: self.intern(sig),
            ret: 0,
            args: Vec::new(),
            class_desc: self.intern(class_desc),
        };
        let target = self.resolve_target(InvokeKind::Static, &mref, None, 0)?;
        self.call_target(target, args)
    }

    /// Finds any loaded class declaring an instance or static method called
    /// `name`; returns its full descriptor (e.g. `Lcom/foo/Ext;`).
    pub fn find_class_with_method(&self, name: &str) -> Option<String> {
        for c in &self.classes {
            for m in &c.methods {
                if self.str_of(m.name) == name {
                    return Some(self.str_of(c.descriptor).to_string());
                }
            }
        }
        None
    }

    /// Scans the raw dex (not just already-loaded classes) for a class
    /// declaring a method named `name`, loading it on the way. This is how
    /// the keiyoushi factory (`ExtensionGenerated`) is located.
    pub fn find_factory_class(&mut self, name: &str) -> Result<String, JvmError> {
        for dex in &self.dexes {
            for cd in &dex.classes {
                let type_str = dex
                    .types
                    .get(cd.class_idx as usize)
                    .and_then(|&s| dex.strings.get(s as usize).cloned())
                    .unwrap_or_default();
                let has = match &cd.class_data {
                    Some(data) => data
                        .direct_methods
                        .iter()
                        .chain(data.virtual_methods.iter())
                        .any(|m| {
                            dex.strings
                                .get(dex.methods[m.method_idx as usize].name as usize)
                                .map(|s| s.as_ref() == name)
                                .unwrap_or(false)
                        }),
                    None => false,
                };
                if has {
                    let cid = self.ensure_class_by_desc(&type_str)?;
                    return Ok(self
                        .str_of(self.classes[cid as usize].descriptor)
                        .to_string());
                }
            }
        }
        Err(JvmError::Resolution(format!(
            "no class with method {name} in dex"
        )))
    }

    /// Legacy (pre-factory) extension shape: a class that *is* the source and
    /// inherits from `HttpSource` directly (e.g. old `ExtensionGenerated`
    /// classes). Returns the first *concrete* subclass found in the raw dex —
    /// abstract base classes (modern keiyoushi shape: the abstract source plus
    /// a generated concrete `ExtensionGenerated` subclass) are skipped.
    pub fn find_http_source_subclass(&mut self) -> Result<String, JvmError> {
        const TARGET: &str = "Leu/kanade/tachiyomi/source/online/HttpSource;";
        const ACC_ABSTRACT: u32 = 0x0400;
        let mut candidates: Vec<(Arc<str>, bool)> = Vec::new();
        for dex in &self.dexes {
            for cd in &dex.classes {
                let type_str = dex
                    .types
                    .get(cd.class_idx as usize)
                    .and_then(|&s| dex.strings.get(s as usize).cloned())
                    .unwrap_or_default();
                if type_str.as_ref() == TARGET {
                    continue;
                }
                // walk the superclass chain across dexes via descriptors
                let mut sup_desc: Option<Arc<str>> = match cd.superclass_idx {
                    u32::MAX => None,
                    s => dex
                        .types
                        .get(s as usize)
                        .and_then(|&si| dex.strings.get(si as usize).cloned()),
                };
                let mut reaches = false;
                for _ in 0..4 {
                    let Some(sup) = sup_desc else {
                        break;
                    };
                    if sup.as_ref() == TARGET {
                        reaches = true;
                        break;
                    }
                    sup_desc = self.class_location(&sup).and_then(|(di, def_idx)| {
                        let d = &self.dexes[di as usize];
                        let cd2 = &d.classes[def_idx];
                        match cd2.superclass_idx {
                            u32::MAX => None,
                            s => d
                                .types
                                .get(s as usize)
                                .and_then(|&si| d.strings.get(si as usize).cloned()),
                        }
                    });
                }
                if !reaches {
                    continue;
                }
                if cd.access_flags & ACC_ABSTRACT != 0 {
                    continue;
                }
                candidates.push((type_str.clone(), type_str.ends_with("ExtensionGenerated;")));
            }
        }
        candidates.sort_by_key(|(_, is_entry)| !*is_entry);
        for (type_str, _) in candidates {
            let cid = self.ensure_class_by_desc(&type_str)?;
            return Ok(self
                .str_of(self.classes[cid as usize].descriptor)
                .to_string());
        }
        Err(JvmError::Resolution("no HttpSource subclass in dex".into()))
    }

    /// String payload of a java.lang.String value, if any.
    pub fn str_of_jvalue(&self, v: JValue) -> Option<String> {
        if v.is_null() {
            return None;
        }
        let id = v.as_obj();
        match &self.arena.objects[id as usize].native {
            Some(object::Native::Str(s)) => Some(s.clone()),
            _ => None,
        }
    }

    /// Owned payload for a non-null object value.
    pub fn payload_of(&self, v: JValue) -> Option<object::Native> {
        if v.is_null() {
            return None;
        }
        let id = v.as_obj();
        self.arena.objects[id as usize].native.clone()
    }

    /// Resolved class id for a non-null object value.
    pub fn object_class(&self, v: JValue) -> Option<u32> {
        if v.is_null() {
            return None;
        }
        let id = v.as_obj();
        Some(self.arena.objects[id as usize].class)
    }

    /// Calls a resolved target with a fresh frame (native or bytecode).
    pub fn call_target(&mut self, target: Target, args: Vec<JValue>) -> Result<JValue, JvmError> {
        match target {
            Target::Native(key) => {
                if std::env::var("DEXVM_TRACE").is_ok() {
                    eprintln!(
                        "DEXVM_TRACE native {}.{}{}",
                        crate::vm::value::dotted_name(self.str_of(key.0)),
                        self.str_of(key.1),
                        self.str_of(key.2)
                    );
                }
                let f = *self
                    .natives
                    .get(&key)
                    .ok_or_else(|| JvmError::Fatal(format!("no native for {key:?}")))?;
                match f(self, &args) {
                    Ok(v) => Ok(v),
                    Err(NatErr::Throw(ex)) => Err(JvmError::Uncaught(ex)),
                    Err(NatErr::Fatal(e)) => Err(e),
                }
            }
            Target::Bytecode { class, slot, .. } => {
                if std::env::var("DEXVM_TRACE").is_ok() {
                    let name = self
                        .classes
                        .get(class as usize)
                        .and_then(|c| c.methods.get(slot as usize))
                        .map(|m| format!("{}.{}", self.class_desc_str(class), self.str_of(m.name)));
                    if let Some(name) = name {
                        let recv = args.first().copied().and_then(|v| match v {
                            JValue::Obj(o) => Some(
                                self.class_desc_str(self.object_class(JValue::Obj(o)).unwrap_or(0)),
                            ),
                            _ => None,
                        });
                        eprintln!(
                            "DEXVM_TRACE call {name} recv={}",
                            recv.unwrap_or_else(|| format!("{args:?}"))
                        );
                    }
                }
                interpret::run(self, class, slot, args)
            }
        }
    }

    // ---- prototypes/signatures ----

    pub fn proto_sig(&self, dex_idx: u32, proto_id: u32) -> String {
        let p = &self.dex_at(dex_idx).protos[proto_id as usize];
        let mut s = String::from("(");
        for &t in &p.params {
            s.push_str(self.dex_at(dex_idx).type_descriptor(t));
        }
        s.push(')');
        s.push_str(self.dex_at(dex_idx).type_descriptor(p.return_type));
        s
    }

    pub fn method_ref(&mut self, dex_idx: u32, method_idx: u32) -> Result<MethodRef, JvmError> {
        if let Some(mr) = self.method_refs.get(&(dex_idx, method_idx)) {
            return Ok(mr.clone());
        }
        let m = self
            .dex_at(dex_idx)
            .methods
            .get(method_idx as usize)
            .cloned()
            .ok_or_else(|| JvmError::Resolution(format!("bad method idx {method_idx}")))?;
        let m_name = self.dex_at(dex_idx).strings[m.name as usize].clone();
        let name = self.intern(&m_name);
        let sig = self.intern(&self.proto_sig(dex_idx, m.proto));
        let ret_desc = self
            .dex_at(dex_idx)
            .type_descriptor(self.dex_at(dex_idx).protos[m.proto as usize].return_type)
            .to_string();
        let ret = self.intern(&ret_desc);
        let arg_descs: Vec<String> = self.dex_at(dex_idx).protos[m.proto as usize]
            .params
            .iter()
            .map(|&t| self.dex_at(dex_idx).type_descriptor(t).to_string())
            .collect();
        let args: Vec<u32> = arg_descs.iter().map(|d| self.intern(d)).collect();
        let class_desc_s = self.dex_at(dex_idx).type_descriptor(m.class).to_owned();
        let class_desc = self.intern(&class_desc_s);
        let mr = MethodRef {
            name,
            sig,
            ret,
            args,
            class_desc,
        };
        self.method_refs.insert((dex_idx, method_idx), mr.clone());
        Ok(mr)
    }

    // ---- call target resolution ----

    pub fn resolve_target(
        &mut self,
        kind: InvokeKind,
        mref: &MethodRef,
        receiver: Option<u32>,
        current_class: u32,
    ) -> Result<Target, JvmError> {
        let key = (mref.name, mref.sig);
        // The first class to search depends on the invoke kind.
        let mut c = match kind {
            InvokeKind::Virtual | InvokeKind::Interface => receiver
                .and_then(|o| self.arena.objects.get(o as usize))
                .map(|o| o.class)
                .ok_or_else(|| JvmError::Resolution("null receiver for virtual call".into()))?,
            InvokeKind::Static | InvokeKind::Direct => {
                self.ensure_class_by_desc_id(mref.class_desc)?
            }
            InvokeKind::Super => {
                // Dalvik: resolve against the correct superclass start point.
                // The ref class names the declaring class, but the search
                // starts at the superclass of the currently executing method's
                // class and walks up (resolves to the nearest override, which
                // is at or above the ref class).
                let sc = self
                    .classes
                    .get(current_class as usize)
                    .and_then(|pc| pc.superclass);
                match sc {
                    Some(s) => s,
                    None => self.ensure_class_by_desc_id(self.hot.object)?,
                }
            }
        };
        let c0 = c;
        let mut slot: Option<(u32, u32)> = None;
        while slot.is_none() {
            let found = {
                let class = self
                    .classes
                    .get(c as usize)
                    .ok_or_else(|| JvmError::Resolution(format!("missing class id {c}")))?;
                class.dispatch.get(&key).copied()
            };
            if let Some(s) = found {
                slot = Some((c, s));
                break;
            }
            let next = self.classes[c as usize].superclass;
            match next {
                Some(s) => c = s,
                // interfaces have no superclass in the dex model, but
                // every interface implicitly extends Object
                None if self.classes[c as usize].is_interface => {
                    c = self.ensure_class_by_desc_id(self.hot.object)?
                }
                None => break,
            }
        }
        // interface default methods (search the receiver's class, not the
        // class we walked up to; interfaces have no superclass chain entry)
        if slot.is_none() {
            if let Some((iface, s)) = self.search_interfaces(c0, &key) {
                slot = Some((iface, s));
            }
        }
        let (found_class, slot) = slot.ok_or_else(|| {
            JvmError::Resolution(format!(
                "no method {} {} found (on {} starting from {})",
                self.str_of(mref.name),
                self.str_of(mref.sig),
                self.str_of(self.classes[c0 as usize].descriptor),
                self.str_of(self.classes[c as usize].descriptor)
            ))
        })?;
        let (native_key, code, decoded, ins_size, registers, ret, args, static_method) = {
            let m = &self.classes[found_class as usize].methods[slot as usize];
            if m.native_decl {
                return Err(JvmError::Resolution(format!(
                    "native method {} {} has no JNI bridge (JNI unsupported)",
                    self.str_of(m.name),
                    self.str_of(m.sig)
                )));
            }
            let native_key = m.native_key;
            let code = m.code.clone();
            let decoded = if native_key.is_some() {
                None
            } else {
                let code = code
                    .as_ref()
                    .ok_or_else(|| JvmError::Resolution("method has no code".into()))?;
                Some(match m.insns.get() {
                    Some(d) => d.clone(),
                    None => {
                        let d = Arc::new(
                            crate::dex::insn::decode_all(&code.insns).map_err(JvmError::from)?,
                        );
                        let _ = m.insns.set(d.clone());
                        d
                    }
                })
            };
            (
                native_key,
                code.clone(),
                decoded,
                code.as_ref().map(|c| c.ins_size).unwrap_or(0),
                code.as_ref().map(|c| c.registers_size).unwrap_or(0),
                m.ret,
                m.args.clone(),
                m.static_method,
            )
        };
        if let Some(key) = native_key {
            return Ok(Target::Native(key));
        }
        Ok(Target::Bytecode {
            class: found_class,
            slot,
            decoded: decoded
                .ok_or_else(|| JvmError::Resolution("method has no decoded code".into()))?,
            code,
            ins_size,
            registers,
            ret,
            args,
            static_method,
        })
    }

    fn search_interfaces(&self, class_id: u32, key: &(u32, u32)) -> Option<(u32, u32)> {
        let mut queue = self.classes[class_id as usize].interfaces.clone();
        let mut seen: Vec<u32> = Vec::new();
        while let Some(iface) = queue.pop() {
            if seen.contains(&iface) {
                continue;
            }
            seen.push(iface);
            if let Some(&s) = self.classes[iface as usize].dispatch.get(key) {
                return Some((iface, s));
            }
            queue.extend(self.classes[iface as usize].interfaces.iter().copied());
        }
        None
    }

    // ---- objects ----

    /// Allocates a host-backed object of a shim class (resolved on demand)
    /// with the given native payload. Useful to feed interceptor chains and
    /// other host objects from tests and embedders.
    pub fn alloc_native(&mut self, desc: &str, native: object::Native) -> Result<JValue, JvmError> {
        let class = self.ensure_class_by_desc(desc)?;
        Ok(JValue::Obj(self.arena.alloc(
            class,
            Vec::new(),
            Some(native),
        )))
    }

    /// Reads the instance field `name` of an in-dex object (host shim
    /// objects carry no dex fields and return `None`).
    pub fn instance_field(&self, obj: u32, name: &str) -> Option<JValue> {
        let o = self.arena.get(obj)?;
        let c = self.classes.get(o.class as usize)?;
        for (off, f) in c.instance_fields.iter().enumerate() {
            if self.str_of(f.0) == name {
                return o.fields.get(off).copied();
            }
        }
        None
    }

    /// Reads an instance field by name, returning its slot index too.
    pub fn instance_field_id(&self, obj: u32, name: &str) -> Option<(usize, JValue)> {
        let o = self.arena.get(obj)?;
        let c = self.classes.get(o.class as usize)?;
        for (off, f) in c.instance_fields.iter().enumerate() {
            if self.str_of(f.0) == name {
                return Some((off, o.fields.get(off).copied().unwrap_or(JValue::Null)));
            }
        }
        None
    }

    /// Overwrites the instance field `name` of an in-dex object.
    pub fn instance_field_set(&mut self, obj: u32, name: &str, v: JValue) -> bool {
        let Some(o) = self.arena.get_mut(obj) else {
            return false;
        };
        let class = o.class;
        let Some(c) = self.classes.get(class as usize) else {
            return false;
        };
        let Some(idx) = c
            .instance_fields
            .iter()
            .position(|f| self.str_of(f.0) == name)
        else {
            return false;
        };
        let Some(o) = self.arena.get_mut(obj) else {
            return false;
        };
        let Some(slot) = o.fields.get_mut(idx) else {
            return false;
        };
        *slot = v;
        true
    }

    /// Whether `class` declares any instance fields. Host shims carry none.
    pub fn class_has_instance_fields(&self, class: u32) -> bool {
        self.classes
            .get(class as usize)
            .is_some_and(|c| !c.instance_fields.is_empty())
    }

    /// Byte offset of the named instance field on `class`, or `None`.
    pub fn class_field_offset(&self, class: u32, name: &str) -> Option<u32> {
        let c = self.classes.get(class as usize)?;
        c.instance_fields
            .iter()
            .position(|f| self.str_of(f.0) == name)
            .map(|i| i as u32)
    }

    /// Depth-first search for the `[B` byte array held somewhere inside an
    /// in-dex object graph (e.g. an okhttp `ResponseBody` subclass wrapping
    /// a byte container like MoeTruyen's `Lc`).
    pub fn object_bytes(&self, v: JValue) -> Option<Vec<u8>> {
        fn walk(vm: &Vm, v: JValue, depth: usize) -> Option<Vec<u8>> {
            if depth > 4 {
                return None;
            }
            if let Some(n) = crate::vm::native::payload(vm, v) {
                if let object::Native::Array(object::ArrayData::Byte(bs)) = n {
                    return Some(bs.iter().map(|&b| b as u8).collect());
                }
                return None;
            }
            let JValue::Obj(id) = v else {
                return None;
            };
            let o = vm.arena.get(id)?;
            for f in &o.fields {
                if let Some(bytes) = walk(vm, *f, depth + 1) {
                    return Some(bytes);
                }
            }
            None
        }
        walk(self, v, 0)
    }

    pub fn alloc_instance(&mut self, class_id: u32) -> Result<u32, JvmError> {
        let fields: Vec<JValue> = {
            let c = &self.classes[class_id as usize];
            c.instance_fields
                .iter()
                .map(|&(_, ty, _)| default_of(self.str_of(ty)))
                .collect()
        };
        Ok(self.arena.alloc(class_id, fields, None))
    }

    pub fn alloc_string(&mut self, s: &str) -> JValue {
        if let Some(&id) = self.runtime_strings.get(s) {
            return JValue::Obj(id);
        }
        let id = self.arena.alloc(
            self.hot.string,
            Vec::new(),
            Some(Native::Str(s.to_string())),
        );
        self.runtime_strings.insert(s.to_string(), id);
        JValue::Obj(id)
    }

    pub fn dex_string(&mut self, dex_idx: u32, string_id: u32) -> Result<JValue, JvmError> {
        if let Some(&o) = self.string_objs.get(&(dex_idx, string_id)) {
            return Ok(JValue::Obj(o));
        }
        let s = self
            .dex_at(dex_idx)
            .strings
            .get(string_id as usize)
            .ok_or_else(|| JvmError::Resolution(format!("bad string idx {string_id}")))?;
        let o = self.arena.alloc(
            self.hot.string,
            Vec::new(),
            Some(Native::Str(s.to_string())),
        );
        self.string_objs.insert((dex_idx, string_id), o);
        Ok(JValue::Obj(o))
    }

    pub fn class_obj(&mut self, class_id: u32) -> Result<JValue, JvmError> {
        if let Some(o) = self.classes[class_id as usize].class_obj {
            return Ok(JValue::Obj(o));
        }
        let class_class = self.ensure_class_by_desc("Ljava/lang/Class;")?;
        let native = Native::ClassObj(ClassOrPrim::Class(class_id));
        let o = self.arena.alloc(class_class, Vec::new(), Some(native));
        self.classes[class_id as usize].class_obj = Some(o);
        Ok(JValue::Obj(o))
    }

    // ---- throwables ----

    /// Real host directory backing `Context.getCacheDir()`: a fresh
    /// `<tmp>/dexvm-cache-<pid>-<n>` created on first use, so extension
    /// cache logic runs against a genuine filesystem.
    pub fn cache_root_path(&mut self) -> &str {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static SEQ: AtomicUsize = AtomicUsize::new(0);
        if self.cache_root.is_none() {
            let n = SEQ.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("dexvm-cache-{}-{n}", std::process::id()));
            self.cache_root = Some(dir.to_string_lossy().into_owned());
        }
        self.cache_root.as_deref().unwrap_or("")
    }

    pub fn throwable_of(&mut self, class_desc: &str, message: impl Into<String>) -> u32 {
        let class = self.ensure_class_by_desc(class_desc).unwrap_or(0);
        self.arena.alloc(
            class,
            Vec::new(),
            Some(Native::Throwable {
                message: Some(message.into()),
                cause: JValue::Null,
            }),
        )
    }

    pub fn err_npe(&mut self) -> u32 {
        if std::env::var("DEXVM_TRACE").is_ok() {
            eprintln!("NPE@created");
        }
        self.throwable_of("Ljava/lang/NullPointerException;", "")
    }
    pub fn err_npe_msg(&mut self, msg: impl Into<String>) -> u32 {
        self.throwable_of("Ljava/lang/NullPointerException;", msg)
    }
    pub fn err_arithmetic(&mut self, msg: impl Into<String>) -> u32 {
        self.throwable_of("Ljava/lang/ArithmeticException;", msg)
    }
    pub fn err_aioobe(&mut self, idx: i32, len: i32) -> u32 {
        self.throwable_of(
            "Ljava/lang/ArrayIndexOutOfBoundsException;",
            format!("Index {idx} out of bounds for length {len}"),
        )
    }
    pub fn err_ioobe(&mut self, idx: i32) -> u32 {
        self.throwable_of(
            "Ljava/lang/IndexOutOfBoundsException;",
            format!("Index {idx}"),
        )
    }
    pub fn err_cce(&mut self, msg: impl Into<String>) -> u32 {
        self.throwable_of("Ljava/lang/ClassCastException;", msg)
    }
    pub fn err_neg_arr_size(&mut self) -> u32 {
        self.throwable_of("Ljava/lang/NegativeArraySizeException;", "")
    }
    pub fn err_uoe(&mut self, msg: impl Into<String>) -> u32 {
        self.throwable_of("Ljava/lang/UnsupportedOperationException;", msg)
    }
    pub fn err_iae(&mut self, msg: impl Into<String>) -> u32 {
        self.throwable_of("Ljava/lang/IllegalArgumentException;", msg)
    }
    pub fn err_ise(&mut self, msg: impl Into<String>) -> u32 {
        self.throwable_of("Ljava/lang/IllegalStateException;", msg)
    }
    pub fn err_nfe(&mut self, msg: impl Into<String>) -> u32 {
        self.throwable_of("Ljava/lang/NumberFormatException;", msg)
    }
    pub fn err_fnf(&mut self, msg: impl Into<String>) -> u32 {
        self.throwable_of("Ljava/io/FileNotFoundException;", msg)
    }
    pub fn err_ioe(&mut self, msg: impl Into<String>) -> u32 {
        self.throwable_of("Ljava/io/IOException;", msg)
    }
    pub fn err_sioobe(&mut self, msg: impl Into<String>) -> u32 {
        self.throwable_of("Ljava/lang/StringIndexOutOfBoundsException;", msg)
    }

    // ---- assignability ----

    pub fn is_assignable(&mut self, obj_class: u32, target: u32) -> Result<bool, JvmError> {
        self.is_assignable_inner(obj_class, target, 0)
    }

    fn is_assignable_inner(
        &mut self,
        obj_class: u32,
        target: u32,
        depth: usize,
    ) -> Result<bool, JvmError> {
        if obj_class == target {
            return Ok(true);
        }
        if depth > 64 {
            return Err(JvmError::Resolution(format!(
                "type graph cycle or too deep in assignability check (class {obj_class} -> {target})"
            )));
        }
        let is_target_array = self.classes[target as usize].array_elem.is_some();
        let mut c = Some(obj_class);
        let mut level = 0usize;
        while let Some(cc) = c {
            if level > 64 {
                return Err(JvmError::Resolution("class hierarchy too deep".into()));
            }
            level += 1;
            let (array_elem, interfaces, superclass) = {
                let cl = &self.classes[cc as usize];
                (cl.array_elem, cl.interfaces.clone(), cl.superclass)
            };
            if cc == target {
                return Ok(true);
            }
            if let Some((edx, elem)) = array_elem {
                // array targets
                if is_target_array {
                    let Some((tdx, t_elem)) = self.classes[target as usize].array_elem else {
                        return Ok(false);
                    };
                    let e_desc = self.dex_at(edx).type_descriptor(elem).to_string();
                    let t_desc = self.dex_at(tdx).type_descriptor(t_elem).to_string();
                    if e_desc == t_desc {
                        return Ok(true);
                    }
                    if !e_desc.starts_with('[') && e_desc.len() == 1 && t_desc.starts_with('[') {
                        return Ok(false);
                    }
                    if e_desc.len() == 1 || t_desc.len() == 1 {
                        return Ok(false); // primitive arrays only match identical
                    }
                    let ec = self.ensure_class_by_type(edx, elem)?;
                    let tc = self.ensure_class_by_type(tdx, t_elem)?;
                    if depth > 6 {
                        eprintln!(
                            "DEXDBG assign array recur obj={} ({}::elem {}) -> {} ({})",
                            obj_class,
                            self.class_desc_str(obj_class),
                            self.dex_at(edx).type_descriptor(elem),
                            target,
                            self.class_desc_str(target)
                        );
                    }
                    return self.is_assignable_inner(ec, tc, depth + 1);
                }
                // array to Cloneable/Serializable/Object
                for &i in &interfaces {
                    if i == target {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            let mut match_interface = false;
            for &i in &interfaces {
                if i == target {
                    match_interface = true;
                    break;
                }
                if self.is_assignable_inner(i, target, depth + 1)? {
                    match_interface = true;
                    break;
                }
            }
            if match_interface {
                return Ok(true);
            }
            c = superclass;
        }
        Ok(false)
    }

    // ---- encoded values ----

    fn enc_to_value(
        &mut self,
        ev: &EncodedValue,
        dex_idx: u32,
        _ty: u32,
    ) -> Result<JValue, JvmError> {
        Ok(match ev {
            EncodedValue::Byte(v) => JValue::Int(i32::from(*v)),
            EncodedValue::Short(v) => JValue::Int(i32::from(*v)),
            EncodedValue::Char(v) => JValue::Int(i32::from(*v)),
            EncodedValue::Int(v) => JValue::Int(*v),
            EncodedValue::Long(v) => JValue::Long(*v),
            EncodedValue::Float(v) => JValue::Float(*v),
            EncodedValue::Double(v) => JValue::Double(*v),
            EncodedValue::Bool(v) => JValue::Int(i32::from(*v)),
            EncodedValue::Null => JValue::Null,
            EncodedValue::String(s) => self.dex_string(dex_idx, *s)?,
            EncodedValue::Type(t) => {
                let c = self.ensure_class_by_type(dex_idx, *t)?;
                self.class_obj(c)?
            }
            _ => JValue::Null,
        })
    }

    pub fn register_natives(&mut self) {
        native::register(self);
    }

    // ---- sandbox permissions ----

    /// Does the current grant set cover `p`?
    pub fn has_permission(&self, p: &crate::permission::Permission) -> bool {
        self.perms.has(p)
    }

    /// Fails with `JvmError::Resolution("permission denied: ...")` when the
    /// capability `p` is not granted. Host-registered natives call this
    /// before performing host side effects.
    pub fn check_permission(&self, p: &crate::permission::Permission) -> Result<(), JvmError> {
        if self.perms.has(p) {
            Ok(())
        } else {
            Err(JvmError::Resolution(format!("permission denied: {p:?}")))
        }
    }

    /// Registers a host API: a native method for the shim class `e.class`
    /// (e.g. `Lcom/example/host/Http;`), callable from dex code with
    /// `invoke-static`/`invoke-virtual`. The class is loaded on demand with
    /// this native as its only method.
    pub fn register_native(&mut self, e: NativeEntry) -> Result<(), JvmError> {
        let (class, name, sig) = (
            self.intern(e.class),
            self.intern(e.name),
            self.intern(e.sig),
        );
        let key = (class, name, sig);
        self.natives.insert(key, e.f);
        let already = self.class_by_desc.contains_key(&class);
        self.host_natives.push(e);
        if already {
            // Patch the loaded class's dispatch so resolution finds the method.
            let cid = self.class_by_desc[&class];
            let slot = self.classes[cid as usize].methods.len() as u32;
            let (args, ret) = parse_sig(e.sig);
            let static_method = !e.instance;
            let ret_id = self.intern(ret);
            let args: Vec<u32> = args.iter().map(|a| self.intern(a)).collect();
            self.classes[cid as usize].methods.push(Method {
                slot,
                class: cid,
                name,
                sig,
                ret: ret_id,
                args,
                access_flags: class::ACC_PUBLIC | if static_method { ACC_STATIC } else { 0 },
                static_method,
                dex_idx: 0,
                native_key: Some((class, name, sig)),
                native_decl: false,
                code: None,
                insns: OnceLock::new(),
            });
            self.classes[cid as usize]
                .dispatch
                .insert((name, sig), slot);
        }
        Ok(())
    }

    pub fn write_out(&mut self, s: &str) {
        let _ = self.out.write_all(s.as_bytes());
        let _ = self.out.flush();
    }
}

/// Parse `(args)ret` into (arg descriptors, return descriptor).
pub fn parse_sig(sig: &str) -> (Vec<&str>, &str) {
    let open = sig.find('(').unwrap_or(0);
    let close = sig.find(')').unwrap_or(sig.len());
    let args = &sig[open + 1..close];
    let ret = &sig[close + 1..];
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let c = args.as_bytes()[i] as char;
        if c == 'L' {
            let end = args[i..].find(';').map(|p| i + p + 1).unwrap_or(args.len());
            out.push(&args[i..end]);
            i = end;
        } else if c == '[' {
            let mut j = i;
            while args.as_bytes()[j] == b'[' {
                j += 1;
            }
            let end = if args.as_bytes()[j] == b'L' {
                args[j..].find(';').map(|p| j + p + 1).unwrap_or(args.len())
            } else {
                j + 1
            };
            out.push(&args[i..end]);
            i = end;
        } else {
            out.push(&args[i..i + 1]);
            i += 1;
        }
    }
    (out, ret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dex::DexFile;

    fn uleb(d: &mut Vec<u8>, mut v: u32) {
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            d.push(b);
            if v == 0 {
                break;
            }
        }
    }

    fn pad4(d: &mut Vec<u8>) {
        while !d.len().is_multiple_of(4) {
            d.push(0);
        }
    }

    // Hand-assembled dex containing:
    //   class LHello; : Ljava/lang/Object;
    //     static int f(int a, int b) { return a + b; }  // add-int, return
    //     static int g() { return f(1, 2); }            // const/16, invoke-static, return
    fn hello_dex() -> Vec<u8> {
        let mut d = Vec::new();
        let push4 = |d: &mut Vec<u8>, v: u32| d.extend_from_slice(&v.to_le_bytes());

        // header placeholder: 20 u32 fields, remember their offsets for patching
        d.extend_from_slice(b"dex\n035\0");
        d.extend_from_slice(&[0u8; 4]); // checksum
        d.extend_from_slice(&[0u8; 20]); // signature
        let mut hdr = Vec::new();
        for _ in 0..20 {
            hdr.push(d.len());
            push4(&mut d, 0);
        }
        assert_eq!(d.len(), 0x70);

        // string_data
        let strings: [&[u8]; 6] = [b"LHello;", b"Ljava/lang/Object;", b"f", b"g", b"I", b"II"];
        let mut str_off = [0u32; 6];
        for (i, s) in strings.iter().enumerate() {
            str_off[i] = d.len() as u32;
            uleb(&mut d, s.len() as u32);
            d.extend_from_slice(s);
            d.push(0);
        }
        let string_data_off = str_off[0];

        // string_ids
        pad4(&mut d);
        let string_ids_off = d.len() as u32;
        for o in str_off {
            push4(&mut d, o);
        }

        // type_ids: 0 = LHello;, 1 = Ljava/lang/Object;, 2 = I
        let type_ids_off = d.len() as u32;
        push4(&mut d, 0);
        push4(&mut d, 1);
        push4(&mut d, 4); // "I"

        // type_list: params (I, I)
        let type_list_off = d.len() as u32;
        push4(&mut d, 2);
        d.extend_from_slice(&2u16.to_le_bytes());
        d.extend_from_slice(&2u16.to_le_bytes());
        pad4(&mut d);

        // proto_ids: p0 = (II)I, p1 = ()I
        let proto_ids_off = d.len() as u32;
        push4(&mut d, 5); // shorty "II"
        push4(&mut d, 2); // return "I"
        push4(&mut d, type_list_off);
        push4(&mut d, 4); // shorty "I"
        push4(&mut d, 2); // return "I"
        push4(&mut d, 0); // no params

        // method_ids: m0 = LHello;.f(II)I, m1 = LHello;.g()I
        let method_ids_off = d.len() as u32;
        push4(&mut d, 0); // class 0, proto 0
        push4(&mut d, 2); // name "f"
        push4(&mut d, 1u32 << 16); // class 0, proto 1
        push4(&mut d, 3); // name "g"

        // class_defs: one class
        let class_defs_off = d.len() as u32;
        push4(&mut d, 0); // class_idx
        push4(&mut d, 0x1); // access_flags: public
        push4(&mut d, 1); // superclass_idx = Ljava/lang/Object;
        push4(&mut d, 0); // interfaces_off
        push4(&mut d, u32::MAX); // source_file_idx
        push4(&mut d, 0); // annotations_off
        let class_data_off_pos = d.len();
        push4(&mut d, 0); // class_data_off (patched)
        push4(&mut d, 0); // static_values_off

        // code_item f: add-int v0,v0,v1; return v0
        let code_f_off = d.len() as u32;
        d.extend_from_slice(&2u16.to_le_bytes()); // registers
        d.extend_from_slice(&2u16.to_le_bytes()); // ins
        d.extend_from_slice(&0u16.to_le_bytes()); // outs
        d.extend_from_slice(&0u16.to_le_bytes()); // tries
        push4(&mut d, 0); // debug_info_off
        push4(&mut d, 3); // insns_size
        d.extend_from_slice(&0x0090u16.to_le_bytes()); // add-int v0,v0,v1
        d.extend_from_slice(&0x0100u16.to_le_bytes());
        d.extend_from_slice(&0x000fu16.to_le_bytes()); // return v0

        // code_item g: const/16 v0,#1; const/16 v1,#2;
        //               invoke-static {v0,v1} m0; move-result v0; return v0
        pad4(&mut d);
        let code_g_off = d.len() as u32;
        d.extend_from_slice(&2u16.to_le_bytes()); // registers
        d.extend_from_slice(&0u16.to_le_bytes()); // ins
        d.extend_from_slice(&2u16.to_le_bytes()); // outs
        d.extend_from_slice(&0u16.to_le_bytes()); // tries
        push4(&mut d, 0); // debug_info_off
        push4(&mut d, 9); // insns_size
        d.extend_from_slice(&0x0013u16.to_le_bytes()); // const/16 v0, #1
        d.extend_from_slice(&0x0001u16.to_le_bytes());
        d.extend_from_slice(&0x0113u16.to_le_bytes()); // const/16 v1, #2
        d.extend_from_slice(&0x0002u16.to_le_bytes());
        d.extend_from_slice(&0x1271u16.to_le_bytes()); // invoke-static {v0,v1}, m0 (A=2)
        d.extend_from_slice(&0x0000u16.to_le_bytes()); // BBBB=m0
        d.extend_from_slice(&0x0010u16.to_le_bytes()); // C=0, D=1
        d.extend_from_slice(&0x000au16.to_le_bytes()); // move-result v0
        d.extend_from_slice(&0x000fu16.to_le_bytes()); // return v0

        // class_data: 2 direct (static) methods
        pad4(&mut d);
        let class_data_off = d.len() as u32;
        uleb(&mut d, 0); // static_fields
        uleb(&mut d, 0); // instance_fields
        uleb(&mut d, 2); // direct_methods
        uleb(&mut d, 0); // virtual_methods
        uleb(&mut d, 0); // m0 idx diff
        uleb(&mut d, 0x8); // access_flags: static
        uleb(&mut d, code_f_off);
        uleb(&mut d, 1); // m1 idx diff
        uleb(&mut d, 0x8); // access_flags: static
        uleb(&mut d, code_g_off);

        // map_list
        pad4(&mut d);
        let map_off = d.len() as u32;
        let map_entries: [(u16, u32, u32); 10] = [
            (0x0000, 1, 0), // header
            (0x0001, 6, string_ids_off),
            (0x0002, 2, type_ids_off),
            (0x0003, 2, proto_ids_off),
            (0x0005, 2, method_ids_off),
            (0x0006, 1, class_defs_off),
            (0x1000, 1, map_off),
            (0x1001, 1, type_list_off),
            (0x2001, 2, code_f_off),
            (0x2002, 6, string_data_off),
        ];
        push4(&mut d, map_entries.len() as u32);
        for (ty, size, off) in map_entries {
            push4(&mut d, u32::from(ty));
            push4(&mut d, size);
            push4(&mut d, off);
        }

        // patch header
        let file_size = d.len() as u32;
        let patch = |d: &mut Vec<u8>, off: usize, v: u32| {
            d[off..off + 4].copy_from_slice(&v.to_le_bytes());
        };
        let mut it = hdr.into_iter();
        let mut f = |d: &mut Vec<u8>, v: u32| patch(d, it.next().unwrap(), v);
        f(&mut d, file_size); // file_size
        f(&mut d, 0x70); // header_size
        f(&mut d, 0x1234_5678); // endian
        f(&mut d, 0); // link_size
        f(&mut d, 0); // link_off
        f(&mut d, map_off);
        f(&mut d, 6); // string_ids_size
        f(&mut d, string_ids_off);
        f(&mut d, 2); // type_ids_size
        f(&mut d, type_ids_off);
        f(&mut d, 2); // proto_ids_size
        f(&mut d, proto_ids_off);
        f(&mut d, 0); // field_ids_size
        f(&mut d, 0); // field_ids_off
        f(&mut d, 2); // method_ids_size
        f(&mut d, method_ids_off);
        f(&mut d, 1); // class_defs_size
        f(&mut d, class_defs_off);
        f(&mut d, 0); // data_size
        f(&mut d, 0); // data_off
        patch(&mut d, class_data_off_pos, class_data_off);
        d
    }

    #[test]
    fn runs_hello_dex() {
        let dex = DexFile::parse(&hello_dex()).expect("parse dex");
        let mut vm = Vm::new(vec![dex], Box::new(Vec::new())).expect("vm");
        let cid = vm.ensure_class_by_desc("LHello;").expect("load LHello;");
        let f_slot = vm.classes[cid as usize]
            .methods
            .iter()
            .position(|m| vm.str_of(m.name) == "f")
            .expect("method f") as u32;
        let r = interpret::run(&mut vm, cid, f_slot, vec![JValue::Int(2), JValue::Int(3)])
            .expect("run f");
        assert_eq!(r, JValue::Int(5));
        let g_slot = vm.classes[cid as usize]
            .methods
            .iter()
            .position(|m| vm.str_of(m.name) == "g")
            .expect("method g") as u32;
        let r = interpret::run(&mut vm, cid, g_slot, vec![]).expect("run g");
        assert_eq!(r, JValue::Int(3));
    }

    // Hand-assembled dex containing just class LHello; whose only method
    // `static int g()` is declared `native` (access_flags 0x100 | 0x8) with
    // no code item.
    fn native_dex() -> Vec<u8> {
        let mut d = Vec::new();
        let push4 = |d: &mut Vec<u8>, v: u32| d.extend_from_slice(&v.to_le_bytes());
        d.extend_from_slice(b"dex\n035\0");
        d.extend_from_slice(&[0u8; 4]);
        d.extend_from_slice(&[0u8; 20]);
        let mut hdr = Vec::new();
        for _ in 0..20 {
            hdr.push(d.len());
            push4(&mut d, 0);
        }
        assert_eq!(d.len(), 0x70);

        let strings: [&[u8]; 4] = [b"LHello;", b"Ljava/lang/Object;", b"g", b"I"];
        let mut str_off = [0u32; 4];
        for (i, s) in strings.iter().enumerate() {
            str_off[i] = d.len() as u32;
            uleb(&mut d, s.len() as u32);
            d.extend_from_slice(s);
            d.push(0);
        }
        let string_data_off = str_off[0];

        pad4(&mut d);
        let string_ids_off = d.len() as u32;
        for o in str_off {
            push4(&mut d, o);
        }

        let type_ids_off = d.len() as u32;
        push4(&mut d, 0); // "LHello;"
        push4(&mut d, 1); // "Ljava/lang/Object;"
        push4(&mut d, 3); // "I"

        let proto_ids_off = d.len() as u32;
        push4(&mut d, 3); // shorty "I"
        push4(&mut d, 2); // return "I"
        push4(&mut d, 0); // no params

        let method_ids_off = d.len() as u32;
        push4(&mut d, 0); // class 0, proto 0
        push4(&mut d, 2); // name "g"

        let class_defs_off = d.len() as u32;
        push4(&mut d, 0); // class_idx
        push4(&mut d, 0x1); // access_flags: public
        push4(&mut d, 1); // superclass_idx
        push4(&mut d, 0); // interfaces_off
        push4(&mut d, u32::MAX); // source_file_idx
        push4(&mut d, 0); // annotations_off
        let class_data_off_pos = d.len();
        push4(&mut d, 0); // class_data_off (patched)
        push4(&mut d, 0); // static_values_off

        // class_data: 1 direct (static) native method, no code
        pad4(&mut d);
        let class_data_off = d.len() as u32;
        uleb(&mut d, 0); // static_fields
        uleb(&mut d, 0); // instance_fields
        uleb(&mut d, 1); // direct_methods
        uleb(&mut d, 0); // virtual_methods
        uleb(&mut d, 0); // m0 idx diff
        uleb(&mut d, 0x108); // access_flags: static | native
        uleb(&mut d, 0); // code_off = 0 (native: no code item)

        pad4(&mut d);
        let map_off = d.len() as u32;
        let map_entries: [(u16, u32, u32); 8] = [
            (0x0000, 1, 0), // header
            (0x0001, 4, string_ids_off),
            (0x0002, 3, type_ids_off),
            (0x0003, 1, proto_ids_off),
            (0x0005, 1, method_ids_off),
            (0x0006, 1, class_defs_off),
            (0x1000, 1, map_off),
            (0x2002, 4, string_data_off),
        ];
        push4(&mut d, map_entries.len() as u32);
        for (ty, size, off) in map_entries {
            push4(&mut d, u32::from(ty));
            push4(&mut d, size);
            push4(&mut d, off);
        }

        let file_size = d.len() as u32;
        let patch = |d: &mut Vec<u8>, off: usize, v: u32| {
            d[off..off + 4].copy_from_slice(&v.to_le_bytes());
        };
        let mut it = hdr.into_iter();
        let mut f = |d: &mut Vec<u8>, v: u32| patch(d, it.next().unwrap(), v);
        f(&mut d, file_size); // file_size
        f(&mut d, 0x70); // header_size
        f(&mut d, 0x1234_5678); // endian
        f(&mut d, 0); // link_size
        f(&mut d, 0); // link_off
        f(&mut d, map_off);
        f(&mut d, 4); // string_ids_size
        f(&mut d, string_ids_off);
        f(&mut d, 3); // type_ids_size
        f(&mut d, type_ids_off);
        f(&mut d, 1); // proto_ids_size
        f(&mut d, proto_ids_off);
        f(&mut d, 0); // field_ids_size
        f(&mut d, 0); // field_ids_off
        f(&mut d, 1); // method_ids_size
        f(&mut d, method_ids_off);
        f(&mut d, 1); // class_defs_size
        f(&mut d, class_defs_off);
        f(&mut d, 0); // data_size
        f(&mut d, 0); // data_off
        patch(&mut d, class_data_off_pos, class_data_off);
        d
    }

    #[test]
    fn native_method_reports_missing_jni_bridge() {
        let dex = DexFile::parse(&native_dex()).expect("parse dex");
        let mut vm = Vm::new(vec![dex], Box::new(Vec::new())).expect("vm");
        let cid = vm.ensure_class_by_desc("LHello;").expect("load LHello;");
        let g_slot = vm.classes[cid as usize]
            .methods
            .iter()
            .position(|m| vm.str_of(m.name) == "g")
            .expect("method g") as u32;
        assert!(vm.classes[cid as usize].methods[g_slot as usize].native_decl);
        let err = interpret::run(&mut vm, cid, g_slot, vec![]).unwrap_err();
        assert!(
            format!("{err}").contains("has no JNI bridge"),
            "unexpected error: {err}"
        );
    }
}
