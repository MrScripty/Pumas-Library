//! Source-neutral package completeness for local, HF and acquired model sets.
use crate::acquisition::{AcquisitionRecord, AcquisitionWorkspace};
use crate::error::{PumasError, Result};
use crate::model_library::sharding;
use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::Read,
};

// Same input-size ceiling as the existing package-facts JSON reader.
const MAX_PACKAGE_JSON_BYTES: u64 = 16 * 1024 * 1024;

fn invalid(message: impl Into<String>) -> PumasError {
    PumasError::Validation {
        field: "download.package".into(),
        message: message.into(),
    }
}

pub(crate) fn is_weight_index(path: &str) -> bool {
    path.ends_with(".safetensors.index.json") || path.ends_with(".bin.index.json")
}

pub(crate) fn required_indexes<'a>(paths: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    paths
        .into_iter()
        .filter_map(sharding::extract_shard_info)
        .filter(|(base, _, _)| base.ends_with(".safetensors") || base.ends_with(".bin"))
        .map(|(base, _, _)| format!("{base}.index.json"))
        .collect()
}

pub(crate) fn validate_selected(paths: &[String], diffusers: bool) -> Result<()> {
    sharding::validate_explicit_shard_sets(paths.iter().map(String::as_str)).map_err(invalid)?;
    for index in required_indexes(paths.iter().map(String::as_str)) {
        if !paths.contains(&index) {
            return Err(invalid(format!("Selected shards require index {index}")));
        }
    }
    if diffusers && !paths.iter().any(|path| path == "model_index.json") {
        return Err(invalid("Diffusers package requires model_index.json"));
    }
    Ok(())
}

#[derive(Deserialize)]
struct WeightIndex {
    #[serde(deserialize_with = "unique_weight_map")]
    weight_map: BTreeMap<String, String>,
}

fn unique_weight_map<'de, D: Deserializer<'de>>(
    decoder: D,
) -> std::result::Result<BTreeMap<String, String>, D::Error> {
    struct WeightMap;
    impl<'de> Visitor<'de> for WeightMap {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a nonempty tensor-to-shard map")
        }
        fn visit_map<A: MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some((tensor, file)) = map.next_entry::<String, String>()? {
                if tensor.is_empty() || file.is_empty() || values.insert(tensor, file).is_some() {
                    return Err(de::Error::custom("empty or duplicate weight-map entry"));
                }
            }
            if values.is_empty() {
                return Err(de::Error::custom("empty weight map"));
            }
            Ok(values)
        }
    }
    decoder.deserialize_map(WeightMap)
}

fn is_weight_payload_for_index(index_path: &str, path: &str) -> bool {
    // Reuse known shard naming, including part/numeric suffixes, before
    // interpreting the payload format. The library supports these PyTorch
    // suffixes; an index/config document is never a weight payload.
    let family = sharding::extract_shard_info(path).map(|(base, _, _)| base);
    let name = family.as_deref().unwrap_or(path);
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension)
        .unwrap_or("");
    if index_path.ends_with(".safetensors.index.json") {
        extension.eq_ignore_ascii_case("safetensors")
    } else {
        index_path.ends_with(".bin.index.json")
            && ["bin", "pt", "pth"]
                .iter()
                .any(|format| extension.eq_ignore_ascii_case(format))
    }
}

fn validate_index(index_path: &str, bytes: &[u8], selected: &BTreeSet<&str>) -> Result<()> {
    let index: WeightIndex = serde_json::from_slice(bytes)
        .map_err(|_| invalid(format!("Invalid weight index {index_path}")))?;
    let directory = index_path.rsplit_once('/').map(|(dir, _)| dir);
    let referenced: BTreeSet<String> = index
        .weight_map
        .values()
        .map(|file| match directory {
            Some(dir) => format!("{dir}/{file}"),
            None => file.clone(),
        })
        .collect();
    for path in &referenced {
        // Membership in the validated manifest is also path authority. Index
        // references never open a path, normalize traversal or expand selection.
        if !selected.contains(path.as_str()) {
            return Err(invalid(format!(
                "Index {index_path} references unselected shard {path}"
            )));
        }
        if !is_weight_payload_for_index(index_path, path) {
            return Err(invalid(format!(
                "Index {index_path} references a target outside its weight payload format: {path}"
            )));
        }
    }
    for path in selected {
        if required_indexes([*path]).contains(index_path) && !referenced.contains(*path) {
            return Err(invalid(format!(
                "Index {index_path} omits selected shard {path}"
            )));
        }
    }
    sharding::validate_explicit_shard_sets(referenced.iter().map(String::as_str)).map_err(invalid)
}

