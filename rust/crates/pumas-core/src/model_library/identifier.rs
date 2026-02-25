//! Model type detection from file contents.
//!
//! Analyzes file headers and content to determine:
//! - File format (GGUF, Safetensors, Pickle, ONNX)
//! - Model type (LLM, diffusion, embedding)
//! - Model family (llama, mistral, qwen3, stable-diffusion, etc.)

use crate::error::{PumasError, Result};
use crate::model_library::types::{FileFormat, ModelFamily, ModelType};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Key GGUF metadata fields for model identification.
#[derive(Debug, Default)]
struct GgufMetadata {
    /// Model architecture (e.g., "qwen3", "llama")
    architecture: Option<String>,
    /// Model name (e.g., "Qwen3 Embedding 0.6b")
    name: Option<String>,
    /// Model basename (e.g., "qwen3-embedding")
    basename: Option<String>,
    /// Model type field from GGUF (usually "model")
    model_type: Option<String>,
}

/// Magic bytes for file format detection.
mod magic {
    /// GGUF format magic bytes
    pub const GGUF: &[u8; 4] = b"GGUF";
    /// GGML format magic bytes (legacy)
    pub const GGML: &[u8; 4] = b"lmgg";
    pub const GGJT: &[u8; 4] = b"ggjt";
    /// ZIP header (used by PyTorch .pt files)
    pub const ZIP: &[u8; 4] = &[0x50, 0x4B, 0x03, 0x04];
    /// Pickle protocol markers
    pub const PICKLE_V2: u8 = 0x80;
    pub const PICKLE_PROTO_MIN: u8 = 2;
    pub const PICKLE_PROTO_MAX: u8 = 5;
}

/// Result of model type identification.
#[derive(Debug, Clone)]
pub struct ModelTypeInfo {
    /// Detected file format
    pub format: FileFormat,
    /// Detected model type (LLM, diffusion, etc.)
    pub model_type: ModelType,
    /// Detected model family/architecture
    pub family: Option<ModelFamily>,
    /// Additional metadata extracted
    pub extra: HashMap<String, String>,
}

impl Default for ModelTypeInfo {
    fn default() -> Self {
        Self {
            format: FileFormat::Unknown,
            model_type: ModelType::Unknown,
            family: None,
            extra: HashMap::new(),
        }
    }
}

/// Identify model type from file contents.
///
/// Reads file headers to determine:
/// 1. File format (GGUF, Safetensors, Pickle, etc.)
/// 2. Model type (LLM, diffusion)
/// 3. Model family (llama, mistral, stable-diffusion, etc.)
///
/// # Arguments
///
/// * `path` - Path to the model file
///
/// # Returns
///
/// ModelTypeInfo with detected format, type, and family.
pub fn identify_model_type(path: impl AsRef<Path>) -> Result<ModelTypeInfo> {
    let path = path.as_ref();
    let mut file = std::fs::File::open(path).map_err(|e| PumasError::io_with_path(e, path))?;

    // Read first bytes for magic detection
    let mut header = [0u8; 64];
    let bytes_read = file
        .read(&mut header)
        .map_err(|e| PumasError::io_with_path(e, path))?;

    if bytes_read < 4 {
        return Ok(ModelTypeInfo::default());
    }

    // Check file extension as hint
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // Detect format from magic bytes
    let format = detect_format(&header[..bytes_read], &extension);

    // Get detailed info based on format
    file.seek(SeekFrom::Start(0))
        .map_err(|e| PumasError::io_with_path(e, path))?;

    match format {
        FileFormat::Gguf => identify_gguf(&mut file, path),
        FileFormat::Safetensors => identify_safetensors(&mut file, path),
        _ => Ok(ModelTypeInfo {
            format,
            model_type: ModelType::Unknown,
            family: None,
            extra: HashMap::new(),
        }),
    }
}

