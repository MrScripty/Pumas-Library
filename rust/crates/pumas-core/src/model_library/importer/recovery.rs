use super::{
    InPlaceImportSpec, IncompleteShardRecovery, InterruptedDownload, ModelImporter,
    OrphanScanResult, TEMP_IMPORT_PREFIX,
};
use crate::model_library::sharding;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

impl ModelImporter {
    /// Scan the library tree for orphan model directories and adopt them.
    ///
    /// An orphan is a directory that contains model files but no `metadata.json`.
    /// Metadata is inferred from the directory path structure
    /// (`{library_root}/{model_type}/{family}/{name}/`).
    pub async fn adopt_orphans(&self, compute_hashes: bool) -> OrphanScanResult {
        let mut result = OrphanScanResult::default();
        let importer = self.clone();
        let orphan_dirs = tokio::task::spawn_blocking(move || {
            importer.find_orphan_dirs(importer.library.library_root(), false)
        })
        .await
        .unwrap_or_default();
        result.orphans_found = orphan_dirs.len();

        if orphan_dirs.is_empty() {
            tracing::debug!("No orphan model directories found");
            return result;
        }

        tracing::info!("Found {} orphan model directories", orphan_dirs.len());

        for orphan_dir in orphan_dirs {
            let inferred = match self.infer_spec_from_path(&orphan_dir) {
                Some(spec) => spec,
                None => {
                    result.errors.push((
                        orphan_dir.clone(),
                        "Could not infer metadata from directory path".to_string(),
                    ));
                    continue;
                }
            };

            let spec = InPlaceImportSpec {
                model_dir: orphan_dir.clone(),
                official_name: inferred.official_name,
                family: inferred.family,
                model_type: inferred.model_type,
                repo_id: None,
                known_sha256: None,
                compute_hashes,
                expected_files: None,
                pipeline_tag: None,
                huggingface_evidence: None,
                release_date: None,
                download_url: None,
                model_card_json: None,
                license_status: None,
            };

            match self.import_in_place(&spec).await {
                Ok(import_result) => {
                    if import_result.success {
                        result.adopted += 1;
                        tracing::info!(
                            "Adopted orphan model: {:?} -> {:?}",
                            orphan_dir,
                            import_result.model_id
                        );
                    } else {
                        result.errors.push((
                            orphan_dir,
                            import_result
                                .error
                                .unwrap_or_else(|| "Unknown error".to_string()),
                        ));
                    }
                }
                Err(err) => {
                    result.errors.push((orphan_dir, err.to_string()));
                }
            }
        }

        tracing::info!(
            "Orphan scan complete: {} found, {} adopted, {} errors",
            result.orphans_found,
            result.adopted,
            result.errors.len()
        );

        result
    }

    /// Cheap clean-state probe used to avoid spawning startup orphan adoption
    /// work when the library tree has no orphan candidates.
    pub fn has_orphan_candidates(&self) -> bool {
        !self
            .find_orphan_dirs(self.library.library_root(), true)
            .is_empty()
    }

    /// Async wrapper for orphan candidate probing used on startup paths.
    pub async fn has_orphan_candidates_async(&self) -> bool {
        let importer = self.clone();
        tokio::task::spawn_blocking(move || importer.has_orphan_candidates())
            .await
            .unwrap_or(false)
    }

