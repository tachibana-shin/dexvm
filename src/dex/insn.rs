//! DEX instruction decoding.
//!
//! Decodes raw `u16` code units into a pre-decoded `Insn` form: literals are
//! sign/zero extended, register numbers extracted, branch targets and switch
//! payloads resolved to absolute code-unit positions. The interpreter then
//! only has to match on `Insn`, which keeps the hot loop small.
//!
//! Format reference: https://source.android.com/docs/core/runtime/dalvik-bytecode
//! Register field sizes: 4-bit A (12x, 11n, 22s, 22b, 22c, 22t, 35c, 10t)
//! vs 8-bit A (11x, 10t?, 21c, 21s, 21h, 21t, 31i, 31c, 31t, 22x, 23x, 32x, 3rc, 51l).

use std::sync::Arc;

use super::read::DexError;

/// Argument registers of an invoke (format 35c: up to 5 packed regs, /range: base).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Args {
    pub count: u8,
    pub regs: [u8; 5],
    pub range: bool,
    pub base: u16,
}

impl Args {
    pub fn reg_at(&self, i: u8) -> u16 {
        if self.range {
            u16::from(self.base) + u16::from(i)
        } else {
            u16::from(self.regs[i as usize])
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvokeKind {
    Virtual,
    Super,
    Direct,
    Static,
    Interface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayElem {
    Int,
    Wide,
    Obj,
    Bool,
    Byte,
    Char,
    Short,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    CmplFloat,
    CmpgFloat,
    CmplDouble,
    CmpgDouble,
    CmpLong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IfOp {
    Eq,
    Ne,
    Lt,
    Ge,
    Gt,
    Le,
    Ez,
    Nz,
    Ltz,
    Gez,
    Gtz,
    Lez,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unop {
    NegInt,
    NotInt,
    NegLong,
    NotLong,
    NegFloat,
    NegDouble,
    IntToLong,
    IntToFloat,
    IntToDouble,
    LongToInt,
    LongToFloat,
    LongToDouble,
    FloatToInt,
    FloatToLong,
    FloatToDouble,
    DoubleToInt,
    DoubleToLong,
    DoubleToFloat,
    IntToByte,
    IntToChar,
    IntToShort,
}

/// Binary operation; the operand widths (int/long/float/double) are determined
/// by the operand `JValue` variants at execution time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binop {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Ushr,
}

/// `rsub-int`: computed as `lit - src` rather than `src op lit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitOp {
    Bin(Binop),
    Rsub,
}

impl LitOp {
    /// The underlying binary operation (rsub maps to Sub with operands
    /// swapped, which the interpreter handles).
    pub fn to_binop(self) -> Binop {
        match self {
            LitOp::Bin(b) => b,
            LitOp::Rsub => Binop::Sub,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FillArray {
    Byte(Vec<u8>),
    Short(Vec<u16>),
    Char(Vec<u16>),
    Int(Vec<u32>),
    Wide(Vec<u64>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Insn {
    Nop,
    Move(u8, u16),
    MoveWide(u8, u16),
    MoveResult(u8),
    MoveResultWide(u8),
    MoveException(u8),
    ReturnVoid,
    Return(u8),
    ReturnWide(u8),
    Const4(u8, i8),
    Const16(u8, i16),
    Const(u8, i32),
    ConstHigh16(u8, i32),
    ConstWide16(u8, i64),
    ConstWide32(u8, i64),
    ConstWide(u8, i64),
    ConstWideHigh16(u8, i64),
    ConstString(u8, u32),
    ConstStringJumbo(u8, u32),
    ConstClass(u8, u32),
    ConstMethodHandle(u8, u32),
    ConstMethodType(u8, u32),
    MonitorEnter(u8),
    MonitorExit(u8),
    CheckCast(u8, u32),
    InstanceOf(u8, u8, u32),
    ArrayLength(u8, u8),
    NewInstance(u8, u32),
    NewArray(u8, u8, u32),
    FilledNewArray(Args, u32),
    FillArrayData(u8, u32, FillArray),
    Throw(u8),
    Goto(u32),
    PackedSwitch(u8, u32, i32, Vec<i32>),
    SparseSwitch(u8, u32, Vec<i32>, Vec<i32>),
    Cmp(CmpOp, u8, u8, u8),
    If(IfOp, u8, u8, u32),
    IfZ(IfOp, u8, u32),
    AGet(ArrayElem, u8, u8, u8),
    APut(ArrayElem, u8, u8, u8),
    IGet(u8, u8, u32),
    IGetWide(u8, u8, u32),
    IGetObj(u8, u8, u32),
    IPut(u8, u8, u32),
    IPutWide(u8, u8, u32),
    IPutObj(u8, u8, u32),
    SGet(u8, u32),
    SGetWide(u8, u32),
    SGetObj(u8, u32),
    SPut(u8, u32),
    SPutWide(u8, u32),
    SPutObj(u8, u32),
    Invoke(InvokeKind, u32, Args),
    Unop(Unop, u8, u8),
    Binop(Binop, u8, u8, u8),
    BinopLit(LitOp, u8, u8, i32),
}

/// Decode one instruction at `pc` (code-unit index) in `insns`.
/// Returns `(Insn, next_pc)`.
pub fn decode(insns: &[u16], pc: usize) -> Result<(Insn, usize), DexError> {
    let word = *insns
        .get(pc)
        .ok_or_else(|| DexError::new(pc * 2, "instruction past end of code"))?;
    let op = (word & 0xff) as u8;
    // 4-bit A for formats 12x/11n/22s/22b/22c/22t/35c/10t
    let a4 = ((word >> 8) & 0x0f) as u8;
    let b4 = ((word >> 12) & 0x0f) as u8;
    // 8-bit A for AA formats
    let a8 = ((word >> 8) & 0xff) as u8;
    // target helper: offset is relative to the instruction address, in code units
    let tgt = |off: i32| (pc as i64 + i64::from(off)) as u32;
    let w1 = || -> Result<u16, DexError> {
        insns
            .get(pc + 1)
            .copied()
            .ok_or_else(|| DexError::new((pc + 1) * 2, "short instruction"))
    };
    let w2 = || -> Result<u16, DexError> {
        insns
            .get(pc + 2)
            .copied()
            .ok_or_else(|| DexError::new((pc + 2) * 2, "short instruction"))
    };

    match op {
        0x00 => Ok((Insn::Nop, pc + 1)),
        0x01 => Ok((Insn::Move(a4, u16::from(b4)), pc + 1)),
        0x02 => Ok((Insn::Move(a8, w1()?), pc + 2)),
        0x03 => Ok((Insn::Move(w2()? as u8, w1()?), pc + 3)), // 32x: dst=AAAA, src=BBBB
        0x04 => Ok((Insn::MoveWide(a4, u16::from(b4)), pc + 1)),
        0x05 => Ok((Insn::MoveWide(a8, w1()?), pc + 2)),
        0x06 => Ok((Insn::MoveWide(w2()? as u8, w1()?), pc + 3)),
        0x07 => Ok((Insn::Move(a4, u16::from(b4)), pc + 1)),
        0x08 => Ok((Insn::Move(a8, w1()?), pc + 2)),
        0x09 => Ok((Insn::Move(w2()? as u8, w1()?), pc + 3)),
        0x0a => Ok((Insn::MoveResult(a8), pc + 1)),
        0x0b => Ok((Insn::MoveResultWide(a8), pc + 1)),
        0x0c => Ok((Insn::MoveResult(a8), pc + 1)),
        0x0d => Ok((Insn::MoveException(a8), pc + 1)),
        0x0e => Ok((Insn::ReturnVoid, pc + 1)),
        0x0f => Ok((Insn::Return(a8), pc + 1)),
        0x10 => Ok((Insn::ReturnWide(a8), pc + 1)),
        0x11 => Ok((Insn::Return(a8), pc + 1)),
        // const forms
        0x12 => Ok((Insn::Const4(a4, ((b4 << 4) as i8) >> 4), pc + 1)), // sign-extend 4 bits
        0x13 => Ok((Insn::Const16(a8, w1()? as i16), pc + 2)),
        0x14 => Ok((Insn::Const(a8, i32::from(w1()?) | (i32::from(w2()?) << 16)), pc + 3)),
        0x15 => Ok((Insn::ConstHigh16(a8, i32::from(w1()? as i16) << 16), pc + 2)),
        0x16 => Ok((Insn::ConstWide16(a8, i64::from(w1()? as i16)), pc + 2)),
        0x17 => Ok((Insn::ConstWide32(a8, i64::from(w1()?) | (i64::from(w2()?) << 16)), pc + 3)),
        0x18 => {
            let v = i64::from(w1()?)
                | (i64::from(w2()?) << 16)
                | (i64::from(w3(insns, pc)?) << 32)
                | (i64::from(w4(insns, pc)?) << 48);
            Ok((Insn::ConstWide(a8, v), pc + 5))
        }
        0x19 => Ok((Insn::ConstWideHigh16(a8, i64::from(w1()? as i16) << 48), pc + 2)),
        0x1a => Ok((Insn::ConstString(a8, u32::from(w1()?)), pc + 2)),
        0x1b => Ok((Insn::ConstStringJumbo(a8, u32::from(w1()?) | (u32::from(w2()?) << 16)), pc + 3)),
        0x1c => Ok((Insn::ConstClass(a8, u32::from(w1()?)), pc + 2)),
        0x1d => Ok((Insn::MonitorEnter(a8), pc + 1)),
        0x1e => Ok((Insn::MonitorExit(a8), pc + 1)),
        0x1f => Ok((Insn::CheckCast(a8, u32::from(w1()?)), pc + 2)),
        0x20 => Ok((Insn::InstanceOf(a4, b4, u32::from(w1()? & 0xff)), pc + 2)),
        0x21 => Ok((Insn::ArrayLength(a4, b4), pc + 1)),
        0x22 => Ok((Insn::NewInstance(a8, u32::from(w1()?)), pc + 2)),
        0x23 => Ok((Insn::NewArray(a4, b4, u32::from(w1()?)), pc + 2)),
        0x24 => {
            let (args, type_idx) = args_35c(word, w1()?, w2()?)?;
            Ok((Insn::FilledNewArray(args, type_idx), pc + 3))
        }
        0x25 => {
            let args = Args { count: a8, regs: [0; 5], range: true, base: w2()? };
            Ok((Insn::FilledNewArray(args, u32::from(w1()?)), pc + 3))
        }
         0x26 => {
            let off = i32::from(w1()?) | (i32::from(w2()?) << 16);
            let payload = tgt(off) as usize;
            let data = decode_fill_array_data(insns, payload)?;
            Ok((Insn::FillArrayData(a8, payload as u32, data), pc + 3))
        }
        0x27 => Ok((Insn::Throw(a8), pc + 1)),
        0x28 => Ok((Insn::Goto(tgt(((word >> 8) as u8) as i8 as i32)), pc + 1)),
        0x29 => Ok((Insn::Goto(tgt(i32::from(w1()? as i16))), pc + 2)),
        0x2a => {
            let off = i32::from(w1()?) | (i32::from(w2()?) << 16);
            Ok((Insn::Goto(tgt(off)), pc + 3))
        }
         0x2b => {
            let off = i32::from(w1()?) | (i32::from(w2()?) << 16);
            let payload = tgt(off) as usize;
            if insns.get(payload).copied() != Some(0x0100) {
                return Err(DexError::new(payload * 2, "bad packed-switch payload"));
            }
            let size = *insns.get(payload + 1).ok_or_else(|| DexError::new(payload * 2, "switch oob"))? as usize;
            let first_key = i32::from(*insns.get(payload + 2).ok_or_else(|| DexError::new(payload * 2, "switch oob"))?)
                | (i32::from(*insns.get(payload + 3).ok_or_else(|| DexError::new(payload * 2, "switch oob"))?) << 16);
            let mut targets = Vec::with_capacity(size);
            for i in 0..size {
                let t = i32::from(*insns.get(payload + 4 + i * 2).ok_or_else(|| DexError::new(payload * 2, "switch oob"))?)
                    | (i32::from(*insns.get(payload + 5 + i * 2).ok_or_else(|| DexError::new(payload * 2, "switch oob"))?) << 16);
                targets.push(tgt(t) as i32);
            }
            Ok((Insn::PackedSwitch(a8, payload as u32, first_key, targets), pc + 3))
        }
        0x2c => {
            let off = i32::from(w1()?) | (i32::from(w2()?) << 16);
            let payload = tgt(off) as usize;
            if insns.get(payload).copied() != Some(0x0200) {
                return Err(DexError::new(payload * 2, "bad sparse-switch payload"));
            }
            let size = *insns.get(payload + 1).ok_or_else(|| DexError::new(payload * 2, "switch oob"))? as usize;
            let mut keys = Vec::with_capacity(size);
            let mut targets = Vec::with_capacity(size);
            for i in 0..size {
                let k = i32::from(*insns.get(payload + 2 + i * 2).ok_or_else(|| DexError::new(payload * 2, "switch oob"))?)
                    | (i32::from(*insns.get(payload + 3 + i * 2).ok_or_else(|| DexError::new(payload * 2, "switch oob"))?) << 16);
                keys.push(k);
                let t = i32::from(*insns.get(payload + 2 + (size + i) * 2).ok_or_else(|| DexError::new(payload * 2, "switch oob"))?)
                    | (i32::from(*insns.get(payload + 3 + (size + i) * 2).ok_or_else(|| DexError::new(payload * 2, "switch oob"))?) << 16);
                targets.push(tgt(t) as i32);
            }
            Ok((Insn::SparseSwitch(a8, payload as u32, keys, targets), pc + 3))
        }
        0x2d | 0x2e | 0x2f | 0x30 | 0x31 => {
            let cmp = match op {
                0x2d => CmpOp::CmplFloat,
                0x2e => CmpOp::CmpgFloat,
                0x2f => CmpOp::CmplDouble,
                0x30 => CmpOp::CmpgDouble,
                _ => CmpOp::CmpLong,
            };
            Ok((Insn::Cmp(cmp, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2))
        }
        0x32 | 0x33 | 0x34 | 0x35 | 0x36 | 0x37 => {
            let ifop = match op {
                0x32 => IfOp::Eq,
                0x33 => IfOp::Ne,
                0x34 => IfOp::Lt,
                0x35 => IfOp::Ge,
                0x36 => IfOp::Gt,
                _ => IfOp::Le,
            };
            Ok((Insn::If(ifop, a4, b4, tgt(i32::from(w1()? as i16))), pc + 2))
        }
        0x38..=0x3d => {
            let ifop = match op {
                0x38 => IfOp::Ez,
                0x39 => IfOp::Nz,
                0x3a => IfOp::Ltz,
                0x3b => IfOp::Gez,
                0x3c => IfOp::Gtz,
                _ => IfOp::Lez,
            };
            Ok((Insn::IfZ(ifop, a8, tgt(i32::from(w1()? as i16))), pc + 2))
        }
        // aget/aput (23x: AA|op CC|BB)
        0x44 => Ok((Insn::AGet(ArrayElem::Int, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x45 => Ok((Insn::AGet(ArrayElem::Wide, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x46 => Ok((Insn::AGet(ArrayElem::Obj, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x47 => Ok((Insn::AGet(ArrayElem::Bool, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x48 => Ok((Insn::AGet(ArrayElem::Byte, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x49 => Ok((Insn::AGet(ArrayElem::Char, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x4a => Ok((Insn::AGet(ArrayElem::Short, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x4b => Ok((Insn::APut(ArrayElem::Int, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x4c => Ok((Insn::APut(ArrayElem::Wide, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x4d => Ok((Insn::APut(ArrayElem::Obj, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x4e => Ok((Insn::APut(ArrayElem::Bool, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x4f => Ok((Insn::APut(ArrayElem::Byte, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x50 => Ok((Insn::APut(ArrayElem::Char, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        0x51 => Ok((Insn::APut(ArrayElem::Short, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2)),
        // iget/iput (22c: A|B 4-bit each)
        0x52 => Ok((Insn::IGet(a4, b4, u32::from(w1()?)), pc + 2)),
        0x53 => Ok((Insn::IGetWide(a4, b4, u32::from(w1()?)), pc + 2)),
        0x54 => Ok((Insn::IGetObj(a4, b4, u32::from(w1()?)), pc + 2)),
        0x55 | 0x56 | 0x57 | 0x58 => Ok((Insn::IGet(a4, b4, u32::from(w1()?)), pc + 2)),
        0x59 => Ok((Insn::IPut(a4, b4, u32::from(w1()?)), pc + 2)),
        0x5a => Ok((Insn::IPutWide(a4, b4, u32::from(w1()?)), pc + 2)),
        0x5b => Ok((Insn::IPutObj(a4, b4, u32::from(w1()?)), pc + 2)),
        0x5c | 0x5d | 0x5e | 0x5f => Ok((Insn::IPut(a4, b4, u32::from(w1()?)), pc + 2)),
        // sget/sput (21c: AA 8-bit)
        0x60 => Ok((Insn::SGet(a8, u32::from(w1()?)), pc + 2)),
        0x61 => Ok((Insn::SGetWide(a8, u32::from(w1()?)), pc + 2)),
        0x62 => Ok((Insn::SGetObj(a8, u32::from(w1()?)), pc + 2)),
        0x63 | 0x64 | 0x65 | 0x66 => Ok((Insn::SGet(a8, u32::from(w1()?)), pc + 2)),
        0x67 => Ok((Insn::SPut(a8, u32::from(w1()?)), pc + 2)),
        0x68 => Ok((Insn::SPutWide(a8, u32::from(w1()?)), pc + 2)),
        0x69 => Ok((Insn::SPutObj(a8, u32::from(w1()?)), pc + 2)),
        0x6a | 0x6b | 0x6c | 0x6d => Ok((Insn::SPut(a8, u32::from(w1()?)), pc + 2)),
        // invoke (35c / 3rc)
        0x6e..=0x72 => {
            let (method_idx, args) = args_35c(word, w1()?, w2()?)?;
            let kind = match op {
                0x6e => InvokeKind::Virtual,
                0x6f => InvokeKind::Super,
                0x70 => InvokeKind::Direct,
                0x71 => InvokeKind::Static,
                _ => InvokeKind::Interface,
            };
            Ok((Insn::Invoke(kind, args, method_idx), pc + 3))
        }
        0x74..=0x78 => {
            let args = Args { count: a8, regs: [0; 5], range: true, base: w2()? };
            let kind = match op {
                0x74 => InvokeKind::Virtual,
                0x75 => InvokeKind::Super,
                0x76 => InvokeKind::Direct,
                0x77 => InvokeKind::Static,
                _ => InvokeKind::Interface,
            };
            Ok((Insn::Invoke(kind, u32::from(w1()?), args), pc + 3))
        }
        // unops (12x)
        0x80 => Ok((Insn::Unop(Unop::NegInt, a4, b4), pc + 1)),
        0x81 => Ok((Insn::Unop(Unop::NotInt, a4, b4), pc + 1)),
        0x82 => Ok((Insn::Unop(Unop::NegLong, a4, b4), pc + 1)),
        0x83 => Ok((Insn::Unop(Unop::NotLong, a4, b4), pc + 1)),
        0x84 => Ok((Insn::Unop(Unop::NegFloat, a4, b4), pc + 1)),
        0x85 => Ok((Insn::Unop(Unop::NegDouble, a4, b4), pc + 1)),
        0x86 => Ok((Insn::Unop(Unop::IntToLong, a4, b4), pc + 1)),
        0x87 => Ok((Insn::Unop(Unop::IntToFloat, a4, b4), pc + 1)),
        0x88 => Ok((Insn::Unop(Unop::IntToDouble, a4, b4), pc + 1)),
        0x89 => Ok((Insn::Unop(Unop::LongToInt, a4, b4), pc + 1)),
        0x8a => Ok((Insn::Unop(Unop::LongToFloat, a4, b4), pc + 1)),
        0x8b => Ok((Insn::Unop(Unop::LongToDouble, a4, b4), pc + 1)),
        0x8c => Ok((Insn::Unop(Unop::FloatToInt, a4, b4), pc + 1)),
        0x8d => Ok((Insn::Unop(Unop::FloatToLong, a4, b4), pc + 1)),
        0x8e => Ok((Insn::Unop(Unop::FloatToDouble, a4, b4), pc + 1)),
        0x8f => Ok((Insn::Unop(Unop::DoubleToInt, a4, b4), pc + 1)),
        0x90 => Ok((Insn::Unop(Unop::DoubleToLong, a4, b4), pc + 1)),
        0x91 => Ok((Insn::Unop(Unop::DoubleToFloat, a4, b4), pc + 1)),
        0x92 => Ok((Insn::Unop(Unop::IntToByte, a4, b4), pc + 1)),
        0x93 => Ok((Insn::Unop(Unop::IntToChar, a4, b4), pc + 1)),
        0x94 => Ok((Insn::Unop(Unop::IntToShort, a4, b4), pc + 1)),
        // binops (23x)
        0xa0..=0xbf => {
            let (binop, _wide) = binop_of(op);
            Ok((Insn::Binop(binop, a8, (w1()? & 0xff) as u8, (w1()? >> 8) as u8), pc + 2))
        }
        // 2addr binops (12x)
        0xc0..=0xdf => {
            let (binop, _wide) = binop_of(op - 0x20);
            Ok((Insn::Binop(binop, a4, a4, b4), pc + 1))
        }
        // lit16 (22s)
        0xe0..=0xe7 => {
            let litop = match op {
                0xe0 => LitOp::Bin(Binop::Add),
                0xe1 => LitOp::Rsub,
                0xe2 => LitOp::Bin(Binop::Mul),
                0xe3 => LitOp::Bin(Binop::Div),
                0xe4 => LitOp::Bin(Binop::Rem),
                0xe5 => LitOp::Bin(Binop::And),
                0xe6 => LitOp::Bin(Binop::Or),
                _ => LitOp::Bin(Binop::Xor),
            };
            Ok((Insn::BinopLit(litop, a4, b4, i32::from(w1()? as i16)), pc + 2))
        }
        // lit8 (22b)
        0xe8..=0xf2 => {
            let litop = match op {
                0xe8 => LitOp::Bin(Binop::Add),
                0xe9 => LitOp::Rsub,
                0xea => LitOp::Bin(Binop::Mul),
                0xeb => LitOp::Bin(Binop::Div),
                0xec => LitOp::Bin(Binop::Rem),
                0xed => LitOp::Bin(Binop::And),
                0xee => LitOp::Bin(Binop::Or),
                0xef => LitOp::Bin(Binop::Xor),
                0xf0 => LitOp::Bin(Binop::Shl),
                0xf1 => LitOp::Bin(Binop::Shr),
                _ => LitOp::Bin(Binop::Ushr),
            };
            let lit = i32::from(w1()? >> 8) as i8 as i32;
            Ok((Insn::BinopLit(litop, a4, (w1()? & 0xff) as u8, lit), pc + 2))
        }
        0xfb | 0xfc => Err(DexError::new(pc * 2, "invoke-polymorphic not supported")),
        0xfd | 0xfe => Err(DexError::new(pc * 2, "invoke-custom not supported")),
        0xff => Ok((Insn::ConstMethodHandle(a8, u32::from(w1()?)), pc + 2)),
        other => Err(DexError::new(pc * 2, format!("unknown opcode {other:#04x}"))),
    }
}


fn w3(insns: &[u16], pc: usize) -> Result<u16, DexError> {
    insns
        .get(pc + 3)
        .copied()
        .ok_or_else(|| DexError::new((pc + 3) * 2, "short instruction"))
}
fn w4(insns: &[u16], pc: usize) -> Result<u16, DexError> {
    insns
        .get(pc + 4)
        .copied()
        .ok_or_else(|| DexError::new((pc + 4) * 2, "short instruction"))
}

/// Decode format 35c (used by invoke-* and filled-new-array).
/// word0: A|G|op -> A=count (bits 15-12), G=5th reg (bits 11-8)
/// word1: BBBB = full 16-bit reference index
/// word2: F|E|D|C -> C=1st reg (bits 0-3), D=2nd (4-7), E=3rd (8-11), F=4th (12-15)
fn args_35c(word: u16, w1: u16, w2: u16) -> Result<(Args, u32), DexError> {
    let count = ((word >> 12) & 0x0f) as u8;
    let regs = [
        (w2 & 0x0f) as u8,           // C: arg 0
        ((w2 >> 4) & 0x0f) as u8,    // D: arg 1
        ((w2 >> 8) & 0x0f) as u8,    // E: arg 2
        ((w2 >> 12) & 0x0f) as u8,   // F: arg 3
        ((word >> 8) & 0x0f) as u8,  // G: arg 4
    ];
    Ok((Args { count, regs, range: false, base: 0 }, u32::from(w1)))
}

fn binop_of(op: u8) -> (Binop, bool) {
    // op in [0xa0,0xbf] or [0xc0,0xdf]; the "wide" flag distinguishes
    // long/double operand widths which execution derives from the value itself.
    let (idx, wide) = if op < 0xc0 { (op, false) } else { (op - 0x20, true) };
    let b = match idx {
        0xa0 | 0xab | 0xb6 | 0xbb => Binop::Add,
        0xa1 | 0xac | 0xb7 | 0xbc => Binop::Sub,
        0xa2 | 0xad | 0xb8 | 0xbd => Binop::Mul,
        0xa3 | 0xae | 0xb9 | 0xbe => Binop::Div,
        0xa4 | 0xaf | 0xba | 0xbf => Binop::Rem,
        0xa5 | 0xb0 => Binop::And,
        0xa6 | 0xb1 => Binop::Or,
        0xa7 | 0xb2 => Binop::Xor,
        0xa8 | 0xb3 => Binop::Shl,
        0xa9 | 0xb4 => Binop::Shr,
        _ => Binop::Ushr,
    };
    (b, wide)
}

fn decode_fill_array_data(insns: &[u16], payload: usize) -> Result<FillArray, DexError> {
    if insns.get(payload).copied() != Some(0x0300) {
        return Err(DexError::new(payload * 2, "bad fill-array-data payload"));
    }
    let width = *insns.get(payload + 1).ok_or_else(|| DexError::new(payload * 2, "fill-array oob"))? as usize;
    let size = (u32::from(*insns.get(payload + 2).ok_or_else(|| DexError::new(payload * 2, "fill-array oob"))?)
        | (u32::from(*insns.get(payload + 3).ok_or_else(|| DexError::new(payload * 2, "fill-array oob"))?) << 16)) as usize;
    let bytes = size * width;
    let start = payload + 4;
    let mut raw = Vec::with_capacity(bytes);
    for i in 0..bytes {
        let w = insns.get(start + i / 2).ok_or_else(|| DexError::new((start + i / 2) * 2, "fill-array oob"))?;
        let b = if i % 2 == 0 { (w & 0xff) as u8 } else { (w >> 8) as u8 };
        raw.push(b);
    }
    Ok(match width {
        1 => FillArray::Byte(raw),
        2 => {
            let mut v = Vec::with_capacity(size);
            for i in 0..size {
                v.push(u16::from_le_bytes([raw[i * 2], raw[i * 2 + 1]]));
            }
            FillArray::Short(v)
        }
        4 => {
            let mut v = Vec::with_capacity(size);
            for i in 0..size {
                v.push(u32::from_le_bytes([raw[i * 4], raw[i * 4 + 1], raw[i * 4 + 2], raw[i * 4 + 3]]));
            }
            FillArray::Int(v)
        }
        8 => {
            let mut v = Vec::with_capacity(size);
            for i in 0..size {
                v.push(u64::from_le_bytes([
                    raw[i * 8], raw[i * 8 + 1], raw[i * 8 + 2], raw[i * 8 + 3],
                    raw[i * 8 + 4], raw[i * 8 + 5], raw[i * 8 + 6], raw[i * 8 + 7],
                ]));
            }
            FillArray::Wide(v)
        }
        other => return Err(DexError::new(payload * 2, format!("bad fill-array-data width {other}"))),
    })
}

/// Code-unit span of an inline data payload (switch table or array data),
/// if this instruction owns one. The payload may sit anywhere in the code
/// stream, so decode_all must skip over it when the linear scan reaches it.
fn payload_span(insn: &Insn) -> Option<(u32, u32)> {
    match insn {
        Insn::PackedSwitch(_, payload, _, targets) => {
            Some((*payload, *payload + 4 + 2 * targets.len() as u32))
        }
        Insn::SparseSwitch(_, payload, keys, _) => {
            Some((*payload, *payload + 2 + 4 * keys.len() as u32))
        }
        Insn::FillArrayData(_, payload, data) => {
            let words = match data {
                FillArray::Byte(v) => (v.len() as u32 + 1) / 2,
                FillArray::Short(v) | FillArray::Char(v) => v.len() as u32,
                FillArray::Int(v) => 2 * v.len() as u32,
                FillArray::Wide(v) => 4 * v.len() as u32,
            };
            Some((*payload, *payload + 4 + words))
        }
        _ => None,
    }
}

/// Fully decoded code: instructions plus their code-unit offsets (needed to
/// map try/catch ranges, which address code units, onto instruction indices).
#[derive(Debug, Clone)]
pub struct Decoded {
    pub insns: Arc<[Insn]>,
    pub units: Arc<[u32]>,
    /// Length in code units of each instruction (same order as `insns`).
    pub sizes: Arc<[u32]>,
    /// Total number of code units.
    pub words: u32,
}

/// Decode an entire code item into `Insn`s, recording code-unit offsets.
pub fn decode_all(insns: &[u16]) -> Result<Decoded, DexError> {
    let mut v = Vec::new();
    let mut u = Vec::new();
    let mut s = Vec::new();
    let mut skips: Vec<(u32, u32)> = Vec::new();
    let mut pc = 0usize;
    while pc < insns.len() {
        if let Some(&(_, end)) = skips.iter().find(|(start, _)| *start as usize == pc) {
            pc = end as usize;
            continue;
        }
        let (insn, next) = decode(insns, pc)?;
        if let Some(span) = payload_span(&insn) {
            skips.push(span);
        }
        u.push(pc as u32);
        s.push((next - pc) as u32);
        pc = next;
        v.push(insn);
    }
    Ok(Decoded {
        insns: Arc::from(v),
        units: Arc::from(u),
        sizes: Arc::from(s),
        words: insns.len() as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(insns: &[u16], pc: usize) -> Insn {
        decode(insns, pc).expect("decode").0
    }

    #[test]
    fn const_forms() {
        assert_eq!(dec(&[0x12 | (0 << 8) | (0xf << 12)], 0), Insn::Const4(0, -1));
        assert_eq!(dec(&[0x12 | (2 << 8) | (3 << 12)], 0), Insn::Const4(2, 3));
        assert_eq!(dec(&[0x13 | (3 << 8), 0x1234], 0), Insn::Const16(3, 0x1234));
        assert_eq!(dec(&[0x14 | (1 << 8), 0x5678, 0x1234], 0), Insn::Const(1, 0x12345678));
        assert_eq!(dec(&[0x15 | (4 << 8), 0x1234], 0), Insn::ConstHigh16(4, 0x12340000));
        assert_eq!(
            dec(&[0x18, 0xdef0, 0x9abc, 0x5678, 0x1234], 0),
            Insn::ConstWide(0, 0x123456789abcdef0)
        );
        // const-wide/16 v9, -2
        assert_eq!(dec(&[0x16 | (9 << 8), 0xfffe], 0), Insn::ConstWide16(9, -2));
    }

    #[test]
    fn branches() {
        assert_eq!(dec(&[0x28 | (3 << 8)], 0), Insn::Goto(3));
        assert_eq!(dec(&[0x28 | (0xfd << 8)], 0), Insn::Goto((-3i32) as u32));
        assert_eq!(dec(&[0, 0, 0x29, 0xfff6], 2), Insn::Goto(2 + (-10i32) as u32));
        assert_eq!(dec(&[0, 0, 0, 0, 0x38 | (1 << 8), 0x7f], 4), Insn::IfZ(IfOp::Ez, 1, 4 + 0x7f));
        assert_eq!(
            dec(&[0, 0, 0, 0, 0, 0, 0, 0, 0x33 | (0 << 8) | (1 << 12), 0xfffc], 8),
            Insn::If(IfOp::Ne, 0, 1, 8u32.wrapping_add((-4i32) as u32))
        );
    }

    #[test]
    fn invoke_forms() {
        // invoke-static {v1, v2}, method#0x1234
        // word0: A=2 (count), G=0; word1: BBBB=0x1234; word2: C=1, D=2, E=0, F=0
        let insns = [0x71 | (2 << 12), 0x1234, 1 | (2 << 4)];
        assert_eq!(
            dec(&insns, 0),
            Insn::Invoke(
                InvokeKind::Static,
                0x1234,
                Args { count: 2, regs: [1, 2, 0, 0, 0], range: false, base: 0 }
            )
        );
        // invoke-virtual/range {v5..v8}, method#1
        assert_eq!(
            dec(&[0x74 | (4 << 8), 1, 5], 0),
            Insn::Invoke(InvokeKind::Virtual, 1, Args { count: 4, regs: [0; 5], range: true, base: 5 })
        );
        // invoke-interface with 5 args: regs C=1,D=2,E=3,F=4,G=5
        // word0: A=5, G=5; word1: BBBB=0; word2: F=4,E=3,D=2,C=1
        let insns = [0x72 | (5 << 12) | (5 << 8), 0, (4 << 12) | (3 << 8) | (2 << 4) | 1];
        assert_eq!(
            dec(&insns, 0),
            Insn::Invoke(
                InvokeKind::Interface,
                0,
                Args { count: 5, regs: [1, 2, 3, 4, 5], range: false, base: 0 }
            )
        );
        // real akuma bytes: invoke-direct {v1, v0}, Lk;-><init>(I)V (method#138)
        let insns = [0x70 | (2 << 12), 0x8a, 1];
        assert_eq!(
            dec(&insns, 0),
            Insn::Invoke(
                InvokeKind::Direct,
                138,
                Args { count: 2, regs: [1, 0, 0, 0, 0], range: false, base: 0 }
            )
        );
    }

    #[test]
    fn field_ops() {
        assert_eq!(dec(&[0x52 | (0 << 8) | (1 << 12), 0x2a], 0), Insn::IGet(0, 1, 0x2a));
        assert_eq!(dec(&[0x62 | (2 << 8), 0x100], 0), Insn::SGetObj(2, 0x100));
        assert_eq!(dec(&[0x5a | (0 << 8) | (1 << 12), 7], 0), Insn::IPutWide(0, 1, 7));
        assert_eq!(dec(&[0x61 | (3 << 8), 0x4], 0), Insn::SGetWide(3, 4));
    }

    #[test]
    fn arrays() {
        assert_eq!(dec(&[0x23 | (0 << 8) | (1 << 12), 0x4], 0), Insn::NewArray(0, 1, 4));
        assert_eq!(dec(&[0x44 | (0 << 8), (1 & 0xff) | (2 << 8)], 0), Insn::AGet(ArrayElem::Int, 0, 1, 2));
        assert_eq!(dec(&[0x4d | (0 << 8), (1 & 0xff) | (2 << 8)], 0), Insn::APut(ArrayElem::Obj, 0, 1, 2));
    }

    #[test]
    fn arithmetic() {
        assert_eq!(dec(&[0xa0 | (0 << 8), (1 & 0xff) | (2 << 8)], 0), Insn::Binop(Binop::Add, 0, 1, 2));
        assert_eq!(dec(&[0xcc | (3 << 8) | (4 << 12)], 0), Insn::Binop(Binop::Sub, 3, 3, 4));
        assert_eq!(dec(&[0xe0 | (0 << 8) | (1 << 12), 0xffff], 0), Insn::BinopLit(LitOp::Bin(Binop::Add), 0, 1, -1));
        assert_eq!(dec(&[0xe1 | (0 << 8) | (1 << 12), 5], 0), Insn::BinopLit(LitOp::Rsub, 0, 1, 5));
        assert_eq!(dec(&[0xf0 | (0 << 8), (1 & 0xf) | (2 << 8)], 0), Insn::BinopLit(LitOp::Bin(Binop::Shl), 0, 1, 2));
        assert_eq!(dec(&[0x80 | (0 << 8) | (1 << 12)], 0), Insn::Unop(Unop::NegInt, 0, 1));
        assert_eq!(dec(&[0x86 | (0 << 8) | (1 << 12)], 0), Insn::Unop(Unop::IntToLong, 0, 1));
        let r = decode(&[0x9f, 0x12], 0);
        assert!(r.is_err());
    }
}
