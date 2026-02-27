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
use tracing::{debug, info, warn};

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
                return Ok(models);
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
        let enriched = self.enrich_models_with_download_options(&models).await;

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
            .map(|r| Self::convert_search_result(r))
            .collect();

        Ok(models)
    }

    /// Enrich models with download options (file sizes) from cache or API.
    async fn enrich_models_with_download_options(
        &self,
        models: &[HuggingFaceModel],
    ) -> Vec<HuggingFaceModel> {
        let mut enriched = Vec::with_capacity(models.len());

        for model in models {
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
                            if !needs_regroup {
                                model.download_options = cached.download_options;
                                model.total_size_bytes = cached.total_size_bytes;
                                enriched.push(model);
                                continue;
                            }
                        }
                    }
                }
            }

            // Fetch from API
            match self.get_repo_files(&model.repo_id).await {
                Ok(tree) => {
                    let download_options =
                        Self::extract_download_options_from_tree(&tree, &model.quants);
                    let total_size = tree.lfs_files.iter().map(|f| f.size).sum();

                    model.download_options = download_options;
                    model.total_size_bytes = Some(total_size);
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
        let mut options = Vec::new();

        let quant_pattern =
            regex::Regex::new(r"[._-](Q\d+_[A-Z0-9_]+|fp16|fp32|bf16|int8|int4)[._-]?").ok();

        for lfs_file in &tree.lfs_files {
            if !lfs_file.filename.ends_with(".gguf")
                && !lfs_file.filename.ends_with(".safetensors")
                && !lfs_file.filename.ends_with(".bin")
            {
                continue;
            }

            let quant = if let Some(ref pattern) = quant_pattern {
                pattern
                    .captures(&lfs_file.filename)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str().to_string())
            } else {
                None
            };

            if let Some(q) = quant {
                options.push(DownloadOption {
                    quant: q,
                    size_bytes: Some(lfs_file.size),
                    file_group: None,
                });
            } else if quants.iter().any(|q| lfs_file.filename.contains(q)) {
                for q in quants {
                    if lfs_file.filename.contains(q) {
                        options.push(DownloadOption {
                            quant: q.clone(),
                            size_bytes: Some(lfs_file.size),
                            file_group: None,
                        });
                        break;
                    }
                }
            }
        }

        options.sort_by(|a, b| a.quant.cmp(&b.quant));
        options.dedup_by(|a, b| a.quant == b.quant);

        options
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
            .last()
            .unwrap_or(&result.model_id)
            .to_string();

        // Extract developer from modelId (before the /)
        let developer = result.model_id.split('/').next().unwrap_or("").to_string();

        // Determine kind: prefer pipeline_tag, fall back to config-based inference
        let kind = result
            .pipeline_tag
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
            .filter(|t| {
                t.starts_with("Q") && t.contains("_")
                    || ["fp16", "fp32", "bf16", "int8", "int4"].contains(&t.as_str())
            })
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

        let quant_pattern =
            regex::Regex::new(r"[._-](Q\d+_[A-Z0-9_]+|fp16|fp32|bf16|int8|int4)[._-]?")
                .unwrap_or_else(|_| regex::Regex::new(r"$^").unwrap()); // fallback to never-match

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
}
