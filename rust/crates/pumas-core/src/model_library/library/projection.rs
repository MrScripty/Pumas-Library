use super::*;

/// Non-mutating report for SQLite metadata projection cleanup.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct MetadataProjectionCleanupDryRunReport {
    pub generated_at: String,
    pub total_models: usize,
    pub models_with_cleanup: usize,
    pub total_removed_field_count: usize,
    pub before_payload_bytes: usize,
    pub after_payload_bytes: usize,
    pub payload_size_reduction_bytes: usize,
    pub removed_field_counts: std::collections::BTreeMap<String, usize>,
    pub preserved_exception_fields: Vec<String>,
    pub items: Vec<MetadataProjectionCleanupDryRunItem>,
}

/// Per-model row for SQLite metadata projection cleanup reporting.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct MetadataProjectionCleanupDryRunItem {
    pub model_id: String,
    pub removed_fields: Vec<String>,
    pub preserved_exception_fields: Vec<String>,
    pub before_payload_bytes: usize,
    pub after_payload_bytes: usize,
    pub payload_size_reduction_bytes: usize,
}

/// Convert `ModelMetadata` into the canonical index row projection.
pub(super) fn metadata_to_record(
    model_id: &str,
    model_dir: &Path,
    metadata: &ModelMetadata,
) -> ModelRecord {
    let inferred_type_from_id = model_id
        .split('/')
        .next()
        .map(str::to_string)
        .unwrap_or_else(|| "unknown".to_string());
    let (download_incomplete, download_has_part_files, download_missing_expected_files) =
        download_projection_status(model_dir, metadata);
    let mut metadata_json = serde_json::to_value(metadata).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = metadata_json.as_object_mut() {
        obj.insert(
            "download_incomplete".to_string(),
            Value::Bool(download_incomplete),
        );
        obj.insert(
            "download_has_part_files".to_string(),
            Value::Bool(download_has_part_files),
        );
        obj.insert(
            "download_missing_expected_files".to_string(),
            Value::Number(serde_json::Number::from(
                download_missing_expected_files as u64,
            )),
        );
        obj.insert(
            PRIMARY_FORMAT_METADATA_KEY.to_string(),
            derive_primary_format_value(obj),
        );
        obj.insert(
            QUANTIZATION_METADATA_KEY.to_string(),
            derive_quantization_value(obj),
        );
        cleanup_metadata_projection_fields(obj);
    }

    ModelRecord {
        id: model_id.to_string(),
        path: model_dir.display().to_string(),
        cleaned_name: metadata.cleaned_name.clone().unwrap_or_else(|| {
            model_id
                .split('/')
                .next_back()
                .unwrap_or(model_id)
                .to_string()
        }),
        official_name: metadata
            .official_name
            .clone()
            .unwrap_or_else(|| model_id.to_string()),
        model_type: metadata.model_type.clone().unwrap_or(inferred_type_from_id),
        tags: metadata.tags.clone().unwrap_or_default(),
        hashes: metadata
            .hashes
            .as_ref()
            .map(|h| {
                let mut map = HashMap::new();
                if let Some(ref sha) = h.sha256 {
                    map.insert("sha256".to_string(), sha.clone());
                }
                if let Some(ref blake) = h.blake3 {
                    map.insert("blake3".to_string(), blake.clone());
                }
                map
            })
            .unwrap_or_default(),
        metadata: metadata_json,
        updated_at: projected_record_updated_at(model_dir, metadata),
    }
}

fn projected_record_updated_at(model_dir: &Path, metadata: &ModelMetadata) -> String {
    metadata
        .updated_date
        .clone()
        .or_else(|| latest_filesystem_timestamp(model_dir))
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339())
}

fn latest_filesystem_timestamp(model_dir: &Path) -> Option<String> {
    let newest = WalkDir::new(model_dir)
        .min_depth(0)
        .max_depth(2)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.metadata().ok())
        .filter_map(|metadata| metadata.modified().ok())
        .max()?;
    Some(chrono::DateTime::<chrono::Utc>::from(newest).to_rfc3339())
}

