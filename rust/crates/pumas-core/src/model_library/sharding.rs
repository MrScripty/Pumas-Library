//! Sharded model set detection and validation.
//!
//! Large models are often split into multiple "shard" files for easier distribution.
//! This module detects and groups sharded files, and validates completeness.
//!
//! # Supported Patterns
//!
//! 1. **With total count**: `model-00001-of-00005.safetensors`
//! 2. **Part suffix**: `model.safetensors.part1`
//! 3. **Numeric suffix**: `model_00001.safetensors`

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

/// Result of shard completeness validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardValidation {
    /// Whether the shard set is complete
    pub is_complete: bool,
    /// Total number of expected shards (from filename pattern)
    pub total_shards: usize,
    /// Indices of shards that were found
    pub found_shards: Vec<usize>,
    /// Indices of shards that are missing
    pub missing_shards: Vec<usize>,
    /// Error message if validation failed
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
}

// Regex patterns for detecting sharded files
// Using LazyLock for thread-safe lazy initialization

/// Pattern 1: model-00001-of-00005.safetensors
/// Captures: (base_name, shard_index, total_count, extension)
static PATTERN_WITH_TOTAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+)-(\d+)-of-(\d+)(\.[^.]+)$").unwrap());

/// Pattern 2: model.safetensors.part1
/// Captures: (base_name_with_extension, part_number)
static PATTERN_PART_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+\.[^.]+)\.part(\d+)$").unwrap());

/// Pattern 3: model_00001.safetensors
/// Captures: (base_name, shard_index, extension)
static PATTERN_NUMERIC_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+)_(\d{5})(\.[^.]+)$").unwrap());

/// Pattern for extracting shard info for validation
static VALIDATION_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-(\d+)-of-(\d+)\.").unwrap());

/// Detect and group sharded model files.
///
/// Common patterns:
/// - `model-00001-of-00005.safetensors`, `model-00002-of-00005.safetensors`, ...
/// - `pytorch_model-00001-of-00003.bin`, `pytorch_model-00002-of-00003.bin`, ...
/// - `model.safetensors.part1`, `model.safetensors.part2`, ...
/// - `model_00001.safetensors`, `model_00002.safetensors`, ...
///
/// # Arguments
///
/// * `files` - List of file paths to analyze
///
/// # Returns
///
/// HashMap mapping base name to list of shard files.
/// Single files that don't match any pattern are included as single-item groups.
///
/// # Example
///
/// ```
/// use std::path::PathBuf;
/// use pumas_library::model_library::sharding::detect_sharded_sets;
///
/// let files = vec![
///     PathBuf::from("model-00001-of-00003.safetensors"),
///     PathBuf::from("model-00002-of-00003.safetensors"),
///     PathBuf::from("model-00003-of-00003.safetensors"),
///     PathBuf::from("standalone.gguf"),
/// ];
///
/// let groups = detect_sharded_sets(&files);
/// assert_eq!(groups.get("model.safetensors").map(|v| v.len()), Some(3));
/// assert_eq!(groups.get("standalone.gguf").map(|v| v.len()), Some(1));
/// ```
pub fn detect_sharded_sets(files: &[PathBuf]) -> HashMap<String, Vec<PathBuf>> {
    let mut sharded_groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut standalone_files: Vec<PathBuf> = Vec::new();

    for file_path in files {
        let filename = match file_path.file_name().and_then(|s| s.to_str()) {
            Some(name) => name,
            None => continue,
        };

        // Try pattern 1: model-00001-of-00005.ext
        if let Some(caps) = PATTERN_WITH_TOTAL.captures(filename) {
            let base_name = &caps[1];
            let ext = &caps[4];
            let group_key = format!("{}{}", base_name, ext);

            sharded_groups
                .entry(group_key)
                .or_default()
                .push(file_path.clone());
            continue;
        }

        // Try pattern 2: model.ext.part1
        if let Some(caps) = PATTERN_PART_SUFFIX.captures(filename) {
            let base_name = caps[1].to_string();

            sharded_groups
                .entry(base_name)
                .or_default()
                .push(file_path.clone());
            continue;
        }

        // Try pattern 3: model_00001.ext
        if let Some(caps) = PATTERN_NUMERIC_SUFFIX.captures(filename) {
            let base_name = &caps[1];
            let ext = &caps[3];
            let group_key = format!("{}{}", base_name, ext);

            sharded_groups
                .entry(group_key)
                .or_default()
                .push(file_path.clone());
            continue;
        }

        // No pattern matched - standalone file
        standalone_files.push(file_path.clone());
    }

    // Filter out groups with only one file (false positives)
    let mut filtered_groups: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for (key, mut files_list) in sharded_groups {
        // Sort by filename for consistent ordering
        files_list.sort_by(|a, b| {
            a.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .cmp(b.file_name().and_then(|s| s.to_str()).unwrap_or(""))
        });

        if files_list.len() > 1 {
            // True sharded set
            filtered_groups.insert(key, files_list);
        } else {
            // False positive - treat as standalone
            standalone_files.extend(files_list);
        }
    }

    // Add standalone files as single-item groups
    for file_path in standalone_files {
        if let Some(filename) = file_path.file_name().and_then(|s| s.to_str()) {
            filtered_groups.insert(filename.to_string(), vec![file_path]);
        }
    }

    filtered_groups
}

