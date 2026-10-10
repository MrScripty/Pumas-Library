//! Strict bounded header/metadata parsing for the explicit vision import class.
//! Does not validate tensor arithmetic, native compatibility or inference.
use super::*;
use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom};

const METADATA_BUDGET: u64 = 16 * 1024 * 1024;
struct Reader<'a> {
    file: &'a mut std::fs::File,
    remaining: u64,
}
fn invalid() -> PumasError {
    PumasError::Validation {
        field: "import.acquired_vision.gguf".into(),
        message: "GGUF vision header/metadata is malformed, duplicate, truncated or exceeds its bounded parser".into(),
    }
}
impl Reader<'_> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        if self.remaining < N as u64 {
            return Err(invalid());
        }
        self.remaining -= N as u64;
        let mut b = [0; N];
        self.file.read_exact(&mut b).map_err(|_| invalid())?;
        Ok(b)
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes()?))
    }
    fn string(&mut self, limit: u64) -> Result<String> {
        let length = self.u64()?;
        if length > limit || length > self.remaining {
            return Err(invalid());
        }
        self.remaining -= length;
        let mut b = vec![0; length as usize];
        self.file.read_exact(&mut b).map_err(|_| invalid())?;
        String::from_utf8(b).map_err(|_| invalid())
    }
    fn value(&mut self, kind: u32, array: bool) -> Result<()> {
        match kind {
            0 | 1 => {
                self.bytes::<1>()?;
            }
            2 | 3 => {
                self.bytes::<2>()?;
            }
            4..=6 => {
                self.bytes::<4>()?;
            }
            7 => {
                if self.bytes::<1>()?[0] > 1 {
                    return Err(invalid());
                }
            }
            8 => {
                self.string(1024 * 1024)?;
            }
            9 if !array => {
                let element = self.u32()?;
                let count = self.u64()?;
                if element == 9 || element > 12 || count > 1_000_000 {
                    return Err(invalid());
                }
                for _ in 0..count {
                    self.value(element, true)?;
                }
            }
            10..=12 => {
                self.bytes::<8>()?;
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }
}

pub(super) fn architecture(file: &mut std::fs::File) -> Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let result = (|| {
        let mut r = Reader {
            file,
            remaining: METADATA_BUDGET,
        };
        if r.bytes::<4>()? != *b"GGUF" || r.u32()? != 3 {
            return Err(invalid());
        }
        let tensors = r.u64()?;
        let count = r.u64()?;
        if tensors > 1_000_000 || count == 0 || count > 4096 {
            return Err(invalid());
        }
        let mut keys = HashSet::new();
        let mut architecture = None;
        for _ in 0..count {
            let key = r.string(4096)?;
            if key.is_empty() || !keys.insert(key.clone()) {
                return Err(invalid());
            }
            let kind = r.u32()?;
            if key == "general.architecture" {
                if kind != 8 {
                    return Err(invalid());
                }
                let value = r.string(256)?;
                if value.is_empty() {
                    return Err(invalid());
                }
                architecture = Some(value);
            } else {
                r.value(kind, false)?;
            }
        }
        architecture.ok_or_else(invalid)
    })();
    file.seek(SeekFrom::Start(0))?;
    result
}
