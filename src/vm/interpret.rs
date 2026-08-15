//! The interpreter core: stepping through dex bytecode one instruction at a
//! time, with try/catch unwinding and inlined native dispatch.

use std::sync::Arc;

use log::debug;


use crate::dex::insn::{Args, Binop, CmpOp, FillArray, IfOp, Insn, InvokeKind, Unop};
use crate::dex::TryItem;
use crate::vm::error::JvmError;
use crate::vm::object::{ArrayData, Native};
use crate::vm::value::JValue;
use crate::vm::{MethodRef, NatErr, Target, Vm};

#[derive(Debug, Clone)]
pub struct Frame {
    pub(crate) class: u32,
    pub(crate) slot: u32,
    /// Dex file (index into `Vm::dexes`) the method's ids refer to.
    dex: u32,
    decoded: Arc<crate::dex::insn::Decoded>,
    tries: Arc<[TryItem]>,
    regs: Vec<JValue>,
    pc: usize,
    err_pc: usize,
    result: JValue,
    pending_exc: Option<JValue>,
}

#[derive(Debug)]
enum Flow {
    Next(usize),
    Jump(usize),
    Ret(JValue),
    Call(InvokeKind, MethodRef, Target, Args, usize),
}

#[derive(Debug)]
enum StepOutcome {
    Ok(Flow),
    Throw(JValue),
}

impl Frame {
    /// Dalvik lays out the incoming arguments as the *last* `ins_size`
    /// registers of the frame (vN-ins+1 .. vN), not the first.
    fn make_regs(args: &[JValue], ins_size: u16, registers: u16) -> Vec<JValue> {
        let mut regs = vec![JValue::Null; registers as usize];
        let base = registers as usize - ins_size as usize;
        for (i, a) in args.iter().enumerate() {
            if i < ins_size as usize {
                regs[base + i] = *a;
            }
        }
        regs
    }
}

/// Runs a method with a fresh frame. Used for `<clinit>` and for calls made
/// from native code. Native-backing methods (host APIs) run directly.
pub fn run(vm: &mut Vm, class: u32, slot: u32, args: Vec<JValue>) -> Result<JValue, JvmError> {
    let m = &vm.classes[class as usize].methods[slot as usize];
    if vm.recursion_depth >= vm.depth_limit {
        let chain = vm
            .trace_ring
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  ");
        return Err(JvmError::Fatal(format!(
            "vm re-entry stack overflow (recur_depth {} above limit {}):\n  {}",
            vm.recursion_depth, vm.depth_limit, chain
        )));
    }
    vm.recursion_depth += 1;
    if vm.trace_ring.len() >= 24 {
        vm.trace_ring.pop_front();
    }
    vm.trace_ring.push_back(format!(
        "{}.{}{}",
        vm.class_desc_str(class),
        vm.str_of(m.name),
        vm.str_of(m.sig)
    ));
    if std::env::var("DEXVM_TRACE").is_ok() {
        eprintln!(
            "DEXVM_TRACE enter {}.{}{}",
            vm.class_desc_str(class),
            vm.str_of(m.name),
            vm.str_of(m.sig)
        );
    }
    if m.native_decl {
        return Err(JvmError::Resolution(format!(
            "native method {} {} has no JNI bridge (JNI unsupported)",
            vm.str_of(m.name),
            vm.str_of(m.sig)
        )));
    }
    let native_key = m.native_key;
    if let Some(key) = native_key {
        let f = *vm
            .natives
            .get(&key)
            .ok_or_else(|| JvmError::Fatal(format!("no native for {key:?}")))?;
        return match f(vm, &args) {
            Ok(v) => Ok(v),
            Err(NatErr::Throw(ex)) => Err(JvmError::Uncaught(ex)),
            Err(NatErr::Fatal(e)) => Err(e),
        };
    }
    let saved = std::mem::take(&mut vm.frames);
    let r = (|| {
        push_frame(vm, class, slot, args)?;
        vm.run_loop()
    })();
    vm.frames = saved;
    vm.recursion_depth -= 1;
    r
}