/// Detect file format from magic bytes.
fn detect_format(header: &[u8], extension: &str) -> FileFormat {
    if header.len() < 4 {
        return FileFormat::Unknown;
    }

    // GGUF format
    if &header[..4] == magic::GGUF {
        return FileFormat::Gguf;
    }

    // GGML legacy formats
    if &header[..4] == magic::GGML || &header[..4] == magic::GGJT {
        return FileFormat::Ggml;
    }

    // ZIP header (PyTorch .pt files)
    if &header[..4] == magic::ZIP {
        return FileFormat::Pickle;
    }

    // Pickle protocol marker
    if header[0] == magic::PICKLE_V2 {
        if header.len() > 1
            && header[1] >= magic::PICKLE_PROTO_MIN
            && header[1] <= magic::PICKLE_PROTO_MAX
        {
            return FileFormat::Pickle;
        }
    }

    // Safetensors: 8-byte little-endian header size followed by JSON starting with '{'
    if header.len() >= 16 {
        // First 8 bytes are header size (little-endian u64)
        let header_size = u64::from_le_bytes(header[..8].try_into().unwrap_or([0; 8]));
        // Check for reasonable header size and JSON opening brace
        if header_size > 0 && header_size < 100_000_000 && header[8] == b'{' {
            return FileFormat::Safetensors;
        }
    }

    // ONNX: protobuf format (less reliable detection)
    // Check for protobuf wire type markers typical in ONNX
    if extension == "onnx" {
        return FileFormat::Onnx;
    }

    // Fall back to extension-based detection
    match extension.as_ref() {
        "gguf" => FileFormat::Gguf,
        "ggml" | "bin" => FileFormat::Ggml,
        "safetensors" => FileFormat::Safetensors,
        "pt" | "pth" | "ckpt" => FileFormat::Pickle,
        "onnx" => FileFormat::Onnx,
        _ => FileFormat::Unknown,
    }
}

/// Identify GGUF model details.
fn identify_gguf<R: Read + Seek>(file: &mut R, path: &Path) -> Result<ModelTypeInfo> {
    // GGUF header format:
    // 0-3: magic "GGUF"
    // 4-7: version (u32, little-endian)
    // 8-15: tensor_count (u64, little-endian)
    // 16-23: metadata_kv_count (u64, little-endian)
    // Then: metadata key-value pairs

    let mut header = [0u8; 24];
    file.read_exact(&mut header)
        .map_err(|e| PumasError::io_with_path(e, path))?;

    let version = u32::from_le_bytes(header[4..8].try_into().unwrap());
    let metadata_count = u64::from_le_bytes(header[16..24].try_into().unwrap());

    let mut info = ModelTypeInfo {
        format: FileFormat::Gguf,
        model_type: ModelType::Llm, // Default, will be refined based on metadata
        family: None,
        extra: HashMap::new(),
    };

    info.extra.insert("gguf_version".to_string(), version.to_string());

    // Parse metadata to find architecture, name, basename, etc.
    if let Ok(metadata) = extract_gguf_key_metadata(file, metadata_count as usize) {
        // Set family from architecture (preserves version, e.g., "qwen3" not "qwen")
        if let Some(ref arch) = metadata.architecture {
            info.family = Some(ModelFamily::new(arch));
            info.extra.insert("architecture".to_string(), arch.clone());
        }

        // Store additional metadata
        if let Some(ref name) = metadata.name {
            info.extra.insert("name".to_string(), name.clone());
        }
        if let Some(ref basename) = metadata.basename {
            info.extra.insert("basename".to_string(), basename.clone());
        }

        // Detect embedding models from metadata
        info.model_type = detect_model_type_from_gguf_metadata(&metadata);
    }

    Ok(info)
}

