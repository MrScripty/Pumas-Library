//! Model qualification over held descriptors, separate from transport verification.
//! This bounded bridge accepts safe tensor representations; it never executes code.
use super::staging::VerifiedCopyInput;
use super::*;
use crate::model_library::{identifier::identify_model_descriptor, package_selection};
use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::{Read, Seek, SeekFrom},
};

const JSON_LIMIT: u64 = 16 * 1024 * 1024;
const AUXILIARIES: &[&str] = &["json", "txt", "md", "model", "tiktoken", "vocab", "merges"];

fn invalid(message: impl Into<String>) -> PumasError {
    PumasError::Validation {
        field: "import.acquired_package".into(),
        message: message.into(),
    }
}

pub(super) struct Qualification {
    pub info: ModelTypeInfo,
    pub diffusers: bool,
    pub directory: bool,
}

/// No logical path is interpreted as ambient filesystem authority.
pub(super) fn qualify(inputs: &mut [VerifiedCopyInput], primary: usize) -> Result<Qualification> {
    qualify_inner(inputs, primary, None)
}

pub(super) fn qualify_vision(
    inputs: &mut [VerifiedCopyInput],
    primary: usize,
    projector: &str,
) -> Result<Qualification> {
    // Roles come from the issued selection; GGUF metadata supplies only a
    // structural cross-check, never a compatibility or inference attestation.
    if inputs.len() != 2 {
        return Err(invalid("Vision import requires exactly two files"));
    }
    for (index, input) in inputs.iter_mut().enumerate() {
        input.file.seek(SeekFrom::Start(0))?;
        let architecture = super::acquired_vision_gguf::architecture(&mut input.file)?;
        let arch = architecture.as_str();
        let role_ok = if index == primary {
            !arch.is_empty() && arch != "clip"
        } else {
            input.receipt.path == projector && arch == "clip"
        };
        if !role_ok {
            return Err(invalid("Vision roles require GGUF v3 with a declared primary architecture and clip projector structure"));
        }
    }
    qualify_inner(inputs, primary, Some(projector))
}

