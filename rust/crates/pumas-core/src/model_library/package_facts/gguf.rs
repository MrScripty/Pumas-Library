use crate::error::{PumasError, Result};
use crate::models::{GgufPackageEvidence, PackageFactStatus, PackageFactValueSource};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const GGUF_MAGIC: &[u8; 4] = b"GGUF";
const GGUF_HEADER_LEN: u64 = 24;
const MAX_METADATA_KV_COUNT: u64 = 4096;
const MAX_METADATA_BYTES: u64 = 64 * 1024 * 1024;
const MAX_STRING_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ARRAY_ELEMENTS: u64 = 1_000_000;
const MAX_ARRAY_DEPTH: usize = 10;

pub(crate) fn gguf_package_evidence(
    path: &Path,
    companion_artifacts: &[String],
) -> Result<GgufPackageEvidence> {
    let mut file = File::open(path).map_err(|err| PumasError::io_with_path(err, path))?;
    read_gguf_package_evidence(&mut file, path, companion_artifacts)
}

pub(crate) fn invalid_gguf_package_evidence(companion_artifacts: &[String]) -> GgufPackageEvidence {
    GgufPackageEvidence {
        status: PackageFactStatus::Invalid,
        companion_artifacts: companion_artifacts.to_vec(),
        ..GgufPackageEvidence::default()
    }
}

fn read_gguf_package_evidence<R: Read + Seek>(
    reader: &mut R,
    path: &Path,
    companion_artifacts: &[String],
) -> Result<GgufPackageEvidence> {
    let mut cursor = BoundedGgufReader::new(reader);
    let header = cursor.read_array::<24>()?;
    if &header[..4] != GGUF_MAGIC {
        return Ok(invalid_gguf_package_evidence(companion_artifacts));
    }

    let _version = u32::from_le_bytes(header[4..8].try_into().unwrap());
    let _tensor_count = u64::from_le_bytes(header[8..16].try_into().unwrap());
    let metadata_kv_count = u64::from_le_bytes(header[16..24].try_into().unwrap());
    if metadata_kv_count > MAX_METADATA_KV_COUNT {
        return Err(PumasError::Other(format!(
            "GGUF metadata key count {metadata_kv_count} exceeds bounded limit {MAX_METADATA_KV_COUNT}"
        )));
    }

    let mut evidence = GgufPackageEvidence {
        status: PackageFactStatus::Present,
        companion_artifacts: companion_artifacts.to_vec(),
        ..GgufPackageEvidence::default()
    };
    let mut metadata_keys = BTreeSet::new();

    for _ in 0..metadata_kv_count {
        let key = cursor.read_string()?;
        metadata_keys.insert(key.clone());
        let value_type = cursor.read_u32()?;
        match read_value(&mut cursor, value_type)? {
            GgufValue::String(value) => apply_string_value(&mut evidence, &key, value),
            GgufValue::Unsigned(value) => apply_unsigned_value(&mut evidence, &key, value),
            GgufValue::Signed(value) => apply_signed_value(&mut evidence, &key, value),
            GgufValue::Bool(_) | GgufValue::Other => {}
        }
    }

    evidence.metadata_keys = metadata_keys.into_iter().collect();
    apply_filename_quantization_fallback(&mut evidence, path);
    Ok(evidence)
}

fn apply_string_value(evidence: &mut GgufPackageEvidence, key: &str, value: String) {
    match key {
        "general.architecture" => evidence.architecture = non_empty(value),
        "general.file_type" => {
            evidence.file_type = non_empty(value.clone());
            set_header_quantization(evidence, value);
        }
        "general.quantization" | "general.quantization_type" | "quantization" => {
            set_header_quantization(evidence, value);
        }
        "tokenizer.ggml.model" | "tokenizer.model" => evidence.tokenizer_model = non_empty(value),
        "tokenizer.chat_template" => evidence.chat_template_present = Some(!value.is_empty()),
        "general.type" | "general.task" | "general.task_type" => {
            evidence.task_type = non_empty(value)
        }
        _ => {}
    }
}