fn push_frame(vm: &mut Vm, class: u32, slot: u32, args: Vec<JValue>) -> Result<(), JvmError> {
    if vm.frames.len() >= vm.depth_limit {
        let mut dump = Vec::new();
        for f in vm.frames.iter().rev().take(20) {
            let m = &vm.classes[f.class as usize].methods[f.slot as usize];
            dump.push(format!(
                "  {} . {}",
                vm.class_desc_str(f.class),
                vm.str_of(m.name)
            ));
        }
        let m = &vm.classes[class as usize].methods[slot as usize];
        dump.push(format!(
            "  {} . {}",
            vm.class_desc_str(class),
            vm.str_of(m.name)
        ));
        return Err(JvmError::Fatal(format!(
            "guest stack overflow (depth {}):\n{}",
            vm.depth_limit,
            dump.join("\n")
        )));
    }
    if std::env::var("DEXVM_TRACE").is_ok() {
        let m = &vm.classes[class as usize].methods[slot as usize];
        eprintln!(
            "DEXVM_TRACE push {}.{} argscount={} args={:?}",
            vm.class_desc_str(class),
            vm.str_of(m.name),
            args.len(),
            args.iter()
                .map(|a| match a {
                    JValue::Obj(o) => vm
                        .object_class(JValue::Obj(*o))
                        .map(|c| vm.class_desc_str(c))
                        .unwrap_or_else(|| "?".into()),
                    JValue::Null => "Null".into(),
                    JValue::Int(v) => format!("Int({v})"),
                    JValue::Long(v) => format!("Long({v})"),
                    other => format!("{other:?}"),
                })
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let (dex, ins_size, registers, decoded, tries) = {
        let m = &vm.classes[class as usize].methods[slot as usize];
        if std::env::var("DEXVM_TRACE").is_ok() {
            if let Some(code) = &m.code {
                eprintln!(
                    "DEXVM_TRACE codeitem {}.{} ins={} regs={}",
                    vm.class_desc_str(class),
                    vm.str_of(m.name),
                    code.ins_size,
                    code.registers_size
                );
            }
        }
        let code = m
            .code
            .as_ref()
            .ok_or_else(|| JvmError::Resolution("no code item".into()))?;
        let decoded = match m.insns.get() {
            Some(d) => d.clone(),
            None => {
                let d =
                    Arc::new(crate::dex::insn::decode_all(&code.insns).map_err(JvmError::from)?);
                let _ = m.insns.set(d.clone());
                d
            }
        };
        let tries: Arc<[TryItem]> = Arc::from(code.tries.clone());
        (
            m.dex_idx,
            code.ins_size,
            code.registers_size,
            decoded,
            tries,
        )
    };
    vm.frames.push(Frame {
        class,
        slot,
        dex,
        decoded,
        tries,
        regs: Frame::make_regs(&args, ins_size, registers),
        pc: 0,
        err_pc: 0,
        result: JValue::Null,
        pending_exc: None,
    });
    Ok(())
}

impl Vm {
    fn native_label(&self, nf: crate::vm::NativeFn) -> String {
        for t in crate::vm::native::native_tables() {
            for e in t {
                if e.f as usize == nf as usize {
                    return format!("{}::{}{}", e.class, e.name, e.sig);
                }
            }
        }
        "?".into()
    }

    pub fn run_loop(&mut self) -> Result<JValue, JvmError> {
        let mut f = match self.frames.pop() {
            Some(f) => f,
            None => return Err(JvmError::Fatal("run_loop with no frames".into())),
        };
        loop {
            if self.budget <= 0 {
                return Err(JvmError::Fatal(
                    "interpreter budget exhausted (possible infinite loop)".into(),
                ));
            }
            self.budget -= 1;
            let step = match self.step(&mut f) {
                Ok(s) => s,
                Err(e) => {
                    debug!(
                        "ERR {} pc={:#06x} class={} method={}: {e}",
                        self.str_of(self.classes[f.class as usize].descriptor),
                        f.pc,
                        self.class_desc_str(f.class),
                        self.str_of(self.classes[f.class as usize].methods[f.slot as usize].name),
                    );
                    return Err(e);
                }
            };
            match step {
                StepOutcome::Ok(flow) => match flow {
                    Flow::Next(pc) => f.pc = pc,
                    Flow::Jump(t) => f.pc = t,
                    Flow::Ret(v) => match self.frames.last_mut() {
                        Some(top) => {
                            top.result = v;
                            f = self.frames.pop().ok_or_else(|| {
                                JvmError::Fatal("empty frame stack on return".into())
                            })?;
                        }
                        None => return Ok(v),
                    },
                    Flow::Call(kind, mref, target, args, ret_pc) => {
                        let receiver = if kind == InvokeKind::Static {
                            None
                        } else {
                            Some(f.regs[args.reg_at(0) as usize])
                        };
                        if std::env::var("DEXVM_TRACE").is_ok() {
                            let recv = receiver
                                .and_then(|v| match v {
                                    JValue::Obj(o) => self
                                        .object_class(JValue::Obj(o))
                                        .map(|c| self.class_desc_str(c)),
                                    _ => None,
                                })
                                .unwrap_or_else(|| format!("{:?}", receiver));
                            let tgt = match &target {
                                Target::Native(key) => {
                                    format!("NAT {}.{}", self.str_of(key.0), self.str_of(key.1))
                                }
                                Target::Bytecode { class, slot, .. } => {
                                    format!(
                                        "VM {}.{}",
                                        self.class_desc_str(*class),
                                        self.str_of(
                                            self.classes[*class as usize].methods[*slot as usize]
                                                .name
                                        )
                                    )
                                }
                            };
                            if std::env::var("DEXVM_TRACE").is_ok() {
                                if let Target::Bytecode {
                                    class,
                                    slot,
                                    registers,
                                    ins_size,
                                    ..
                                } = &target
                                {
                                    eprintln!(
                                        "DEXVM_TRACE   target {}.{}.{} regs={} ins={} callcount={}",
                                        self.class_desc_str(*class),
                                        self.str_of(
                                            self.classes[*class as usize].methods[*slot as usize]
                                                .name
                                        ),
                                        self.str_of(
                                            self.classes[*class as usize].methods[*slot as usize]
                                                .sig
                                        ),
                                        registers,
                                        ins_size,
                                        args.count
                                    );
                                }
                            }
                            eprintln!(
                                "DEXVM_TRACE call {} in {} -> {tgt} recv={recv}",
                                self.str_of(mref.name),
                                self.class_desc_str(f.class),
                            );
                        }
                        match &target {
                            Target::Native(key) => {
                                if self.str_of(mref.name) == "<init>" {
                                    let (kc, _, ks) = *key;
                                    debug!(
                                        "INV native <init> sig={} class={} args={}",
                                        self.str_of(ks),
                                        self.str_of(kc),
                                        args.count
                                    );
                                }
                                let nf = *self.natives.get(key).ok_or_else(|| {
                                    JvmError::Fatal(format!("no native for {key:?}"))
                                })?;
                                let mut call_args = Vec::with_capacity(
                                    args.count as usize + usize::from(receiver.is_some()),
                                );
                                if let Some(r) = receiver {
                                    call_args.push(r);
                                }
                                let mut ri = if kind == InvokeKind::Static { 0 } else { 1 };
                                for &arg_desc in &mref.args {
                                    call_args.push(f.regs[args.reg_at(ri) as usize]);
                                    ri += 1;
                                    if is_wide_desc(self.str_of(arg_desc)) {
                                        ri += 1;
                                    }
                                }
                                match nf(self, &call_args) {
                                    Ok(v) => {
                                        // <init> natives conventionally
                                        // return a freshly allocated object;
                                        // the bytecode keeps the pre-allocated
                                        // receiver, so carry the payload over.
                                        if self.str_of(mref.name) == "<init>" {
                                            if let (Some(JValue::Obj(r)), JValue::Obj(res)) =
                                                (receiver, v)
                                            {
                                                let payload = self
                                                    .arena
                                                    .objects
                                                    .get(res as usize)
                                                    .and_then(|o| o.native.clone());
                                                if payload.is_some() {
                                                    if let Some(o) =
                                                        self.arena.objects.get_mut(r as usize)
                                                    {
                                                        o.native = payload;
                                                    }
                                                }
                                            }
                                        }
                                        f.result = v;
                                        f.pc = ret_pc;
                                    }
                                    Err(NatErr::Throw(ex)) => {
                                        let (dbg_c, dbg_s) = (f.class, f.slot);
                                        f.pc = ret_pc;
                                        f.pending_exc = Some(JValue::Obj(ex));
                                        self.frames.push(f);
                                        match self.unwind()? {
                                            true => {
                                                f = self.frames.pop().ok_or_else(|| {
                                                    JvmError::Fatal(
                                                        "empty frame stack after unwind".into(),
                                                    )
                                                })?
                                            }
                                            false => {
                                                let exc_cls = self
                                                    .arena
                                                    .objects
                                                    .get(ex as usize)
                                                    .map(|o| self.class_desc_str(o.class))
                                                    .unwrap_or_default();
                                                if std::env::var("DEXVM_TRACE").is_ok() {
                                                    eprintln!(
                                                        "DEXVM_TRACE uncaught native-throw at {}::{} from native {} exc-class={}",
                                                        self.class_desc_str(dbg_c),
                                                        self.str_of(self.classes[dbg_c as usize].methods[dbg_s as usize].name),
                                                        self.native_label(nf),
                                                        exc_cls,
                                                    );
                                                }
                                                debug!(
                                                    "DBG uncaught native-throw at {}::{} from native {} exc-class={}",
                                                    self.class_desc_str(dbg_c),
                                                    self.str_of(self.classes[dbg_c as usize].methods[dbg_s as usize].name),
                                                    self.native_label(nf),
                                                    exc_cls,
                                                );
                                                return Err(JvmError::Uncaught(ex));
                                            }
                                        }
                                    }
                                    Err(NatErr::Fatal(e)) => return Err(e),
                                }
                            }
                            Target::Bytecode {
                                class,
                                slot,
                                decoded,
                                code,
                                ins_size,
                                registers,
                                args: m_args,
                                ..
                            } => {
                                let mut call_args = Vec::with_capacity(
                                    args.count as usize + usize::from(receiver.is_some()),
                                );
                                if let Some(r) = receiver {
                                    call_args.push(r);
                                }
                                let mut ri = if kind == InvokeKind::Static { 0 } else { 1 };
                                for &arg_desc in m_args {
                                    let a = f.regs[args.reg_at(ri) as usize];
                                    ri += 1;
                                    if is_wide_desc(self.str_of(arg_desc)) {
                                        // wide args occupy two registers: duplicate so
                                        // the callee's register file stays aligned.
                                        call_args.push(a);
                                        ri += 1;
                                    }
                                    call_args.push(a);
                                }
                                let code = code.clone().ok_or_else(|| {
                                    JvmError::Resolution("callee has no code item".into())
                                })?;
                                let tries: Arc<[TryItem]> = Arc::from(code.tries.clone());
                                let callee_dex =
                                    self.classes[*class as usize].methods[*slot as usize].dex_idx;
                                f.pc = ret_pc;
                                let caller = std::mem::replace(
                                    &mut f,
                                    Frame {
                                        class: *class,
                                        slot: *slot,
                                        dex: callee_dex,
                                        decoded: decoded.clone(),
                                        tries,
                                        regs: Frame::make_regs(&call_args, *ins_size, *registers),
                                        pc: 0,
                                        err_pc: 0,
                                        result: JValue::Null,
                                        pending_exc: None,
                                    },
                                );
                                self.frames.push(caller);
                            }
                        }
                    }
                },
                StepOutcome::Throw(ex) => {
                    if let JValue::Obj(o) = ex {
                        let m = match self.payload_of(JValue::Obj(o)) {
                            Some(Native::Throwable { message, .. }) => {
                                message.clone().unwrap_or_default()
                            }
                            _ => String::new(),
                        };
                        if std::env::var("DEXVM_TRACE").is_ok() {
                            eprintln!(
                                "DEXVM_TRACE throw pc={:04x} cls={} m={} msg={:?}",
                                f.pc,
                                self.class_desc_str(f.class),
                                self.method_desc_str(f.class, f.slot),
                                m
                            );
                        }
                        debug!(
                            "DBG throw pc={:04x} cls={} msg={:?}",
                            f.pc,
                            self.class_desc_str(f.class),
                            m
                        );
                        if m.is_empty() {
                            let mut acc = 0usize;
                            let mut iv: Option<&crate::dex::insn::Insn> = None;
                            for (i, s) in f.decoded.sizes.iter().enumerate() {
                                if acc + *s as usize > f.pc {
                                    iv = Some(&f.decoded.insns[i]);
                                    break;
                                }
                                acc += *s as usize;
                            }
                            let r6 = f.regs.get(6).copied().unwrap_or(JValue::Null);
                            let r6cls = match r6 {
                                JValue::Obj(o) => Some(self.class_desc_str(
                                    self.object_class(JValue::Obj(o)).unwrap_or(0),
                                )),
                                _ => None,
                            };
                            debug!(
                                "DBG NPE-throw pc={:04x} cls={} insn={:?} v6={:?}",
                                f.pc,
                                self.class_desc_str(f.class),
                                iv,
                                r6cls
                            );
                            for &ninst in &[
                                f.regs.len().saturating_sub(2),
                                f.regs.len().saturating_sub(1),
                                f.regs.len().saturating_sub(3),
                            ] {
                                let Some(JValue::Obj(fo)) = f.regs.get(ninst).copied() else {
                                    continue;
                                };
                                let fcls = self.class_desc_str(
                                    self.object_class(JValue::Obj(fo)).unwrap_or(0),
                                );
                                for name in ["a", "b", "c"] {
                                    let field_value = self.instance_field(fo, name);
                                    let Some(JValue::Obj(lo)) = field_value else {
                                        continue;
                                    };
                                    if let Native::List(l) =
                                        self.payload_of(JValue::Obj(lo)).unwrap_or(Native::Opaque)
                                    {
                                        debug!("DBG {fcls}#{name} list len={}", l.len());
                                        for (i, it) in l.iter().enumerate() {
                                            let c = match *it {
                                                JValue::Obj(o) => self.class_desc_str(
                                                    self.object_class(JValue::Obj(o)).unwrap_or(0),
                                                ),
                                                _ => format!("{it:?}"),
                                            };
                                            debug!("DBG rules[{i}] = {c}");
                                            if let JValue::Obj(ro) = *it {
                                                for (n, _) in [("a", ""), ("b", ""), ("c", "")] {
                                                    let (idx, val) = match self
                                                        .instance_field_id(ro, n)
                                                    {
                                                        Some((i, val)) => (i, format!("{val:?}")),
                                                        None => (usize::MAX, "none".into()),
                                                    };
                                                    debug!("DBG   c1.{n}[{idx}] = {val}");
                                                }
                                                let o = self.arena.objects.get(ro as usize);
                                                match o {
                                                    Some(o) => {
                                                        debug!(
                                                            "DBG   c1-cid={} fields-len={}",
                                                            o.class,
                                                            o.fields.len()
                                                        );
                                                        for (i, fv) in o.fields.iter().enumerate() {
                                                            debug!("DBG   c1.F[{i}] = {fv:?}");
                                                        }
                                                        let Some(ccls) =
                                                            self.classes.get(o.class as usize)
                                                        else {
                                                            debug!("DBG   c1-class-missing");
                                                            continue;
                                                        };
                                                        let names: Vec<&str> = ccls
                                                            .instance_fields
                                                            .iter()
                                                            .map(|(n, ..)| self.str_of(*n))
                                                            .collect();
                                                        debug!("DBG   c1-i-field-names={names:?}");
                                                    }
                                                    None => debug!("DBG   c1-not-found"),
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let (dbg_c, dbg_s) = (f.class, f.slot);
                    f.pending_exc = Some(ex);
                    self.frames.push(f);
                    match self.unwind()? {
                        true => {
                            f = self.frames.pop().ok_or_else(|| {
                                JvmError::Fatal("empty frame stack after unwind".into())
                            })?
                        }
                        false => {
                            let o = match ex {
                                JValue::Obj(o) => o,
                                _ => 0,
                            };
                            debug!(
                                "DBG uncaught throw at {}::{}",
                                self.class_desc_str(dbg_c),
                                self.str_of(
                                    self.classes[dbg_c as usize].methods[dbg_s as usize].name
                                ),
                            );
                            return Err(JvmError::Uncaught(o));
                        }
                    }
                }
            }
        }
    }
}

impl Vm {
    /// Finds the nearest enclosing handler for the top frame's `pending_exc`
    /// and jumps there. Returns false when the exception escapes everything.
    fn unwind(&mut self) -> Result<bool, JvmError> {
        let mut carried: Option<JValue> = None;
        loop {
            let (dex, insn_addr, tries, exc) = {
                let f = match self.frames.last_mut() {
                    Some(f) => f,
                    None => return Ok(false),
                };
                let exc = match f.pending_exc.take().or_else(|| carried.take()) {
                    Some(e) => e,
                    None => return Err(JvmError::Fatal("unwind without pending exception".into())),
                };
                // err_pc is a code-unit offset; find the exact instruction
                // address to match against try-region spans.
                let idx = f
                    .decoded
                    .units
                    .binary_search(&(f.err_pc as u32))
                    .unwrap_or_else(|i| i.saturating_sub(1));
                let insn_addr = f.decoded.units.get(idx).copied().unwrap_or(0);
                (f.dex, insn_addr, f.tries.clone(), exc)
            };
            let catch_addr = {
                for t in tries.iter() {
                    let end = t.start_addr + u32::from(t.insn_count);
                    debug!(
                        "DBG try [{:04x},{:04x}) handlers={:?} catchall={:?}",
                        t.start_addr, end, t.handlers, t.catch_all
                    );
                }
                let mut found = None;
                for t in tries.iter() {
                    let end = t.start_addr + u32::from(t.insn_count);
                    if insn_addr < t.start_addr || insn_addr >= end {
                        continue;
                    }
                    for &(type_idx, addr) in &t.handlers {
                        let matches = {
                            let catch_class = self.ensure_class_by_type(dex, type_idx)?;
                            let obj_class = match exc {
                                JValue::Obj(o) => Some(self.arena.objects[o as usize].class),
                                _ => None,
                            };
                            match obj_class {
                                Some(oc) => self.is_assignable(oc, catch_class)?,
                                None => false,
                            }
                        };
                        if matches {
                            found = Some(addr);
                            break;
                        }
                    }
                    if found.is_none() {
                        found = t.catch_all;
                    }
                    if found.is_some() {
                        break;
                    }
                }
                found
            };
            match catch_addr {
                Some(addr) => {
                    debug!("DBG unwind catch @ {:04x}", addr);
                    let Some(f) = self.frames.last_mut() else {
                        return Err(JvmError::Fatal(
                            "empty frame stack while installing catch".into(),
                        ));
                    };
                    // f.pc is a code-unit address, not an instruction index;
                    // jump to the handler's exact address.
                    f.pc = addr as usize;
                    f.regs[0] = exc;
                    return Ok(true);
                }
                None => {
                    carried = Some(exc);
                    self.frames.pop();
                }
            }
        }
    }

    fn step(&mut self, f: &mut Frame) -> Result<StepOutcome, JvmError> {
        let decoded = f.decoded.clone();
        let insns = &decoded.insns;
        let pc = f.pc;
        if pc >= decoded.words as usize {
            return Err(JvmError::Fatal(format!(
                "pc {pc} past end of method {}/{}",
                self.str_of(self.classes[f.class as usize].descriptor),
                self.str_of(self.classes[f.class as usize].methods[f.slot as usize].name)
            )));
        }
        f.err_pc = pc;
        let idx = decoded
            .units
            .binary_search(&(pc as u32))
            .unwrap_or_else(|i| i.saturating_sub(1));
        let next_pc = pc + decoded.sizes[idx] as usize;
        let insn = &insns[idx];
        if matches!(
            self.class_desc_str(f.class).as_str(),
            "e1" | "a0" | "f1" | "m"
        ) {
            let mut rs = String::new();
            if self.class_desc_str(f.class) == "m" && pc == 0 {
                for (_i, r) in f.regs.iter().enumerate().take(6) {
                    if let JValue::Obj(o) = r {
                        if let Some(obj) = self.arena.objects.get(*o as usize) {
                            rs.push_str(&format!("\nDBG   o{o} fields={:?}", obj.fields));
                        }
                    }
                }
            }
            debug!(
                "DBG step {} @ {:04x} {:?}{}",
                self.class_desc_str(f.class),
                pc,
                insns[idx],
                rs
            );
        }
        let out = match insn {
            Insn::Nop => Flow::Next(0),
            Insn::Move(d, s) => {
                f.regs[*d as usize] = f.regs[*s as usize];
                Flow::Next(0)
            }
            Insn::MoveWide(d, s) => {
                f.regs[*d as usize] = f.regs[*s as usize];
                f.regs[*d as usize + 1] = f.regs[*s as usize + 1];
                Flow::Next(0)
            }
            Insn::Const4(d, lit) => {
                f.regs[*d as usize] = JValue::Int(i32::from(*lit));
                Flow::Next(0)
            }
            Insn::Const16(d, lit) => {
                f.regs[*d as usize] = JValue::Int(i32::from(*lit));
                Flow::Next(0)
            }
            Insn::Const(d, lit) => {
                f.regs[*d as usize] = JValue::Int(*lit);
                Flow::Next(0)
            }
            Insn::ConstHigh16(d, lit) => {
                f.regs[*d as usize] = JValue::Int(lit.wrapping_shl(16));
                Flow::Next(0)
            }
            Insn::ConstWide16(d, lit) | Insn::ConstWide32(d, lit) => {
                f.regs[*d as usize] = JValue::Long(*lit);
                Flow::Next(0)
            }
            Insn::ConstWide(d, lit) => {
                f.regs[*d as usize] = JValue::Long(*lit);
                Flow::Next(0)
            }
            Insn::ConstWideHigh16(d, lit) => {
                f.regs[*d as usize] = JValue::Long(i64::from(lit.wrapping_shl(16) as u32) << 32);
                Flow::Next(0)
            }
            Insn::ConstString(d, str_idx) | Insn::ConstStringJumbo(d, str_idx) => {
                let s = self.dex_at(f.dex).strings[*str_idx as usize].clone();
                f.regs[*d as usize] = self.alloc_string(&s);
                Flow::Next(0)
            }
            Insn::ConstClass(d, type_idx) => {
                let c = self.ensure_class_by_type(f.dex, *type_idx)?;
                f.regs[*d as usize] = self.class_obj(c)?;
                Flow::Next(0)
            }
            Insn::MoveResult(d) | Insn::MoveResultWide(d) => {
                f.regs[*d as usize] = f.result;
                Flow::Next(0)
            }
            Insn::MoveException(d) => {
                f.regs[*d as usize] = f.regs[0];
                Flow::Next(0)
            }
            Insn::ConstMethodHandle(..) | Insn::ConstMethodType(..) => {
                return Err(JvmError::Fatal(
                    "const-method-handle/type unsupported".into(),
                ));
            }
            Insn::MonitorEnter(d) | Insn::MonitorExit(d) => {
                let o = match f.regs[*d as usize] {
                    JValue::Obj(o) => o,
                    _ => return Ok(StepOutcome::Throw(JValue::Obj(self.err_npe()))),
                };
                let is_enter = matches!(insn, Insn::MonitorEnter(_));
                if is_enter {
                    *self.monitors.entry(o).or_insert(0) += 1;
                } else {
                    let n = self.monitors.entry(o).or_insert(0);
                    *n = n.saturating_sub(1);
                    let done = *n == 0;
                    if done {
                        self.monitors.remove(&o);
                    }
                }
                Flow::Next(0)
            }
            Insn::CheckCast(d, type_idx) => {
                let target = self.ensure_class_by_type(f.dex, *type_idx)?;
                if let JValue::Obj(o) = f.regs[*d as usize] {
                    let oc = self.arena.objects[o as usize].class;
                    if !self.is_assignable(oc, target)? {
                        return Ok(StepOutcome::Throw(JValue::Obj(self.err_cce(format!(
                            "{} cannot be cast to {}",
                            self.class_desc_str(oc),
                            self.dex_at(f.dex).type_descriptor(*type_idx)
                        )))));
                    }
                }
                Flow::Next(0)
            }
            Insn::InstanceOf(d, src, type_idx) => {
                let target = self.ensure_class_by_type(f.dex, *type_idx)?;
                let r = match f.regs[*src as usize] {
                    JValue::Obj(o) => {
                        self.is_assignable(self.arena.objects[o as usize].class, target)?
                    }
                    // untyped register zero in a reference slot behaves as null
                    _ => false,
                };
                f.regs[*d as usize] = JValue::Int(i32::from(r));
                Flow::Next(0)
            }
            Insn::ArrayLength(d, src) => {
                let o = match f.regs[*src as usize] {
                    JValue::Obj(o) => o,
                    _ => return Ok(StepOutcome::Throw(JValue::Obj(self.err_npe()))),
                };
                let len = match &self.arena.objects[o as usize].native {
                    Some(Native::Array(a)) => a.len(),
                    _ => return Err(JvmError::Fatal("array-length on non-array".into())),
                };
                f.regs[*d as usize] = JValue::Int(len as i32);
                Flow::Next(0)
            }
            Insn::NewInstance(d, type_idx) => {
                let class_id = self.ensure_class_by_type(f.dex, *type_idx)?;
                let fields = self.classes[class_id as usize].field_offsets.len();
                f.regs[*d as usize] =
                    JValue::Obj(self.arena.alloc(class_id, vec![JValue::Null; fields], None));
                Flow::Next(0)
            }
            Insn::NewArray(d, size_reg, type_idx) => {
                let n = f.regs[*size_reg as usize].as_int();
                if n < 0 {
                    return Ok(StepOutcome::Throw(JValue::Obj(self.err_neg_arr_size())));
                }
                let inner_tid = self
                    .dex_at(f.dex)
                    .type_descriptor(*type_idx)
                    .strip_prefix('[')
                    .and_then(|inner| self.dex_at(f.dex).type_id_of(inner))
                    .unwrap_or(*type_idx);
                let arr_class = self.array_class(f.dex, inner_tid)?;
                let elem_desc = self
                    .dex_at(f.dex)
                    .type_descriptor(*type_idx)
                    .strip_prefix('[')
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| self.dex_at(f.dex).type_descriptor(*type_idx).to_string());
                let data = ArrayData::new(&elem_desc, n as usize);
                f.regs[*d as usize] = JValue::Obj(self.arena.alloc(
                    arr_class,
                    Vec::new(),
                    Some(Native::Array(data)),
                ));
                Flow::Next(0)
            }
            Insn::FilledNewArray(args, type_idx) => {
                let desc = self.dex_at(f.dex).type_descriptor(*type_idx).to_string();
                let elem_desc = desc
                    .strip_prefix('[')
                    .map(|s| s.to_string())
                    .unwrap_or(desc);
                let count = args.count as usize;
                let mut data = ArrayData::new(&elem_desc, count);
                let mut ri = 0u8;
                if std::env::var("DEXVM_TRACE").is_ok()
                    && self
                        .str_of(self.classes[f.class as usize].descriptor)
                        .contains("MangaDex")
                {
                    let vals = (0..count)
                        .map(|i| format!("{:?}", f.regs[args.reg_at(i as u8) as usize]))
                        .collect::<Vec<_>>()
                        .join(",");
                    eprintln!(
                        "DEXVM_TRACE filled-new-array {} regs=[{vals}]",
                        self.dex_at(f.dex).type_descriptor(*type_idx)
                    );
                }
                for i in 0..count {
                    data.set(i, f.regs[args.reg_at(ri) as usize]);
                    ri += 1;
                    if is_wide_desc(&elem_desc) {
                        ri += 1;
                    }
                }
                let inner_tid = self
                    .dex_at(f.dex)
                    .type_descriptor(*type_idx)
                    .strip_prefix('[')
                    .and_then(|inner| self.dex_at(f.dex).type_id_of(inner))
                    .unwrap_or(*type_idx);
                let arr_class = self.array_class(f.dex, inner_tid)?;
                let o = self
                    .arena
                    .alloc(arr_class, Vec::new(), Some(Native::Array(data)));
                f.result = JValue::Obj(o);
                Flow::Next(0)
            }
            Insn::FillArrayData(d, _, data) => {
                let o = match f.regs[*d as usize] {
                    JValue::Obj(o) => o,
                    _ => return Ok(StepOutcome::Throw(JValue::Obj(self.err_npe()))),
                };
                self.fill_array(o, data)?;
                Flow::Next(0)
            }
            Insn::Throw(d) => {
                let v = f.regs[*d as usize];
                if v.is_null_ref() {
                    return Ok(StepOutcome::Throw(JValue::Obj(self.err_npe())));
                }
                return Ok(StepOutcome::Throw(v));
            }
            Insn::Goto(t) => Flow::Jump(*t as usize),
            Insn::PackedSwitch(d, _, first, targets) => {
                let v = f.regs[*d as usize].as_int();
                let idx = v.wrapping_sub(*first);
                if idx >= 0 && (idx as usize) < targets.len() {
                    Flow::Jump(targets[idx as usize] as usize)
                } else {
                    Flow::Next(0)
                }
            }
            Insn::SparseSwitch(d, _, keys, targets) => {
                let v = f.regs[*d as usize].as_int();
                match keys.binary_search(&v) {
                    Ok(i) => Flow::Jump(targets[i] as usize),
                    Err(_) => Flow::Next(0),
                }
            }
            Insn::Cmp(kind, d, a, b) => {
                let (va, vb) = (f.regs[*a as usize], f.regs[*b as usize]);
                let c = match (*kind, va, vb) {
                    (CmpOp::CmpLong, JValue::Long(x), JValue::Long(y)) => x.cmp(&y) as i32,
                    (CmpOp::CmpgFloat, JValue::Float(x), JValue::Float(y)) => fcmp(x, y, true),
                    (CmpOp::CmplFloat, JValue::Float(x), JValue::Float(y)) => fcmp(x, y, false),
                    (CmpOp::CmpgDouble, JValue::Double(x), JValue::Double(y)) => fcmp(x, y, true),
                    (CmpOp::CmplDouble, JValue::Double(x), JValue::Double(y)) => fcmp(x, y, false),
                    _ => 0,
                };
                f.regs[*d as usize] = JValue::Int(c);
                Flow::Next(0)
            }
            Insn::If(op, a, b, t) => {
                let (va, vb) = (f.regs[*a as usize], f.regs[*b as usize]);
                let cond = match *op {
                    IfOp::Eq => jval_eq(&va, &vb),
                    IfOp::Ne => !jval_eq(&va, &vb),
                    IfOp::Lt => match (&va, &vb) {
                        (JValue::Int(x), JValue::Int(y)) => x < y,
                        (JValue::Float(x), JValue::Float(y)) => x < y,
                        (JValue::Double(x), JValue::Double(y)) => x < y,
                        _ => false,
                    },
                    IfOp::Ge => match (&va, &vb) {
                        (JValue::Int(x), JValue::Int(y)) => x >= y,
                        (JValue::Float(x), JValue::Float(y)) => x >= y,
                        (JValue::Double(x), JValue::Double(y)) => x >= y,
                        _ => false,
                    },
                    IfOp::Gt => match (&va, &vb) {
                        (JValue::Int(x), JValue::Int(y)) => x > y,
                        (JValue::Float(x), JValue::Float(y)) => x > y,
                        (JValue::Double(x), JValue::Double(y)) => x > y,
                        _ => false,
                    },
                    IfOp::Le => match (&va, &vb) {
                        (JValue::Int(x), JValue::Int(y)) => x <= y,
                        (JValue::Float(x), JValue::Float(y)) => x <= y,
                        (JValue::Double(x), JValue::Double(y)) => x <= y,
                        _ => false,
                    },
                    _ => false,
                };
                if cond {
                    Flow::Jump(*t as usize)
                } else {
                    Flow::Next(0)
                }
            }
            Insn::IfZ(op, a, t) => {
                let va = f.regs[*a as usize];
                let cond = match *op {
                    IfOp::Ez => va.is_zero(),
                    IfOp::Nz => !va.is_zero(),
                    IfOp::Ltz => match &va {
                        JValue::Int(x) => *x < 0,
                        JValue::Float(x) => *x < 0.0,
                        JValue::Double(x) => *x < 0.0,
                        _ => false,
                    },
                    IfOp::Gez => match &va {
                        JValue::Int(x) => *x >= 0,
                        JValue::Float(x) => *x >= 0.0,
                        JValue::Double(x) => *x >= 0.0,
                        _ => false,
                    },
                    IfOp::Gtz => match &va {
                        JValue::Int(x) => *x > 0,
                        JValue::Float(x) => *x > 0.0,
                        JValue::Double(x) => *x > 0.0,
                        _ => false,
                    },
                    IfOp::Lez => match &va {
                        JValue::Int(x) => *x <= 0,
                        JValue::Float(x) => *x <= 0.0,
                        JValue::Double(x) => *x <= 0.0,
                        _ => false,
                    },
                    _ => false,
                };
                if cond {
                    Flow::Jump(*t as usize)
                } else {
                    Flow::Next(0)
                }
            }
            Insn::AGet(_, d, arr, idx) => match self.array_get(f, *arr, *idx) {
                Ok(v) => {
                    f.regs[*d as usize] = v;
                    Flow::Next(0)
                }
                Err(ex) => return Ok(StepOutcome::Throw(ex)),
            },
            Insn::APut(_, src, arr, idx) => match self.array_put(f, *src, *arr, *idx) {
                Ok(()) => Flow::Next(0),
                Err(ex) => return Ok(StepOutcome::Throw(ex)),
            },
            Insn::SGet(d, field_idx)
            | Insn::SGetWide(d, field_idx)
            | Insn::SGetObj(d, field_idx) => {
                let fr = self.field_ref(f.dex, *field_idx)?;
                f.regs[*d as usize] = self.static_field_get(fr)?;
                Flow::Next(0)
            }
            Insn::SPut(src, field_idx)
            | Insn::SPutWide(src, field_idx)
            | Insn::SPutObj(src, field_idx) => {
                let fr = self.field_ref(f.dex, *field_idx)?;
                let v = f.regs[*src as usize];
                match self.static_field_put(fr, v) {
                    Ok(()) => Flow::Next(0),
                    Err(e) => return Err(e),
                }
            }
            Insn::IGet(d, obj, field_idx)
            | Insn::IGetWide(d, obj, field_idx)
            | Insn::IGetObj(d, obj, field_idx) => {
                let o = match f.regs[*obj as usize] {
                    JValue::Obj(o) => o,
                    _ => return Ok(StepOutcome::Throw(JValue::Obj(self.err_npe()))),
                };
                let oc = self.arena.objects[o as usize].class;
                let fr = self.field_ref(f.dex, *field_idx)?;
                let off = self.field_offset(oc, fr.name, fr.ty).ok_or_else(|| {
                    JvmError::Resolution(format!(
                        "no field {} in {}",
                        self.str_of(fr.name),
                        self.class_desc_str(oc)
                    ))
                })?;
                let fv = self.arena.objects[o as usize].fields[off as usize];
                let fcls = self.class_desc_str(oc);
                if fcls == "g1" || fcls == "c1" {
                    let ob_cls = match fv {
                        JValue::Obj(oo) => {
                            self.class_desc_str(self.object_class(JValue::Obj(oo)).unwrap_or(0))
                        }
                        _ => format!("{fv:?}"),
                    };
                    debug!(
                        "DBG IGet {} {} -> d{} = {}",
                        fcls,
                        self.str_of(fr.name),
                        f.regs.len().saturating_sub(2),
                        ob_cls
                    );
                }
                f.regs[*d as usize] = fv;
                Flow::Next(0)
            }
            Insn::IPut(src, obj, field_idx)
            | Insn::IPutWide(src, obj, field_idx)
            | Insn::IPutObj(src, obj, field_idx) => {
                let o = match f.regs[*obj as usize] {
                    JValue::Obj(o) => o,
                    _ => return Ok(StepOutcome::Throw(JValue::Obj(self.err_npe()))),
                };
                let oc = self.arena.objects[o as usize].class;
                let fr = self.field_ref(f.dex, *field_idx)?;
                let off = self.field_offset(oc, fr.name, fr.ty).ok_or_else(|| {
                    JvmError::Resolution(format!(
                        "no field {} in {}",
                        self.str_of(fr.name),
                        self.class_desc_str(oc)
                    ))
                })?;
                let v = f.regs[*src as usize];
                let fcls = self.class_desc_str(oc);
                if fcls == "g1" || fcls == "c1" {
                    let s = match v {
                        JValue::Obj(oo) => {
                            self.class_desc_str(self.object_class(JValue::Obj(oo)).unwrap_or(0))
                        }
                        _ => format!("{v:?}"),
                    };
                    debug!(
                        "DBG IPut {} {} <- src{}({}) = {}",
                        fcls,
                        self.str_of(fr.name),
                        *src,
                        f.regs.len(),
                        s
                    );
                }
                let stored = match insn {
                    Insn::IPutObj(..) if v.is_null_ref() => JValue::Null,
                    Insn::IPutObj(..) => {
                        let fty = self.str_of(fr.ty);
                        match (fty, v) {
                            ("Ljava/lang/Integer;", JValue::Int(i)) => {
                                crate::vm::native::boxed(
                                    self,
                                    "Ljava/lang/Integer;",
                                    Native::IntBox(i),
                                )?
                            }
                            ("Ljava/lang/Long;", JValue::Long(l)) => crate::vm::native::boxed(
                                self,
                                "Ljava/lang/Long;",
                                Native::LongBox(l),
                            )?,
                            ("Ljava/lang/Float;", JValue::Float(fv)) => crate::vm::native::boxed(
                                self,
                                "Ljava/lang/Float;",
                                Native::FloatBox(fv),
                            )?,
                            ("Ljava/lang/Double;", JValue::Float(fv)) => crate::vm::native::boxed(
                                self,
                                "Ljava/lang/Double;",
                                Native::DoubleBox(f64::from(fv)),
                            )?,
                            ("Ljava/lang/Boolean;", JValue::Int(b)) => crate::vm::native::boxed(
                                self,
                                "Ljava/lang/Boolean;",
                                Native::BoolBox(b != 0),
                            )?,
                            ("Ljava/lang/Character;", JValue::Int(c)) => crate::vm::native::boxed(
                                self,
                                "Ljava/lang/Character;",
                                Native::CharBox(c as u16),
                            )?,
                            ("Ljava/lang/Byte;", JValue::Int(b)) => crate::vm::native::boxed(
                                self,
                                "Ljava/lang/Byte;",
                                Native::ByteBox(b as i8),
                            )?,
                            ("Ljava/lang/Short;", JValue::Int(s)) => crate::vm::native::boxed(
                                self,
                                "Ljava/lang/Short;",
                                Native::ShortBox(s as i16),
                            )?,
                            _ => v,
                        }
                    }
                    _ => v,
                };
                self.arena.objects[o as usize].fields[off as usize] = stored;
                Flow::Next(0)
            }
            Insn::Invoke(kind, method_idx, args) => {
                let mref = self.method_ref(f.dex, *method_idx)?;
                let receiver = if *kind == InvokeKind::Static {
                    None
                } else {
                    let r = f.regs[args.reg_at(0) as usize];
                    if std::env::var("DEXVM_TRACE").is_ok()
                        && !matches!(r, JValue::Obj(_) | JValue::Null)
                    {
                        eprintln!(
                            "DEXTRACE bad-recv {} {} kind={kind:?} reg={:?} regs={:?}",
                            self.class_desc_str(f.class),
                            self.str_of(mref.name),
                            r,
                            &f.regs[..f.regs.len().min(8)]
                        );
                    }
                    if r.is_null_ref() {
                        if std::env::var("DEXVM_TRACE").is_ok() {
                            eprintln!(
                                "DEXVM_TRACE nullrecv {}.{} on {} reg0={:?}",
                                self.class_desc_str(f.class),
                                self.str_of(mref.name),
                                self.class_desc_str(f.class),
                                r
                            );
                        }
                        return Ok(StepOutcome::Throw(JValue::Obj(self.err_npe())));
                    }
                    Some(r.as_obj())
                };
                let target = self.resolve_target(*kind, &mref, receiver, f.class)?;
                let tcls = match &target {
                    Target::Bytecode { class, .. } => Some(self.class_desc_str(*class)),
                    _ => None,
                };
                if matches!(tcls.as_deref(), Some("m" | "a0" | "c" | "b" | "y2")) {
                    debug!("DBG inv {tcls:?} {}", self.str_of(mref.name));
                }
                if *kind == InvokeKind::Direct && self.str_of(mref.name) == "<init>" {
                    let a0 = f.regs[args.reg_at(0) as usize];
                    if let JValue::Obj(a0) = a0 {
                        let cdesc =
                            self.class_desc_str(self.object_class(JValue::Obj(a0)).unwrap_or(0));
                        debug!("DBG CTOR {cdesc} argc={}", args.count);
                        if cdesc == "c1" || cdesc == "g1" || cdesc == "f1" {
                            for ai in 1..7 {
                                if ai >= args.count as usize {
                                    break;
                                }
                                let av = f.regs[args.reg_at(ai as u8) as usize];
                                let s = match av {
                                    JValue::Obj(o) => self.class_desc_str(
                                        self.object_class(JValue::Obj(o)).unwrap_or(0),
                                    ),
                                    _ => format!("{av:?}"),
                                };
                                debug!("DBG   {cdesc} arg{ai}: {s}");
                            }
                        }
                    }
                }
                if *kind == InvokeKind::Static {
                    if let Target::Bytecode { class, .. } = &target {
                        self.ensure_class_initialized(*class)?;
                    }
                }
                Flow::Call(*kind, mref, target, *args, next_pc)
            }
            Insn::Unop(op, d, s) => {
                let v = f.regs[*s as usize];
                let r = match (*op, v) {
                    (Unop::NegInt, JValue::Int(x)) => JValue::Int(x.wrapping_neg()),
                    (Unop::NotInt, JValue::Int(x)) => JValue::Int(!x),
                    (Unop::NegLong, JValue::Long(x)) => JValue::Long(x.wrapping_neg()),
                    (Unop::NotLong, JValue::Long(x)) => JValue::Long(!x),
                    (Unop::NegFloat, JValue::Float(x)) => JValue::Float(-x),
                    (Unop::NegDouble, JValue::Double(x)) => JValue::Double(-x),
                    (Unop::IntToLong, JValue::Int(x)) => JValue::Long(i64::from(x)),
                    (Unop::IntToFloat, JValue::Int(x)) => JValue::Float(x as f32),
                    (Unop::IntToDouble, JValue::Int(x)) => JValue::Double(f64::from(x)),
                    (Unop::LongToInt, JValue::Long(x)) => JValue::Int(x as i32),
                    (Unop::LongToFloat, JValue::Long(x)) => JValue::Float(x as f32),
                    (Unop::LongToDouble, JValue::Long(x)) => JValue::Double(x as f64),
                    (Unop::FloatToInt, JValue::Float(x)) => JValue::Int(f2i(f64::from(x))),
                    (Unop::FloatToLong, JValue::Float(x)) => JValue::Long(f2l(f64::from(x))),
                    (Unop::FloatToDouble, JValue::Float(x)) => JValue::Double(f64::from(x)),
                    (Unop::DoubleToInt, JValue::Double(x)) => JValue::Int(f2i(x)),
                    (Unop::DoubleToLong, JValue::Double(x)) => JValue::Long(f2l(x)),
                    (Unop::DoubleToFloat, JValue::Double(x)) => JValue::Float(x as f32),
                    (Unop::IntToByte, JValue::Int(x)) => JValue::Int(x as i8 as i32),
                    (Unop::IntToChar, JValue::Int(x)) => JValue::Int(x as u16 as i32),
                    (Unop::IntToShort, JValue::Int(x)) => JValue::Int(x as i16 as i32),
                    _ => JValue::Int(0),
                };
                f.regs[*d as usize] = r;
                Flow::Next(0)
            }
            Insn::Binop(op, d, a, b) => {
                let (va, vb) = (f.regs[*a as usize], f.regs[*b as usize]);
                match binop(self, *op, va, vb) {
                    Ok(v) => {
                        f.regs[*d as usize] = v;
                        Flow::Next(0)
                    }
                    Err(ex) => return Ok(StepOutcome::Throw(JValue::Obj(ex))),
                }
            }
            Insn::BinopLit(op, d, a, lit) => {
                // rsub-int computes `lit - src`
                let (va, vb) = if *op == crate::dex::insn::LitOp::Rsub {
                    (JValue::Int(*lit), f.regs[*a as usize])
                } else {
                    (f.regs[*a as usize], JValue::Int(*lit))
                };
                match binop(self, op.to_binop(), va, vb) {
                    Ok(v) => {
                        f.regs[*d as usize] = v;
                        Flow::Next(0)
                    }
                    Err(ex) => return Ok(StepOutcome::Throw(JValue::Obj(ex))),
                }
            }
            Insn::Return(v) | Insn::ReturnWide(v) => {
                return Ok(StepOutcome::Ok(Flow::Ret(f.regs[*v as usize])));
            }
            Insn::ReturnVoid => return Ok(StepOutcome::Ok(Flow::Ret(JValue::Null))),
        };
        Ok(StepOutcome::Ok(match out {
            Flow::Next(_) => Flow::Next(next_pc),
            Flow::Call(kind, mref, target, args, _) => {
                Flow::Call(kind, mref, target, args, next_pc)
            }
            other => other,
        }))
    }

    fn fill_array(&mut self, obj: u32, data: &FillArray) -> Result<(), JvmError> {
        let values: Vec<JValue> = match data {
            FillArray::Byte(raw) => raw
                .iter()
                .map(|x| JValue::Int(i32::from(*x as i8)))
                .collect(),
            FillArray::Short(v) => v.iter().map(|x| JValue::Int(i32::from(*x))).collect(),
            FillArray::Char(v) => v.iter().map(|x| JValue::Int(i32::from(*x))).collect(),
            FillArray::Int(v) => v.iter().map(|x| JValue::Int(*x as i32)).collect(),
            FillArray::Wide(v) => v.iter().map(|x| JValue::Long(*x as i64)).collect(),
        };
        let len = match &self.arena.objects[obj as usize].native {
            Some(Native::Array(a)) => a.len(),
            _ => return Err(JvmError::Fatal("fill-array on non-array".into())),
        };
        if values.len() > len {
            return Err(JvmError::Fatal("fill-array too long".into()));
        }
        for (i, v) in values.iter().enumerate() {
            if let Some(Native::Array(a)) = self.arena.objects[obj as usize].native.as_mut() {
                a.set(i, *v);
            }
        }
        Ok(())
    }

    fn array_get(&mut self, f: &Frame, arr_reg: u8, idx_reg: u8) -> Result<JValue, JValue> {
        let arr = match f.regs[arr_reg as usize] {
            JValue::Obj(o) => o,
            _ => return Err(JValue::Obj(self.err_npe())),
        };
        let idx = f.regs[idx_reg as usize].as_int();
        let len = match &self.arena.objects[arr as usize].native {
            Some(Native::Array(a)) => a.len(),
            _ => return Err(JValue::Obj(self.err_npe())),
        };
        if idx < 0 || idx as usize >= len {
            return Err(JValue::Obj(self.err_aioobe(idx, len as i32)));
        }
        let v = match &self.arena.objects[arr as usize].native {
            Some(Native::Array(a)) => a.get(idx as usize),
            _ => return Err(JValue::Obj(self.err_npe())),
        };
        Ok(v)
    }

    fn array_put(
        &mut self,
        f: &Frame,
        src_reg: u8,
        arr_reg: u8,
        idx_reg: u8,
    ) -> Result<(), JValue> {
        let arr = match f.regs[arr_reg as usize] {
            JValue::Obj(o) => o,
            _ => return Err(JValue::Obj(self.err_npe())),
        };
        let idx = f.regs[idx_reg as usize].as_int();
        let v = f.regs[src_reg as usize];
        let len = match &self.arena.objects[arr as usize].native {
            Some(Native::Array(a)) => a.len(),
            _ => return Err(JValue::Obj(self.err_npe())),
        };
        if idx < 0 || idx as usize >= len {
            return Err(JValue::Obj(self.err_aioobe(idx, len as i32)));
        }
        if let Some(Native::Array(a)) = self.arena.objects[arr as usize].native.as_mut() {
            let v = if matches!(a, ArrayData::Obj(_)) && v.is_null_ref() {
                JValue::Null
            } else {
                v
            };
            a.set(idx as usize, v);
        }
        Ok(())
    }
}

fn is_wide_desc(d: &str) -> bool {
    matches!(d, "J" | "D")
}

fn fcmp<T: PartialOrd + PartialEq>(a: T, b: T, nan_is_gt: bool) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else if a == b {
        0
    } else if nan_is_gt {
        1
    } else {
        -1
    }
}

fn jval_eq(a: &JValue, b: &JValue) -> bool {
    match (a, b) {
        (JValue::Int(x), JValue::Int(y)) => x == y,
        (JValue::Long(x), JValue::Long(y)) => x == y,
        (JValue::Float(x), JValue::Float(y)) => x.to_bits() == y.to_bits(),
        (JValue::Double(x), JValue::Double(y)) => x.to_bits() == y.to_bits(),
        (JValue::Obj(x), JValue::Obj(y)) => x == y,
        (JValue::Null, JValue::Null) => true,
        _ => false,
    }
}

/// Java `f2i` / `d2i`: NaN saturates to 0 (JVM throws for out-of-range
/// int/long conversions; we clamp instead).
fn f2i(x: f64) -> i32 {
    if x.is_nan() {
        0
    } else if x >= 2147483648.0 {
        i32::MAX
    } else if x < -2147483648.0 {
        i32::MIN
    } else {
        x as i32
    }
}

fn f2l(x: f64) -> i64 {
    if x.is_nan() {
        0
    } else if x >= 9223372036854775808.0 {
        i64::MAX
    } else if x < -9223372036854775808.0 {
        i64::MIN
    } else {
        x as i64
    }
}

fn binop(vm: &mut Vm, op: Binop, a: JValue, b: JValue) -> Result<JValue, u32> {
    let div_zero = |vm: &mut Vm| vm.err_arithmetic("/ by zero");
    match (op, a, b) {
        // int
        (Binop::Add, JValue::Int(x), JValue::Int(y)) => Ok(JValue::Int(x.wrapping_add(y))),
        (Binop::Sub, JValue::Int(x), JValue::Int(y)) => Ok(JValue::Int(x.wrapping_sub(y))),
        (Binop::Mul, JValue::Int(x), JValue::Int(y)) => Ok(JValue::Int(x.wrapping_mul(y))),
        (Binop::Div, JValue::Int(x), JValue::Int(y)) => {
            if y == 0 {
                Err(div_zero(vm))
            } else {
                Ok(JValue::Int(x.wrapping_div(y)))
            }
        }
        (Binop::Rem, JValue::Int(x), JValue::Int(y)) => {
            if y == 0 {
                Err(div_zero(vm))
            } else {
                Ok(JValue::Int(x.wrapping_rem(y)))
            }
        }
        (Binop::And, JValue::Int(x), JValue::Int(y)) => Ok(JValue::Int(x & y)),
        (Binop::Or, JValue::Int(x), JValue::Int(y)) => Ok(JValue::Int(x | y)),
        (Binop::Xor, JValue::Int(x), JValue::Int(y)) => Ok(JValue::Int(x ^ y)),
        (Binop::Shl, JValue::Int(x), JValue::Int(y)) => {
            Ok(JValue::Int(x.wrapping_shl(y as u32 & 31)))
        }
        (Binop::Shr, JValue::Int(x), JValue::Int(y)) => {
            Ok(JValue::Int(x.wrapping_shr(y as u32 & 31)))
        }
        (Binop::Ushr, JValue::Int(x), JValue::Int(y)) => {
            Ok(JValue::Int((x as u32).wrapping_shr(y as u32 & 31) as i32))
        }
        // long
        (Binop::Add, JValue::Long(x), JValue::Long(y)) => Ok(JValue::Long(x.wrapping_add(y))),
        (Binop::Sub, JValue::Long(x), JValue::Long(y)) => Ok(JValue::Long(x.wrapping_sub(y))),
        (Binop::Mul, JValue::Long(x), JValue::Long(y)) => Ok(JValue::Long(x.wrapping_mul(y))),
        (Binop::Div, JValue::Long(x), JValue::Long(y)) => {
            if y == 0 {
                Err(div_zero(vm))
            } else {
                Ok(JValue::Long(x.wrapping_div(y)))
            }
        }
        (Binop::Rem, JValue::Long(x), JValue::Long(y)) => {
            if y == 0 {
                Err(div_zero(vm))
            } else {
                Ok(JValue::Long(x.wrapping_rem(y)))
            }
        }
        (Binop::And, JValue::Long(x), JValue::Long(y)) => Ok(JValue::Long(x & y)),
        (Binop::Or, JValue::Long(x), JValue::Long(y)) => Ok(JValue::Long(x | y)),
        (Binop::Xor, JValue::Long(x), JValue::Long(y)) => Ok(JValue::Long(x ^ y)),
        (Binop::Shl, JValue::Long(x), JValue::Long(y)) => {
            Ok(JValue::Long(x.wrapping_shl(y as u32 & 63)))
        }
        (Binop::Shr, JValue::Long(x), JValue::Long(y)) => {
            Ok(JValue::Long(x.wrapping_shr(y as u32 & 63)))
        }
        (Binop::Ushr, JValue::Long(x), JValue::Long(y)) => {
            Ok(JValue::Long((x as u64).wrapping_shr(y as u32 & 63) as i64))
        }
        // float
        (Binop::Add, JValue::Float(x), JValue::Float(y)) => Ok(JValue::Float(x + y)),
        (Binop::Sub, JValue::Float(x), JValue::Float(y)) => Ok(JValue::Float(x - y)),
        (Binop::Mul, JValue::Float(x), JValue::Float(y)) => Ok(JValue::Float(x * y)),
        (Binop::Div, JValue::Float(x), JValue::Float(y)) => Ok(JValue::Float(x / y)),
        (Binop::Rem, JValue::Float(x), JValue::Float(y)) => Ok(JValue::Float(x % y)),
        // double
        (Binop::Add, JValue::Double(x), JValue::Double(y)) => Ok(JValue::Double(x + y)),
        (Binop::Sub, JValue::Double(x), JValue::Double(y)) => Ok(JValue::Double(x - y)),
        (Binop::Mul, JValue::Double(x), JValue::Double(y)) => Ok(JValue::Double(x * y)),
        (Binop::Div, JValue::Double(x), JValue::Double(y)) => Ok(JValue::Double(x / y)),
        (Binop::Rem, JValue::Double(x), JValue::Double(y)) => Ok(JValue::Double(x % y)),
        _ => Err(vm.err_arithmetic("bad binop operands")),
    }
}
