//! The VM: class loading/resolution, the object arena, strings, exceptions.

pub mod class;
pub mod error;
pub mod interpret;
pub mod native;
pub mod object;
pub mod value;

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, OnceLock};

use class::{Class, Method, ShimValue, ACC_STATIC, SHIM_CLASSES};
use error::JvmError;
use object::{Arena, ClassOrPrim, Native};
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
    /// true = instance method (receiver in args[0]).
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
    pub dex: DexFile,
    pub intern: Vec<Arc<str>>,
    pub intern_map: HashMap<String, u32>,
    pub classes: Vec<Class>,
    pub class_by_desc: HashMap<u32, u32>,
    pub class_by_type: HashMap<u32, u32>,
    pub arena: Arena,
    pub string_objs: Vec<Option<u32>>,
    pub runtime_strings: HashMap<String, u32>,
    pub natives: HashMap<(u32, u32, u32), NativeFn>,
    pub monitors: HashMap<u32, usize>,
    pub method_refs: HashMap<u32, MethodRef>,
    pub field_refs: HashMap<u32, FieldRef>,
    pub out: Box<dyn Write>,
    pub budget: i64,
    pub depth_limit: usize,
    pub hot: Hot,
    pub frames: Vec<crate::vm::interpret::Frame>,
    /// Host-registered natives (see [`Vm::register_native`]).
    pub host_natives: Vec<NativeEntry>,
    /// Sandbox capability grants checked by host natives.
    pub perms: crate::permission::Permissions,
    pub array_classes: HashMap<u32, u32>,
    #[cfg(feature = "keiyoushi")]
    pub http: Option<std::rc::Rc<dyn Fn(&native::keiyoushi::HttpData) -> native::keiyoushi::HttpResp>>,
    loading: Vec<usize>,
}