fn apply_unsigned_value(evidence: &mut GgufPackageEvidence, key: &str, value: u64) {
    match key {
        "general.file_type" => {
            let label = gguf_file_type_label(value).to_string();
            evidence.file_type = Some(label.clone());
            set_header_quantization(evidence, label);
        }
        key if key.ends_with(".context_length") => evidence.context_length = Some(value),
        key if key.ends_with(".embedding_length") => evidence.embedding_length = Some(value),
        key if key.ends_with(".block_count") => evidence.block_count = Some(value),
        key if key.ends_with(".attention.head_count") => {
            evidence.attention_head_count = Some(value)
        }
        _ => {}
    }
}

fn apply_signed_value(evidence: &mut GgufPackageEvidence, key: &str, value: i64) {
    let Ok(unsigned) = u64::try_from(value) else {
        return;
    };
    apply_unsigned_value(evidence, key, unsigned);
}

fn set_header_quantization(evidence: &mut GgufPackageEvidence, value: String) {
    if value.is_empty() {
        return;
    }
    evidence.quantization = Some(value);
    evidence.value_source = Some(PackageFactValueSource::Header);
}

fn apply_filename_quantization_fallback(evidence: &mut GgufPackageEvidence, path: &Path) {
    if evidence.quantization.is_some() {
        return;
    }
    if let Some(quantization) = quantization_from_filename(path) {
        evidence.quantization = Some(quantization);
        evidence.value_source = Some(PackageFactValueSource::FilenameWeak);
    }
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
enum GgufValue {
    String(String),
    Unsigned(u64),
    Signed(i64),
    Bool(bool),
    Other,
}

fn read_value<R: Read + Seek>(
    cursor: &mut BoundedGgufReader<'_, R>,
    value_type: u32,
) -> Result<GgufValue> {
    match value_type {
        0 => Ok(GgufValue::Unsigned(u64::from(cursor.read_u8()?))),
        1 => Ok(GgufValue::Signed(i64::from(cursor.read_i8()?))),
        2 => Ok(GgufValue::Unsigned(u64::from(cursor.read_u16()?))),
        3 => Ok(GgufValue::Signed(i64::from(cursor.read_i16()?))),
        4 => Ok(GgufValue::Unsigned(u64::from(cursor.read_u32()?))),
        5 => Ok(GgufValue::Signed(i64::from(cursor.read_i32()?))),
        6 => {
            cursor.skip_bytes(4)?;
            Ok(GgufValue::Other)
        }
        7 => Ok(GgufValue::Bool(cursor.read_u8()? != 0)),
        8 => Ok(GgufValue::String(cursor.read_string()?)),
        9 => {
            skip_array(cursor, 0)?;
            Ok(GgufValue::Other)
        }
        10 => Ok(GgufValue::Unsigned(cursor.read_u64()?)),
        11 => Ok(GgufValue::Signed(cursor.read_i64()?)),
        12 => {
            cursor.skip_bytes(8)?;
            Ok(GgufValue::Other)
        }
        _ => Err(PumasError::Other(format!(
            "Unsupported GGUF metadata value type {value_type}"
        ))),
    }
}

fn skip_array<R: Read + Seek>(cursor: &mut BoundedGgufReader<'_, R>, depth: usize) -> Result<()> {
    if depth > MAX_ARRAY_DEPTH {
        return Err(PumasError::Other(
            "GGUF metadata array nesting exceeds bounded limit".into(),
        ));
    }
    let array_type = cursor.read_u32()?;
    let len = cursor.read_u64()?;
    if len > MAX_ARRAY_ELEMENTS {
        return Err(PumasError::Other(format!(
            "GGUF metadata array length {len} exceeds bounded limit {MAX_ARRAY_ELEMENTS}"
        )));
    }
    for _ in 0..len {
        skip_value(cursor, array_type, depth + 1)?;
    }
    Ok(())
}

fn skip_value<R: Read + Seek>(
    cursor: &mut BoundedGgufReader<'_, R>,
    value_type: u32,
    depth: usize,
) -> Result<()> {
    match value_type {
        0 | 1 | 7 => cursor.skip_bytes(1),
        2 | 3 => cursor.skip_bytes(2),
        4 | 5 | 6 => cursor.skip_bytes(4),
        8 => {
            let len = cursor.read_u64()?;
            cursor.skip_string_bytes(len)
        }
        9 => skip_array(cursor, depth),
        10 | 11 | 12 => cursor.skip_bytes(8),
        _ => Err(PumasError::Other(format!(
            "Unsupported GGUF metadata value type {value_type}"
        ))),
    }
}

struct BoundedGgufReader<'a, R> {
    inner: &'a mut R,
    metadata_bytes_read: u64,
}