/// Detect model type from GGUF metadata fields.
///
/// Checks name, basename for embedding indicators.
fn detect_model_type_from_gguf_metadata(metadata: &GgufMetadata) -> ModelType {
    // Check for embedding model indicators in name and basename
    let check_for_embedding = |s: &str| -> bool {
        let lower = s.to_lowercase();
        lower.contains("embedding") || lower.contains("embed-")
    };

    if let Some(ref basename) = metadata.basename {
        if check_for_embedding(basename) {
            return ModelType::Embedding;
        }
    }

    if let Some(ref name) = metadata.name {
        if check_for_embedding(name) {
            return ModelType::Embedding;
        }
    }

    // Default to LLM for GGUF files (most common use case)
    ModelType::Llm
}

/// Extract key metadata fields from GGUF for model identification.
///
/// Extracts: general.architecture, general.name, general.basename
fn extract_gguf_key_metadata<R: Read>(file: &mut R, metadata_count: usize) -> Result<GgufMetadata> {
    // GGUF string format: length (u64) + bytes
    // GGUF metadata KV: key_string + value_type (u32) + value

    let mut metadata = GgufMetadata::default();
    let target_keys = [
        "general.architecture",
        "general.name",
        "general.basename",
        "general.type",
    ];

    for _ in 0..std::cmp::min(metadata_count, 100) {
        // Read key
        let key = match read_gguf_string(file) {
            Ok(k) => k,
            Err(_) => break,
        };

        // Read value type
        let mut type_buf = [0u8; 4];
        if file.read_exact(&mut type_buf).is_err() {
            break;
        }
        let value_type = u32::from_le_bytes(type_buf);

        // Check if this is a key we want (string type = 8)
        if target_keys.contains(&key.as_str()) && value_type == 8 {
            if let Ok(value) = read_gguf_string(file) {
                match key.as_str() {
                    "general.architecture" => metadata.architecture = Some(value),
                    "general.name" => metadata.name = Some(value),
                    "general.basename" => metadata.basename = Some(value),
                    "general.type" => metadata.model_type = Some(value),
                    _ => {}
                }
            }
        } else {
            // Skip this value based on type
            if skip_gguf_value(file, value_type).is_err() {
                break;
            }
        }

        // Early exit if we have all the info we need
        if metadata.architecture.is_some()
            && metadata.name.is_some()
            && metadata.basename.is_some()
        {
            break;
        }
    }

    Ok(metadata)
}

/// Read a GGUF string (length-prefixed).
fn read_gguf_string<R: Read>(file: &mut R) -> Result<String> {
    let mut len_buf = [0u8; 8];
    file.read_exact(&mut len_buf)?;
    let len = u64::from_le_bytes(len_buf) as usize;

    if len > 1024 * 1024 {
        return Err(PumasError::Other("GGUF string too long".into()));
    }

    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf)?;

    String::from_utf8(buf).map_err(|_| PumasError::Other("Invalid UTF-8 in GGUF string".into()))
}

/// Skip a GGUF value based on its type.
fn skip_gguf_value<R: Read>(file: &mut R, value_type: u32) -> Result<()> {
    skip_gguf_value_impl(file, value_type, 0)
}

/// Skip a GGUF value with depth tracking to prevent stack overflow on nested arrays.
fn skip_gguf_value_impl<R: Read>(file: &mut R, value_type: u32, depth: usize) -> Result<()> {
    // Prevent stack overflow on deeply nested or malformed files
    if depth > 10 {
        return Err(PumasError::Other("GGUF array nesting too deep".into()));
    }

    let skip_bytes = match value_type {
        0 => 1,  // uint8
        1 => 1,  // int8
        2 => 2,  // uint16
        3 => 2,  // int16
        4 => 4,  // uint32
        5 => 4,  // int32
        6 => 4,  // float32
        7 => 1,  // bool
        8 => {
            // string
            let mut len_buf = [0u8; 8];
            file.read_exact(&mut len_buf)?;
            u64::from_le_bytes(len_buf) as usize
        }
        9 => {
            // array - properly skip all elements
            let mut type_buf = [0u8; 4];
            file.read_exact(&mut type_buf)?;
            let array_type = u32::from_le_bytes(type_buf);

            let mut len_buf = [0u8; 8];
            file.read_exact(&mut len_buf)?;
            let array_len = u64::from_le_bytes(len_buf) as usize;

            // Skip each element in the array
            for _ in 0..array_len {
                skip_gguf_value_impl(file, array_type, depth + 1)?;
            }
            return Ok(());
        }
        10 => 8, // uint64
        11 => 8, // int64
        12 => 8, // float64
        _ => return Err(PumasError::Other("Unknown GGUF type".into())),
    };

    let mut skip_buf = vec![0u8; skip_bytes];
    file.read_exact(&mut skip_buf)?;

    Ok(())
}