fn validate_diffusers_index(bytes: &[u8], selected: &BTreeSet<&str>) -> Result<()> {
    use crate::model_library::external_assets::{
        is_diffusers_component_entry, is_optional_component_marker,
        is_supported_text_to_image_pipeline, normalized_component_relative_path,
    };
    let index: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| invalid("Invalid Diffusers model_index.json"))?;
    let fields = index
        .as_object()
        .ok_or_else(|| invalid("Invalid Diffusers model_index.json"))?;
    let class = fields
        .get("_class_name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid("Diffusers model index requires a pipeline class"))?;
    if !is_supported_text_to_image_pipeline(class.trim()) {
        return Err(invalid("Unsupported Diffusers pipeline class"));
    }
    for (name, value) in fields {
        if name.starts_with('_')
            || !is_diffusers_component_entry(value)
            || is_optional_component_marker(value)
        {
            continue;
        }
        normalized_component_relative_path(name)?;
        let prefix = format!("{name}/");
        if !selected
            .iter()
            .any(|path| *path == name || path.starts_with(&prefix))
        {
            return Err(invalid(format!(
                "Diffusers selection is missing component {name}"
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_acquired(
    workspace: &AcquisitionWorkspace,
    record: &AcquisitionRecord,
    diffusers: bool,
) -> Result<()> {
    validate_readers(
        &record
            .manifest
            .files()
            .iter()
            .map(|file| file.logical_path().to_owned())
            .collect::<Vec<_>>(),
        diffusers,
        |path| {
            let index = record
                .manifest
                .files()
                .iter()
                .position(|file| file.logical_path() == path)
                .ok_or_else(|| invalid("Selected package input is unavailable"))?;
            workspace.open_verified_readonly(&record.manifest.files()[index], &record.files[index])
        },
    )
}

/// Read only caller-selected, held inputs. References never expand the file set.
pub(crate) fn validate_readers(
    paths: &[String],
    diffusers: bool,
    mut open: impl FnMut(&str) -> Result<std::fs::File>,
) -> Result<()> {
    validate_selected(paths, diffusers)?;
    let selected: BTreeSet<&str> = paths.iter().map(String::as_str).collect();
    for path in paths {
        let diffusers_index = diffusers && path == "model_index.json";
        if !is_weight_index(path) && !diffusers_index {
            continue;
        }
        let mut bytes = Vec::new();
        open(path)?
            .take(MAX_PACKAGE_JSON_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_PACKAGE_JSON_BYTES {
            return Err(invalid("Package index exceeds JSON byte limit"));
        }
        if diffusers_index {
            validate_diffusers_index(&bytes, &selected)?;
        } else {
            validate_index(path, &bytes, &selected)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diffusers_index_uses_existing_component_and_optional_path_semantics() {
        let bytes = br#"{"_class_name":"StableDiffusionPipeline","unet":["diffusers","UNet2DConditionModel"],"tokenizer":["transformers","CLIPTokenizer"],"safety_checker":[null,null]}"#;
        validate_diffusers_index(
            bytes,
            &BTreeSet::from(["unet/model.safetensors", "tokenizer/vocab.txt"]),
        )
        .unwrap();
        assert!(
            validate_diffusers_index(bytes, &BTreeSet::from(["unet/model.safetensors"])).is_err()
        );
        for invalid in [
            br#"{"_class_name":"Unknown","unet":["diffusers","Model"]}"#.as_slice(),
            br#"{"_class_name":"StableDiffusionPipeline","../escape":["diffusers","Model"]}"#
                .as_slice(),
            b"null",
        ] {
            assert!(validate_diffusers_index(invalid, &BTreeSet::new()).is_err());
        }
    }

    #[test]
    fn index_targets_must_be_weight_payloads_in_the_index_format() {
        for path in [
            "weights.bin",
            "weights.pt",
            "weights.pth",
            "weights.BIN",
            "weights.bin.part1",
        ] {
            let selected = BTreeSet::from([path]);
            let map = serde_json::json!({"weight_map":{"a":path}});
            validate_index(
                "pytorch_model.bin.index.json",
                &serde_json::to_vec(&map).unwrap(),
                &selected,
            )
            .unwrap();
        }
        for extension in ["safetensors", "bin"] {
            let first = format!("model-00001-of-00002.{extension}");
            let second = format!("model-00002-of-00002.{extension}");
            let index_path = format!("model.{extension}.index.json");
            let other = if extension == "bin" {
                "other.safetensors"
            } else {
                "other.bin"
            };
            let selected = BTreeSet::from([
                first.as_str(),
                second.as_str(),
                index_path.as_str(),
                "config.json",
                other,
            ]);
            let good = serde_json::json!({"weight_map":{"a":first,"b":second}});
            validate_index(&index_path, &serde_json::to_vec(&good).unwrap(), &selected).unwrap();
            for target in ["config.json", index_path.as_str(), other] {
                let mut bad = good.clone();
                bad["weight_map"]["c"] = serde_json::Value::String(target.to_owned());
                assert!(
                    validate_index(&index_path, &serde_json::to_vec(&bad).unwrap(), &selected)
                        .is_err(),
                    "{extension} index accepted target {target}"
                );
            }
            let part = format!("model.{extension}.part1");
            let selected = BTreeSet::from([part.as_str()]);
            let map = serde_json::json!({"weight_map":{"a":part}});
            validate_index(&index_path, &serde_json::to_vec(&map).unwrap(), &selected).unwrap();
        }
    }

    #[test]
    fn shard_index_requires_its_complete_exact_selected_closure() {
        let selected = BTreeSet::from([
            "unet/model-00001-of-00002.safetensors",
            "unet/model-00002-of-00002.safetensors",
        ]);
        let good = br#"{"weight_map":{"a":"model-00001-of-00002.safetensors","b":"model-00002-of-00002.safetensors"}}"#;
        validate_index("unet/model.safetensors.index.json", good, &selected).unwrap();
        for bad in [
            br#"{"weight_map":{"a":"model-00001-of-00002.safetensors"}}"#.as_slice(),
            br#"{"weight_map":{"a":"missing.safetensors"}}"#.as_slice(),
            br#"{"weight_map":{"a":"model-00001-of-00002.safetensors","a":"model-00002-of-00002.safetensors"}}"#.as_slice(),
            br#"{"weight_map":{}}"#.as_slice(),
            br#"{"weight_map":{"a":12}}"#.as_slice(),
            br#"{"weight_map":{"a":"../model.safetensors"}}"#.as_slice(),
        ] { assert!(validate_index("unet/model.safetensors.index.json", bad, &selected).is_err()); }
    }
}