impl<'a, R: Read + Seek> BoundedGgufReader<'a, R> {
    fn new(inner: &'a mut R) -> Self {
        Self {
            inner,
            metadata_bytes_read: 0,
        }
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut bytes = [0_u8; N];
        self.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        self.track_bytes(buf.len() as u64)?;
        self.inner
            .read_exact(buf)
            .map_err(|err| PumasError::Other(format!("Corrupt GGUF metadata: {err}")))
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_array::<1>()?[0])
    }

    fn read_i8(&mut self) -> Result<i8> {
        Ok(i8::from_le_bytes(self.read_array::<1>()?))
    }

    fn read_u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read_array::<2>()?))
    }

    fn read_i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.read_array::<2>()?))
    }

    fn read_u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_array::<4>()?))
    }

    fn read_i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.read_array::<4>()?))
    }

    fn read_u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_array::<8>()?))
    }

    fn read_i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.read_array::<8>()?))
    }

    fn read_string(&mut self) -> Result<String> {
        let len = self.read_u64()?;
        if len > MAX_STRING_BYTES {
            return Err(PumasError::Other(format!(
                "GGUF metadata string length {len} exceeds bounded limit {MAX_STRING_BYTES}"
            )));
        }
        let len = usize::try_from(len).map_err(|_| {
            PumasError::Other("GGUF metadata string length does not fit usize".into())
        })?;
        let mut bytes = vec![0_u8; len];
        self.read_exact(&mut bytes)?;
        String::from_utf8(bytes)
            .map_err(|_| PumasError::Other("Invalid UTF-8 in GGUF metadata string".into()))
    }

    fn skip_bytes(&mut self, len: u64) -> Result<()> {
        self.track_bytes(len)?;
        let offset = i64::try_from(len)
            .map_err(|_| PumasError::Other("GGUF metadata skip length does not fit i64".into()))?;
        self.inner
            .seek(SeekFrom::Current(offset))
            .map_err(|err| PumasError::Other(format!("Corrupt GGUF metadata: {err}")))?;
        Ok(())
    }

    fn skip_string_bytes(&mut self, len: u64) -> Result<()> {
        if len > MAX_STRING_BYTES {
            return Err(PumasError::Other(format!(
                "GGUF metadata string length {len} exceeds bounded limit {MAX_STRING_BYTES}"
            )));
        }
        self.skip_bytes(len)
    }

    fn track_bytes(&mut self, len: u64) -> Result<()> {
        self.metadata_bytes_read = self
            .metadata_bytes_read
            .checked_add(len)
            .ok_or_else(|| PumasError::Other("GGUF metadata byte count overflow".into()))?;
        let metadata_without_header = self.metadata_bytes_read.saturating_sub(GGUF_HEADER_LEN);
        if metadata_without_header > MAX_METADATA_BYTES {
            return Err(PumasError::Other(format!(
                "GGUF metadata section exceeds bounded limit {MAX_METADATA_BYTES}"
            )));
        }
        Ok(())
    }
}