/// Identify safetensors model details.
fn identify_safetensors<R: Read + Seek>(file: &mut R, path: &Path) -> Result<ModelTypeInfo> {
    // Safetensors format:
    // 0-7: header size (u64, little-endian)
    // 8+: JSON header with tensor metadata

    let mut size_buf = [0u8; 8];
    file.read_exact(&mut size_buf)
        .map_err(|e| PumasError::io_with_path(e, path))?;
    let header_size = u64::from_le_bytes(size_buf) as usize;

    if header_size > 100_000_000 {
        return Err(PumasError::Other("Safetensors header too large".into()));
    }

    // Read JSON header
    let mut header_buf = vec![0u8; header_size];
    file.read_exact(&mut header_buf)
        .map_err(|e| PumasError::io_with_path(e, path))?;

    let header_str = String::from_utf8_lossy(&header_buf);

    // Parse to get tensor names
    let header: serde_json::Value = serde_json::from_str(&header_str)?;

    // Analyze tensor names to determine model type
    let (mut model_type, family) = analyze_tensor_names(&header);

    // Check directory context for embedding indicators
    // This catches embedding models that don't have distinctive tensor patterns
    if model_type != ModelType::Embedding {
        if is_embedding_from_context(path) {
            model_type = ModelType::Embedding;
        }
    }

    // Check directory context for audio indicators
    // Audio models using diffusion architectures would otherwise be mis-detected as Diffusion
    if model_type == ModelType::Diffusion || model_type == ModelType::Unknown {
        if is_audio_from_context(path) {
            model_type = ModelType::Audio;
        }
    }

    Ok(ModelTypeInfo {
        format: FileFormat::Safetensors,
        model_type,
        family,
        extra: HashMap::new(),
    })
}

/// Check directory context for embedding model indicators.
///
/// This supplements tensor analysis by checking:
/// 1. Presence of sentence_transformers config file
/// 2. Model path/name containing "embedding"
fn is_embedding_from_context(path: &Path) -> bool {
    // Check parent directory for sentence_transformers config
    if let Some(parent) = path.parent() {
        let sentence_transformers_config = parent.join("config_sentence_transformers.json");
        if sentence_transformers_config.exists() {
            return true;
        }
    }

    // Check if path contains "embedding" indicator
    let path_str = path.to_string_lossy().to_lowercase();
    if path_str.contains("embedding") || path_str.contains("embed-") {
        return true;
    }

    false
}

/// Check directory context for audio model indicators.
///
/// This supplements tensor analysis by checking the parent directory's
/// `config.json` for audio-specific metadata fields.
fn is_audio_from_context(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };

    let config_path = parent.join("config.json");
    let config_str = match std::fs::read_to_string(&config_path) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let config: serde_json::Value = match serde_json::from_str(&config_str) {
        Ok(v) => v,
        Err(_) => return false,
    };

    // Check for audio-specific config fields
    if config.get("sample_rate").is_some()
        || config.get("audio_channels").is_some()
        || config.get("num_audio_channels").is_some()
        || config.get("audio_encoder").is_some()
        || config.get("mel_channels").is_some()
    {
        return true;
    }

    // Check model_type field for audio-related values
    if let Some(model_type) = config.get("model_type").and_then(|v| v.as_str()) {
        let lower = model_type.to_lowercase();
        if lower.contains("audio")
            || lower.contains("speech")
            || lower.contains("whisper")
            || lower.contains("musicgen")
            || lower.contains("encodec")
            || lower.contains("bark")
        {
            return true;
        }
    }

    false
}

