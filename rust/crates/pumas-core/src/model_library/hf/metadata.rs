//! HuggingFace metadata fetching and model verification.
//!
//! Handles direct model info lookups, repository file tree retrieval,
//! metadata lookup by filename/hash, and candidate verification.

use super::types::{HfFileEntry, HfSearchResult, HF_API_BASE, HF_HUB_BASE, REPO_CACHE_TTL_SECS};
use super::HuggingFaceClient;
use crate::error::{PumasError, Result};
use crate::model_library::hashing::compute_fast_hash;
use crate::model_library::naming::extract_base_name;
use crate::model_library::types::{
    HfMetadataResult, HfSearchParams, HuggingFaceModel, LfsFileInfo, RepoFileTree,
};
use std::path::Path;
use std::time::Duration;

impl HuggingFaceClient {
    /// Fetch model info directly by repo_id from the HuggingFace API.
    ///
    /// Uses `GET /api/models/{repo_id}` which returns the exact model
    /// without any search or cache involvement.
    pub async fn get_model_info(&self, repo_id: &str) -> Result<HuggingFaceModel> {
        // repo_id is "owner/model" -- the slash is part of the URL path,
        // so we must not encode the whole string (that would turn / into %2F).
        let url = format!("{}/models/{}", HF_API_BASE, repo_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PumasError::Network {
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

        let result: HfSearchResult = response.json().await.map_err(|e| PumasError::Json {
            message: format!("Failed to parse HuggingFace response: {}", e),
            source: None,
        })?;

        Ok(Self::convert_search_result(result))
    }

    /// Get repository file tree with LFS information.
    ///
    /// Results are cached for 24 hours.
    pub async fn get_repo_files(&self, repo_id: &str) -> Result<RepoFileTree> {
        // Check cache first
        let cache_file = self.get_cache_path(repo_id, "files");
        if let Some(cached) = self.read_cache::<RepoFileTree>(&cache_file)? {
            // Check if cache is still valid
            if let Ok(meta) = std::fs::metadata(&cache_file) {
                if let Ok(modified) = meta.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        if elapsed.as_secs() < REPO_CACHE_TTL_SECS {
                            return Ok(cached);
                        }
                    }
                }
            }
        }

        // Fetch from API
        let url = format!("{}/api/models/{}/tree/main", HF_HUB_BASE, repo_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PumasError::Network {
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
        };

        // Cache the result
        self.write_cache(&cache_file, &tree)?;

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
            let fast_hash = compute_fast_hash(path).ok();

            // Try to match against top candidates
            for candidate in candidates.iter().take(2) {
                if let Ok(result) = self
                    .verify_candidate(&candidate.repo_id, filename, path, fast_hash.as_deref())
                    .await
                {
                    if let Some(result) = result {
                        return Ok(Some(result));
                    }
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