fn qualify_inner(
    inputs: &mut [VerifiedCopyInput],
    primary: usize,
    projector: Option<&str>,
) -> Result<Qualification> {
    let paths: Vec<String> = inputs
        .iter()
        .map(|input| input.receipt.path.clone())
        .collect();
    let selected: BTreeSet<&str> = paths.iter().map(String::as_str).collect();
    let primary_path = Path::new(&paths[primary]);
    let extension = primary_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension != "gguf" && paths.len() > 256 {
        return Err(invalid("Acquired package exceeds 256-file limit"));
    }
    if extension == "onnx" {
        return super::acquired_onnx::qualify(inputs, primary);
    }
    let diffusers = extension == "safetensors" && selected.contains("model_index.json");
    let directory = diffusers || (extension == "safetensors" && inputs.len() > 1);
    // Keep existing GGUF intent compatibility while admitting genuine safe tensors.
    if !["gguf", "safetensors"].contains(&extension.as_str()) {
        return Err(invalid("Unsupported acquired model representation; bytes may still be acquired without model publication"));
    }
    let mut tensors = BTreeMap::new();
    let mut tensor_header_bytes = 0_u64;
    for input in inputs.iter_mut() {
        let path = Path::new(&input.receipt.path);
        let ext = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        input.file.seek(SeekFrom::Start(0))?;
        match ext.as_str() {
            "gguf"
                if extension == "gguf"
                    && (input.receipt.path == paths[primary]
                        || projector == Some(input.receipt.path.as_str())) =>
            {
                let mut magic = [0; 4];
                input.file.read_exact(&mut magic)?;
                if magic != *b"GGUF" {
                    return Err(PumasError::Validation {
                        field: "import.acquired".into(),
                        message: "Primary GGUF magic is invalid".into(),
                    });
                }
            }
            "safetensors" if extension == "safetensors" => {
                let (names, header_bytes) = validate_safetensors(&mut input.file)?;
                tensor_header_bytes = tensor_header_bytes.saturating_add(header_bytes);
                if tensor_header_bytes > JSON_LIMIT {
                    return Err(invalid(
                        "Selected safetensors headers exceed aggregate byte limit",
                    ));
                }
                tensors.insert(input.receipt.path.clone(), names);
            }
            ext if AUXILIARIES.contains(&ext) => {}
            _ => {
                return Err(invalid(format!(
                    "Unsupported package member {}",
                    input.receipt.path
                )))
            }
        }
        input.file.seek(SeekFrom::Start(0))?;
    }
    if extension == "gguf" {
        let info = identify_model_descriptor(&mut inputs[primary].file, primary_path)?;
        inputs[primary].file.seek(SeekFrom::Start(0))?;
        return Ok(Qualification {
            info,
            diffusers: false,
            directory: false,
        });
    }
    package_selection::validate_readers(&paths, diffusers, |path| {
        let input = inputs
            .iter()
            .find(|input| input.receipt.path == path)
            .ok_or_else(|| invalid("Missing selected package input"))?;
        let mut file = input.file.try_clone()?;
        file.seek(SeekFrom::Start(0))?;
        Ok(file)
    })?;
    // Parse all selected JSON documents through held inputs; malformed configs,
    // indexes and processor/tokenizer documents cannot qualify a package.
    let mut config_type = None;
    let mut documents = BTreeMap::new();
    let mut document_bytes = 0_u64;
    if directory {
        for input in inputs
            .iter_mut()
            .filter(|input| input.receipt.path.ends_with(".json"))
        {
            input.file.seek(SeekFrom::Start(0))?;
            let mut bytes = Vec::new();
            (&mut input.file)
                .take(JSON_LIMIT + 1)
                .read_to_end(&mut bytes)?;
            document_bytes = document_bytes.saturating_add(bytes.len() as u64);
            if document_bytes > JSON_LIMIT {
                return Err(invalid("Package JSON exceeds byte limit"));
            }
            let value: UniqueJson = serde_json::from_slice(&bytes)
                .map_err(|_| invalid(format!("Invalid package JSON {}", input.receipt.path)))?;
            if !value.0.is_object() {
                return Err(invalid("Package JSON must be an object"));
            }
            if has_custom_code(&value.0) {
                return Err(invalid("Custom model code requires the existing separately authorized execution policy; this bridge does not admit it"));
            }
            documents.insert(input.receipt.path.clone(), value.0);
            input.file.seek(SeekFrom::Start(0))?;
        }
        validate_tensor_indexes(&documents, &tensors)?;
        let nonempty = |path: &str| {
            inputs
                .iter()
                .any(|input| input.receipt.path == path && input.receipt.bytes > 0)
        };
        if diffusers {
            let index = documents
                .get("model_index.json")
                .ok_or_else(|| invalid("Missing Diffusers index"))?;
            let class = index
                .get("_class_name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let required: &[&str] = match class {
                "StableDiffusionPipeline" => {
                    &["unet", "vae", "text_encoder", "tokenizer", "scheduler"]
                }
                "StableDiffusionXLPipeline" => &[
                    "unet",
                    "vae",
                    "text_encoder",
                    "text_encoder_2",
                    "tokenizer",
                    "tokenizer_2",
                    "scheduler",
                ],
                _ => return Err(invalid("Unsupported acquired Diffusers pipeline")),
            };
            for component in required {
                let value = index.get(*component).ok_or_else(|| {
                    invalid(format!(
                        "Diffusers index omits required component {component}"
                    ))
                })?;
                if !crate::model_library::external_assets::is_diffusers_component_entry(value)
                    || crate::model_library::external_assets::is_optional_component_marker(value)
                {
                    return Err(invalid(format!(
                        "Invalid required Diffusers component {component}"
                    )));
                }
            }
            for (component, value) in index.as_object().unwrap() {
                if component.starts_with('_')
                    || !crate::model_library::external_assets::is_diffusers_component_entry(value)
                    || crate::model_library::external_assets::is_optional_component_marker(value)
                {
                    continue;
                }
                if !value.as_array().is_some_and(|parts| {
                    parts.len() == 2
                        && parts
                            .iter()
                            .all(|part| part.as_str().is_some_and(|name| !name.is_empty()))
                }) {
                    return Err(invalid(
                        "Diffusers component requires an exact library/class pair",
                    ));
                }
                let library = value[0].as_str().unwrap_or("");
                if !["diffusers", "transformers"].contains(&library) {
                    return Err(invalid(
                        "Custom Diffusers components are outside this bridge",
                    ));
                }
                if component.starts_with("tokenizer") {
                    require_tokenizer(&documents, &nonempty, &format!("{component}/"))?;
                } else if component == "scheduler" {
                    require_document(&documents, &format!("{component}/scheduler_config.json"))?;
                } else if component == "feature_extractor" || component == "processor" {
                    require_document(&documents, &format!("{component}/preprocessor_config.json"))?;
                } else {
                    require_document(&documents, &format!("{component}/config.json"))?;
                    let prefix = format!("{component}/");
                    if !selected
                        .iter()
                        .any(|path| path.starts_with(&prefix) && path.ends_with(".safetensors"))
                    {
                        return Err(invalid(format!(
                            "Missing safe tensor weights for component {component}"
                        )));
                    }
                }
            }
        } else {
            let config = require_document(&documents, "config.json")?;
            let model_type = config
                .get("model_type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            // A finite standard package contract. Adapters and additional families
            // can widen this only with their own completeness evidence.
            if ![
                "llama", "mistral", "qwen2", "qwen3", "bert", "roberta", "vit", "whisper",
            ]
            .contains(&model_type)
            {
                return Err(invalid(
                    "Unsupported Transformers config model_type for acquired package",
                ));
            }
            config_type = Some(model_type.to_owned());
            if model_type != "vit" {
                require_tokenizer(&documents, &nonempty, "")?;
            }
            if ["vit", "whisper"].contains(&model_type) || config.get("processor_class").is_some() {
                let path = if documents.contains_key("preprocessor_config.json") {
                    "preprocessor_config.json"
                } else {
                    "processor_config.json"
                };
                require_document(&documents, path)?;
            }
            if paths.iter().any(|path| path.contains('/')) {
                return Err(invalid(
                    "Nested Transformers packages are outside this bounded contract",
                ));
            }
        }
    }
    for input in inputs.iter_mut() {
        input.file.seek(SeekFrom::Start(0))?;
    }
    let mut info = identify_model_descriptor(&mut inputs[primary].file, primary_path)?;
    inputs[primary].file.seek(SeekFrom::Start(0))?;
    if let Some(model_type) = config_type {
        info.model_type = match model_type.as_str() {
            "vit" => ModelType::Vision,
            "whisper" => ModelType::Audio,
            "bert" | "roberta" => ModelType::Embedding,
            _ => ModelType::Llm,
        };
    }
    Ok(Qualification {
        info,
        diffusers,
        directory,
    })
}

fn require_document<'a>(
    documents: &'a BTreeMap<String, serde_json::Value>,
    path: &str,
) -> Result<&'a serde_json::Value> {
    documents
        .get(path)
        .filter(|value| value.as_object().is_some_and(|object| !object.is_empty()))
        .ok_or_else(|| invalid(format!("Missing or empty required configuration {path}")))
}