/// Analyze tensor names to determine model type and family.
fn analyze_tensor_names(header: &serde_json::Value) -> (ModelType, Option<ModelFamily>) {
    let Some(obj) = header.as_object() else {
        return (ModelType::Unknown, None);
    };

    // Collect all tensor keys
    let tensor_names: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();

    // Count indicators for each type
    let mut llm_indicators = 0;
    let mut diffusion_indicators = 0;
    let mut embedding_indicators = 0;
    let mut has_lm_head = false;

    // LLM patterns (transformer architecture, but also used by embedding models)
    let llm_patterns = [
        "self_attn",
        "embed_tokens",
        "model.layers.",
        "transformer.h.",
        "attention.wq",
        "attention.wk",
        "attention.wv",
        "feed_forward",
        "mlp.gate",
        "mlp.up",
        "mlp.down",
        "rotary_emb",
    ];

    // Diffusion patterns
    let diffusion_patterns = [
        "down_blocks",
        "up_blocks",
        "mid_block",
        "time_embedding",
        "conv_in",
        "conv_out",
        "unet",
        "vae",
        "text_encoder",
        "controlnet",
        "diffusion_model",
    ];

    // Embedding-specific patterns (pooling, sentence transformers, etc.)
    let embedding_patterns = [
        "pooler",
        "sentence_",
        "dense_layer",
        "projection",
    ];

    for name in &tensor_names {
        let lower = name.to_lowercase();

        // Check for lm_head specifically (indicates text generation, not embedding)
        if lower.contains("lm_head") {
            has_lm_head = true;
            llm_indicators += 1;
        }

        for pattern in llm_patterns {
            if lower.contains(pattern) {
                llm_indicators += 1;
            }
        }

        for pattern in diffusion_patterns {
            if lower.contains(pattern) {
                diffusion_indicators += 1;
            }
        }

        for pattern in embedding_patterns {
            if lower.contains(pattern) {
                embedding_indicators += 1;
            }
        }
    }

    // Determine type based on indicators
    // Embedding models often have transformer architecture but NO lm_head,
    // and may have pooler or projection layers
    let model_type = if diffusion_indicators > llm_indicators && diffusion_indicators > 5 {
        ModelType::Diffusion
    } else if llm_indicators > 5 {
        // Transformer-based model - is it LLM or embedding?
        if !has_lm_head && embedding_indicators > 0 {
            // Has transformer layers but no lm_head and has embedding patterns
            ModelType::Embedding
        } else if has_lm_head {
            ModelType::Llm
        } else {
            // Has transformer layers but no clear indicator - default to LLM
            // since embedding models typically have explicit pooling layers
            ModelType::Llm
        }
    } else {
        ModelType::Unknown
    };

    // Try to detect family
    let family = detect_family_from_tensors(&tensor_names, model_type);

    (model_type, family)
}