fn latest_payload_filesystem_timestamp(model_dir: &Path) -> Option<String> {
    let newest = WalkDir::new(model_dir)
        .min_depth(0)
        .max_depth(2)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return false;
            };
            !matches!(
                name,
                METADATA_FILENAME
                    | OVERRIDES_FILENAME
                    | "metadata.json.bak"
                    | "metadata.json.tmp"
                    | "overrides.json.bak"
                    | "overrides.json.tmp"
            ) && !name.starts_with("metadata.json.")
                && !name.starts_with("overrides.json.")
        })
        .filter_map(|entry| entry.metadata().ok())
        .filter_map(|metadata| metadata.modified().ok())
        .max()?;
    Some(chrono::DateTime::<chrono::Utc>::from(newest).to_rfc3339())
}

pub(super) fn payload_filesystem_is_newer(model_dir: &Path, indexed_updated_at: &str) -> bool {
    let Some(filesystem_updated_at) = latest_payload_filesystem_timestamp(model_dir) else {
        return false;
    };
    let Ok(indexed_updated_at) = chrono::DateTime::parse_from_rfc3339(indexed_updated_at) else {
        return false;
    };
    let Ok(filesystem_updated_at) = chrono::DateTime::parse_from_rfc3339(&filesystem_updated_at)
    else {
        return false;
    };
    filesystem_updated_at > indexed_updated_at
}

pub(super) fn project_display_fields_for_record(record: &mut ModelRecord) {
    if !record.metadata.is_object() {
        record.metadata = Value::Object(Default::default());
    }

    let Some(obj) = record.metadata.as_object_mut() else {
        return;
    };

    obj.insert(
        PRIMARY_FORMAT_METADATA_KEY.to_string(),
        derive_primary_format_value(obj),
    );
    obj.insert(
        QUANTIZATION_METADATA_KEY.to_string(),
        derive_quantization_value(obj),
    );
}

fn derive_primary_format_value(metadata: &serde_json::Map<String, Value>) -> Value {
    derive_primary_format(metadata)
        .map(Value::String)
        .unwrap_or(Value::Null)
}

fn derive_quantization_value(metadata: &serde_json::Map<String, Value>) -> Value {
    derive_quantization(metadata)
        .map(Value::String)
        .unwrap_or(Value::Null)
}

const COLUMN_OWNED_METADATA_FIELDS: &[&str] = &[
    "model_id",
    "model_type",
    "cleaned_name",
    "official_name",
    "hashes",
    "tags",
];

const NON_MEANINGFUL_METADATA_PROJECTION_FIELDS: &[&str] = &[
    "compatible_apps",
    "conversion_source",
    "last_lookup_attempt",
    "license_artifact",
    "model_card_artifact",
    "reviewed_at",
    "reviewed_by",
    "subtype",
];

fn cleanup_metadata_projection_fields(metadata: &mut serde_json::Map<String, Value>) {
    for field in COLUMN_OWNED_METADATA_FIELDS {
        metadata.remove(*field);
    }
    for field in NON_MEANINGFUL_METADATA_PROJECTION_FIELDS {
        metadata.remove(*field);
    }
    if metadata
        .get("validation_errors")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        metadata.remove("validation_errors");
    }
}

pub(super) fn metadata_projection_cleanup_dry_run_report(
    records: &[ModelRecord],
) -> MetadataProjectionCleanupDryRunReport {
    let mut report = MetadataProjectionCleanupDryRunReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        total_models: records.len(),
        ..Default::default()
    };
    let mut preserved_exception_fields = BTreeSet::new();

    for record in records {
        let item = metadata_projection_cleanup_dry_run_item(record);
        report.before_payload_bytes += item.before_payload_bytes;
        report.after_payload_bytes += item.after_payload_bytes;
        report.payload_size_reduction_bytes += item.payload_size_reduction_bytes;
        for field in &item.preserved_exception_fields {
            preserved_exception_fields.insert(field.clone());
        }
        for field in &item.removed_fields {
            *report
                .removed_field_counts
                .entry(field.clone())
                .or_default() += 1;
        }
        report.total_removed_field_count += item.removed_fields.len();
        if !item.removed_fields.is_empty() {
            report.models_with_cleanup += 1;
            report.items.push(item);
        }
    }

    report.preserved_exception_fields = preserved_exception_fields.into_iter().collect();
    report
        .items
        .sort_by(|left, right| left.model_id.cmp(&right.model_id));
    report
}