fn require_tokenizer(
    documents: &BTreeMap<String, serde_json::Value>,
    nonempty: &impl Fn(&str) -> bool,
    prefix: &str,
) -> Result<()> {
    require_document(documents, &format!("{prefix}tokenizer_config.json"))?;
    let value = require_document(documents, &format!("{prefix}tokenizer.json"))?;
    // Only the JSON vocabulary encodings whose closure can be checked here.
    // SentencePiece binary and tiktoken formats require their own validator.
    if value.get("version").and_then(serde_json::Value::as_str) != Some("1.0") {
        return Err(invalid("Unsupported tokenizer serialization version"));
    }
    let model = value
        .get("model")
        .ok_or_else(|| invalid("Missing tokenizer model"))?;
    let model_type = model
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if !["WordLevel", "WordPiece", "BPE"].contains(&model_type) {
        return Err(invalid(
            "Unsupported tokenizer model encoding for acquired package",
        ));
    }
    let vocab = model
        .get("vocab")
        .and_then(serde_json::Value::as_object)
        .filter(|map| !map.is_empty())
        .ok_or_else(|| invalid("Missing tokenizer vocabulary"))?;
    let mut ids = BTreeSet::new();
    for id in vocab.values() {
        let id = id
            .as_u64()
            .filter(|id| *id <= u32::MAX as u64)
            .ok_or_else(|| invalid("Invalid tokenizer vocabulary ID"))?;
        if !ids.insert(id) {
            return Err(invalid("Duplicate tokenizer vocabulary ID"));
        }
    }
    if model_type != "BPE" {
        let unk = model
            .get("unk_token")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| invalid("Missing tokenizer unknown token"))?;
        if !vocab.contains_key(unk) {
            return Err(invalid("Tokenizer unknown token is absent from vocabulary"));
        }
    } else {
        let merges = model
            .get("merges")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid("BPE tokenizer requires its merges array"))?;
        let mut selected = BTreeSet::new();
        for merge in merges {
            let pair: Vec<&str> = if let Some(text) = merge.as_str() {
                text.split(' ').collect()
            } else if let Some(parts) = merge
                .as_array()
                .filter(|parts| parts.len() == 2 && parts.iter().all(serde_json::Value::is_string))
            {
                parts.iter().filter_map(serde_json::Value::as_str).collect()
            } else {
                Vec::new()
            };
            if pair.len() != 2
                || pair
                    .iter()
                    .any(|token| token.is_empty() || !vocab.contains_key(*token))
                || !vocab.contains_key(&pair.concat())
                || !selected.insert((pair[0], pair[1]))
            {
                return Err(invalid("Invalid or unclosed BPE merge vocabulary"));
            }
        }
    }
    if !nonempty(&format!("{prefix}tokenizer.json")) {
        return Err(invalid("Missing tokenizer bytes"));
    }
    Ok(())
}