fn gguf_file_type_label(value: u64) -> &'static str {
    match value {
        0 => "ALL_F32",
        1 => "MOSTLY_F16",
        2 => "MOSTLY_Q4_0",
        3 => "MOSTLY_Q4_1",
        4 => "MOSTLY_Q4_1_SOME_F16",
        5 => "MOSTLY_Q8_0",
        6 => "MOSTLY_Q5_0",
        7 => "MOSTLY_Q5_1",
        8 => "MOSTLY_Q2_K",
        9 => "MOSTLY_Q3_K_S",
        10 => "MOSTLY_Q3_K_M",
        11 => "MOSTLY_Q3_K_L",
        12 => "MOSTLY_Q4_K_S",
        13 => "MOSTLY_Q4_K_M",
        14 => "MOSTLY_Q5_K_S",
        15 => "MOSTLY_Q5_K_M",
        16 => "MOSTLY_Q6_K",
        17 => "MOSTLY_IQ2_XXS",
        18 => "MOSTLY_IQ2_XS",
        19 => "MOSTLY_Q2_K_S",
        20 => "MOSTLY_IQ3_XS",
        21 => "MOSTLY_IQ3_XXS",
        22 => "MOSTLY_IQ1_S",
        23 => "MOSTLY_IQ4_NL",
        24 => "MOSTLY_IQ3_S",
        25 => "MOSTLY_IQ3_M",
        26 => "MOSTLY_IQ2_S",
        27 => "MOSTLY_IQ2_M",
        28 => "MOSTLY_IQ4_XS",
        29 => "MOSTLY_IQ1_M",
        30 => "MOSTLY_BF16",
        31 => "MOSTLY_Q4_0_4_4",
        32 => "MOSTLY_Q4_0_4_8",
        33 => "MOSTLY_Q4_0_8_8",
        34 => "MOSTLY_TQ1_0",
        35 => "MOSTLY_TQ2_0",
        _ => "UNKNOWN",
    }
}

fn quantization_from_filename(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let upper = file_name.to_uppercase();
    KNOWN_GGUF_QUANTS
        .iter()
        .find(|quant| upper.contains(**quant))
        .map(|quant| (*quant).to_string())
}