/// Validate that a sharded set is complete.
///
/// Checks for the `-NNNNN-of-NNNNN.` pattern to determine expected total
/// and validates all shards are present.
///
/// # Arguments
///
/// * `shard_files` - List of shard files in the group
///
/// # Returns
///
/// [`ShardValidation`] with completeness information.
///
/// # Example
///
/// ```
/// use std::path::PathBuf;
/// use pumas_library::model_library::sharding::validate_shard_completeness;
///
/// let shards = vec![
///     PathBuf::from("model-00001-of-00003.safetensors"),
///     PathBuf::from("model-00002-of-00003.safetensors"),
///     // Missing shard 3
/// ];
///
/// let result = validate_shard_completeness(&shards);
/// assert!(!result.is_complete);
/// assert_eq!(result.missing_shards, vec![3]);
/// ```
pub fn validate_shard_completeness(shard_files: &[PathBuf]) -> ShardValidation {
    if shard_files.is_empty() {
        return ShardValidation {
            is_complete: false,
            total_shards: 0,
            found_shards: Vec::new(),
            missing_shards: Vec::new(),
            error: String::new(),
        };
    }

    // Extract shard indices from filenames
    let mut indices: Vec<usize> = Vec::new();
    let mut expected_total: Option<usize> = None;

    for file_path in shard_files {
        let filename = match file_path.file_name().and_then(|s| s.to_str()) {
            Some(name) => name,
            None => continue,
        };

        if let Some(caps) = VALIDATION_PATTERN.captures(filename) {
            let current_idx: usize = match caps[1].parse() {
                Ok(idx) => idx,
                Err(_) => continue,
            };
            let total_count: usize = match caps[2].parse() {
                Ok(count) => count,
                Err(_) => continue,
            };

            indices.push(current_idx);

            match expected_total {
                None => expected_total = Some(total_count),
                Some(existing) if existing != total_count => {
                    // Inconsistent total counts
                    return ShardValidation {
                        is_complete: false,
                        total_shards: existing,
                        found_shards: indices,
                        missing_shards: Vec::new(),
                        error: "Inconsistent shard counts in filenames".to_string(),
                    };
                }
                _ => {}
            }
        }
    }

    // If we couldn't determine expected total from patterns, assume complete
    let expected_total = match expected_total {
        Some(total) => total,
        None => {
            // Could not determine expected total - assume complete
            return ShardValidation {
                is_complete: true,
                total_shards: shard_files.len(),
                found_shards: (1..=shard_files.len()).collect(),
                missing_shards: Vec::new(),
                error: String::new(),
            };
        }
    };

    // Check for missing shards (shards are 1-indexed)
    let expected_indices: std::collections::HashSet<usize> = (1..=expected_total).collect();
    let found_indices: std::collections::HashSet<usize> = indices.iter().copied().collect();
    let mut missing_indices: Vec<usize> = expected_indices
        .difference(&found_indices)
        .copied()
        .collect();
    missing_indices.sort();

    let mut found_sorted: Vec<usize> = found_indices.into_iter().collect();
    found_sorted.sort();

    ShardValidation {
        is_complete: missing_indices.is_empty(),
        total_shards: expected_total,
        found_shards: found_sorted,
        missing_shards: missing_indices,
        error: String::new(),
    }
}

