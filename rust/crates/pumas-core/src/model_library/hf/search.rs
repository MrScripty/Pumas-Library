//! HuggingFace model search with caching and enrichment.
//!
//! Provides search against the HuggingFace Hub API with transparent
//! SQLite caching, result enrichment with download options, and
//! conversion from API response types to internal model types.

use super::types::{infer_pipeline_tag_from_config, HfSearchResult, HfSibling, HF_API_BASE};
use super::HuggingFaceClient;
use crate::error::{PumasError, Result};
use crate::model_library::sharding::group_weight_files;
use crate::model_library::types::{HfSearchParams, HuggingFaceModel, RepoFileTree};
use crate::models::{DownloadOption, FileGroup};
use std::collections::BTreeMap;
use std::sync::OnceLock;
use tracing::{debug, info, warn};

const MIN_QUANT_COVERAGE_GAP_BYTES: u64 = 5 * 1024 * 1024 * 1024;
const MIN_EXPECTED_QUANT_COVERAGE_PERCENT: u64 = 90;

fn quant_token_regex() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(?:^|[._/-])((?:UD-)?(?:IQ\d+_[A-Z0-9_]+|Q\d+_[A-Z0-9_]+)|fp16|fp32|bf16|int8|int4)(?:$|[._/-])",
        )
        .ok()
    })
    .as_ref()
}

fn quant_tag_regex() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)^(?:(?:UD-)?(?:IQ\d+_[A-Z0-9_]+|Q\d+_[A-Z0-9_]+)|fp16|fp32|bf16|int8|int4)$",
        )
        .ok()
    })
    .as_ref()
}

fn quant_option_size_sum(download_options: &[DownloadOption]) -> u64 {
    download_options.iter().filter_map(|o| o.size_bytes).sum()
}

fn likely_missing_quant_variants(
    quants: &[String],
    total_size_bytes: Option<u64>,
    download_options: &[DownloadOption],
) -> bool {
    if quants.is_empty() || download_options.is_empty() {
        return false;
    }

    // Quant-based GGUF options do not use file groups.
    if download_options.iter().any(|o| o.file_group.is_some()) {
        return false;
    }
    // If we already expose at least as many options as known quants,
    // low size coverage is likely due to auxiliary LFS files, not missing quants.
    if download_options.len() >= quants.len() {
        return false;
    }

    let Some(total) = total_size_bytes else {
        return false;
    };
    if total == 0 {
        return false;
    }

    let covered = quant_option_size_sum(download_options);
    if covered == 0 || covered >= total {
        return false;
    }

    let gap = total - covered;
    let coverage_percent = covered.saturating_mul(100) / total;

    gap > MIN_QUANT_COVERAGE_GAP_BYTES && coverage_percent < MIN_EXPECTED_QUANT_COVERAGE_PERCENT
}

impl HuggingFaceClient {
    /// Search for models on HuggingFace with automatic caching.
    ///
    /// This method transparently handles caching:
    /// - Checks SQLite cache for recent search results
    /// - Falls back to HuggingFace API if cache miss or stale
    /// - Enriches results with download options (file sizes)
    /// - Caches results for future queries
    ///
    /// # Arguments
    ///
    /// * `params` - Search parameters
    pub async fn search(&self, params: &HfSearchParams) -> Result<Vec<HuggingFaceModel>> {
        // If we have a cache, use it transparently
        let cache = match &self.search_cache {
            Some(c) => c,
            None => {
                // No cache configured, use direct API
                return self.search_api(params).await;
            }
        };

        let limit = params.limit.unwrap_or(20);
        let offset = params.offset.unwrap_or(0);
        let kind = params.kind.as_deref();

        // Check cache for existing search results
        match cache.get_search_results(&params.query, kind, limit, offset) {
            Ok(Some(models)) => {
                info!(
                    "Cache hit for search '{}': {} models",
                    params.query,
                    models.len()
                );
                // Re-enrich cached search hits so extraction fixes and cache
                // migration heuristics can self-heal stale repo entries.
                let enriched = self
                    .enrich_models_with_download_options(
                        &models,
                        params.hydrate_limit.unwrap_or(limit),
                    )
                    .await;

                for model in &enriched {
                    if let Err(e) = cache.cache_repo_details(model) {
                        warn!(
                            "Failed to refresh cached repo details for {}: {}",
                            model.repo_id, e
                        );
                    }
                }

                return Ok(enriched);
            }
            Ok(None) => {
                debug!("Cache miss for search '{}'", params.query);
            }
            Err(e) => {
                warn!("Cache error, falling back to API: {}", e);
            }
        }

        // Cache miss - perform API search
        let models = self.search_api(params).await?;

        // Enrich models with download options from cache or API
        let enriched = self
            .enrich_models_with_download_options(&models, params.hydrate_limit.unwrap_or(limit))
            .await;

        // Cache the search results
        let repo_ids: Vec<String> = enriched.iter().map(|m| m.repo_id.clone()).collect();
        if let Err(e) = cache.cache_search_results(&params.query, kind, limit, offset, &repo_ids) {
            warn!("Failed to cache search results: {}", e);
        }

        // Cache individual model details
        for model in &enriched {
            if let Err(e) = cache.cache_repo_details(model) {
                warn!("Failed to cache repo details for {}: {}", model.repo_id, e);
            }
        }

        Ok(enriched)
    }