fn metadata_projection_cleanup_dry_run_item(
    record: &ModelRecord,
) -> MetadataProjectionCleanupDryRunItem {
    let before_payload_bytes = serde_json::to_vec(&record.metadata)
        .map(|payload| payload.len())
        .unwrap_or_default();
    let mut after = record.metadata.clone();
    let mut removed_fields = Vec::new();
    let mut preserved_exception_fields = Vec::new();

    if let Some(after_obj) = after.as_object_mut() {
        let before_keys = after_obj.keys().cloned().collect::<BTreeSet<_>>();
        cleanup_metadata_projection_fields(after_obj);
        let after_keys = after_obj.keys().cloned().collect::<BTreeSet<_>>();
        removed_fields = before_keys.difference(&after_keys).cloned().collect();
        preserved_exception_fields = METADATA_PROJECTION_CLEANUP_EXCEPTION_FIELDS
            .iter()
            .filter(|field| after_obj.contains_key(**field))
            .map(|field| (*field).to_string())
            .collect();
    }

    let after_payload_bytes = serde_json::to_vec(&after)
        .map(|payload| payload.len())
        .unwrap_or(before_payload_bytes);

    MetadataProjectionCleanupDryRunItem {
        model_id: record.id.clone(),
        removed_fields,
        preserved_exception_fields,
        before_payload_bytes,
        after_payload_bytes,
        payload_size_reduction_bytes: before_payload_bytes.saturating_sub(after_payload_bytes),
    }
}

const METADATA_PROJECTION_CLEANUP_EXCEPTION_FIELDS: &[&str] = &[
    "license",
    "license_status",
    "model_card",
    "notes",
    "preview_image",
];

fn derive_primary_format(metadata: &serde_json::Map<String, Value>) -> Option<String> {
    conversion_source_format(metadata)
        .or_else(|| detect_format_from_file_entries(metadata.get("files")))
        .or_else(|| detect_format_from_bundle_entry_path(metadata))
        .or_else(|| detect_format_from_string_list(metadata.get("expected_files")))
        .or_else(|| detect_format_from_string_list(metadata.get("tags")))
}

fn derive_quantization(metadata: &serde_json::Map<String, Value>) -> Option<String> {
    conversion_source_quant(metadata)
        .or_else(|| detect_quant_from_file_entries(metadata.get("files")))
        .or_else(|| detect_quant_from_string_list(metadata.get("expected_files")))
        .or_else(|| {
            metadata
                .get("official_name")
                .and_then(Value::as_str)
                .and_then(extract_quant_token)
        })
        .or_else(|| {
            metadata
                .get("cleaned_name")
                .and_then(Value::as_str)
                .and_then(extract_quant_token)
        })
}

fn conversion_source_format(metadata: &serde_json::Map<String, Value>) -> Option<String> {
    let conversion = metadata.get("conversion_source")?.as_object()?;
    conversion
        .get("target_format")
        .and_then(Value::as_str)
        .or_else(|| conversion.get("source_format").and_then(Value::as_str))
        .map(normalize_format_token)
        .filter(|value| value != "unknown")
}

fn conversion_source_quant(metadata: &serde_json::Map<String, Value>) -> Option<String> {
    let conversion = metadata.get("conversion_source")?.as_object()?;
    conversion
        .get("target_quant")
        .and_then(Value::as_str)
        .or_else(|| conversion.get("source_quant").and_then(Value::as_str))
        .and_then(extract_quant_token)
}

