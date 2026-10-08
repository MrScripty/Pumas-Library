//! Controlled, retained ELF dependency-graph evidence, not loader admission.
//!
//! Parse actual held selected files, never ldd output, process maps, receipts
//! or caller-declared hashes. This bounded Linux ELF64/x86_64 slice supports
//! static entries and shared objects with explicit retained DT_NEEDED edges.
//! PT_INTERP, native search paths, audits and filter loaders remain unsupported.
//! A graph does not contain late dlopen/manual loads, interpreter import/data
//! reads or prove actual loader search isolation. Shipping audio stays refused.
#![allow(dead_code)] // Private diagnostic foundation; no shipping admission hook.

use crate::runtime_read_source::{RetainedRuntimeReadSource, RuntimeReadRole};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;
use std::sync::Arc;

const MAX_NODES: usize = 128;
const MAX_PROGRAM_HEADERS: usize = 256;
const MAX_DYNAMIC_ENTRIES: usize = 4096;
const MAX_STRING_TABLE: usize = 1024 * 1024;
const MAX_NEEDED: usize = 128;
const MAX_NATIVE_NAME: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NativeMember {
    pub(crate) role: RuntimeReadRole,
    pub(crate) path: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NativeDependency {
    pub(crate) requested: String,
    pub(crate) selected: NativeMember,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NativeNode {
    pub(crate) selected: NativeMember,
    pub(crate) soname: Option<String>,
    pub(crate) dependencies: Vec<NativeDependency>,
}

/// Owns the original byte source and inspected files; no availability flag,
/// serialization, replay constructor or production execution authority exists.
pub(crate) struct RetainedNativeDependencyGraph {
    _files: Vec<File>,
    _source: Arc<RetainedRuntimeReadSource>,
    nodes: Vec<NativeNode>,
}

impl RetainedNativeDependencyGraph {
    pub(crate) fn inspect(
        source: Arc<RetainedRuntimeReadSource>,
        members: &[NativeMember],
    ) -> io::Result<Self> {
        if members.is_empty() || members.len() > MAX_NODES {
            return Err(refusal("native graph member count is unsupported"));
        }
        source.validate()?;
        let mut unique = BTreeSet::new();
        let mut files = Vec::with_capacity(members.len());
        let mut parsed = Vec::with_capacity(members.len());
        let mut aliases = BTreeMap::new();
        for (index, member) in members.iter().enumerate() {
            if !unique.insert(member.clone()) {
                return Err(refusal("duplicate native selected member"));
            }
            // The held byte owner validates the exact name, identity and bytes.
            let file = source.clone_member(member.role, &member.path)?;
            let elf = inspect_elf(&file)?;
            let basename = member.path.rsplit('/').next().unwrap_or_default();
            native_name(basename)?;
            for name in std::iter::once(basename).chain(elf.soname.as_deref()) {
                if aliases
                    .insert(name.to_owned(), index)
                    .is_some_and(|old| old != index)
                {
                    return Err(refusal("ambiguous native basename or SONAME"));
                }
            }
            files.push(file);
            parsed.push(elf);
        }
        let mut nodes = Vec::with_capacity(members.len());
        for (member, elf) in members.iter().zip(parsed) {
            let mut dependencies = Vec::with_capacity(elf.needed.len());
            for requested in elf.needed {
                let index = aliases.get(&requested).ok_or_else(|| {
                    refusal("native dependency is outside the retained ELF graph")
                })?;
                dependencies.push(NativeDependency {
                    requested,
                    selected: members[*index].clone(),
                });
            }
            nodes.push(NativeNode {
                selected: member.clone(),
                soname: elf.soname,
                dependencies,
            });
        }
        // Hash/identity recheck follows the metadata read before returning proof.
        source.validate()?;
        Ok(Self {
            _source: source,
            _files: files,
            nodes,
        })
    }

    pub(crate) fn nodes(&self) -> &[NativeNode] {
        &self.nodes
    }
}

struct Elf {
    needed: Vec<String>,
    soname: Option<String>,
}
struct Segment {
    kind: u32,
    offset: u64,
    virtual_address: u64,
    file_size: u64,
    memory_size: u64,
}

fn inspect_elf(file: &File) -> io::Result<Elf> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(refusal("native source is not regular"));
    }
    let length = metadata.len();
    let header = read_range(file, 0, 64, length)?;
    if &header[..7] != b"\x7fELF\x02\x01\x01"
        || !matches!(u16_at(&header, 16), 2 | 3)
        || u16_at(&header, 18) != 62
        || u32_at(&header, 20) != 1
        || u16_at(&header, 52) != 64
        || u16_at(&header, 54) != 56
    {
        return Err(refusal("unsupported native ELF ABI or header"));
    }
    let count = usize::from(u16_at(&header, 56));
    let offset = u64_at(&header, 32);
    if count == 0 || count > MAX_PROGRAM_HEADERS || offset < 64 {
        return Err(refusal("unsupported native program-header table"));
    }
    let table = read_range(file, offset, count * 56, length)?;
    let mut segments = Vec::with_capacity(count);
    let mut dynamic = None;
    for row in table.chunks_exact(56) {
        let segment = Segment {
            kind: u32_at(row, 0),
            offset: u64_at(row, 8),
            virtual_address: u64_at(row, 16),
            file_size: u64_at(row, 32),
            memory_size: u64_at(row, 40),
        };
        checked_end(segment.offset, segment.file_size, length)?;
        segment
            .virtual_address
            .checked_add(segment.memory_size)
            .ok_or_else(|| refusal("native segment address overflow"))?;
        if segment.file_size > segment.memory_size {
            return Err(refusal("native segment file size exceeds memory size"));
        }
        match segment.kind {
            // A held interpreter file is insufficient: the kernel still opens
            // the PT_INTERP pathname before our worker/owning channel executes.
            3 => return Err(refusal("ambient ELF interpreter is not qualified")),
            2 => {
                if dynamic.replace(segments.len()).is_some() {
                    return Err(refusal("duplicate native dynamic segment"));
                }
            }
            _ => {}
        }
        segments.push(segment);
    }
    if !segments.iter().any(|segment| segment.kind == 1) {
        return Err(refusal("native ELF has no load segment"));
    }
    let Some(dynamic) = dynamic.map(|index| &segments[index]) else {
        return Ok(Elf {
            needed: vec![],
            soname: None,
        });
    };
    let dynamic_end = dynamic
        .virtual_address
        .checked_add(dynamic.file_size)
        .ok_or_else(|| refusal("native dynamic address overflow"))?;
    let mut dynamic_loads = segments.iter().filter(|segment| {
        segment.kind == 1
            && segment.virtual_address <= dynamic.virtual_address
            && segment
                .virtual_address
                .checked_add(segment.file_size)
                .is_some_and(|end| dynamic_end <= end)
    });
    let loaded = dynamic_loads
        .next()
        .ok_or_else(|| refusal("native dynamic table is outside load bytes"))?;
    if dynamic_loads.next().is_some()
        || loaded
            .offset
            .checked_add(dynamic.virtual_address - loaded.virtual_address)
            != Some(dynamic.offset)
    {
        return Err(refusal("native dynamic table has incoherent load mapping"));
    }
    if dynamic.file_size == 0
        || dynamic.file_size % 16 != 0
        || dynamic.file_size / 16 > MAX_DYNAMIC_ENTRIES as u64
    {
        return Err(refusal("native dynamic table exceeds supported bound"));
    }
    let entries = read_range(file, dynamic.offset, dynamic.file_size as usize, length)?;
    let mut terminated = false;
    let mut strings = None;
    let mut string_size = None;
    let mut soname = None;
    let mut needed = Vec::new();
    for entry in entries.chunks_exact(16) {
        let tag = u64_at(entry, 0);
        let value = u64_at(entry, 8);
        if terminated {
            if tag != 0 || value != 0 {
                return Err(refusal("native dynamic data follows terminator"));
            }
            continue;
        }
        match tag {
            0 => {
                if value != 0 {
                    return Err(refusal("native dynamic terminator is incoherent"));
                }
                terminated = true;
            }
            1 => {
                if needed.len() >= MAX_NEEDED {
                    return Err(refusal("native dependency count exceeded"));
                }
                needed.push(value);
            }
            5 => singleton(&mut strings, value)?,
            10 => singleton(&mut string_size, value)?,
            14 => singleton(&mut soname, value)?,
            // RPATH/RUNPATH, CONFIG/DEPAUDIT/AUDIT, AUXILIARY/FILTER all
            // introduce unqualified native search or secondary loading.
            15 | 29 | 0x6fff_fdf6 | 0x6fff_fdf7 | 0x6fff_fef8 | 0x6fff_fef9 | 0x6fff_fefa
            | 0x6fff_fefb | 0x6fff_fefc | 0x7fff_fffd | 0x7fff_ffff => {
                return Err(refusal(
                    "native search, audit or filter loader is unsupported",
                ));
            }
            0x6fff_fffb if value & (0x10 | 0x2000) != 0 => {
                return Err(refusal(
                    "native filter or alternative-configuration flags are unsupported",
                ));
            }
            _ => {}
        }
    }
    if !terminated {
        return Err(refusal("native dynamic table has no terminator"));
    }
    if needed.is_empty() && soname.is_none() && strings.is_none() && string_size.is_none() {
        return Ok(Elf {
            needed: vec![],
            soname: None,
        });
    }
    let strings = strings.ok_or_else(|| refusal("native string table missing"))?;
    let size = string_size
        .filter(|size| *size > 0 && *size <= MAX_STRING_TABLE as u64)
        .ok_or_else(|| refusal("native string-table size is unsupported"))?;
    let end = strings
        .checked_add(size)
        .ok_or_else(|| refusal("native string-table address overflow"))?;
    let mut mappings = segments.iter().filter(|segment| {
        segment.kind == 1
            && segment.virtual_address <= strings
            && segment
                .virtual_address
                .checked_add(segment.file_size)
                .is_some_and(|limit| end <= limit)
    });
    let load = mappings
        .next()
        .ok_or_else(|| refusal("native string table is outside held load bytes"))?;
    if mappings.next().is_some() {
        return Err(refusal("ambiguous native string-table mapping"));
    }
    let offset = load
        .offset
        .checked_add(strings - load.virtual_address)
        .ok_or_else(|| refusal("native string-table offset overflow"))?;
    let bytes = read_range(file, offset, size as usize, length)?;
    let mut unique = BTreeSet::new();
    let mut names = Vec::with_capacity(needed.len());
    for offset in needed {
        let name = read_name(&bytes, offset)?;
        if !unique.insert(name.clone()) {
            return Err(refusal("duplicate native dependency"));
        }
        names.push(name);
    }
    Ok(Elf {
        needed: names,
        soname: soname.map(|offset| read_name(&bytes, offset)).transpose()?,
    })
}

