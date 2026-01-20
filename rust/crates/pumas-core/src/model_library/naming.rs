//! Filesystem-safe name normalization.
//!
//! Ensures model names are safe for use as filenames across platforms.

use regex::Regex;
use std::sync::LazyLock;

/// Maximum length for normalized names.
const MAX_NAME_LENGTH: usize = 128;

/// Characters reserved on NTFS that must be removed.
const NTFS_RESERVED_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Reserved names on Windows NTFS.
const NTFS_RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Regex for consecutive underscores/hyphens.
static CONSECUTIVE_SEPARATORS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[-_]{2,}").unwrap());

/// Regex for non-alphanumeric characters (except - and _).
static NON_ALNUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-zA-Z0-9\-_]").unwrap());

/// Normalize a name for use as a filesystem-safe directory/file name.
///
/// # Rules Applied
/// 1. Convert to lowercase
/// 2. Replace spaces with underscores
/// 3. Remove NTFS-reserved characters
/// 4. Replace non-alphanumeric characters (except - and _) with underscore
/// 5. Collapse consecutive separators
/// 6. Trim leading/trailing separators
/// 7. Truncate to MAX_NAME_LENGTH
/// 8. Handle NTFS reserved names
/// 9. Ensure non-empty result
///
/// # Examples
///
/// ```
/// use pumas_core::model_library::normalize_name;
///
/// assert_eq!(normalize_name("Llama 2 7B"), "llama_2_7b");
/// assert_eq!(normalize_name("SDXL-1.0-Base"), "sdxl-1_0-base");
/// assert_eq!(normalize_name("model/test:file"), "modeltestfile");
/// ```
pub fn normalize_name(name: &str) -> String {
    let mut result = name.to_lowercase();

    // Replace spaces with underscores
    result = result.replace(' ', "_");

    // Remove NTFS-reserved characters
    for &c in NTFS_RESERVED_CHARS {
        result = result.replace(c, "");
    }

    // Replace non-alphanumeric (except - and _) with underscore
    result = NON_ALNUM.replace_all(&result, "_").to_string();

    // Collapse consecutive separators
    result = CONSECUTIVE_SEPARATORS.replace_all(&result, "_").to_string();

    // Trim leading/trailing separators
    result = result.trim_matches(|c| c == '-' || c == '_').to_string();

    // Truncate to max length (preserve whole words if possible)
    if result.len() > MAX_NAME_LENGTH {
        result = result[..MAX_NAME_LENGTH].to_string();
        // Try to break at a separator
        if let Some(pos) = result.rfind(|c| c == '-' || c == '_') {
            if pos > MAX_NAME_LENGTH / 2 {
                result = result[..pos].to_string();
            }
        }
        result = result.trim_matches(|c| c == '-' || c == '_').to_string();
    }

    // Handle NTFS reserved names
    let upper = result.to_uppercase();
    if NTFS_RESERVED_NAMES.contains(&upper.as_str()) {
        result = format!("{}_model", result);
    }

    // Ensure non-empty
    if result.is_empty() {
        result = "unnamed_model".to_string();
    }

    result
}

/// Normalize a filename while preserving its extension.
///
/// # Examples
///
/// ```ignore
/// use pumas_core::model_library::naming::normalize_filename;
///
/// assert_eq!(normalize_filename("My Model.safetensors"), "my_model.safetensors");
/// assert_eq!(normalize_filename("test/file.gguf"), "testfile.gguf");
/// ```
pub fn normalize_filename(filename: &str) -> String {
    // Split into name and extension
    if let Some(dot_pos) = filename.rfind('.') {
        let name = &filename[..dot_pos];
        let ext = &filename[dot_pos..];

        // Normalize the name part, keep extension lowercase
        format!("{}{}", normalize_name(name), ext.to_lowercase())
    } else {
        normalize_name(filename)
    }
}

