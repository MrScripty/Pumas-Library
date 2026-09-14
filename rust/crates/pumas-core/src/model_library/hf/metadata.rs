//! HuggingFace metadata fetching and model verification.
//!
//! Handles direct model info lookups, repository file tree retrieval,
//! metadata lookup by filename/hash, and candidate verification.

use super::types::{
    infer_pipeline_tag_from_config, HfFileEntry, HfSearchResult, HF_HUB_BASE, REPO_CACHE_TTL_SECS,
};
use super::HuggingFaceClient;
use crate::error::{PumasError, Result};
use crate::metadata::{atomic_read_json, atomic_write_json};
use crate::model_library::artifact_identity::DownloadRevision;
use crate::model_library::hashing::compute_fast_hash;
use crate::model_library::naming::extract_base_name;
use crate::model_library::types::{
    DownloadRequest, HfMetadataResult, HfSearchParams, HuggingFaceEvidence, HuggingFaceModel,
    LfsFileInfo, RepoFileTree, REPO_FILE_TREE_VERSION,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(serde::Deserialize)]
struct HfModelInfoResponse {
    #[serde(flatten)]
    model: HfSearchResult,
    #[serde(default)]
    sha: Option<String>,
}

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
        self.get_model_snapshot_at_revision(repo_id, &DownloadRevision::legacy_main())
            .await
    }

    pub(crate) async fn get_model_snapshot_at_revision(
        &self,
        repo_id: &str,
        revision: &DownloadRevision,
    ) -> Result<(HuggingFaceModel, HuggingFaceEvidence)> {
        let result = self.fetch_model_info_response(repo_id, revision).await?;
        let evidence = Self::build_huggingface_evidence(repo_id, &result);
        let model = Self::convert_search_result(result);
        Ok((model, evidence))
    }

    pub(crate) async fn resolve_download_revision(
        &self,
        repo_id: &str,
        selector: Option<&str>,
    ) -> Result<DownloadRevision> {
        validate_hf_repo_id(repo_id)?;
        let selector = match selector {
            Some(value) => validate_revision_selector(value)?,
            None => "main",
        };
        let encoded_selector = urlencoding::encode(selector);
        let url = format!(
            "{}/api/models/{repo_id}/revision/{encoded_selector}",
            self.hub_base_url()
        );
        let response = self.fetch_model_info_url(repo_id, url).await?;
        if response.model.model_id.trim() != repo_id {
            return Err(PumasError::Validation {
                field: "repo_id".to_string(),
                message: "HuggingFace revision response identified a different repository"
                    .to_string(),
            });
        }
        let commit = response
            .sha
            .as_deref()
            .ok_or_else(|| PumasError::Validation {
                field: "revision".to_string(),
                message: "HuggingFace revision response did not identify an immutable commit"
                    .to_string(),
            })?;
        let resolved =
            DownloadRevision::from_commit(commit.trim()).map_err(|_| PumasError::Validation {
                field: "revision".to_string(),
                message: "HuggingFace revision response did not contain a valid immutable commit"
                    .to_string(),
            })?;
        if DownloadRevision::from_commit(selector).is_ok_and(|requested| requested != resolved) {
            return Err(PumasError::Validation {
                field: "revision".to_string(),
                message:
                    "HuggingFace revision response contradicted the requested immutable commit"
                        .to_string(),
            });
        }
        Ok(resolved)
    }

    async fn fetch_model_info_response(
        &self,
        repo_id: &str,
        revision: &DownloadRevision,
    ) -> Result<HfSearchResult> {
        let url = match revision.as_persisted() {
            Some(commit) => format!(
                "{}/api/models/{}/revision/{}",
                self.hub_base_url(),
                repo_id,
                commit
            ),
            None => format!("{}/models/{}", self.api_base_url(), repo_id),
        };

        let response = self.fetch_model_info_url(repo_id, url).await?;
        if let Some(expected) = revision.as_persisted() {
            if response.model.model_id.trim() != repo_id {
                return Err(PumasError::Validation {
                    field: "repo_id".to_string(),
                    message: "HuggingFace revision response identified a different repository"
                        .to_string(),
                });
            }
            let actual = response.sha.as_deref().map(str::trim);
            if actual != Some(expected) {
                return Err(PumasError::Validation {
                    field: "revision".to_string(),
                    message: "HuggingFace revision response did not confirm the requested commit"
                        .to_string(),
                });
            }
        }
        Ok(response.model)
    }

    async fn fetch_model_info_url(
        &self,
        repo_id: &str,
        url: String,
    ) -> Result<HfModelInfoResponse> {
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
        self.get_repo_files_at_revision(repo_id, &DownloadRevision::legacy_main())
            .await
    }

    pub(crate) async fn get_repo_files_at_revision(
        &self,
        repo_id: &str,
        revision: &DownloadRevision,
    ) -> Result<RepoFileTree> {
        // Check cache first
        let cache_repo_id = revision_cache_key(repo_id, revision);
        let cache_file = self.get_cache_path(&cache_repo_id, "files");
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
            "{}/api/models/{}/tree/{}?recursive=true",
            self.hub_base_url(),
            repo_id,
            revision.as_str()
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

fn revision_cache_key(repo_id: &str, revision: &DownloadRevision) -> String {
    match revision.as_persisted() {
        Some(commit) => format!("{repo_id}@{commit}"),
        None => repo_id.to_string(),
    }
}

fn validate_hf_repo_id(repo_id: &str) -> Result<()> {
    let mut segments = repo_id.split('/');
    let owner = segments.next().unwrap_or_default();
    let name = segments.next().unwrap_or_default();
    let valid_segment = |segment: &str| {
        !segment.is_empty()
            && segment.len() <= 96
            && segment.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            })
            && !segment.starts_with('-')
            && !segment.starts_with('.')
            && !segment.ends_with('-')
            && !segment.ends_with('.')
            && !segment.contains("..")
            && !segment.contains("--")
    };
    if segments.next().is_none()
        && valid_segment(owner)
        && valid_segment(name)
        && !name.to_ascii_lowercase().ends_with(".git")
    {
        Ok(())
    } else {
        Err(PumasError::Validation {
            field: "repo_id".to_string(),
            message: "HuggingFace repository ID must be a valid owner/name identifier".to_string(),
        })
    }
}