fn singleton(slot: &mut Option<u64>, value: u64) -> io::Result<()> {
    if slot.replace(value).is_some() {
        return Err(refusal("duplicate native dynamic singleton"));
    }
    Ok(())
}
fn read_name(bytes: &[u8], offset: u64) -> io::Result<String> {
    let offset = usize::try_from(offset).map_err(|_| refusal("native string offset overflow"))?;
    let tail = bytes
        .get(offset..)
        .ok_or_else(|| refusal("native string offset is outside table"))?;
    let end = tail
        .iter()
        .take(MAX_NATIVE_NAME + 1)
        .position(|byte| *byte == 0)
        .ok_or_else(|| refusal("native name is unterminated or oversized"))?;
    let name =
        std::str::from_utf8(&tail[..end]).map_err(|_| refusal("native name is not UTF-8"))?;
    native_name(name)?;
    Ok(name.into())
}
fn native_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.len() > MAX_NATIVE_NAME
        || matches!(name, "." | "..")
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+'))
    {
        return Err(refusal(
            "native library name escapes closed basename namespace",
        ));
    }
    Ok(())
}
fn read_range(file: &File, offset: u64, size: usize, length: u64) -> io::Result<Vec<u8>> {
    checked_end(offset, size as u64, length)?;
    let mut bytes = vec![0; size];
    file.read_exact_at(&mut bytes, offset)?;
    Ok(bytes)
}
fn checked_end(offset: u64, size: u64, length: u64) -> io::Result<()> {
    if offset.checked_add(size).is_none_or(|end| end > length) {
        return Err(refusal("native file range is truncated or overflows"));
    }
    Ok(())
}
fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("fixed ELF record"),
    )
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed ELF record"),
    )
}
fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("fixed ELF record"),
    )
}
fn refusal(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

#[cfg(test)]
#[path = "runtime_native_closure/tests.rs"]
mod tests;