impl Vm {
    pub fn new(dex: DexFile, out: Box<dyn Write>) -> Result<Vm, JvmError> {
        let mut vm = Vm {
            dex,
            intern: Vec::new(),
            intern_map: HashMap::new(),
            classes: Vec::new(),
            class_by_desc: HashMap::new(),
            class_by_type: HashMap::new(),
            arena: Arena::default(),
            string_objs: Vec::new(),
            runtime_strings: HashMap::new(),
            natives: HashMap::new(),
            monitors: HashMap::new(),
            method_refs: HashMap::new(),
            field_refs: HashMap::new(),
            out,
            budget: 50_000_000,
            depth_limit: 20_000,
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
            #[cfg(feature = "keiyoushi")]
            http: None,
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

    // ---- class loading ----

    pub fn ensure_class_by_type(&mut self, type_id: u32) -> Result<u32, JvmError> {
        if let Some(&c) = self.class_by_type.get(&type_id) {
            return Ok(c);
        }
        let desc = self.dex.type_descriptor(type_id).to_string();
        if desc.starts_with('[') {
            return self.array_class(type_id);
        }
        let desc_id = self.intern(&desc);
        if let Some(&c) = self.class_by_desc.get(&desc_id) {
            self.class_by_type.insert(type_id, c);
            return Ok(c);
        }
        if let Some(def_idx) = self.dex.class_by_descriptor(&desc) {
            let c = self.load_dex_class(def_idx)?;
            self.class_by_type.insert(type_id, c);
            return Ok(c);
        }
        if SHIM_CLASSES.iter().any(|d| d.desc == desc) {
            let c = self.load_shim_class(desc_id)?;
            self.class_by_type.insert(type_id, c);
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
            // find the dex type id for this array descriptor
            let sid = self
                .dex
                .strings
                .iter()
                .position(|s| s.as_ref() == desc)
                .ok_or_else(|| JvmError::Resolution(format!("array descriptor not in dex: {desc}")))? as u32;
            let type_id = self
                .dex
                .types
                .iter()
                .position(|t| *t == sid)
                .ok_or_else(|| JvmError::Resolution(format!("array type not in dex: {desc}")))?
                as u32;
            return self.array_class(type_id);
        }
        if let Some(def_idx) = self.dex.class_by_descriptor(&desc) {
            return self.load_dex_class(def_idx);
        }
        if SHIM_CLASSES.iter().any(|d| d.desc == desc) {
            return self.load_shim_class(desc_id);
        }
        Err(JvmError::Resolution(format!("class not found: {desc}")))
    }

    fn load_dex_class(&mut self, def_idx: usize) -> Result<u32, JvmError> {
        let def = self.dex.classes[def_idx].clone();
        if let Some(&c) = self.class_by_type.get(&def.class_idx) {
            return Ok(c);
        }
        let desc = self.dex.type_descriptor(def.class_idx).to_string();
        let desc_id = self.intern(&desc);
        if let Some(&c) = self.class_by_desc.get(&desc_id) {
            self.class_by_type.insert(def.class_idx, c);
            return Ok(c);
        }
        if self.loading.contains(&def_idx) {
            return Err(JvmError::Resolution("cyclic class hierarchy".into()));
        }
        self.loading.push(def_idx);

        let is_object = desc_id == self.hot.object;
        let superclass = if def.superclass_idx == u32::MAX {
            if is_object {
                None
            } else {
                Some(self.ensure_class_by_desc_id(self.hot.object)?)
            }
        } else {
            Some(self.ensure_class_by_type(def.superclass_idx)?)
        };
        let mut interfaces = Vec::new();
        for &t in &def.interfaces {
            interfaces.push(self.ensure_class_by_type(t)?);
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
        self.class_by_type.insert(def.class_idx, id);

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
                let f = self.dex.fields[ef.field_idx as usize].clone();
                let name = self.intern(&self.dex.strings[f.name as usize].to_string());
                let ty = self.intern(&self.dex.type_descriptor(f.ty).to_string());
                let off = statics.len() as u32;
                statics.push(JValue::Null);
                statics_lazy.push(None);
                static_fields.insert((name, ty), (id, off));
            }
            for ef in &cd.instance_fields {
                let f = self.dex.fields[ef.field_idx as usize].clone();
                let name = self.intern(&self.dex.strings[f.name as usize].to_string());
                let ty = self.intern(&self.dex.type_descriptor(f.ty).to_string());
                let off = instance_fields.len() as u32;
                field_offsets.insert((name, ty), off);
                instance_fields.push((name, ty, ef.access_flags));
            }
            // encoded static values (in declaration order)
            for (i, ev) in def.static_values.iter().enumerate() {
                let ty = self.dex.fields[cd.static_fields[i].field_idx as usize].ty;
                let ty_desc = self.dex.type_descriptor(ty).to_string();
                let ty_id = self.intern(&ty_desc);
                statics[i] = self.enc_to_value(ev, ty_id)?;
            }
            let push_methods = |vm: &mut Self,
                                list: &[crate::dex::EncodedMethod],
                                methods: &mut Vec<Method>,
                                dispatch: &mut HashMap<(u32, u32), u32>,
                                class_id: u32|
             -> Result<(), JvmError> {
                for em in list {
                    let m = vm.dex.methods[em.method_idx as usize].clone();
                    let name = vm.intern(&vm.dex.strings[m.name as usize].to_string());
                    let sig = vm.intern(&vm.proto_sig(m.proto));
                    let ret_desc = vm.dex.type_descriptor(vm.dex.protos[m.proto as usize].return_type).to_string();
                    let ret = vm.intern(&ret_desc);
                    let arg_descs: Vec<String> = vm.dex.protos[m.proto as usize]
                        .params
                        .iter()
                        .map(|&t| vm.dex.type_descriptor(t).to_string())
                        .collect();
                    let args: Vec<u32> = arg_descs.iter().map(|d| vm.intern(d)).collect();
                    let static_method = em.access_flags & ACC_STATIC != 0;
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
                        native_key: None,
                        code: em.code.clone(),
                        insns: OnceLock::new(),
                    });
                }
                Ok(())
            };
            push_methods(self, &cd.direct_methods, &mut methods, &mut dispatch, id)?;
            push_methods(self, &cd.virtual_methods, &mut methods, &mut dispatch, id)?;
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

    fn load_shim_class(&mut self, desc_id: u32) -> Result<u32, JvmError> {
        let desc = self.str_of(desc_id).to_string();
        let def = SHIM_CLASSES
            .iter()
            .find(|d| d.desc == desc)
            .ok_or_else(|| JvmError::Resolution(format!("not a shim class: {desc}")))?;
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
        let mut shim_natives: Vec<&NativeEntry> =
            native::native_tables().into_iter().flatten().collect();
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
                native_key: Some((desc_id, name, sig)),
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

    pub(crate) fn array_class(&mut self, elem_type: u32) -> Result<u32, JvmError> {
        if let Some(&c) = self.array_classes.get(&elem_type) {
            return Ok(c);
        }
        let elem_desc = self.dex.type_descriptor(elem_type);
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
            array_elem: Some(elem_type),
            ..Default::default()
        });
        self.class_by_desc.insert(desc_id, id);
        self.array_classes.insert(elem_type, id);
        Ok(id)
    }