/// Extract shard information from a filename.
///
/// Returns `Some((base_name, shard_index, total_count))` if the filename
/// matches a known shard pattern, `None` otherwise.
///
/// The `total_count` may be `None` for patterns that don't include it
/// (like `.partN` or `_NNNNN` patterns).
pub fn extract_shard_info(filename: &str) -> Option<(String, usize, Option<usize>)> {
    // Try pattern 1: model-00001-of-00005.ext
    if let Some(caps) = PATTERN_WITH_TOTAL.captures(filename) {
        let base_name = format!("{}{}", &caps[1], &caps[4]);
        let shard_idx: usize = caps[2].parse().ok()?;
        let total: usize = caps[3].parse().ok()?;
        return Some((base_name, shard_idx, Some(total)));
    }

    // Try pattern 2: model.ext.part1
    if let Some(caps) = PATTERN_PART_SUFFIX.captures(filename) {
        let base_name = caps[1].to_string();
        let shard_idx: usize = caps[2].parse().ok()?;
        return Some((base_name, shard_idx, None));
    }

    // Try pattern 3: model_00001.ext
    if let Some(caps) = PATTERN_NUMERIC_SUFFIX.captures(filename) {
        let base_name = format!("{}{}", &caps[1], &caps[3]);
        let shard_idx: usize = caps[2].parse().ok()?;
        return Some((base_name, shard_idx, None));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_sharded_sets_pattern1() {
        // Pattern 1: model-00001-of-00005.safetensors
        let files = vec![
            PathBuf::from("/models/model-00001-of-00003.safetensors"),
            PathBuf::from("/models/model-00002-of-00003.safetensors"),
            PathBuf::from("/models/model-00003-of-00003.safetensors"),
        ];

        let groups = detect_sharded_sets(&files);

        assert_eq!(groups.len(), 1);
        let shards = groups.get("model.safetensors").unwrap();
        assert_eq!(shards.len(), 3);
    }

    #[test]
    fn test_detect_sharded_sets_pattern2() {
        // Pattern 2: model.safetensors.part1
        let files = vec![
            PathBuf::from("/models/model.safetensors.part1"),
            PathBuf::from("/models/model.safetensors.part2"),
            PathBuf::from("/models/model.safetensors.part3"),
        ];

        let groups = detect_sharded_sets(&files);

        assert_eq!(groups.len(), 1);
        let shards = groups.get("model.safetensors").unwrap();
        assert_eq!(shards.len(), 3);
    }

    #[test]
    fn test_detect_sharded_sets_pattern3() {
        // Pattern 3: model_00001.safetensors
        let files = vec![
            PathBuf::from("/models/model_00001.safetensors"),
            PathBuf::from("/models/model_00002.safetensors"),
        ];

        let groups = detect_sharded_sets(&files);

        assert_eq!(groups.len(), 1);
        let shards = groups.get("model.safetensors").unwrap();
        assert_eq!(shards.len(), 2);
    }

    #[test]
    fn test_detect_sharded_sets_mixed() {
        // Mix of sharded and standalone files
        let files = vec![
            PathBuf::from("/models/model-00001-of-00002.safetensors"),
            PathBuf::from("/models/model-00002-of-00002.safetensors"),
            PathBuf::from("/models/standalone.gguf"),
            PathBuf::from("/models/another.bin"),
        ];

        let groups = detect_sharded_sets(&files);

        assert_eq!(groups.len(), 3);
        assert_eq!(
            groups.get("model.safetensors").map(|v| v.len()),
            Some(2)
        );
        assert_eq!(groups.get("standalone.gguf").map(|v| v.len()), Some(1));
        assert_eq!(groups.get("another.bin").map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_detect_sharded_sets_single_file_not_grouped() {
        // Single file matching shard pattern should be treated as standalone
        let files = vec![
            PathBuf::from("/models/model-00001-of-00003.safetensors"),
            PathBuf::from("/models/other.gguf"),
        ];

        let groups = detect_sharded_sets(&files);

        // The single shard file should be standalone since it's the only one
        assert_eq!(groups.len(), 2);
        assert!(groups.contains_key("model-00001-of-00003.safetensors"));
        assert!(groups.contains_key("other.gguf"));
    }

    #[test]
    fn test_detect_sharded_sets_empty() {
        let files: Vec<PathBuf> = vec![];
        let groups = detect_sharded_sets(&files);
        assert!(groups.is_empty());
    }

    #[test]
    fn test_validate_shard_completeness_complete() {
        let shards = vec![
            PathBuf::from("model-00001-of-00003.safetensors"),
            PathBuf::from("model-00002-of-00003.safetensors"),
            PathBuf::from("model-00003-of-00003.safetensors"),
        ];

        let result = validate_shard_completeness(&shards);

        assert!(result.is_complete);
        assert_eq!(result.total_shards, 3);
        assert_eq!(result.found_shards, vec![1, 2, 3]);
        assert!(result.missing_shards.is_empty());
        assert!(result.error.is_empty());
    }

    #[test]
    fn test_validate_shard_completeness_missing() {
        let shards = vec![
            PathBuf::from("model-00001-of-00005.safetensors"),
            PathBuf::from("model-00003-of-00005.safetensors"),
            PathBuf::from("model-00005-of-00005.safetensors"),
        ];

        let result = validate_shard_completeness(&shards);

        assert!(!result.is_complete);
        assert_eq!(result.total_shards, 5);
        assert_eq!(result.found_shards, vec![1, 3, 5]);
        assert_eq!(result.missing_shards, vec![2, 4]);
    }

    #[test]
    fn test_validate_shard_completeness_inconsistent_totals() {
        let shards = vec![
            PathBuf::from("model-00001-of-00003.safetensors"),
            PathBuf::from("model-00002-of-00005.safetensors"), // Different total!
        ];

        let result = validate_shard_completeness(&shards);

        assert!(!result.is_complete);
        assert!(!result.error.is_empty());
        assert!(result.error.contains("Inconsistent"));
    }

    #[test]
    fn test_validate_shard_completeness_empty() {
        let shards: Vec<PathBuf> = vec![];
        let result = validate_shard_completeness(&shards);

        assert!(!result.is_complete);
        assert_eq!(result.total_shards, 0);
    }

    #[test]
    fn test_validate_shard_completeness_no_pattern() {
        // Files that don't match the validation pattern
        let shards = vec![
            PathBuf::from("model.safetensors.part1"),
            PathBuf::from("model.safetensors.part2"),
        ];

        let result = validate_shard_completeness(&shards);

        // Should assume complete since we can't determine expected total
        assert!(result.is_complete);
        assert_eq!(result.total_shards, 2);
    }

    #[test]
    fn test_extract_shard_info_pattern1() {
        let result = extract_shard_info("model-00003-of-00010.safetensors");
        assert!(result.is_some());

        let (base, idx, total) = result.unwrap();
        assert_eq!(base, "model.safetensors");
        assert_eq!(idx, 3);
        assert_eq!(total, Some(10));
    }

    #[test]
    fn test_extract_shard_info_pattern2() {
        let result = extract_shard_info("model.safetensors.part5");
        assert!(result.is_some());

        let (base, idx, total) = result.unwrap();
        assert_eq!(base, "model.safetensors");
        assert_eq!(idx, 5);
        assert_eq!(total, None);
    }

    #[test]
    fn test_extract_shard_info_pattern3() {
        let result = extract_shard_info("model_00007.safetensors");
        assert!(result.is_some());

        let (base, idx, total) = result.unwrap();
        assert_eq!(base, "model.safetensors");
        assert_eq!(idx, 7);
        assert_eq!(total, None);
    }

    #[test]
    fn test_extract_shard_info_no_match() {
        let result = extract_shard_info("regular_model.safetensors");
        assert!(result.is_none());
    }

    #[test]
    fn test_pytorch_model_pattern() {
        // Real-world pattern from HuggingFace
        let files = vec![
            PathBuf::from("pytorch_model-00001-of-00002.bin"),
            PathBuf::from("pytorch_model-00002-of-00002.bin"),
        ];

        let groups = detect_sharded_sets(&files);

        assert_eq!(groups.len(), 1);
        let shards = groups.get("pytorch_model.bin").unwrap();
        assert_eq!(shards.len(), 2);

        let validation = validate_shard_completeness(shards);
        assert!(validation.is_complete);
    }

    #[test]
    fn test_files_sorted_by_name() {
        // Ensure files are returned in sorted order
        let files = vec![
            PathBuf::from("/models/model-00003-of-00003.safetensors"),
            PathBuf::from("/models/model-00001-of-00003.safetensors"),
            PathBuf::from("/models/model-00002-of-00003.safetensors"),
        ];

        let groups = detect_sharded_sets(&files);
        let shards = groups.get("model.safetensors").unwrap();

        // Should be sorted by filename
        let filenames: Vec<&str> = shards
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();

        assert_eq!(
            filenames,
            vec![
                "model-00001-of-00003.safetensors",
                "model-00002-of-00003.safetensors",
                "model-00003-of-00003.safetensors"
            ]
        );
    }

    #[test]
    fn test_shard_validation_serialization() {
        let validation = ShardValidation {
            is_complete: false,
            total_shards: 5,
            found_shards: vec![1, 3, 5],
            missing_shards: vec![2, 4],
            error: String::new(),
        };

        let json = serde_json::to_string(&validation).unwrap();
        let deserialized: ShardValidation = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.is_complete, validation.is_complete);
        assert_eq!(deserialized.total_shards, validation.total_shards);
        assert_eq!(deserialized.found_shards, validation.found_shards);
        assert_eq!(deserialized.missing_shards, validation.missing_shards);
    }
}