fn validate_tensor_indexes(
    documents: &BTreeMap<String, serde_json::Value>,
    tensors: &BTreeMap<String, BTreeSet<String>>,
) -> Result<()> {
    let mut indexed = BTreeSet::new();
    let mut index_groups = BTreeSet::new();
    for (path, value) in documents
        .iter()
        .filter(|(path, _)| path.ends_with(".safetensors.index.json"))
    {
        let map = value
            .get("weight_map")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| invalid("Missing weight map"))?;
        let prefix = path
            .rsplit_once('/')
            .map(|(parent, _)| format!("{parent}/"))
            .unwrap_or_default();
        if !index_groups.insert(prefix.clone()) {
            return Err(invalid("Multiple weight indexes in one package component"));
        }
        let mut covered: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (tensor, shard) in map {
            let filename = shard
                .as_str()
                .ok_or_else(|| invalid("Invalid shard name"))?;
            if filename.contains('/') || filename.contains('\\') {
                return Err(invalid(
                    "Weight index must refer to sibling files in its own component",
                ));
            }
            let shard = format!("{prefix}{filename}");
            if !tensors
                .get(&shard)
                .is_some_and(|names| names.contains(tensor))
            {
                return Err(invalid(format!(
                    "Index tensor {tensor} is missing from selected shard {shard}"
                )));
            }
            covered.entry(shard).or_default().insert(tensor.clone());
        }
        for (shard, names) in covered {
            if tensors.get(&shard) != Some(&names) {
                return Err(invalid("Index omits tensors from a selected shard"));
            }
            if !indexed.insert(shard) {
                return Err(invalid("Multiple indexes claim the same shard"));
            }
        }
    }
    let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for path in tensors.keys() {
        groups
            .entry(
                path.rsplit_once('/')
                    .map(|(parent, _)| parent)
                    .unwrap_or(""),
            )
            .or_default()
            .push(path);
    }
    for group in groups.values() {
        if group.len() > 1 && group.iter().any(|path| !indexed.contains(*path)) {
            return Err(invalid(
                "Multiple weight files in a component require one complete tensor index",
            ));
        }
    }
    Ok(())
}

fn has_custom_code(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .any(|(key, value)| key == "auto_map" || has_custom_code(value)),
        serde_json::Value::Array(values) => values.iter().any(has_custom_code),
        _ => false,
    }
}