    /// Direct API search without caching (internal use).
    ///
    /// # Arguments
    ///
    /// * `params` - Search parameters
    pub(super) async fn search_api(
        &self,
        params: &HfSearchParams,
    ) -> Result<Vec<HuggingFaceModel>> {
        let limit = params.limit.unwrap_or(20);
        let offset = params.offset.unwrap_or(0);

        // Build search URL
        // Note: full=true gets lastModified, config=true gets architectures/model_type
        let mut url = format!(
            "{}/models?search={}&limit={}&offset={}&full=true&config=true",
            HF_API_BASE,
            urlencoding::encode(&params.query),
            limit,
            offset
        );

        // Add kind filter
        if let Some(ref kind) = params.kind {
            let pipeline_tag = match kind.as_str() {
                "text-generation" | "llm" => "text-generation",
                "text-ranking" | "reranker" => "text-ranking",
                "text-to-image" | "diffusion" => "text-to-image",
                "image-to-image" => "image-to-image",
                "automatic-speech-recognition" | "audio" => "automatic-speech-recognition",
                _ => kind,
            };
            url.push_str(&format!("&pipeline_tag={}", pipeline_tag));
        }

        // Execute request
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
                message: format!("HuggingFace API returned {}", response.status()),
                cause: None,
            });
        }

        let results: Vec<HfSearchResult> = response.json().await.map_err(|e| PumasError::Json {
            message: format!("Failed to parse HuggingFace response: {}", e),
            source: None,
        })?;

        // Convert to our model type
        let models: Vec<HuggingFaceModel> = results
            .into_iter()
            .map(Self::convert_search_result)
            .collect();

        Ok(models)
    }

    /// Enrich models with download options (file sizes) from cache or API.
    async fn enrich_models_with_download_options(
        &self,
        models: &[HuggingFaceModel],
        hydrate_limit: usize,
    ) -> Vec<HuggingFaceModel> {
        let mut enriched = Vec::with_capacity(models.len());

        for (index, model) in models.iter().enumerate() {
            let mut model = model.clone();

            // Try to get download options from cache first
            if let Some(cache) = &self.search_cache {
                // Check if we need to refresh based on lastModified
                let needs_refresh = cache
                    .needs_refresh(&model.repo_id, model.release_date.as_deref())
                    .unwrap_or(true);

                if !needs_refresh {
                    // Use cached details
                    if let Ok(Some(cached)) = cache.get_repo_details(&model.repo_id) {
                        if !cached.download_options.is_empty() {
                            // For non-quant repos, re-enrich if cached options
                            // lack file_group data (pre-grouping cache entries).
                            let needs_regroup = model.quants.is_empty()
                                && cached
                                    .download_options
                                    .iter()
                                    .all(|o| o.file_group.is_none());
                            // Self-heal stale quant caches where only the first
                            // shard size was stored per quant (shows as ~0.01 GB).
                            let likely_truncated_quant_sizes = !model.quants.is_empty()
                                && cached.download_options.len() > 1
                                && cached.total_size_bytes.unwrap_or(0) > 10 * 1024 * 1024 * 1024
                                && cached.download_options.iter().any(|o| {
                                    o.file_group.is_none()
                                        && matches!(o.size_bytes, Some(size) if size > 0 && size < 128 * 1024 * 1024)
                                });
                            // Self-heal legacy quant extraction where IQ/UD variants
                            // were not included in cached options.
                            let missing_quant_variants = likely_missing_quant_variants(
                                &model.quants,
                                cached.total_size_bytes,
                                &cached.download_options,
                            );

                            if !needs_regroup
                                && !likely_truncated_quant_sizes
                                && !missing_quant_variants
                            {
                                model.download_options = cached.download_options;
                                model.total_size_bytes = cached.total_size_bytes;
                                enriched.push(model);
                                continue;
                            }
                        }
                    }
                }
            }

            if index >= hydrate_limit {
                enriched.push(model);
                continue;
            }

            // Fetch from API
            match self
                .get_download_details(&model.repo_id, &model.quants)
                .await
            {
                Ok(details) => {
                    model.download_options = details.download_options;
                    model.total_size_bytes = details.total_size_bytes;
                }
                Err(e) => {
                    debug!("Failed to fetch repo files for {}: {}", model.repo_id, e);
                    // Keep model without download options
                }
            }

            enriched.push(model);
        }

        enriched
    }

    /// Extract download options from repo file tree.
    ///
    /// For GGUF repos with recognisable quant patterns the options remain
    /// quant-based (unchanged behaviour).  For non-quant repos (safetensors,
    /// diffusers layouts) the LFS files are grouped by shard set so that the
    /// UI shows one entry per logical weight file rather than one per shard.
    fn extract_download_options_from_tree(
        tree: &RepoFileTree,
        quants: &[String],
    ) -> Vec<DownloadOption> {
        // First try quant-based extraction (GGUF repos, fp16/bf16 variants).
        let quant_options = Self::extract_quant_based_options(tree, quants);
        if !quant_options.is_empty() {
            return quant_options;
        }

        // No quants detected – use shard-aware grouping.
        Self::extract_grouped_options(tree)
    }

    /// Quant-based option extraction for GGUF and precision-variant repos.
    fn extract_quant_based_options(tree: &RepoFileTree, quants: &[String]) -> Vec<DownloadOption> {
        // Aggregate all matching files per quant so sharded variants report
        // their total size instead of the first shard's size.
        let mut quant_sizes: BTreeMap<String, u64> = BTreeMap::new();
        let quant_pattern = quant_token_regex();

        for lfs_file in &tree.lfs_files {
            if !lfs_file.filename.ends_with(".gguf")
                && !lfs_file.filename.ends_with(".safetensors")
                && !lfs_file.filename.ends_with(".bin")
            {
                continue;
            }

            let quant = quant_pattern.and_then(|pattern| {
                pattern
                    .captures(&lfs_file.filename)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str().to_string())
            });

            if let Some(q) = quant {
                *quant_sizes.entry(q).or_insert(0) += lfs_file.size;
            } else if quants.iter().any(|q| lfs_file.filename.contains(q)) {
                for q in quants {
                    if lfs_file.filename.contains(q) {
                        *quant_sizes.entry(q.clone()).or_insert(0) += lfs_file.size;
                        break;
                    }
                }
            }
        }

        quant_sizes
            .into_iter()
            .map(|(quant, size_bytes)| DownloadOption {
                quant,
                size_bytes: Some(size_bytes),
                file_group: None,
            })
            .collect()
    }

    /// Shard-aware option extraction for repos without quant patterns.
    ///
    /// Groups sharded files into single entries and includes standalone
    /// weight files as individual options, each with a [`FileGroup`]
    /// describing the exact files to download.
    fn extract_grouped_options(tree: &RepoFileTree) -> Vec<DownloadOption> {
        let (weight_groups, _non_weight) = group_weight_files(&tree.lfs_files);

        weight_groups
            .into_iter()
            .map(|g| {
                let display = if g.shard_count > 1 {
                    format!("{} ({} shards)", g.label, g.shard_count)
                } else {
                    g.label.clone()
                };
                DownloadOption {
                    quant: display,
                    size_bytes: Some(g.total_size),
                    file_group: Some(FileGroup {
                        filenames: g.filenames,
                        shard_count: g.shard_count,
                        label: g.label,
                    }),
                }
            })
            .collect()
    }

    /// Convert HF search result to our model type.
    pub(super) fn convert_search_result(result: HfSearchResult) -> HuggingFaceModel {
        // Extract name from modelId (after the /)
        let name = result
            .model_id
            .split('/')
            .next_back()
            .unwrap_or(&result.model_id)
            .to_string();

        // Extract developer from modelId (before the /)
        let developer = result.model_id.split('/').next().unwrap_or("").to_string();

        // Determine kind: prefer pipeline_tag, fall back to config-based inference
        let kind = result
            .pipeline_tag
            .or_else(|| Self::infer_pipeline_tag_from_tags(&result.tags))
            .or_else(|| infer_pipeline_tag_from_config(result.config.as_ref()))
            .unwrap_or_else(|| "unknown".to_string());

        // Extract formats and quants from tags
        let formats: Vec<String> = result
            .tags
            .iter()
            .filter(|t| ["gguf", "safetensors", "pytorch", "onnx"].contains(&t.as_str()))
            .cloned()
            .collect();

        // Extract quants from tags first
        let mut quants: Vec<String> = result
            .tags
            .iter()
            .filter(|t| quant_tag_regex().is_some_and(|pattern| pattern.is_match(t)))
            .cloned()
            .collect();

        // If no quants from tags, extract from sibling filenames (GGUF models)
        if quants.is_empty() {
            quants = Self::extract_quants_from_filenames(&result.siblings);
        }

        // Build URL for the model page
        let url = format!("https://huggingface.co/{}", result.model_id);

        // Detect compatible inference engines based on formats
        let compatible_engines = crate::models::detect_compatible_engines(&formats);

        HuggingFaceModel {
            repo_id: result.model_id,
            name,
            developer,
            kind,
            formats,
            quants,
            download_options: vec![], // Populated by get_download_options
            url,
            release_date: result.last_modified,
            downloads: result.downloads,
            total_size_bytes: None,
            quant_sizes: None,
            compatible_engines,
        }
    }

    /// Extract quantization names from sibling filenames.
    ///
    /// Looks for patterns like Q4_K_M, Q8_0, etc. in GGUF/model filenames.
    fn extract_quants_from_filenames(siblings: &[HfSibling]) -> Vec<String> {
        use std::collections::HashSet;

        let Some(quant_pattern) = quant_token_regex() else {
            return Vec::new();
        };

        let mut quants: HashSet<String> = HashSet::new();

        for sibling in siblings {
            let filename = &sibling.rfilename;
            // Only check model files (gguf, safetensors, etc.)
            if filename.ends_with(".gguf")
                || filename.ends_with(".safetensors")
                || filename.ends_with(".bin")
            {
                for cap in quant_pattern.captures_iter(filename) {
                    if let Some(m) = cap.get(1) {
                        quants.insert(m.as_str().to_string());
                    }
                }
            }
        }

        let mut sorted: Vec<String> = quants.into_iter().collect();
        sorted.sort();
        sorted
    }

    pub(crate) fn infer_pipeline_tag_from_tags(tags: &[String]) -> Option<String> {
        for tag in tags {
            let normalized = tag.trim().to_lowercase().replace([' ', '_'], "-");
            match normalized.as_str() {
                "text-ranking" | "text-reranking" | "reranking" => {
                    return Some("text-ranking".to_string());
                }
                "text-to-speech" | "speech-synthesis" => {
                    return Some("text-to-speech".to_string());
                }
                "text-to-audio" => {
                    return Some("text-to-audio".to_string());
                }
                "automatic-speech-recognition" | "speech-recognition" | "asr" => {
                    return Some("automatic-speech-recognition".to_string());
                }
                "audio-classification" => {
                    return Some("audio-classification".to_string());
                }
                _ => {}
            }
        }
        None
    }

    pub async fn get_download_details(
        &self,
        repo_id: &str,
        quants: &[String],
    ) -> Result<crate::models::HfDownloadDetails> {
        let tree = self.get_repo_files(repo_id).await?;
        let download_options = Self::extract_download_options_from_tree(&tree, quants);
        let total_size_bytes = Some(tree.lfs_files.iter().map(|f| f.size).sum());

        Ok(crate::models::HfDownloadDetails {
            repo_id: repo_id.to_string(),
            download_options,
            total_size_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_library::hf_cache::HfSearchCache;
    use crate::model_library::types::LfsFileInfo;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn lfs(filename: &str, size: u64) -> LfsFileInfo {
        LfsFileInfo {
            filename: filename.to_string(),
            size,
            sha256: String::new(),
        }
    }

    #[test]
    fn test_quant_options_sum_sharded_sizes() {
        let tree = RepoFileTree {
            repo_id: "unsloth/Qwen3.5-122B-A10B-GGUF".to_string(),
            lfs_files: vec![
                lfs("UD-Q3_K_M/model-00001-of-00003.gguf", 10_900_000),
                lfs("UD-Q3_K_M/model-00002-of-00003.gguf", 49_700_000_000),
                lfs("UD-Q3_K_M/model-00003-of-00003.gguf", 37_500_000_000),
                lfs("Q2_K/model-Q2_K.gguf", 41_800_000_000),
            ],
            regular_files: vec![],
            cached_at: "2026-01-01T00:00:00Z".to_string(),
            last_modified: None,
            cache_version: crate::model_library::types::REPO_FILE_TREE_VERSION,
        };

        let options = HuggingFaceClient::extract_quant_based_options(
            &tree,
            &["Q2_K".into(), "Q3_K_M".into()],
        );

        assert_eq!(options.len(), 2);
        assert_eq!(options[0].quant, "Q2_K");
        assert_eq!(options[0].size_bytes, Some(41_800_000_000));
        assert_eq!(options[1].quant, "UD-Q3_K_M");
        assert_eq!(options[1].size_bytes, Some(87_210_900_000));
    }

    #[test]
    fn test_extract_quants_from_filenames_includes_ud_and_iq_variants() {
        let siblings = vec![
            HfSibling {
                rfilename: "UD-Q3_K_M/model-00001-of-00003.gguf".to_string(),
            },
            HfSibling {
                rfilename: "Qwen3.5-35B-A3B-IQ4_XS.gguf".to_string(),
            },
            HfSibling {
                rfilename: "Q2_K/model.gguf".to_string(),
            },
        ];

        let quants = HuggingFaceClient::extract_quants_from_filenames(&siblings);

        assert_eq!(quants, vec!["IQ4_XS", "Q2_K", "UD-Q3_K_M"]);
    }

    #[test]
    fn test_likely_missing_quant_variants_detects_large_coverage_gap() {
        let options = vec![
            DownloadOption {
                quant: "Q4_K_M".to_string(),
                size_bytes: Some(22_016_023_168),
                file_group: None,
            },
            DownloadOption {
                quant: "Q6_K".to_string(),
                size_bytes: Some(28_852_861_568),
                file_group: None,
            },
        ];

        let total = Some(369_031_399_680);
        let quants = vec![
            "Q4_K_M".to_string(),
            "Q6_K".to_string(),
            "UD-IQ4_XS".to_string(),
        ];
        assert!(likely_missing_quant_variants(&quants, total, &options));
    }

    #[test]
    fn test_likely_missing_quant_variants_false_when_coverage_is_reasonable() {
        let options = vec![
            DownloadOption {
                quant: "Q4_K_M".to_string(),
                size_bytes: Some(22_016_023_168),
                file_group: None,
            },
            DownloadOption {
                quant: "Q6_K".to_string(),
                size_bytes: Some(28_852_861_568),
                file_group: None,
            },
        ];

        let total = Some(52_000_000_000);
        let quants = vec!["Q4_K_M".to_string(), "Q6_K".to_string()];
        assert!(!likely_missing_quant_variants(&quants, total, &options));
    }

    #[test]
    fn test_likely_missing_quant_variants_false_when_options_cover_known_quants() {
        let options = vec![
            DownloadOption {
                quant: "Q4_K_M".to_string(),
                size_bytes: Some(22_016_023_168),
                file_group: None,
            },
            DownloadOption {
                quant: "Q6_K".to_string(),
                size_bytes: Some(28_852_861_568),
                file_group: None,
            },
        ];

        // Low coverage vs total could happen if aux LFS files are very large.
        let total = Some(369_031_399_680);
        let quants = vec!["Q4_K_M".to_string(), "Q6_K".to_string()];
        assert!(!likely_missing_quant_variants(&quants, total, &options));
    }

    #[test]
    fn test_infer_pipeline_tag_from_tags_detects_text_ranking() {
        let tags = vec!["GGUF".to_string(), "Text Ranking".to_string()];
        assert_eq!(
            HuggingFaceClient::infer_pipeline_tag_from_tags(&tags).as_deref(),
            Some("text-ranking")
        );
    }

    #[test]
    fn test_convert_search_result_prefers_text_ranking_tags_when_pipeline_missing() {
        let result = HfSearchResult {
            model_id: "QuantFactory/Qwen3-Reranker-4B-GGUF".to_string(),
            tags: vec!["gguf".to_string(), "text-ranking".to_string()],
            pipeline_tag: None,
            last_modified: None,
            downloads: None,
            siblings: vec![],
            config: None,
        };
        let converted = HuggingFaceClient::convert_search_result(result);
        assert_eq!(converted.kind, "text-ranking");
    }

    #[test]
    fn test_infer_pipeline_tag_from_tags_detects_text_to_speech() {
        let tags = vec!["onnx".to_string(), "Text To Speech".to_string()];
        assert_eq!(
            HuggingFaceClient::infer_pipeline_tag_from_tags(&tags).as_deref(),
            Some("text-to-speech")
        );
    }

    #[test]
    fn test_convert_search_result_prefers_audio_tags_when_pipeline_missing() {
        let result = HfSearchResult {
            model_id: "KittenML/kitten-tts-mini-0.8".to_string(),
            tags: vec!["onnx".to_string(), "speech_synthesis".to_string()],
            pipeline_tag: None,
            last_modified: None,
            downloads: None,
            siblings: vec![],
            config: None,
        };
        let converted = HuggingFaceClient::convert_search_result(result);
        assert_eq!(converted.kind, "text-to-speech");
    }

    #[tokio::test]
    async fn test_enrich_models_with_zero_hydrate_limit_preserves_cached_details() {
        let temp = TempDir::new().unwrap();
        let cache_path = temp.path().join("search.sqlite");
        let cache = Arc::new(HfSearchCache::new(&cache_path).unwrap());
        let client = HuggingFaceClient::with_cache(temp.path(), cache.clone()).unwrap();

        let model = HuggingFaceModel {
            repo_id: "test/model".to_string(),
            name: "model".to_string(),
            developer: "test".to_string(),
            kind: "text-generation".to_string(),
            formats: vec!["gguf".to_string()],
            quants: vec!["Q4_K_M".to_string()],
            download_options: vec![],
            url: "https://huggingface.co/test/model".to_string(),
            release_date: Some("2026-01-01T00:00:00Z".to_string()),
            downloads: Some(1),
            total_size_bytes: None,
            quant_sizes: None,
            compatible_engines: vec!["ollama".to_string()],
        };

        let mut cached = model.clone();
        cached.download_options = vec![DownloadOption {
            quant: "Q4_K_M".to_string(),
            size_bytes: Some(42),
            file_group: None,
        }];
        cached.total_size_bytes = Some(42);
        cache.cache_repo_details(&cached).unwrap();

        let enriched = client
            .enrich_models_with_download_options(&[model], 0)
            .await;
        assert_eq!(enriched.len(), 1);
        assert_eq!(enriched[0].download_options.len(), 1);
        assert_eq!(enriched[0].download_options[0].quant, "Q4_K_M");
        assert_eq!(enriched[0].download_options[0].size_bytes, Some(42));
        assert_eq!(enriched[0].total_size_bytes, Some(42));
    }
}
