//! Low-level readers for the DEX binary format.

use std::fmt;

#[derive(Debug, Clone)]
pub struct DexError {
    pub msg: String,
    pub off: usize,
}

impl DexError {
    pub fn new(off: usize, msg: impl Into<String>) -> Self {
        DexError {
            msg: msg.into(),
            off,
        }
    }
}

impl fmt::Display for DexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dex error at 0x{:x}: {}", self.off, self.msg)
    }
}

impl std::error::Error for DexError {}

/// Bounds-checked cursor over the raw DEX bytes.
pub struct Cursor<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Cursor { data, pos: 0 }
    }

    pub fn at(&self, pos: usize) -> DexError {
        DexError::new(pos, "out of bounds read")
    }

    pub fn seek(&mut self, pos: usize) -> Result<(), DexError> {
        if pos > self.data.len() {
            return Err(self.at(pos));
        }
        self.pos = pos;
        Ok(())
    }

    pub fn u8(&mut self) -> Result<u8, DexError> {
        if self.pos >= self.data.len() {
            return Err(self.at(self.pos));
        }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    pub fn u16(&mut self) -> Result<u16, DexError> {
        if self.pos + 2 > self.data.len() {
            return Err(self.at(self.pos));
        }
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    pub fn u32(&mut self) -> Result<u32, DexError> {
        if self.pos + 4 > self.data.len() {
            return Err(self.at(self.pos));
        }
        let v = u32::from_le_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    pub fn i32(&mut self) -> Result<i32, DexError> {
        Ok(self.u32()? as i32)
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], DexError> {
        if self.pos + n > self.data.len() {
            return Err(self.at(self.pos));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    pub fn byte_at(&self, pos: usize) -> Result<u8, DexError> {
        self.data
            .get(pos)
            .copied()
            .ok_or_else(|| DexError::new(pos, "out of bounds read"))
    }

    pub fn u16_at(&self, pos: usize) -> Result<u16, DexError> {
        let d = self
            .data
            .get(pos..pos + 2)
            .ok_or_else(|| DexError::new(pos, "out of bounds read"))?;
        Ok(u16::from_le_bytes([d[0], d[1]]))
    }

    pub fn u32_at(&self, pos: usize) -> Result<u32, DexError> {
        let d = self
            .data
            .get(pos..pos + 4)
            .ok_or_else(|| DexError::new(pos, "out of bounds read"))?;
        Ok(u32::from_le_bytes([d[0], d[1], d[2], d[3]]))
    }

    pub fn slice_at(&self, pos: usize, n: usize) -> Result<&'a [u8], DexError> {
        self.data
            .get(pos..pos + n)
            .ok_or_else(|| DexError::new(pos, "out of bounds read"))
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// Unsigned LEB128.
    pub fn uleb128(&mut self) -> Result<u32, DexError> {
        let mut result: u32 = 0;
        let mut shift = 0;
        loop {
            let b = self.u8()?;
            result |= u32::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift >= 35 {
                return Err(DexError::new(self.pos - 1, "uleb128 overflow"));
            }
        }
        Ok(result)
    }

    /// Signed LEB128.
    pub fn sleb128(&mut self) -> Result<i32, DexError> {
        let mut result: i32 = 0;
        let mut shift = 0;
        loop {
            let b = self.u8()?;
            result |= i32::from(b & 0x7f) << shift;
            shift += 7;
            if b & 0x80 == 0 {
                if shift < 32 && b & 0x40 != 0 {
                    result |= -1i32 << shift;
                }
                break;
            }
            if shift >= 35 {
                return Err(DexError::new(self.pos - 1, "sleb128 overflow"));
            }
        }
        Ok(result)
    }
}

/// Decode a MUTF-8 string (DEX variant: NUL is encoded as 0xC0 0x80).
pub fn decode_mutf8(bytes: &[u8]) -> Result<String, DexError> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0 {
            break;
        }
        if b == 0xc0 && i + 1 < bytes.len() && bytes[i + 1] == 0x80 {
            out.push(0);
            i += 2;
            continue;
        }
        if b < 0x80 {
            out.push(b);
            i += 1;
        } else if b < 0xe0 {
            if i + 1 >= bytes.len() {
                return Err(DexError::new(i, "truncated utf8"));
            }
            out.push(b);
            out.push(bytes[i + 1]);
            i += 2;
        } else if b < 0xf0 {
            if i + 2 >= bytes.len() {
                return Err(DexError::new(i, "truncated utf8"));
            }
            out.push(b);
            out.push(bytes[i + 1]);
            out.push(bytes[i + 2]);
            i += 3;
        } else {
            if i + 3 >= bytes.len() {
                return Err(DexError::new(i, "truncated utf8"));
            }
            out.push(b);
            out.push(bytes[i + 1]);
            out.push(bytes[i + 2]);
            out.push(bytes[i + 3]);
            i += 4;
        }
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}