const KNOWN_GGUF_QUANTS: &[&str] = &[
    "IQ2_XXS", "IQ3_XXS", "Q3_K_S", "Q3_K_M", "Q3_K_L", "Q4_K_S", "Q4_K_M", "Q5_K_S", "Q5_K_M",
    "IQ2_XS", "IQ3_XS", "IQ4_XS", "IQ4_NL", "IQ1_S", "IQ1_M", "IQ2_S", "IQ2_M", "IQ3_S", "IQ3_M",
    "Q2_K", "Q3_K", "Q4_0", "Q4_1", "Q4_K", "Q5_0", "Q5_1", "Q5_K", "Q6_K", "Q8_0",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn extracts_header_metadata_without_tensor_data() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("model.gguf");
        write_minimal_gguf(
            &path,
            &[
                kv_string("general.architecture", "llama"),
                kv_u32("general.file_type", 13),
                kv_string("tokenizer.ggml.model", "llama"),
                kv_string("tokenizer.chat_template", "{{ messages }}"),
                kv_u64("llama.context_length", 4096),
                kv_u64("llama.embedding_length", 4096),
                kv_u64("llama.block_count", 32),
                kv_u64("llama.attention.head_count", 32),
                kv_string("general.type", "model"),
            ],
        );
        append_unread_tensor_payload_marker(&path);

        let evidence = gguf_package_evidence(&path, &[]).unwrap();

        assert_eq!(evidence.status, PackageFactStatus::Present);
        assert_eq!(evidence.architecture.as_deref(), Some("llama"));
        assert_eq!(evidence.file_type.as_deref(), Some("MOSTLY_Q4_K_M"));
        assert_eq!(evidence.quantization.as_deref(), Some("MOSTLY_Q4_K_M"));
        assert_eq!(evidence.value_source, Some(PackageFactValueSource::Header));
        assert_eq!(evidence.tokenizer_model.as_deref(), Some("llama"));
        assert_eq!(evidence.chat_template_present, Some(true));
        assert_eq!(evidence.context_length, Some(4096));
        assert_eq!(evidence.embedding_length, Some(4096));
        assert_eq!(evidence.block_count, Some(32));
        assert_eq!(evidence.attention_head_count, Some(32));
        assert_eq!(evidence.task_type.as_deref(), Some("model"));
        assert!(evidence
            .metadata_keys
            .contains(&"general.architecture".to_string()));
    }

    #[test]
    fn reports_invalid_or_corrupt_headers() {
        let temp_dir = tempfile::tempdir().unwrap();
        let invalid_path = temp_dir.path().join("invalid.gguf");
        let mut invalid_header = [0_u8; 24];
        invalid_header[..4].copy_from_slice(b"NOTG");
        std::fs::write(&invalid_path, invalid_header).unwrap();

        let invalid = gguf_package_evidence(&invalid_path, &[]).unwrap();
        assert_eq!(invalid.status, PackageFactStatus::Invalid);
        assert!(invalid.metadata_keys.is_empty());

        let corrupt_path = temp_dir.path().join("corrupt.gguf");
        std::fs::write(&corrupt_path, b"GGUF").unwrap();

        let error = gguf_package_evidence(&corrupt_path, &[]).unwrap_err();
        assert!(error.to_string().contains("Corrupt GGUF metadata"));
    }

    #[test]
    fn uses_filename_quantization_only_as_weak_fallback() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("tiny-Q4_K_M.gguf");
        write_minimal_gguf(&path, &[kv_string("general.architecture", "llama")]);

        let evidence = gguf_package_evidence(&path, &[]).unwrap();

        assert_eq!(evidence.quantization.as_deref(), Some("Q4_K_M"));
        assert_eq!(
            evidence.value_source,
            Some(PackageFactValueSource::FilenameWeak)
        );
        assert_eq!(evidence.file_type, None);
    }

    #[test]
    fn preserves_caller_supplied_companion_evidence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("model.gguf");
        write_minimal_gguf(&path, &[kv_string("general.architecture", "llava")]);

        let evidence = gguf_package_evidence(
            &path,
            &[
                "mmproj-model-f16.gguf".to_string(),
                "vision/mmproj.gguf".to_string(),
            ],
        )
        .unwrap();

        assert_eq!(
            evidence.companion_artifacts,
            vec![
                "mmproj-model-f16.gguf".to_string(),
                "vision/mmproj.gguf".to_string()
            ]
        );
    }

    fn write_minimal_gguf(path: &Path, metadata: &[Vec<u8>]) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(GGUF_MAGIC);
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&(metadata.len() as u64).to_le_bytes());
        for kv in metadata {
            bytes.extend_from_slice(kv);
        }
        std::fs::write(path, bytes).unwrap();
    }

    fn append_unread_tensor_payload_marker(path: &Path) {
        let mut file = std::fs::OpenOptions::new().append(true).open(path).unwrap();
        file.write_all(b"tensor-data-must-not-be-read").unwrap();
    }

    fn kv_string(key: &str, value: &str) -> Vec<u8> {
        let mut bytes = kv_header(key, 8);
        write_string(&mut bytes, value);
        bytes
    }

    fn kv_u32(key: &str, value: u32) -> Vec<u8> {
        let mut bytes = kv_header(key, 4);
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes
    }

    fn kv_u64(key: &str, value: u64) -> Vec<u8> {
        let mut bytes = kv_header(key, 10);
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes
    }

    fn kv_header(key: &str, value_type: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        write_string(&mut bytes, key);
        bytes.extend_from_slice(&value_type.to_le_bytes());
        bytes
    }

    fn write_string(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
    }
}
