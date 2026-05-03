//! HuggingFace metadata fetching and model verification.
//!
//! Handles direct model info lookups, repository file tree retrieval,
//! metadata lookup by filename/hash, and candidate verification.

use super::types::{
    infer_pipeline_tag_from_config, HfFileEntry, HfSearchResult, HF_API_BASE, HF_HUB_BASE,
    REPO_CACHE_TTL_SECS,
};
use super::HuggingFaceClient;
use crate::error::{PumasError, Result};
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::hashing::compute_fast_hash;
use crate::model_library::naming::extract_base_name;
use crate::model_library::types::{
    DownloadRequest, HfMetadataResult, HfSearchParams, HuggingFaceEvidence, HuggingFaceModel,
    LfsFileInfo, RepoFileTree, REPO_FILE_TREE_VERSION,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

impl HuggingFaceClient {
    async fn compute_fast_hash_async(path: PathBuf) -> Option<String> {
        tokio::task::spawn_blocking(move || compute_fast_hash(&path).ok())
            .await
            .ok()
            .flatten()
    }

    pub(crate) async fn get_model_snapshot(
        &self,
        repo_id: &str,
    ) -> Result<(HuggingFaceModel, HuggingFaceEvidence)> {
        let result = self.fetch_model_info_response(repo_id).await?;
        let evidence = Self::build_huggingface_evidence(repo_id, &result);
        let model = Self::convert_search_result(result);
        Ok((model, evidence))
    }

    async fn fetch_model_info_response(&self, repo_id: &str) -> Result<HfSearchResult> {
        let url = format!("{}/models/{}", HF_API_BASE, repo_id);

        let mut request = self.client.get(&url);
        if let Some(auth) = self.auth_header_value().await {
            request = request.header("Authorization", auth);
        }

        let response = request.send().await.map_err(|e| PumasError::Network {
            message: format!("HuggingFace API request failed: {}", e),
            cause: Some(e.to_string()),
        })?;

        if !response.status().is_success() {
            return Err(PumasError::Network {
                message: format!(
                    "HuggingFace API returned {} for repo {}",
                    response.status(),
                    repo_id
                ),
                cause: None,
            });
        }

        response.json().await.map_err(|e| PumasError::Json {
            message: format!("Failed to parse HuggingFace response: {}", e),
            source: None,
        })
    }

    pub(crate) fn build_huggingface_evidence(
        repo_id: &str,
        result: &HfSearchResult,
    ) -> HuggingFaceEvidence {
        let remote_kind = result
            .pipeline_tag
            .clone()
            .or_else(|| Self::infer_pipeline_tag_from_tags(&result.tags))
            .or_else(|| infer_pipeline_tag_from_config(result.config.as_ref()));
        let tags = (!result.tags.is_empty()).then(|| result.tags.clone());
        let architectures = result
            .config
            .as_ref()
            .map(|config| {
                config
                    .architectures
                    .iter()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
            })
            .filter(|values| !values.is_empty());
        let config_model_type = result
            .config
            .as_ref()
            .and_then(|config| config.model_type.as_ref())
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty());

        HuggingFaceEvidence {
            repo_id: Some(repo_id.to_string()),
            captured_at: Some(chrono::Utc::now().to_rfc3339()),
            remote_kind: remote_kind.clone(),
            pipeline_tag: remote_kind,
            tags,
            architectures,
            config_model_type,
            sibling_filenames: None,
            selected_filenames: None,
            requested_model_type: None,
            requested_pipeline_tag: None,
            requested_quant: None,
        }
    }

    pub(crate) fn enrich_huggingface_evidence_for_download(
        evidence: &mut HuggingFaceEvidence,
        tree: &RepoFileTree,
        request: &DownloadRequest,
        selected_filenames: &[String],
    ) {
        evidence.sibling_filenames = Some(
            tree.lfs_files
                .iter()
                .map(|file| file.filename.clone())
                .chain(tree.regular_files.iter().cloned())
                .collect(),
        );
        evidence.selected_filenames = Some(selected_filenames.to_vec());
        evidence.requested_model_type = request.model_type.clone();
        evidence.requested_pipeline_tag = request.pipeline_tag.clone();
        evidence.requested_quant = request.quant.clone();
        evidence.captured_at = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Fetch model info directly by repo_id from the HuggingFace API.
    ///
    /// Uses `GET /api/models/{repo_id}` which returns the exact model
    /// without any search or cache involvement.
    pub async fn get_model_info(&self, repo_id: &str) -> Result<HuggingFaceModel> {
        let (model, _) = self.get_model_snapshot(repo_id).await?;
        Ok(model)
    }

    /// Get repository file tree with LFS information.
    ///
    /// Results are cached for 24 hours.
    pub async fn get_repo_files(&self, repo_id: &str) -> Result<RepoFileTree> {
        // Check cache first
        let cache_file = self.get_cache_path(repo_id, "files");
        if let Some(cached) = read_repo_file_tree_cache(cache_file.clone()).await? {
            // Reject entries from an older cache format (e.g. pre-recursive)
            if cached.cache_version >= REPO_FILE_TREE_VERSION
                && repo_file_tree_cache_is_fresh(&cache_file).await?
            {
                return Ok(cached);
            }
        }

        // Fetch from API
        let url = format!(
            "{}/api/models/{}/tree/main?recursive=true",
            HF_HUB_BASE, repo_id
        );

        let mut request = self.client.get(&url);
        if let Some(auth) = self.auth_header_value().await {
            request = request.header("Authorization", auth);
        }

        let response = request.send().await.map_err(|e| PumasError::Network {
            message: format!("Failed to fetch repo tree: {}", e),
            cause: None,
        })?;

        if !response.status().is_success() {
            return Err(PumasError::Network {
                message: format!("HuggingFace API returned {}", response.status()),
                cause: None,
            });
        }

        let files: Vec<HfFileEntry> = response.json().await.map_err(|e| PumasError::Json {
            message: format!("Failed to parse file tree: {}", e),
            source: None,
        })?;

        // Separate LFS and regular files
        let mut lfs_files = Vec::new();
        let mut regular_files = Vec::new();

        for file in files {
            // Skip directory entries returned by recursive tree listing
            if file.entry_type.as_deref() == Some("directory") {
                continue;
            }
            if let Some(lfs) = file.lfs {
                lfs_files.push(LfsFileInfo {
                    filename: file.path,
                    size: lfs.size,
                    sha256: lfs.oid,
                });
            } else {
                regular_files.push(file.path);
            }
        }

        let tree = RepoFileTree {
            repo_id: repo_id.to_string(),
            lfs_files,
            regular_files,
            cached_at: chrono::Utc::now().to_rfc3339(),
            last_modified: None, // Would need separate API call to get this
            cache_version: REPO_FILE_TREE_VERSION,
        };

        // Cache the result
        write_repo_file_tree_cache(cache_file, &tree).await?;

        Ok(tree)
    }

    /// Look up model metadata by filename and optional file path.
    ///
    /// Uses a hybrid approach:
    /// 1. Search by base filename
    /// 2. Compute fast hash for top candidates
    /// 3. Verify with LFS SHA256 if available
    ///
    /// # Arguments
    ///
    /// * `filename` - Model filename
    /// * `file_path` - Optional local file path for hash verification
    /// * `timeout` - Request timeout
    pub async fn lookup_metadata(
        &self,
        filename: &str,
        file_path: Option<&Path>,
        _timeout: Option<Duration>,
    ) -> Result<Option<HfMetadataResult>> {
        let base_name = extract_base_name(filename);

        // Search for candidates
        let params = HfSearchParams {
            query: base_name.clone(),
            limit: Some(5),
            ..Default::default()
        };

        let candidates = self.search(&params).await?;

        if candidates.is_empty() {
            return Ok(None);
        }

        // If we have a local file, try to verify by hash
        if let Some(path) = file_path {
            // Compute fast hash for filtering
            let fast_hash = Self::compute_fast_hash_async(path.to_path_buf()).await;

            // Try to match against top candidates
            for candidate in candidates.iter().take(2) {
                if let Ok(Some(result)) = self
                    .verify_candidate(&candidate.repo_id, filename, path, fast_hash.as_deref())
                    .await
                {
                    return Ok(Some(result));
                }
            }
        }

        // Fall back to best filename match
        let best_match = &candidates[0];
        let confidence = Self::compute_filename_confidence(&base_name, &best_match.name);

        Ok(Some(HfMetadataResult {
            repo_id: best_match.repo_id.clone(),
            official_name: Some(best_match.name.clone()),
            family: None, // Would need more analysis
            model_type: Some(best_match.kind.clone()),
            subtype: None,
            variant: None,
            precision: None,
            tags: vec![],
            base_model: None,
            download_url: Some(format!(
                "{}/{}/resolve/main/{}",
                HF_HUB_BASE, best_match.repo_id, filename
            )),
            release_date: best_match.release_date.clone(),
            model_card_json: best_match
                .model_card
                .as_ref()
                .and_then(|card| serde_json::to_string(card).ok()),
            license_status: best_match
                .license
                .clone()
                .or_else(|| Some("license_unknown".to_string())),
            description: None,
            match_confidence: confidence,
            match_method: if confidence > 0.9 {
                "filename_exact"
            } else {
                "filename_fuzzy"
            }
            .to_string(),
            requires_confirmation: confidence < 0.6,
            hash_mismatch: false,
            matched_filename: Some(filename.to_string()),
            pending_full_verification: true,
            fast_hash: None,
            expected_sha256: None,
        }))
    }

    /// Verify a candidate repository against a local file.
    async fn verify_candidate(
        &self,
        repo_id: &str,
        filename: &str,
        _file_path: &Path,
        fast_hash: Option<&str>,
    ) -> Result<Option<HfMetadataResult>> {
        // Get repo files to find LFS hash
        let tree = self.get_repo_files(repo_id).await?;

        // Find matching file
        let matching_file = tree.lfs_files.iter().find(|f| {
            f.filename == filename
                || f.filename.ends_with(filename)
                || filename.ends_with(&f.filename)
        });

        if let Some(lfs_file) = matching_file {
            // We have an LFS file with SHA256
            // For now, just return it as a potential match
            // Full verification would require reading the entire file

            return Ok(Some(HfMetadataResult {
                repo_id: repo_id.to_string(),
                official_name: None,
                family: None,
                model_type: None,
                subtype: None,
                variant: None,
                precision: None,
                tags: vec![],
                base_model: None,
                download_url: Some(format!(
                    "{}/{}/resolve/main/{}",
                    HF_HUB_BASE, repo_id, lfs_file.filename
                )),
                release_date: None,
                model_card_json: None,
                license_status: Some("license_unknown".to_string()),
                description: None,
                match_confidence: 0.8, // High confidence from LFS match
                match_method: "lfs_match".to_string(),
                requires_confirmation: false,
                hash_mismatch: false,
                matched_filename: Some(lfs_file.filename.clone()),
                pending_full_verification: true,
                fast_hash: fast_hash.map(String::from),
                expected_sha256: Some(lfs_file.sha256.clone()),
            }));
        }

        Ok(None)
    }

    /// Compute filename match confidence.
    pub(super) fn compute_filename_confidence(query: &str, candidate: &str) -> f64 {
        let query_lower = query.to_lowercase();
        let candidate_lower = candidate.to_lowercase();

        if query_lower == candidate_lower {
            return 1.0;
        }

        if candidate_lower.contains(&query_lower) || query_lower.contains(&candidate_lower) {
            return 0.8;
        }

        // Simple word overlap score
        let query_words: std::collections::HashSet<_> =
            query_lower.split(|c: char| !c.is_alphanumeric()).collect();
        let candidate_words: std::collections::HashSet<_> = candidate_lower
            .split(|c: char| !c.is_alphanumeric())
            .collect();

        let intersection = query_words.intersection(&candidate_words).count();
        let union = query_words.union(&candidate_words).count();

        if union > 0 {
            intersection as f64 / union as f64
        } else {
            0.0
        }
    }
}

async fn read_repo_file_tree_cache(path: PathBuf) -> Result<Option<RepoFileTree>> {
    tokio::task::spawn_blocking(move || atomic_read_json(&path))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join HuggingFace repo tree cache read task: {}",
                err
            ))
        })?
}

async fn write_repo_file_tree_cache(path: PathBuf, tree: &RepoFileTree) -> Result<()> {
    let tree = tree.clone();
    tokio::task::spawn_blocking(move || atomic_write_json(&path, &tree, false))
        .await
        .map_err(|err| {
            PumasError::Other(format!(
                "Failed to join HuggingFace repo tree cache write task: {}",
                err
            ))
        })?
}

async fn repo_file_tree_cache_is_fresh(path: &Path) -> Result<bool> {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(PumasError::io_with_path(err, path)),
    };

    Ok(metadata
        .modified()
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .map(|elapsed| elapsed.as_secs() < REPO_CACHE_TTL_SECS)
        .unwrap_or(false))
}
