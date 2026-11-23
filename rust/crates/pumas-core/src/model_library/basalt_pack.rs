//! Basalt v2 container identification. The runtime owns the tensor and
//! architecture compatibility checks.

use std::{
    collections::BTreeSet,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use super::{
    identifier::ModelTypeInfo,
    types::{FileFormat, ModelType},
};
use crate::{PumasError, Result};

// Layout from basalt/include/basalt/core/model_format.hpp.
pub(super) const MAGIC: &[u8; 8] = b"BASALTMF";
const VERSION: u32 = 2;
const HEADER_BYTES: u64 = 64 * 1024;
const SECTION_ALIGNMENT: u64 = 2 * 1024 * 1024;
const MAX_SECTIONS: u32 = (64 * 1024 - 64) / 128;

pub(super) fn identify<R: Read + Seek>(file: &mut R, path: &Path) -> Result<ModelTypeInfo> {
    let size = file
        .seek(SeekFrom::End(0))
        .map_err(|error| PumasError::io_with_path(error, path))?;
    if size < HEADER_BYTES {
        return Err(invalid("Basalt pack header is truncated"));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| PumasError::io_with_path(error, path))?;
    let mut header = [0; 64];
    file.read_exact(&mut header)
        .map_err(|error| PumasError::io_with_path(error, path))?;
    if !header.starts_with(MAGIC) {
        return Err(invalid("Basalt pack signature is invalid"));
    }
    let version = read_u32(&header[8..12])?;
    if version != VERSION {
        return Err(invalid(format!(
            "Unsupported Basalt pack version {version}; expected {VERSION}"
        )));
    }
    let entries = read_u32(&header[12..16])?;
    let declared_size = read_u64(&header[16..24])?;
    if !(1..=MAX_SECTIONS).contains(&entries) || declared_size != size {
        return Err(invalid(
            "Basalt pack section count or file length is invalid",
        ));
    }

    let mut sections = BTreeSet::new();
    let mut ranges = Vec::new();
    for _ in 0..entries {
        let mut entry = [0; 128];
        file.read_exact(&mut entry)
            .map_err(|error| PumasError::io_with_path(error, path))?;
        let name = std::str::from_utf8(&entry[..64])
            .map_err(|_| invalid("Basalt section name is not text"))?
            .trim_end_matches('\0');
        let offset = read_u64(&entry[64..72])?;
        let length = read_u64(&entry[72..80])?;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| invalid("Basalt section range overflows"))?;
        if name.is_empty()
            || name.contains('\0')
            || offset < HEADER_BYTES
            || offset % SECTION_ALIGNMENT != 0
            || end > size
            || !sections.insert(name.to_owned())
        {
            return Err(invalid("Basalt pack contains an invalid section"));
        }
        if length > 0 {
            ranges.push((offset, end));
        } else if name == "meta" {
            return Err(invalid("Basalt pack metadata section is empty"));
        }
    }
    ranges.sort_unstable();
    if !sections.contains("meta") || ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(invalid(
            "Basalt pack metadata is missing or sections overlap",
        ));
    }

    Ok(ModelTypeInfo {
        format: FileFormat::Basalt,
        model_type: ModelType::Llm,
        ..ModelTypeInfo::default()
    })
}

fn read_u32(bytes: &[u8]) -> Result<u32> {
    bytes
        .try_into()
        .map(u32::from_le_bytes)
        .map_err(|_| invalid("Basalt pack header is truncated"))
}

fn read_u64(bytes: &[u8]) -> Result<u64> {
    bytes
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| invalid("Basalt section table is truncated"))
}

fn invalid(message: impl Into<String>) -> PumasError {
    PumasError::Validation {
        field: "basalt_pack".into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_library::identifier::identify_model_reader;
    use std::io::Cursor;

    fn pack() -> Cursor<Vec<u8>> {
        let offset = usize::try_from(SECTION_ALIGNMENT).unwrap();
        let mut bytes = vec![0; offset + 2];
        let size = u64::try_from(bytes.len()).unwrap();
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..12].copy_from_slice(&VERSION.to_le_bytes());
        bytes[12..16].copy_from_slice(&1_u32.to_le_bytes());
        bytes[16..24].copy_from_slice(&size.to_le_bytes());
        bytes[64..68].copy_from_slice(b"meta");
        bytes[128..136].copy_from_slice(&SECTION_ALIGNMENT.to_le_bytes());
        bytes[136..144].copy_from_slice(&2_u64.to_le_bytes());
        bytes[offset..].copy_from_slice(b"{}");
        Cursor::new(bytes)
    }

    #[test]
    fn identifies_basalt_by_content_and_accepts_the_named_pack() {
        for path in ["model.basalt", "model.unknown"] {
            let result = identify_model_reader(&mut pack(), Path::new(path)).unwrap();
            assert_eq!(result.format, FileFormat::Basalt);
            assert_eq!(result.model_type, ModelType::Llm);
        }
    }

    #[test]
    fn rejects_wrong_signature_unknown_version_and_overflowing_section() {
        let path = Path::new("model.basalt");
        let mut wrong_magic = pack();
        wrong_magic.get_mut()[0] = b'X';
        assert!(identify_model_reader(&mut wrong_magic, path).is_err());

        let mut wrong_version = pack();
        wrong_version.get_mut()[8..12].copy_from_slice(&1_u32.to_le_bytes());
        assert!(identify_model_reader(&mut wrong_version, path).is_err());

        let mut wrong_range = pack();
        wrong_range.get_mut()[136..144].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(identify_model_reader(&mut wrong_range, path).is_err());
    }
}