/// The safetensors wire contract, bounded to byte-aligned standard dtypes.
/// Header inspection validates shape/offset closure without allocating tensor data.
/// https://github.com/huggingface/safetensors#format
fn validate_safetensors(file: &mut std::fs::File) -> Result<(BTreeSet<String>, u64)> {
    let mut length = [0; 8];
    file.read_exact(&mut length)?;
    let length = u64::from_le_bytes(length);
    let data_length = file
        .metadata()?
        .len()
        .checked_sub(8)
        .and_then(|len| len.checked_sub(length))
        .ok_or_else(|| invalid("Truncated safetensors header"))?;
    if length == 0 || length > JSON_LIMIT {
        return Err(invalid("Safetensors header exceeds bounded limit"));
    }
    let mut bytes = vec![0; length as usize];
    file.read_exact(&mut bytes)?;
    if bytes[0] != b'{' {
        return Err(invalid("Invalid safetensors header"));
    }
    let header: UniqueJson =
        serde_json::from_slice(&bytes).map_err(|_| invalid("Invalid safetensors header JSON"))?;
    let header = header
        .0
        .as_object()
        .ok_or_else(|| invalid("Invalid safetensors header object"))?;
    let mut ranges = Vec::new();
    let mut names = BTreeSet::new();
    for (name, value) in header {
        if name == "__metadata__" {
            if !value
                .as_object()
                .is_some_and(|map| map.values().all(serde_json::Value::is_string))
            {
                return Err(invalid("Invalid safetensors metadata"));
            }
            continue;
        }
        names.insert(name.clone());
        let dtype = value
            .get("dtype")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let width = match dtype {
            "BOOL" | "U8" | "I8" | "F8_E4M3" | "F8_E5M2" => 1,
            "I16" | "U16" | "F16" | "BF16" => 2,
            "I32" | "U32" | "F32" => 4,
            "I64" | "U64" | "F64" => 8,
            _ => return Err(invalid("Unsupported safetensors dtype")),
        };
        let shape = value
            .get("shape")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| invalid("Missing tensor shape"))?;
        let size = shape
            .iter()
            .try_fold(width, |size: u64, dimension| {
                size.checked_mul(dimension.as_u64()?)
            })
            .ok_or_else(|| invalid("Invalid or overflowing tensor shape"))?;
        let offsets = value
            .get("data_offsets")
            .and_then(serde_json::Value::as_array)
            .filter(|value| value.len() == 2)
            .ok_or_else(|| invalid("Invalid tensor offsets"))?;
        let start = offsets[0]
            .as_u64()
            .ok_or_else(|| invalid("Invalid tensor offset"))?;
        let end = offsets[1]
            .as_u64()
            .ok_or_else(|| invalid("Invalid tensor offset"))?;
        if end.checked_sub(start) != Some(size) {
            return Err(invalid("Tensor shape and data size disagree"));
        }
        ranges.push((start, end));
    }
    if ranges.is_empty() {
        return Err(invalid("Safetensors model has no tensors"));
    }
    ranges.sort_unstable();
    let mut end = 0;
    for (start, next) in ranges {
        if start != end {
            return Err(invalid("Safetensors offsets overlap or leave holes"));
        }
        end = next;
    }
    if end != data_length {
        return Err(invalid(
            "Safetensors tensor data is missing or has trailing bytes",
        ));
    }
    Ok((names, length))
}

/// Reject duplicate members before a JSON Value can hide them, including nested
/// config auto_map and tensor fields. Serde supplies its ordinary recursion bound.
struct UniqueJson(serde_json::Value);
impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> std::result::Result<Self, D::Error> {
        struct JsonVisitor;
        impl<'de> Visitor<'de> for JsonVisitor {
            type Value = UniqueJson;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON with unique object members")
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut result = serde_json::Map::new();
                while let Some((key, value)) = map.next_entry::<String, UniqueJson>()? {
                    if result.insert(key, value.0).is_some() {
                        return Err(de::Error::custom("duplicate JSON member"));
                    }
                }
                Ok(UniqueJson(serde_json::Value::Object(result)))
            }
            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut result = Vec::new();
                while let Some(value) = sequence.next_element::<UniqueJson>()? {
                    result.push(value.0);
                }
                Ok(UniqueJson(serde_json::Value::Array(result)))
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> std::result::Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|number| UniqueJson(number.into()))
                    .ok_or_else(|| E::custom("invalid number"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(UniqueJson(serde_json::Value::Null))
            }
        }
        decoder.deserialize_any(JsonVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qualification_retains_open_descriptor_after_ambient_replacement() {
        use sha2::{Digest, Sha256};
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("weights.safetensors");
        let header = br#"{"weight":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
        let bytes = [
            (header.len() as u64).to_le_bytes().as_slice(),
            header.as_slice(),
            1_f32.to_le_bytes().as_slice(),
        ]
        .concat();
        std::fs::write(&path, &bytes).unwrap();
        let file = std::fs::File::open(&path).unwrap();
        let mut inputs = vec![VerifiedCopyInput {
            file,
            receipt: crate::acquisition::VerifiedFile {
                path: "weights.safetensors".into(),
                bytes: bytes.len() as u64,
                sha256: hex::encode(Sha256::digest(&bytes)),
            },
        }];
        std::fs::rename(&path, temp.path().join("held-original")).unwrap();
        std::fs::write(&path, b"unrelated replacement").unwrap();
        let qualification = qualify(&mut inputs, 0).unwrap();
        assert_eq!(
            qualification.info.format,
            crate::model_library::types::FileFormat::Safetensors
        );
        let mut observed = Vec::new();
        inputs[0].file.read_to_end(&mut observed).unwrap();
        assert_eq!(observed, bytes);
    }
}