    /// Scan for incomplete sharded models that need recovery downloads.
    ///
    /// Finds directories where:
    /// - No `metadata.json` (shard validation rejected adoption)
    /// - At least one file matches a shard pattern with a known total (e.g. `-00001-of-00004.`)
    /// - Fewer files present than the total indicates
    ///
    /// Returns a list of recovery descriptors with the reconstructed repo_id
    /// derived from the directory path (`{family}/{name}` -> HF repo).
    pub fn recover_incomplete_shards(&self) -> Vec<IncompleteShardRecovery> {
        let library_root = self.library.library_root();
        let model_extensions: &[&str] =
            &["gguf", "safetensors", "pt", "pth", "ckpt", "bin", "onnx"];
        let mut results = Vec::new();

        for entry in WalkDir::new(library_root)
            .min_depth(1)
            .max_depth(3)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            if !entry.file_type().is_dir() {
                continue;
            }

            let dir = entry.path();
            let dir_name = dir.file_name().and_then(|name| name.to_str()).unwrap_or("");

            if dir_name.starts_with(TEMP_IMPORT_PREFIX) || dir_name.starts_with('.') {
                continue;
            }

            if dir.join("metadata.json").exists() {
                continue;
            }

            let file_entries: Vec<_> = match std::fs::read_dir(dir) {
                Ok(reader) => reader.filter_map(|entry| entry.ok()).collect(),
                Err(_) => continue,
            };

            let model_files: Vec<String> = file_entries
                .iter()
                .filter(|entry| entry.file_type().ok().is_some_and(|ty| ty.is_file()))
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".part")
                        || name == "metadata.json"
                        || name == "overrides.json"
                    {
                        return None;
                    }