fn detect_format_from_file_entries(files_value: Option<&Value>) -> Option<String> {
    let files = files_value?.as_array()?;
    let mut weighted = Vec::new();

    for entry in files {
        let Some(file) = entry.as_object() else {
            continue;
        };
        let size = file.get("size").and_then(Value::as_u64).unwrap_or(0);
        for field in ["name", "original_name"] {
            if let Some(name) = file.get(field).and_then(Value::as_str) {
                if let Some(format) = detect_format_from_name(name) {
                    weighted.push((size, format));
                    break;
                }
            }
        }
    }

    weighted.sort_by(|left, right| right.0.cmp(&left.0));
    weighted.into_iter().next().map(|(_, format)| format)
}

fn detect_format_from_bundle_entry_path(
    metadata: &serde_json::Map<String, Value>,
) -> Option<String> {
    let bundle_format = metadata.get("bundle_format")?.as_str()?;
    if bundle_format != "diffusers_directory" {
        return None;
    }

    let entry_path = metadata
        .get("entry_path")
        .and_then(Value::as_str)
        .or_else(|| metadata.get("source_path").and_then(Value::as_str))?;

    detect_format_from_directory_walk(Path::new(entry_path))
}

fn detect_format_from_directory_walk(root: &Path) -> Option<String> {
    if !root.is_dir() {
        return None;
    }

    let mut weighted = Vec::new();
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(file_name) = entry.path().file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(format) = detect_format_from_name(file_name) else {
            continue;
        };
        let size = entry
            .metadata()
            .ok()
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        weighted.push((size, format));
    }

    weighted.sort_by(|left, right| right.0.cmp(&left.0));
    weighted.into_iter().next().map(|(_, format)| format)
}

pub(super) fn canonicalize_display_path(path: &str) -> String {
    let canonical = Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path));
    path_to_display_string(&canonical)
}

#[cfg(windows)]
fn path_to_display_string(path: &Path) -> String {
    crate::platform::platform_display_path(path)
}

#[cfg(not(windows))]
fn path_to_display_string(path: &Path) -> String {
    path.display().to_string()
}

fn detect_quant_from_file_entries(files_value: Option<&Value>) -> Option<String> {
    let files = files_value?.as_array()?;
    let mut weighted = Vec::new();

    for entry in files {
        let Some(file) = entry.as_object() else {
            continue;
        };
        let size = file.get("size").and_then(Value::as_u64).unwrap_or(0);
        for field in ["name", "original_name"] {
            if let Some(name) = file.get(field).and_then(Value::as_str) {
                if detect_format_from_name(name).is_none() {
                    continue;
                }
                if let Some(quant) = extract_quant_token(name) {
                    weighted.push((size, quant));
                    break;
                }
            }
        }
    }

    weighted.sort_by(|left, right| right.0.cmp(&left.0));
    weighted.into_iter().next().map(|(_, quant)| quant)
}

fn detect_format_from_string_list(values: Option<&Value>) -> Option<String> {
    values?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find_map(detect_format_from_name)
}