    // ---- field resolution ----

    pub fn field_ref(&mut self, field_idx: u32) -> Result<FieldRef, JvmError> {
        if let Some(fr) = self.field_refs.get(&field_idx) {
            return Ok(*fr);
        }
        let f = self
            .dex
            .fields
            .get(field_idx as usize)
            .cloned()
            .ok_or_else(|| JvmError::Resolution(format!("bad field idx {field_idx}")))?;
        let name = self.intern(&self.dex.strings[f.name as usize].to_string());
        let ty = self.intern(&self.dex.type_descriptor(f.ty).to_string());
        let class_desc = self.intern(&self.dex.type_descriptor(f.class).to_string());
        let fr = FieldRef { name, ty, class_desc };
        self.field_refs.insert(field_idx, fr);
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
            if let Some(&(owner, off)) = self.classes[cc as usize].static_fields.get(&(fr.name, fr.ty)) {
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
        for &s in self.string_objs.iter().flatten() {
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
        for id in 0..n {
            if !marks[id] {
                self.arena.reclaim(id as u32);
                freed += 1;
            }
        }
        self.monitors.retain(|id, _| marks[*id as usize]);
        freed
    }

    // ---- invocation helpers (used by natives) ----

    pub fn invoke_virtual(&mut self, receiver: JValue, name: &str, sig: &str) -> Result<JValue, JvmError> {
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
        let target = self.resolve_target(InvokeKind::Virtual, &mref, Some(recv))?;
        let mut args = Vec::with_capacity(1 + mref.args.len());
        args.push(receiver);
        self.call_target(target, args)
    }

    pub fn invoke_static(&mut self, class_desc: &str, name: &str, sig: &str, args: Vec<JValue>) -> Result<JValue, JvmError> {
        let mref = MethodRef {
            name: self.intern(name),
            sig: self.intern(sig),
            ret: 0,
            args: Vec::new(),
            class_desc: self.intern(class_desc),
        };
        let target = self.resolve_target(InvokeKind::Static, &mref, None)?;
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
        for cd in &self.dex.classes {
            let type_str = self
                .dex
                .types
                .get(cd.class_idx as usize)
                .and_then(|&s| self.dex.strings.get(s as usize).cloned())
                .unwrap_or_default();
            let has = match &cd.class_data {
                Some(data) => data
                    .direct_methods
                    .iter()
                    .chain(data.virtual_methods.iter())
                    .any(|m| {
                        self.dex
                            .strings
                            .get(self.dex.methods[m.method_idx as usize].name as usize)
                            .map(|s| s.as_ref() == name)
                            .unwrap_or(false)
                    }),
                None => false,
            };
            if has {
                let cid = self.ensure_class_by_desc(&type_str)?;
                return Ok(self.str_of(self.classes[cid as usize].descriptor).to_string());
            }
        }
        Err(JvmError::Resolution(format!("no class with method {name} in dex")))
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

    /// Calls a resolved target with a fresh frame (native or bytecode).
    pub fn call_target(&mut self, target: Target, args: Vec<JValue>) -> Result<JValue, JvmError> {
        match target {
            Target::Native(key) => {
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
            Target::Bytecode { class, slot, .. } => interpret::run(self, class, slot, args),
        }
    }

    // ---- prototypes/signatures ----

    pub fn proto_sig(&self, proto_id: u32) -> String {
        let p = &self.dex.protos[proto_id as usize];
        let mut s = String::from("(");
        for &t in &p.params {
            s.push_str(self.dex.type_descriptor(t));
        }
        s.push(')');
        s.push_str(self.dex.type_descriptor(p.return_type));
        s
    }

    pub fn method_ref(&mut self, method_idx: u32) -> Result<MethodRef, JvmError> {
        if let Some(mr) = self.method_refs.get(&method_idx) {
            return Ok(mr.clone());
        }
        let m = self
            .dex
            .methods
            .get(method_idx as usize)
            .cloned()
            .ok_or_else(|| JvmError::Resolution(format!("bad method idx {method_idx}")))?;
        let name = self.intern(&self.dex.strings[m.name as usize].to_string());
        let sig = self.intern(&self.proto_sig(m.proto));
        let ret_desc = self.dex.type_descriptor(self.dex.protos[m.proto as usize].return_type).to_string();
        let ret = self.intern(&ret_desc);
        let arg_descs: Vec<String> = self.dex.protos[m.proto as usize]
            .params
            .iter()
            .map(|&t| self.dex.type_descriptor(t).to_string())
            .collect();
        let args: Vec<u32> = arg_descs.iter().map(|d| self.intern(d)).collect();
        let class_desc = self.intern(&self.dex.type_descriptor(m.class).to_string());
        let mr = MethodRef { name, sig, ret, args, class_desc };
        self.method_refs.insert(method_idx, mr.clone());
        Ok(mr)
    }

    // ---- call target resolution ----

    pub fn resolve_target(
        &mut self,
        kind: InvokeKind,
        mref: &MethodRef,
        receiver: Option<u32>,
    ) -> Result<Target, JvmError> {
        let key = (mref.name, mref.sig);
        // The first class to search depends on the invoke kind.
        let mut c = match kind {
            InvokeKind::Virtual | InvokeKind::Interface => receiver
                .and_then(|o| self.arena.objects.get(o as usize))
                .map(|o| o.class)
                .ok_or_else(|| JvmError::Resolution("null receiver for virtual call".into()))?,
            InvokeKind::Static | InvokeKind::Direct => self.ensure_class_by_desc_id(mref.class_desc)?,
            InvokeKind::Super => {
                let c = self.ensure_class_by_desc_id(mref.class_desc)?;
                self.classes[c as usize]
                    .superclass
                    .ok_or_else(|| JvmError::Resolution("invoke-super with no superclass".into()))?
            }
        };
        let c0 = c;
        let mut slot: Option<(u32, u32)> = None;
while slot.is_none() {
            let found = {
                let class = self.classes.get(c as usize).ok_or_else(|| {
                    JvmError::Resolution(format!("missing class id {c}"))
                })?;
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
                "no method {} {} found",
                self.str_of(mref.name),
                self.str_of(mref.sig)
            ))
        })?;
        let (native_key, code, decoded, ins_size, registers, ret, args, static_method) = {
            let m = &self.classes[found_class as usize].methods[slot as usize];
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
                        let d = Arc::new(crate::dex::insn::decode_all(&code.insns).map_err(JvmError::from)?);
                        m.insns.set(d.clone()).expect("decode race");
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
            decoded: decoded.unwrap(),
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
        let id = self
            .arena
            .alloc(self.hot.string, Vec::new(), Some(Native::Str(s.to_string())));
        self.runtime_strings.insert(s.to_string(), id);
        JValue::Obj(id)
    }

    pub fn dex_string(&mut self, string_id: u32) -> Result<JValue, JvmError> {
        if let Some(&Some(o)) = self.string_objs.get(string_id as usize) {
            return Ok(JValue::Obj(o));
        }
        let s = self
            .dex
            .strings
            .get(string_id as usize)
            .ok_or_else(|| JvmError::Resolution(format!("bad string idx {string_id}")))?;
        let o = self
            .arena
            .alloc(self.hot.string, Vec::new(), Some(Native::Str(s.to_string())));
        while self.string_objs.len() <= string_id as usize {
            self.string_objs.push(None);
        }
        self.string_objs[string_id as usize] = Some(o);
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

    pub fn throwable_of(&mut self, class_desc: &str, message: impl Into<String>) -> u32 {
        let class = self.ensure_class_by_desc(class_desc).expect("shim throwable");
        self.arena
            .alloc(class, Vec::new(), Some(Native::Throwable { message: Some(message.into()), cause: JValue::Null }))
    }

    pub fn err_npe(&mut self) -> u32 {
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
        self.throwable_of("Ljava/lang/IndexOutOfBoundsException;", format!("Index {idx}"))
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
    pub fn err_sioobe(&mut self, msg: impl Into<String>) -> u32 {
        self.throwable_of("Ljava/lang/StringIndexOutOfBoundsException;", msg)
    }

    // ---- assignability ----

    pub fn is_assignable(&mut self, obj_class: u32, target: u32) -> Result<bool, JvmError> {
        if obj_class == target {
            return Ok(true);
        }
        let is_target_array = self.classes[target as usize].array_elem.is_some();
        let mut c = Some(obj_class);
        let mut depth = 0usize;
        while let Some(cc) = c {
            if depth > 64 {
                return Err(JvmError::Resolution("class hierarchy too deep".into()));
            }
            depth += 1;
            let cl = &self.classes[cc as usize];
            if cc == target {
                return Ok(true);
            }
            if let Some(elem) = cl.array_elem {
                // array targets
                if is_target_array {
                    let t_elem = self.classes[target as usize].array_elem.unwrap();
                    let e_desc = self.dex.type_descriptor(elem);
                    let t_desc = self.dex.type_descriptor(t_elem);
                    if e_desc == t_desc {
                        return Ok(true);
                    }
                    if !e_desc.starts_with('[') && e_desc.len() == 1 && t_desc.starts_with('[') {
                        return Ok(false);
                    }
                    if e_desc.len() == 1 || t_desc.len() == 1 {
                        return Ok(false); // primitive arrays only match identical
                    }
                    let ec = self.ensure_class_by_type(elem)?;
                    let tc = self.ensure_class_by_type(t_elem)?;
                    return self.is_assignable(ec, tc);
                }
                // array to Cloneable/Serializable/Object
                for &i in &cl.interfaces {
                    if i == target {
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            for &i in &cl.interfaces {
                if i == target {
                    return Ok(true);
                }
            }
            c = cl.superclass;
        }
        Ok(false)
    }

    // ---- encoded values ----

    fn enc_to_value(&mut self, ev: &EncodedValue, _ty: u32) -> Result<JValue, JvmError> {
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
            EncodedValue::String(s) => self.dex_string(*s)?,
            EncodedValue::Type(t) => {
                let c = self.ensure_class_by_type(*t)?;
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
        let (class, name, sig) = (self.intern(e.class), self.intern(e.name), self.intern(e.sig));
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
                native_key: Some((class, name, sig)),
                code: None,
                insns: OnceLock::new(),
            });
            self.classes[cid as usize].dispatch.insert((name, sig), slot);
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
        while d.len() % 4 != 0 {
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
        push4(&mut d, (1u32 << 16) | 0); // class 0, proto 1
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
        let mut vm = Vm::new(dex, Box::new(Vec::new())).expect("vm");
        let cid = vm.ensure_class_by_desc("LHello;").expect("load LHello;");
        let f_slot = vm.classes[cid as usize]
            .methods
            .iter()
            .position(|m| vm.str_of(m.name) == "f")
            .expect("method f") as u32;
        let r = interpret::run(&mut vm, cid, f_slot, vec![JValue::Int(2), JValue::Int(3)]).expect("run f");
        assert_eq!(r, JValue::Int(5));
        let g_slot = vm.classes[cid as usize]
            .methods
            .iter()
            .position(|m| vm.str_of(m.name) == "g")
            .expect("method g") as u32;
        let r = interpret::run(&mut vm, cid, g_slot, vec![]).expect("run g");
        assert_eq!(r, JValue::Int(3));
    }
}