                    let extension = entry
                        .path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if model_extensions.contains(&extension.as_str()) {
                        Some(name)
                    } else {
                        None
                    }
                })
                .collect();

            if model_files.is_empty() {
                continue;
            }

            for filename in &model_files {
                if let Some((base_name, _idx, Some(total))) = sharding::extract_shard_info(filename)
                {
                    if total > 1 {
                        let found_count = model_files
                            .iter()
                            .filter(|candidate| {
                                sharding::extract_shard_info(candidate)
                                    .map(|(base, _, _)| base == base_name)
                                    .unwrap_or(false)
                            })
                            .count();

                        if found_count < total {
                            if let Some(inferred) = self.infer_spec_from_path(dir) {
                                let repo_id =
                                    format!("{}/{}", inferred.family, inferred.official_name);
                                tracing::info!(
                                    "Found incomplete shard set in {}: {}/{} shards of '{}', \
                                     candidate repo: {}",
                                    dir.display(),
                                    found_count,
                                    total,
                                    base_name,
                                    repo_id,
                                );
                                results.push(IncompleteShardRecovery {
                                    model_dir: dir.to_path_buf(),
                                    repo_id,
                                    family: inferred.family,
                                    official_name: inferred.official_name,
                                    model_type: inferred.model_type,
                                    existing_files: model_files.clone(),
                                });
                            }
                            break;
                        }
                    }
                }
            }
        }

        results
    }

    /// Find directories with interrupted downloads (`.part` files) that have
    /// no download persistence entry and no metadata.
    ///
    /// These are downloads that were interrupted and lost their tracking state
    /// (e.g. due to a crash). The user must supply the correct repo_id to
    /// recover them via `recover_download()`.
    pub fn find_interrupted_downloads(
        &self,
        known_dest_dirs: &HashSet<PathBuf>,
    ) -> Vec<InterruptedDownload> {
        let library_root = self.library.library_root();
        let mut results = Vec::new();

        for entry in WalkDir::new(library_root)
            .min_depth(1)
            .max_depth(3)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            if !entry.file_type().is_dir() {
                continue;
            }

            let dir = entry.path();
            let dir_name = dir.file_name().and_then(|name| name.to_str()).unwrap_or("");

            if dir_name.starts_with(TEMP_IMPORT_PREFIX) || dir_name.starts_with('.') {
                continue;
            }

            if dir.join("metadata.json").exists() {
                continue;
            }

            if known_dest_dirs.contains(dir) {
                continue;
            }

            let entries: Vec<_> = match std::fs::read_dir(dir) {
                Ok(reader) => reader.filter_map(|entry| entry.ok()).collect(),
                Err(_) => continue,
            };

            let mut part_files = Vec::new();
            let mut completed_files = Vec::new();
            for entry in &entries {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".part") {
                    part_files.push(name);
                } else if name != "metadata.json"
                    && name != "overrides.json"
                    && name != ".pumas_download"
                    && entry.file_type().ok().is_some_and(|ty| ty.is_file())
                {
                    completed_files.push(name);
                }
            }

            if part_files.is_empty() {
                continue;
            }

            let marker: Option<serde_json::Value> =
                std::fs::read_to_string(dir.join(".pumas_download"))
                    .ok()
                    .and_then(|contents| serde_json::from_str(&contents).ok());

            if let Some(inferred) = self.infer_spec_from_path(dir) {
                let (repo_id, family, inferred_name, model_type) = if let Some(ref marker) = marker
                {
                    (
                        marker
                            .get("repo_id")
                            .and_then(|value| value.as_str())
                            .map(String::from),
                        marker
                            .get("family")
                            .and_then(|value| value.as_str())
                            .map(String::from)
                            .unwrap_or(inferred.family),
                        marker
                            .get("official_name")
                            .and_then(|value| value.as_str())
                            .map(String::from)
                            .unwrap_or(inferred.official_name),
                        marker
                            .get("model_type")
                            .and_then(|value| value.as_str())
                            .map(String::from)
                            .or(inferred.model_type),
                    )
                } else {
                    (
                        None,
                        inferred.family,
                        inferred.official_name,
                        inferred.model_type,
                    )
                };

                results.push(InterruptedDownload {
                    model_dir: dir.to_path_buf(),
                    repo_id,
                    model_type,
                    family,
                    inferred_name,
                    part_files,
                    completed_files,
                });
            }
        }

        results
    }

    /// Find directories with model files but no metadata.json.
    fn find_orphan_dirs(&self, library_root: &Path, stop_after_first: bool) -> Vec<PathBuf> {
        let mut orphans = Vec::new();
        let model_extensions: &[&str] =
            &["gguf", "safetensors", "pt", "pth", "ckpt", "bin", "onnx"];

        for entry in WalkDir::new(library_root)
            .min_depth(1)
            .max_depth(3)
            .into_iter()
            .filter_map(|entry| entry.ok())
        {
            if !entry.file_type().is_dir() {
                continue;
            }

            let dir = entry.path();
            let dir_name = dir.file_name().and_then(|name| name.to_str()).unwrap_or("");

            if dir_name.starts_with(TEMP_IMPORT_PREFIX) || dir_name.starts_with('.') {
                continue;
            }

            if dir.join("metadata.json").exists() {
                continue;
            }

            let entries: Vec<_> = match std::fs::read_dir(dir) {
                Ok(reader) => reader.filter_map(|entry| entry.ok()).collect(),
                Err(_) => continue,
            };

            if entries
                .iter()
                .any(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
            {
                continue;
            }

            let has_model_files = entries.iter().any(|entry| {
                if !entry.file_type().ok().is_some_and(|ty| ty.is_file()) {
                    return false;
                }
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| model_extensions.contains(&ext.to_lowercase().as_str()))
                    .unwrap_or(false)
            });

            if has_model_files {
                orphans.push(dir.to_path_buf());
                if stop_after_first {
                    break;
                }
            }
        }

        orphans
    }

    /// Infer model metadata from a directory path.
    ///
    /// Expects `{library_root}/{model_type}/{family}/{name}/`.
    /// Falls back gracefully with fewer path components.
    fn infer_spec_from_path(&self, model_dir: &Path) -> Option<InferredSpec> {
        let relative = model_dir.strip_prefix(self.library.library_root()).ok()?;
        let components: Vec<&str> = relative
            .components()
            .filter_map(|component| component.as_os_str().to_str())
            .collect();

        match components.len() {
            3 => Some(InferredSpec {
                model_type: Some(components[0].to_string()),
                family: components[1].to_string(),
                official_name: components[2].replace('_', " "),
            }),
            2 => Some(InferredSpec {
                model_type: None,
                family: components[0].to_string(),
                official_name: components[1].replace('_', " "),
            }),
            1 => Some(InferredSpec {
                model_type: None,
                family: "unknown".to_string(),
                official_name: components[0].replace('_', " "),
            }),
            _ => None,
        }
    }
}

struct InferredSpec {
    model_type: Option<String>,
    family: String,
    official_name: String,
}
