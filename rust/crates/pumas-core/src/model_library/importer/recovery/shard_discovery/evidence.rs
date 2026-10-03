use super::{Kind, ShardRecoveryDiagnostic};
use crate::model_library::download_recovery::nofollow_options;
use crate::model_library::importer::{
    MissingShardRange, ShardIndexDiscovery, ShardModelDiscovery, ShardSetDiscovery,
    ShardSetDiscoveryStatus as Status,
};
use crate::model_library::sharding;
use cap_std::fs::{Dir, Metadata, OpenOptions};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const MAX_INDEX_BYTES: u64 = 16 * 1024 * 1024;
const WEIGHT_EXTENSIONS: &[&str] = &[
    "gguf",
    "safetensors",
    "pt",
    "pth",
    "ckpt",
    "bin",
    "onnx",
    "msgpack",
];

pub(super) fn is_weight_index(name: &str) -> bool {
    name.ends_with(".safetensors.index.json") || name.ends_with(".bin.index.json")
}

pub(super) fn is_weight(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            WEIGHT_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

pub(super) fn is_model_evidence(name: &str) -> bool {
    is_weight(name)
        || is_weight_index(name)
        || name == "metadata.json"
        || name == "model_index.json"
        || name == ".pumas_download"
        || sharding::extract_shard_info(name).is_some()
        || name.strip_suffix(".part").is_some_and(is_weight)
}

fn diagnostic(
    diagnostics: &mut Vec<ShardRecoveryDiagnostic>,
    root: &Path,
    path: &Path,
    kind: Kind,
    message: impl ToString,
) {
    diagnostics.push(ShardRecoveryDiagnostic {
        path: root.join(path),
        kind,
        message: message.to_string(),
    });
}

pub(super) fn analyze(
    model: &mut ShardModelDiscovery,
    files: &BTreeSet<PathBuf>,
    diagnostics: &mut Vec<ShardRecoveryDiagnostic>,
) {
    let mut sets: BTreeMap<(PathBuf, String), ShardSetDiscovery> = BTreeMap::new();
    let mut seen_ordinals = BTreeSet::new();
    for file in files {
        let Some(name) = file.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.ends_with(".part") || is_weight_index(name) {
            continue;
        }
        let parsed = sharding::extract_shard_info(name);
        if !is_weight(name) && parsed.as_ref().is_none_or(|(base, _, _)| !is_weight(base)) {
            continue;
        }
        // The shared parser accepts usize ordinals and the established patterns.
        // Its None also includes overflow; suspicious counted names must not be
        // silently reclassified as standalone files here.
        let Some((base, ordinal, total)) = parsed else {
            if name.contains("-of-") {
                diagnostic(
                    diagnostics,
                    &model.library_relative_root,
                    file,
                    Kind::InvalidShardName,
                    "malformed or overflowing counted shard name",
                );
            }
            continue;
        };
        let directory = file.parent().unwrap_or(Path::new("")).to_owned();
        let duplicate = !seen_ordinals.insert((directory.clone(), base.clone(), ordinal));
        let set = sets
            .entry((directory.clone(), base.clone()))
            .or_insert_with(|| ShardSetDiscovery {
                relative_directory: directory,
                base_name: base,
                expected_total: total,
                found_ordinals: Vec::new(),
                missing_ordinals: Vec::new(),
                files: Vec::new(),
                status: Status::CountedOrdinalsPresent,
            });
        set.files.push(file.clone());
        let issue = if name.matches("-of-").count() > 1 {
            Some((
                Kind::AmbiguousShardNames,
                "multiple counted suffixes in one filename",
            ))
        } else if total.is_some()
            && Path::new(&set.base_name)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem.ends_with('-'))
        {
            Some((
                Kind::AmbiguousShardNames,
                "negative-looking shard ordinal is ambiguous",
            ))
        } else if total.is_none() {
            Some((
                Kind::AmbiguousShardNames,
                "uncounted shard names do not declare ordinal coverage",
            ))
        } else if ordinal == 0 || total == Some(0) || total.is_some_and(|total| ordinal > total) {
            Some((
                Kind::InvalidShardOrdinal,
                "ordinal must be within its positive declared total",
            ))
        } else if set.expected_total != total {
            Some((
                Kind::InconsistentShardTotals,
                "shard names declare inconsistent totals or conventions",
            ))
        } else if duplicate {
            Some((
                Kind::DuplicateShardOrdinal,
                "multiple filenames declare the same ordinal",
            ))
        } else {
            None
        };
        if let Some((kind, message)) = issue {
            set.status = Status::Ambiguous;
            diagnostic(
                diagnostics,
                &model.library_relative_root,
                file,
                kind,
                message,
            );
        }
        set.found_ordinals.push(ordinal);
    }
    for ((directory, base), mut set) in sets {
        // A standalone and a shard set with the same base cannot be disambiguated
        // by filename counts. Keep both file identities rather than merging them.
        if files.contains(&directory.join(&base)) {
            set.status = Status::Ambiguous;
            diagnostic(
                diagnostics,
                &model.library_relative_root,
                &directory.join(&base),
                Kind::AmbiguousShardNames,
                "standalone file and sharded set share a base name",
            );
        }
        set.found_ordinals.sort_unstable();
        set.found_ordinals.dedup();
        if set.status != Status::Ambiguous {
            if let Some(total) = set.expected_total {
                set.missing_ordinals = missing_ranges(&set.found_ordinals, total);
                if !set.missing_ordinals.is_empty() {
                    set.status = Status::MissingOrdinals;
                }
            }
        }
        if base.ends_with(".safetensors") || base.ends_with(".bin") {
            let index = directory.join(format!("{base}.index.json"));
            if !files.contains(&index) {
                diagnostic(diagnostics, &model.library_relative_root, &index, Kind::MissingShardIndex,
                    "conventional weight-map index is absent; filename coverage is not package completeness");
            }
        }
        model.shard_sets.push(set);
    }
    for index in &mut model.indexes {
        if !index.valid {
            continue;
        }
        index.missing_files = index
            .referenced_files
            .iter()
            .filter(|file| !files.contains(*file))
            .cloned()
            .collect();
        for missing in &index.missing_files {
            diagnostic(
                diagnostics,
                &model.library_relative_root,
                &index.relative_path,
                Kind::MissingIndexedFile,
                format!("index references an unobserved file: {}", missing.display()),
            );
        }
        // A matching index must actually name every observed member of that set.
        for set in &model.shard_sets {
            let expected_index = set
                .relative_directory
                .join(format!("{}.index.json", set.base_name));
            if expected_index == index.relative_path
                && set
                    .files
                    .iter()
                    .any(|file| index.referenced_files.binary_search(file).is_err())
            {
                index.valid = false;
                diagnostic(
                    diagnostics,
                    &model.library_relative_root,
                    &index.relative_path,
                    Kind::InvalidShardIndex,
                    "weight map omits observed members of its counted set",
                );
            }
        }
    }
}