/// Extract base name from a filename, removing quantization suffixes.
///
/// This is used for HuggingFace metadata lookup to find the base model
/// when given a quantized variant.
///
/// # Examples
///
/// ```ignore
/// use pumas_core::model_library::naming::extract_base_name;
///
/// assert_eq!(extract_base_name("llama-2-7b-Q4_K_M.gguf"), "llama-2-7b");
/// assert_eq!(extract_base_name("model-00001-of-00005.safetensors"), "model");
/// ```
pub fn extract_base_name(filename: &str) -> String {
    let mut name = filename.to_string();

    // Remove extension
    if let Some(dot_pos) = name.rfind('.') {
        name = name[..dot_pos].to_string();
    }

    // Remove quantization suffixes (Q4_K_M, Q5_K_S, etc.)
    static QUANT_PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[-_](?i:Q\d+_[A-Z0-9_]+|fp16|fp32|bf16|f16|f32)$").unwrap());
    name = QUANT_PATTERN.replace(&name, "").to_string();

    // Remove shard suffixes (-00001-of-00005)
    static SHARD_PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[-_]?\d{5}-of-\d{5}$").unwrap());
    name = SHARD_PATTERN.replace(&name, "").to_string();

    // Remove common variant suffixes
    static VARIANT_PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[-_](?i:gguf|safetensors|pt|bin|onnx)$").unwrap());
    name = VARIANT_PATTERN.replace(&name, "").to_string();

    name.trim_matches(|c| c == '-' || c == '_').to_string()
}

/// Detect if a filename is part of a sharded model.
///
/// Returns the shard index and total count if detected.
///
/// # Examples
///
/// ```ignore
/// use pumas_core::model_library::naming::detect_shard;
///
/// assert_eq!(detect_shard("model-00001-of-00005.safetensors"), Some((1, 5)));
/// assert_eq!(detect_shard("model.gguf"), None);
/// ```
pub fn detect_shard(filename: &str) -> Option<(u32, u32)> {
    static SHARD_PATTERN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(\d{5})-of-(\d{5})").unwrap());

    if let Some(caps) = SHARD_PATTERN.captures(filename) {
        let index: u32 = caps.get(1)?.as_str().parse().ok()?;
        let total: u32 = caps.get(2)?.as_str().parse().ok()?;
        Some((index, total))
    } else {
        None
    }
}

/// Group files by their shard base name.
///
/// Returns a map of base name -> list of files.
pub fn group_sharded_files(files: &[String]) -> std::collections::HashMap<String, Vec<String>> {
    let mut groups: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

    for file in files {
        let base = extract_base_name(file);
        groups.entry(base).or_default().push(file.clone());
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_name_basic() {
        assert_eq!(normalize_name("Llama 2 7B"), "llama_2_7b");
        assert_eq!(normalize_name("SDXL-1.0-Base"), "sdxl-1_0-base");
        assert_eq!(normalize_name("stable_diffusion_v1.5"), "stable_diffusion_v1_5");
    }

    #[test]
    fn test_normalize_name_special_chars() {
        // After removing special chars and normalizing, we get collapsed results
        assert_eq!(normalize_name("model/test:file"), "modeltestfile");
        assert_eq!(normalize_name("model<>test"), "modeltest");
        assert_eq!(normalize_name("test|model"), "testmodel");
    }

    #[test]
    fn test_normalize_name_consecutive_separators() {
        assert_eq!(normalize_name("test---model"), "test_model");
        assert_eq!(normalize_name("test___model"), "test_model");
        assert_eq!(normalize_name("test-_-model"), "test_model");
    }

    #[test]
    fn test_normalize_name_trim() {
        assert_eq!(normalize_name("--test--"), "test");
        assert_eq!(normalize_name("__model__"), "model");
    }

    #[test]
    fn test_normalize_name_reserved() {
        assert_eq!(normalize_name("CON"), "con_model");
        assert_eq!(normalize_name("nul"), "nul_model");
    }

    #[test]
    fn test_normalize_name_empty() {
        assert_eq!(normalize_name(""), "unnamed_model");
        assert_eq!(normalize_name("---"), "unnamed_model");
    }

    #[test]
    fn test_normalize_filename() {
        assert_eq!(normalize_filename("My Model.safetensors"), "my_model.safetensors");
        assert_eq!(normalize_filename("TEST.GGUF"), "test.gguf");
    }

    #[test]
    fn test_extract_base_name() {
        assert_eq!(extract_base_name("llama-2-7b-Q4_K_M.gguf"), "llama-2-7b");
        assert_eq!(extract_base_name("model-fp16.safetensors"), "model");
        assert_eq!(
            extract_base_name("model-00001-of-00005.safetensors"),
            "model"
        );
    }

    #[test]
    fn test_detect_shard() {
        assert_eq!(detect_shard("model-00001-of-00005.safetensors"), Some((1, 5)));
        assert_eq!(detect_shard("model-00003-of-00010.bin"), Some((3, 10)));
        assert_eq!(detect_shard("model.gguf"), None);
    }

    #[test]
    fn test_long_name_truncation() {
        let long_name = "a".repeat(200);
        let normalized = normalize_name(&long_name);
        assert!(normalized.len() <= MAX_NAME_LENGTH);
    }
}