fn detect_format_from_name(name: &str) -> Option<String> {
    let normalized_name = strip_download_suffix(name);
    let ext = Path::new(normalized_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(normalize_format_token)?;
    canonical_weight_format(&ext).map(str::to_string)
}

fn normalize_format_token(value: &str) -> String {
    value.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn canonical_weight_format(value: &str) -> Option<&'static str> {
    match value {
        "gguf" => Some("gguf"),
        "ggml" => Some("ggml"),
        "safetensors" => Some("safetensors"),
        "onnx" => Some("onnx"),
        "pt" | "pth" | "ckpt" | "bin" | "pickle" | "pkl" => Some("pickle"),
        _ => None,
    }
}

fn strip_download_suffix(name: &str) -> &str {
    name.strip_suffix(".part").unwrap_or(name)
}

fn detect_quant_from_string_list(values: Option<&Value>) -> Option<String> {
    values?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find_map(extract_quant_token)
}

fn extract_quant_token(value: &str) -> Option<String> {
    static QUANT_PATTERN: OnceLock<Option<regex::Regex>> = OnceLock::new();
    let regex = QUANT_PATTERN
        .get_or_init(|| {
            regex::Regex::new(
                r"(?i)(?:^|[._/\- ()])((?:UD-)?(?:IQ\d+_[A-Z0-9_]+|Q\d+_[A-Z0-9_]+)|fp16|fp32|fp8|bf16|f16|f32|int8|int4|nf4|nvfp4|mxfp4)(?:$|[._/\- )])",
            )
            .ok()
        })
        .as_ref()?;

    let captures = regex.captures(value)?;
    let token = captures.get(1)?.as_str();
    Some(normalize_quant_token(token))
}

fn normalize_quant_token(value: &str) -> String {
    let normalized = value.trim().to_ascii_uppercase();
    match normalized.as_str() {
        "FP16" | "F16" => "F16".to_string(),
        "FP32" | "F32" => "F32".to_string(),
        "BF16" => "BF16".to_string(),
        "INT8" => "INT8".to_string(),
        "INT4" => "INT4".to_string(),
        _ => normalized,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_projection_removes_column_owned_duplicates() {
        let mut model_card = HashMap::new();
        model_card.insert("summary".to_string(), Value::String("kept".to_string()));
        let metadata = ModelMetadata {
            model_id: Some("llm/test/projection-cleanup".to_string()),
            model_type: Some("llm".to_string()),
            family: Some("test".to_string()),
            cleaned_name: Some("projection-cleanup".to_string()),
            official_name: Some("Projection Cleanup".to_string()),
            tags: Some(vec!["safetensors".to_string()]),
            files: Some(vec![crate::models::ModelFileInfo {
                name: "model.safetensors".to_string(),
                original_name: None,
                size: Some(4),
                sha256: None,
                blake3: None,
            }]),
            hashes: Some(crate::models::ModelHashes {
                sha256: Some("abc".to_string()),
                blake3: None,
            }),
            model_card: Some(model_card),
            notes: Some("kept notes".to_string()),
            preview_image: Some("preview.png".to_string()),
            license_status: Some("allowed".to_string()),
            ..Default::default()
        };

        let record = metadata_to_record(
            "llm/test/projection-cleanup",
            Path::new("/tmp/projection-cleanup"),
            &metadata,
        );
        let projected = record.metadata.as_object().unwrap();

        for key in [
            "model_id",
            "model_type",
            "cleaned_name",
            "official_name",
            "tags",
            "hashes",
        ] {
            assert!(!projected.contains_key(key), "{key} should be column-owned");
        }
        assert_eq!(record.tags, vec!["safetensors".to_string()]);
        assert_eq!(record.hashes.get("sha256").map(String::as_str), Some("abc"));
        assert!(projected.contains_key("model_card"));
        assert!(projected.contains_key("notes"));
        assert!(projected.contains_key("preview_image"));
        assert!(projected.contains_key("license_status"));
        assert_eq!(
            projected
                .get(PRIMARY_FORMAT_METADATA_KEY)
                .and_then(Value::as_str),
            Some("safetensors")
        );
    }

    #[test]
    fn metadata_projection_removes_non_meaningful_fields_but_keeps_exceptions() {
        let mut metadata = serde_json::json!({
            "compatible_apps": [],
            "conversion_source": {"target_format": "gguf"},
            "last_lookup_attempt": null,
            "license_artifact": {"path": "license.txt"},
            "model_card_artifact": {"path": "README.md"},
            "reviewed_at": "",
            "reviewed_by": "",
            "subtype": "",
            "validation_errors": [],
            "license_status": "allowed",
            "model_card": {"summary": "kept"},
            "notes": "kept",
            "preview_image": "preview.png"
        })
        .as_object()
        .cloned()
        .unwrap();

        cleanup_metadata_projection_fields(&mut metadata);

        for key in NON_MEANINGFUL_METADATA_PROJECTION_FIELDS {
            assert!(!metadata.contains_key(*key), "{key} should be removed");
        }
        assert!(!metadata.contains_key("validation_errors"));
        assert!(metadata.contains_key("license_status"));
        assert!(metadata.contains_key("model_card"));
        assert!(metadata.contains_key("notes"));
        assert!(metadata.contains_key("preview_image"));
    }
}
