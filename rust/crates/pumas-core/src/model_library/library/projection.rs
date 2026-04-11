use super::*;

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
    normalize_windows_path(path).display().to_string()
}

#[cfg(not(windows))]
fn path_to_display_string(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(windows)]
fn normalize_windows_path(path: &Path) -> PathBuf {
    expand_windows_long_path(&strip_windows_verbatim_prefix(path))
        .unwrap_or_else(|| strip_windows_verbatim_prefix(path))
}

#[cfg(windows)]
fn strip_windows_verbatim_prefix(path: &Path) -> PathBuf {
    let raw = path.display().to_string();
    if let Some(stripped) = raw.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{}", stripped))
    } else if let Some(stripped) = raw.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

#[cfg(windows)]
fn expand_windows_long_path(path: &Path) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetLongPathNameW;

    let input: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let required = unsafe { GetLongPathNameW(input.as_ptr(), std::ptr::null_mut(), 0) };
    if required == 0 {
        return None;
    }

    let mut buffer = vec![0u16; required as usize + 1];
    let written =
        unsafe { GetLongPathNameW(input.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) };
    if written == 0 {
        return None;
    }

    buffer.truncate(written as usize);
    Some(PathBuf::from(OsString::from_wide(&buffer)))
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