/// Detect model family from tensor names.
fn detect_family_from_tensors(tensor_names: &[&str], model_type: ModelType) -> Option<ModelFamily> {
    let names_str = tensor_names.join(" ").to_lowercase();

    match model_type {
        ModelType::Llm | ModelType::Embedding => {
            // Check for specific LLM/embedding architectures
            // These patterns work for both LLMs and embedding models based on the same architecture
            if names_str.contains("mistral") {
                Some(ModelFamily::new(ModelFamily::MISTRAL))
            } else if names_str.contains("gemma") {
                Some(ModelFamily::new(ModelFamily::GEMMA))
            } else if names_str.contains("phi") {
                Some(ModelFamily::new(ModelFamily::PHI))
            } else if names_str.contains("qwen") {
                Some(ModelFamily::new(ModelFamily::QWEN))
            } else if names_str.contains("falcon") {
                Some(ModelFamily::new(ModelFamily::FALCON))
            } else if names_str.contains("bert") {
                Some(ModelFamily::new("bert"))
            } else if names_str.contains("llama") || names_str.contains("rotary") {
                Some(ModelFamily::new(ModelFamily::LLAMA))
            } else {
                None
            }
        }
        ModelType::Diffusion => {
            if names_str.contains("sdxl") || names_str.contains("sd_xl") {
                Some(ModelFamily::new(ModelFamily::SDXL))
            } else if names_str.contains("flux") {
                Some(ModelFamily::new(ModelFamily::FLUX))
            } else if names_str.contains("kolors") {
                Some(ModelFamily::new(ModelFamily::KOLORS))
            } else if names_str.contains("pixart") {
                Some(ModelFamily::new(ModelFamily::PIXART))
            } else if names_str.contains("stable_diffusion") || names_str.contains("unet") {
                Some(ModelFamily::new(ModelFamily::STABLE_DIFFUSION))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extract full GGUF metadata as a map.
///
/// This is more expensive than just getting architecture but
/// provides all available metadata.
pub fn extract_gguf_metadata(path: impl AsRef<Path>) -> Result<HashMap<String, String>> {
    let path = path.as_ref();
    let mut file = std::fs::File::open(path).map_err(|e| PumasError::io_with_path(e, path))?;

    let mut header = [0u8; 24];
    file.read_exact(&mut header)
        .map_err(|e| PumasError::io_with_path(e, path))?;

    if &header[..4] != magic::GGUF {
        return Err(PumasError::InvalidFileType {
            expected: "GGUF".to_string(),
            actual: "unknown".to_string(),
        });
    }

    let metadata_count = u64::from_le_bytes(header[16..24].try_into().unwrap()) as usize;

    let mut metadata = HashMap::new();

    // Limit iterations to prevent infinite loops
    for _ in 0..std::cmp::min(metadata_count, 1000) {
        let key = match read_gguf_string(&mut file) {
            Ok(k) => k,
            Err(_) => break,
        };

        let mut type_buf = [0u8; 4];
        if file.read_exact(&mut type_buf).is_err() {
            break;
        }
        let value_type = u32::from_le_bytes(type_buf);

        // Only extract string values for now
        if value_type == 8 {
            if let Ok(value) = read_gguf_string(&mut file) {
                metadata.insert(key, value);
            }
        } else if skip_gguf_value(&mut file, value_type).is_err() {
            break;
        }
    }

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_format_safetensors() {
        // Valid safetensors header: 8 byte size + JSON
        let header: Vec<u8> = {
            let json = b"{}";
            let size = json.len() as u64;
            let mut h = size.to_le_bytes().to_vec();
            h.extend_from_slice(json);
            h
        };
        assert_eq!(detect_format(&header, "safetensors"), FileFormat::Safetensors);
    }

    #[test]
    fn test_detect_format_gguf() {
        let mut header = vec![0u8; 64];
        header[..4].copy_from_slice(magic::GGUF);
        assert_eq!(detect_format(&header, "gguf"), FileFormat::Gguf);
    }

    #[test]
    fn test_detect_format_zip() {
        let mut header = vec![0u8; 64];
        header[..4].copy_from_slice(magic::ZIP);
        assert_eq!(detect_format(&header, "pt"), FileFormat::Pickle);
    }

    #[test]
    fn test_detect_format_extension_fallback() {
        let header = vec![0u8; 64];
        assert_eq!(detect_format(&header, "onnx"), FileFormat::Onnx);
        assert_eq!(detect_format(&header, "gguf"), FileFormat::Gguf);
    }

    #[test]
    fn test_model_type_info_default() {
        let info = ModelTypeInfo::default();
        assert_eq!(info.format, FileFormat::Unknown);
        assert_eq!(info.model_type, ModelType::Unknown);
        assert!(info.family.is_none());
    }
}