fn missing_ranges(found: &[usize], total: usize) -> Vec<MissingShardRange> {
    let mut missing = Vec::new();
    let mut previous = 0;
    for &ordinal in found {
        // Caller proved 1 <= ordinal <= total, with sorted unique ordinals.
        if ordinal - previous > 1 {
            missing.push(MissingShardRange {
                first: previous + 1,
                last: ordinal - 1,
            });
        }
        previous = ordinal;
    }
    if previous < total {
        missing.push(MissingShardRange {
            first: previous + 1,
            last: total,
        });
    }
    missing
}

#[derive(Deserialize)]
struct WeightIndex {
    #[serde(deserialize_with = "unique_weight_map")]
    weight_map: BTreeMap<String, String>,
}

fn unique_weight_map<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    struct UniqueMap;
    impl<'de> Visitor<'de> for UniqueMap {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .write_str("a nonempty weight map with unique tensor names and string filenames")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut values = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, String>()? {
                if key.is_empty() || values.insert(key, value).is_some() {
                    return Err(de::Error::custom("empty or duplicate weight-map key"));
                }
            }
            if values.is_empty() {
                return Err(de::Error::custom("empty weight map"));
            }
            Ok(values)
        }
    }
    deserializer.deserialize_map(UniqueMap)
}

pub(super) fn invalid_index(path: PathBuf) -> ShardIndexDiscovery {
    ShardIndexDiscovery {
        relative_path: path,
        referenced_files: Vec::new(),
        missing_files: Vec::new(),
        valid: false,
    }
}

pub(super) fn read_index(
    directory: &Dir,
    relative: &Path,
    observed: &Metadata,
) -> io::Result<ShardIndexDiscovery> {
    if observed.len() > MAX_INDEX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "weight index exceeds discovery byte budget",
        ));
    }
    let name = relative.file_name().ok_or_else(super::binding_changed)?;
    let mut options = OpenOptions::new();
    options.read(true);
    nofollow_options(&mut options);
    let file = directory.open_with(name, &options)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.is_symlink() {
        return Err(super::binding_changed());
    }
    let mut bytes = Vec::new();
    file.take(MAX_INDEX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_INDEX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "weight index exceeds discovery byte budget",
        ));
    }
    let document: WeightIndex = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let parent = relative.parent().unwrap_or(Path::new(""));
    let mut references = BTreeSet::new();
    for filename in document.weight_map.into_values() {
        // Index paths use repository '/' separators even on Windows. Reject
        // alternate separators, drive prefixes, dot/empty components and escapes.
        if filename.contains(['\\', ':'])
            || filename
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "weight-map reference is not a contained relative path",
            ));
        }
        references.insert(parent.join(filename));
    }
    Ok(ShardIndexDiscovery {
        relative_path: relative.to_owned(),
        referenced_files: references.into_iter().collect(),
        missing_files: Vec::new(),
        valid: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_index_is_not_read_as_an_empty_weight_map() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("model.bin.index.json");
        std::fs::File::create(&path)
            .unwrap()
            .set_len(MAX_INDEX_BYTES + 1)
            .unwrap();
        let directory = crate::platform::capability_fs::open_directory(temp.path()).unwrap();
        let metadata = directory.symlink_metadata("model.bin.index.json").unwrap();
        assert_eq!(
            read_index(&directory, Path::new("model.bin.index.json"), &metadata)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn missing_ranges_do_not_allocate_the_declared_total() {
        assert_eq!(
            missing_ranges(&[1, 3], usize::MAX),
            vec![
                MissingShardRange { first: 2, last: 2 },
                MissingShardRange {
                    first: 4,
                    last: usize::MAX
                },
            ]
        );
        assert!(missing_ranges(&[1], 1).is_empty());
        assert_eq!(missing_ranges(&[usize::MAX], usize::MAX).len(), 1);
    }
}