fn validate_revision_selector(selector: &str) -> Result<&str> {
    if selector.is_empty()
        || selector.trim() != selector
        || selector.len() > 1024
        || selector.chars().any(char::is_control)
        || matches!(selector, "." | "..")
    {
        Err(PumasError::Validation {
            field: "revision".to_string(),
            message: "HuggingFace revision selector is invalid".to_string(),
        })
    } else {
        Ok(selector)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n\r\n") {
            assert!(
                bytes.len() < 8 * 1024,
                "fixture request exceeded header limit"
            );
            bytes.push(socket.read_u8().await.unwrap());
        }
        String::from_utf8(bytes).unwrap()
    }

    async fn serve_once(status: &str, body: String) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let status = status.to_string();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_request(&mut socket).await;
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            request
        });
        (base_url, server)
    }

    #[tokio::test]
    async fn revision_resolver_encodes_selectors_and_returns_immutable_commit() {
        let commit = "abcdefabcdefabcdefabcdefabcdefabcdefabcd";
        for (selector, encoded) in [
            (None, "main"),
            (Some("release-v1.2"), "release-v1.2"),
            (Some("feature/quantized"), "feature%2Fquantized"),
            (Some("percent%branch"), "percent%25branch"),
        ] {
            let (base_url, server) = serve_once(
                "200 OK",
                format!(r#"{{"modelId":"acme/model","sha":"{commit}"}}"#),
            )
            .await;
            let temp = TempDir::new().unwrap();
            let mut client = HuggingFaceClient::new(temp.path()).unwrap();
            client.set_test_download_base_url(base_url);

            let revision = client
                .resolve_download_revision("acme/model", selector)
                .await
                .unwrap();

            assert_eq!(revision.as_str(), commit);
            assert!(server.await.unwrap().starts_with(&format!(
                "GET /api/models/acme/model/revision/{encoded} HTTP/1.1"
            )));
        }
    }

    #[tokio::test]
    async fn revision_resolver_rejects_substitution_of_an_explicit_immutable_commit() {
        let requested = "a".repeat(40);
        let returned = "b".repeat(40);
        let (base_url, server) = serve_once(
            "200 OK",
            format!(r#"{{"modelId":"acme/model","sha":"{returned}"}}"#),
        )
        .await;
        let temp = TempDir::new().unwrap();
        let mut client = HuggingFaceClient::new(temp.path()).unwrap();
        client.set_test_download_base_url(base_url);
        let error = client
            .resolve_download_revision("acme/model", Some(&requested))
            .await
            .unwrap_err();
        assert!(
            matches!(error, PumasError::Validation { ref field, .. } if field == "revision"),
            "expected contradictory revision validation, got {error:?}"
        );
        assert!(server.await.unwrap().starts_with(&format!(
            "GET /api/models/acme/model/revision/{requested} HTTP/1.1"
        )));
    }

    #[tokio::test]
    async fn revision_resolver_rejects_invalid_inputs_and_untrusted_responses() {
        let temp = TempDir::new().unwrap();
        let client = HuggingFaceClient::new(temp.path()).unwrap();
        for (repo_id, selector) in [
            ("model", Some("main")),
            ("../model", Some("main")),
            ("acme/model", Some("")),
            ("acme/model", Some(" branch")),
            ("acme/model", Some("branch\nname")),
        ] {
            assert!(matches!(
                client.resolve_download_revision(repo_id, selector).await,
                Err(PumasError::Validation { .. })
            ));
        }

        for (status, body, expected_field) in [
            (
                "200 OK",
                r#"{"modelId":"acme/model"}"#.to_string(),
                Some("revision"),
            ),
            (
                "200 OK",
                r#"{"modelId":"acme/model","sha":"not-a-commit"}"#.to_string(),
                Some("revision"),
            ),
            (
                "200 OK",
                r#"{"modelId":"other/model","sha":"abcdefabcdefabcdefabcdefabcdefabcdefabcd"}"#
                    .to_string(),
                Some("repo_id"),
            ),
            ("404 Not Found", "{}".to_string(), None),
        ] {
            let (base_url, server) = serve_once(status, body).await;
            let temp = TempDir::new().unwrap();
            let mut client = HuggingFaceClient::new(temp.path()).unwrap();
            client.set_test_download_base_url(base_url);

            let error = client
                .resolve_download_revision("acme/model", Some("release/v1"))
                .await
                .unwrap_err();

            match expected_field {
                Some(expected) => assert!(
                    matches!(
                        error,
                        PumasError::Validation { ref field, .. } if field == expected
                    ),
                    "expected validation field {expected}, got {error:?}"
                ),
                None => assert!(matches!(error, PumasError::Network { .. })),
            }
            assert!(server
                .await
                .unwrap()
                .starts_with("GET /api/models/acme/model/revision/release%2Fv1 HTTP/1.1"));
        }
    }

    #[tokio::test]
    async fn pinned_snapshot_uses_revision_endpoint_and_requires_matching_sha() {
        let commit = "0123456789abcdef0123456789abcdef01234567";
        let (base_url, server) = serve_once(
            "200 OK",
            format!(r#"{{"modelId":"acme/model","sha":"{commit}"}}"#),
        )
        .await;
        let temp = TempDir::new().unwrap();
        let mut client = HuggingFaceClient::new(temp.path()).unwrap();
        client.set_test_download_base_url(base_url);
        let revision = DownloadRevision::from_commit(commit).unwrap();

        let (model, evidence) = client
            .get_model_snapshot_at_revision("acme/model", &revision)
            .await
            .unwrap();

        assert_eq!(model.repo_id, "acme/model");
        assert_eq!(evidence.repo_id.as_deref(), Some("acme/model"));
        assert!(server.await.unwrap().starts_with(&format!(
            "GET /api/models/acme/model/revision/{commit} HTTP/1.1"
        )));
    }

    #[tokio::test]
    async fn legacy_snapshot_keeps_api_route_and_does_not_require_sha() {
        let (base_url, server) =
            serve_once("200 OK", r#"{"modelId":"acme/model"}"#.to_string()).await;
        let temp = TempDir::new().unwrap();
        let mut client = HuggingFaceClient::new(temp.path()).unwrap();
        client.set_test_download_base_url(base_url);

        let (model, _) = client.get_model_snapshot("acme/model").await.unwrap();

        assert_eq!(model.repo_id, "acme/model");
        assert!(server
            .await
            .unwrap()
            .starts_with("GET /models/acme/model HTTP/1.1"));
    }

    #[tokio::test]
    async fn pinned_snapshot_rejects_absent_or_mismatched_sha() {
        let commit = "0123456789abcdef0123456789abcdef01234567";
        for body in [
            r#"{"modelId":"acme/model"}"#.to_string(),
            r#"{"modelId":"acme/model","sha":"ffffffffffffffffffffffffffffffffffffffff"}"#
                .to_string(),
        ] {
            let (base_url, server) = serve_once("200 OK", body).await;
            let temp = TempDir::new().unwrap();
            let mut client = HuggingFaceClient::new(temp.path()).unwrap();
            client.set_test_download_base_url(base_url);
            let revision = DownloadRevision::from_commit(commit).unwrap();

            let error = client
                .get_model_snapshot_at_revision("acme/model", &revision)
                .await
                .unwrap_err();

            assert!(matches!(error, PumasError::Validation { .. }));
            assert!(server.await.unwrap().starts_with(&format!(
                "GET /api/models/acme/model/revision/{commit} HTTP/1.1"
            )));
        }
    }

    #[tokio::test]
    async fn pinned_snapshot_rejects_contradicting_repo_and_upstream_failure() {
        let commit = "0123456789abcdef0123456789abcdef01234567";
        for (status, body, expected_validation) in [
            (
                "200 OK",
                format!(r#"{{"modelId":"other/model","sha":"{commit}"}}"#),
                true,
            ),
            ("404 Not Found", "{}".to_string(), false),
        ] {
            let (base_url, server) = serve_once(status, body).await;
            let temp = TempDir::new().unwrap();
            let mut client = HuggingFaceClient::new(temp.path()).unwrap();
            client.set_test_download_base_url(base_url);
            let revision = DownloadRevision::from_commit(commit).unwrap();

            let error = client
                .get_model_snapshot_at_revision("acme/model", &revision)
                .await
                .unwrap_err();

            if expected_validation {
                assert!(matches!(
                    error,
                    PumasError::Validation { ref field, .. } if field == "repo_id"
                ));
            } else {
                assert!(matches!(error, PumasError::Network { .. }));
            }
            assert!(server.await.unwrap().starts_with(&format!(
                "GET /api/models/acme/model/revision/{commit} HTTP/1.1"
            )));
        }
    }

    #[tokio::test]
    async fn pinned_repo_tree_uses_revision_specific_urls_and_caches() {
        let first = "1111111111111111111111111111111111111111";
        let second = "2222222222222222222222222222222222222222";
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (commit, filename) in [(first, "first.gguf"), (second, "second.gguf")] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let request = read_request(&mut socket).await;
                assert!(request.starts_with(&format!(
                    "GET /api/models/acme/model/tree/{commit}?recursive=true HTTP/1.1"
                )));
                requests.push(request);
                let body = format!(
                    r#"[{{"path":"{filename}","type":"file","lfs":{{"oid":"{commit}","size":1}}}}]"#
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });
        let temp = TempDir::new().unwrap();
        let mut client = HuggingFaceClient::new(temp.path()).unwrap();
        client.set_test_download_base_url(base_url);
        let first_revision = DownloadRevision::from_commit(first).unwrap();
        let second_revision = DownloadRevision::from_commit(second).unwrap();

        let first_tree = client
            .get_repo_files_at_revision("acme/model", &first_revision)
            .await
            .unwrap();
        let cached_first_tree = client
            .get_repo_files_at_revision("acme/model", &first_revision)
            .await
            .unwrap();
        let second_tree = client
            .get_repo_files_at_revision("acme/model", &second_revision)
            .await
            .unwrap();

        assert_eq!(first_tree.lfs_files[0].filename, "first.gguf");
        assert_eq!(cached_first_tree.lfs_files[0].filename, "first.gguf");
        assert_eq!(second_tree.lfs_files[0].filename, "second.gguf");
        assert_eq!(server.await.unwrap().len(), 2);
        assert_ne!(
            client.get_cache_path(&revision_cache_key("acme/model", &first_revision), "files"),
            client.get_cache_path(&revision_cache_key("acme/model", &second_revision), "files")
        );
    }

    #[tokio::test]
    async fn resolved_main_stays_pinned_after_branch_moves() {
        let first = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let second = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let main_commit = std::sync::Arc::new(std::sync::Mutex::new(first.to_string()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let server_main = main_commit.clone();
        let server = tokio::spawn(async move {
            let mut request_lines = Vec::new();
            for _ in 0..3 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let request = read_request(&mut socket).await;
                let request_line = request.lines().next().unwrap().to_string();
                let current_main = server_main.lock().unwrap().clone();
                let body = if request_line == "GET /api/models/acme/model/revision/main HTTP/1.1" {
                    format!(r#"{{"modelId":"acme/model","sha":"{current_main}"}}"#)
                } else if request_line
                    == format!("GET /api/models/acme/model/revision/{first} HTTP/1.1")
                {
                    format!(r#"{{"modelId":"acme/model","sha":"{first}"}}"#)
                } else if request_line
                    == format!("GET /api/models/acme/model/tree/{first}?recursive=true HTTP/1.1")
                {
                    format!(
                        r#"[{{"path":"from-{first}.gguf","type":"file","lfs":{{"oid":"{first}","size":1}}}}]"#
                    )
                } else {
                    panic!("request escaped resolved commit: {request_line}");
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.unwrap();
                request_lines.push(request_line);
            }
            request_lines
        });

        let resolved: serde_json::Value = reqwest::Client::new()
            .get(format!("{base_url}/api/models/acme/model/revision/main"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let revision = DownloadRevision::from_commit(resolved["sha"].as_str().unwrap()).unwrap();
        *main_commit.lock().unwrap() = second.to_string();

        let temp = TempDir::new().unwrap();
        let mut client = HuggingFaceClient::new(temp.path()).unwrap();
        client.set_test_download_base_url(base_url);
        let (model, _) = client
            .get_model_snapshot_at_revision("acme/model", &revision)
            .await
            .unwrap();
        let tree = client
            .get_repo_files_at_revision("acme/model", &revision)
            .await
            .unwrap();

        assert_eq!(revision.as_str(), first);
        assert_eq!(model.repo_id, "acme/model");
        assert_eq!(tree.lfs_files[0].filename, format!("from-{first}.gguf"));
        let request_lines = server.await.unwrap();
        assert_eq!(
            request_lines,
            vec![
                "GET /api/models/acme/model/revision/main HTTP/1.1".to_string(),
                format!("GET /api/models/acme/model/revision/{first} HTTP/1.1"),
                format!("GET /api/models/acme/model/tree/{first}?recursive=true HTTP/1.1"),
            ]
        );
    }
}
