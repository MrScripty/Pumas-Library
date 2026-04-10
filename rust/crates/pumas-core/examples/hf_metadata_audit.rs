//! Audit Hugging Face metadata classification without downloading weights.
//!
//! This example:
//! - samples remote Hugging Face search results across task categories
//! - fetches authoritative repo metadata from the Hugging Face model endpoint
//! - materializes metadata-only library entries in a temporary ModelLibrary
//! - records how Pumas classifies and stores the result in SQLite-backed records
//! - writes JSON and Markdown reports for follow-up analysis

use pumas_library::model_library::{
    normalize_name, normalize_task_signature, push_review_reason,
    resolve_model_type_from_huggingface_evidence, validate_metadata_v2_with_index, HfSearchParams,
    HuggingFaceClient, ModelLibrary, ModelType, TaskNormalizationStatus,
};
use pumas_library::models::{HuggingFaceEvidence, ModelMetadata};
use rand::prelude::{IndexedRandom, SliceRandom};
use rand::{Rng, SeedableRng};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const DEFAULT_SAMPLE_SIZE: usize = 30;
const DEFAULT_MARKDOWN_OUTPUT: &str = "/tmp/pumas-hf-metadata-audit.md";
const DEFAULT_JSON_OUTPUT: &str = "/tmp/pumas-hf-metadata-audit.json";
const SEARCH_PAGE_SIZE: usize = 12;
const MAX_SEARCH_ATTEMPTS_MULTIPLIER: usize = 8;
const HF_API_BASE: &str = "https://huggingface.co/api";

#[derive(Clone, Copy)]
struct SearchPlan {
    label: &'static str,
    kind: Option<&'static str>,
    queries: &'static [&'static str],
    max_offset: usize,
}

const SEARCH_PLANS: &[SearchPlan] = &[
    SearchPlan {
        label: "text-generation",
        kind: Some("text-generation"),
        queries: &["llama", "qwen", "mistral", "gemma"],
        max_offset: 120,
    },
    SearchPlan {
        label: "text-ranking",
        kind: Some("text-ranking"),
        queries: &["reranker", "rank", "bge"],
        max_offset: 40,
    },
    SearchPlan {
        label: "text-to-image",
        kind: Some("text-to-image"),
        queries: &["diffusion", "flux", "sd", "image"],
        max_offset: 120,
    },
    SearchPlan {
        label: "image-to-image",
        kind: Some("image-to-image"),
        queries: &["inpaint", "controlnet", "edit"],
        max_offset: 60,
    },
    SearchPlan {
        label: "text-to-audio",
        kind: Some("text-to-audio"),
        queries: &["audio", "tts", "music"],
        max_offset: 40,
    },
    SearchPlan {
        label: "automatic-speech-recognition",
        kind: Some("automatic-speech-recognition"),
        queries: &["whisper", "asr", "speech"],
        max_offset: 40,
    },
    SearchPlan {
        label: "image-classification",
        kind: Some("image-classification"),
        queries: &["classification", "vit", "siglip"],
        max_offset: 60,
    },
    SearchPlan {
        label: "image-segmentation",
        kind: Some("image-segmentation"),
        queries: &["segmentation", "sam", "mask"],
        max_offset: 60,
    },
    SearchPlan {
        label: "depth-estimation",
        kind: Some("depth-estimation"),
        queries: &["depth", "depth-anything", "depth-pro"],
        max_offset: 40,
    },
    SearchPlan {
        label: "object-detection",
        kind: Some("object-detection"),
        queries: &["detection", "yolo", "detr"],
        max_offset: 60,
    },
    SearchPlan {
        label: "text-to-3d",
        kind: Some("text-to-3d"),
        queries: &["3d", "mesh", "hunyuan"],
        max_offset: 30,
    },
    SearchPlan {
        label: "image-to-3d",
        kind: Some("image-to-3d"),
        queries: &["3d", "mesh", "instantmesh"],
        max_offset: 30,
    },
];

#[derive(Debug, Clone)]
struct CliConfig {
    sample_size: usize,
    seed: u64,
    markdown_output: PathBuf,
    json_output: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct SearchSelection {
    plan_label: String,
    query: String,
    offset: usize,
    repo_id: String,
    search_kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct AuditSample {
    repo_id: String,
    owner: String,
    model_name: String,
    sampled_via: SearchSelection,
    search_kind: String,
    effective_pipeline_tag: Option<String>,
    hf_pipeline_tag: Option<String>,
    raw_config_model_type: Option<String>,
    raw_architectures: Vec<String>,
    top_tags: Vec<String>,
    resolved_model_type: String,
    model_type_resolution_source: String,
    model_type_resolution_confidence: f64,
    sqlite_model_id: String,
    sqlite_model_type: String,
    sqlite_task_type_primary: String,
    sqlite_input_modalities: Vec<String>,
    sqlite_output_modalities: Vec<String>,
    task_classification_source: Option<String>,
    task_classification_confidence: Option<f64>,
    metadata_needs_review: bool,
    review_reasons: Vec<String>,
    issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct AuditSummary {
    sample_size: usize,
    seed: u64,
    sampled_repo_ids: Vec<String>,
    issue_counts: BTreeMap<String, usize>,
    task_counts: BTreeMap<String, usize>,
    search_kind_counts: BTreeMap<String, usize>,
    review_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct AuditReport {
    generated_at: String,
    summary: AuditSummary,
    samples: Vec<AuditSample>,
}

#[derive(Debug, Deserialize)]
struct RawHfModelResponse {
    #[serde(rename = "id")]
    model_id: String,
    #[serde(default, rename = "pipeline_tag")]
    pipeline_tag: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    config: Option<RawHfConfig>,
}

#[derive(Debug, Deserialize)]
struct RawHfConfig {
    #[serde(default)]
    architectures: Vec<String>,
    #[serde(default, rename = "model_type")]
    model_type: Option<String>,
}

fn parse_args() -> Result<CliConfig, String> {
    let mut sample_size = DEFAULT_SAMPLE_SIZE;
    let mut seed = chrono::Utc::now().timestamp().unsigned_abs();
    let mut markdown_output = PathBuf::from(DEFAULT_MARKDOWN_OUTPUT);
    let mut json_output = PathBuf::from(DEFAULT_JSON_OUTPUT);

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--sample-size" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--sample-size requires a value".to_string())?;
                sample_size = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid --sample-size value: {}", value))?;
            }
            "--seed" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--seed requires a value".to_string())?;
                seed = value
                    .parse::<u64>()
                    .map_err(|_| format!("invalid --seed value: {}", value))?;
            }
            "--markdown" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--markdown requires a value".to_string())?;
                markdown_output = PathBuf::from(value);
            }
            "--json" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--json requires a value".to_string())?;
                json_output = PathBuf::from(value);
            }
            "--help" | "-h" => {
                return Err(help_text());
            }
            other => {
                return Err(format!("unknown argument: {}\n\n{}", other, help_text()));
            }
        }
    }

    if sample_size == 0 {
        return Err("--sample-size must be greater than zero".to_string());
    }

    Ok(CliConfig {
        sample_size,
        seed,
        markdown_output,
        json_output,
    })
}

fn help_text() -> String {
    format!(
        "Usage: cargo run -p pumas-library --example hf_metadata_audit -- [options]\n\
         \n\
         Options:\n\
           --sample-size <n>   Number of sampled repositories to audit (default: {})\n\
           --seed <n>          RNG seed for repeatable sampling (default: current UTC timestamp)\n\
           --markdown <path>   Markdown report output path (default: {})\n\
           --json <path>       JSON report output path (default: {})",
        DEFAULT_SAMPLE_SIZE, DEFAULT_MARKDOWN_OUTPUT, DEFAULT_JSON_OUTPUT
    )
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = match parse_args() {
        Ok(config) => config,
        Err(message) if message.starts_with("Usage:") => {
            println!("{}", message);
            return Ok(());
        }
        Err(message) => {
            eprintln!("{}", message);
            std::process::exit(2);
        }
    };

    let temp = TempWorkspace::new()?;
    let library = ModelLibrary::new(temp.library_root()).await?;
    let hf_client = HuggingFaceClient::new(temp.hf_cache_dir())?;
    let raw_client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("pumas-library-hf-metadata-audit/1.0")
        .build()?;

    let mut rng = rand::rngs::StdRng::seed_from_u64(config.seed);
    let selections = select_models(&hf_client, &mut rng, config.sample_size).await?;
    if selections.len() < config.sample_size {
        return Err(format!(
            "sampled only {} repositories after exhausting search attempts",
            selections.len()
        )
        .into());
    }

    let mut samples = Vec::with_capacity(selections.len());
    for selection in selections {
        let raw = fetch_raw_model(&raw_client, &selection.repo_id).await?;
        let sample = materialize_sample(&library, selection, raw).await?;
        samples.push(sample);
    }

    let report = build_report(config.seed, samples);
    write_report(&config.markdown_output, &config.json_output, &report)?;

    println!(
        "Wrote HF metadata audit reports:\n  markdown: {}\n  json: {}",
        config.markdown_output.display(),
        config.json_output.display()
    );

    Ok(())
}

struct TempWorkspace {
    _root: TempDir,
    launcher_root: PathBuf,
    library_root: PathBuf,
    hf_cache_dir: PathBuf,
}

impl TempWorkspace {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let root = tempfile::tempdir()?;
        let launcher_root = root.path().join("launcher-root");
        let library_root = launcher_root.join("shared-resources").join("models");
        let hf_cache_dir = launcher_root.join("launcher-data").join("cache").join("hf");

        fs::create_dir_all(&library_root)?;
        fs::create_dir_all(&hf_cache_dir)?;

        Ok(Self {
            _root: root,
            launcher_root,
            library_root,
            hf_cache_dir,
        })
    }

    fn library_root(&self) -> &Path {
        &self.library_root
    }

    fn hf_cache_dir(&self) -> &Path {
        &self.hf_cache_dir
    }

    #[allow(dead_code)]
    fn launcher_root(&self) -> &Path {
        &self.launcher_root
    }
}

async fn select_models(
    hf_client: &HuggingFaceClient,
    rng: &mut rand::rngs::StdRng,
    sample_size: usize,
) -> Result<Vec<SearchSelection>, Box<dyn std::error::Error>> {
    let max_attempts = sample_size * MAX_SEARCH_ATTEMPTS_MULTIPLIER;
    let mut selections = Vec::with_capacity(sample_size);
    let mut seen_repo_ids = HashSet::new();

    for attempt in 0..max_attempts {
        if selections.len() >= sample_size {
            break;
        }

        let plan = SEARCH_PLANS
            .choose(rng)
            .ok_or("no search plans configured for HF metadata audit")?;
        let query = plan
            .queries
            .choose(rng)
            .ok_or("search plan has no queries configured")?;
        let offset = rng.random_range(0..=plan.max_offset);

        let params = HfSearchParams {
            query: (*query).to_string(),
            kind: plan.kind.map(str::to_string),
            limit: Some(SEARCH_PAGE_SIZE),
            hydrate_limit: Some(0),
            offset: Some(offset),
            format: None,
        };

        let mut models = hf_client.search(&params).await?;
        models.shuffle(rng);

        let Some(model) = models
            .into_iter()
            .find(|candidate| seen_repo_ids.insert(candidate.repo_id.clone()))
        else {
            if attempt + 1 == max_attempts {
                break;
            }
            continue;
        };

        selections.push(SearchSelection {
            plan_label: plan.label.to_string(),
            query: (*query).to_string(),
            offset,
            repo_id: model.repo_id.clone(),
            search_kind: model.kind.clone(),
        });
    }

    Ok(selections)
}

async fn fetch_raw_model(
    client: &Client,
    repo_id: &str,
) -> Result<RawHfModelResponse, Box<dyn std::error::Error>> {
    let url = format!("{}/models/{}", HF_API_BASE, repo_id);
    let response = client.get(&url).send().await?;
    let response = response.error_for_status()?;
    Ok(response.json().await?)
}

async fn materialize_sample(
    library: &ModelLibrary,
    selection: SearchSelection,
    raw: RawHfModelResponse,
) -> Result<AuditSample, Box<dyn std::error::Error>> {
    let owner = raw
        .model_id
        .split('/')
        .next()
        .unwrap_or("unknown-owner")
        .to_string();
    let model_name = raw
        .model_id
        .split('/')
        .next_back()
        .unwrap_or(raw.model_id.as_str())
        .to_string();
    let effective_pipeline_tag =
        derive_effective_pipeline_tag(raw.pipeline_tag.as_deref(), selection.search_kind.as_str());
    let evidence = build_hf_evidence(&raw, effective_pipeline_tag.as_deref());
    let resolution = resolve_model_type_from_huggingface_evidence(
        library.index(),
        Some(&model_name),
        effective_pipeline_tag.as_deref(),
        None,
        Some(&evidence),
    )?;
    let metadata = build_metadata(
        library,
        &raw,
        &owner,
        &model_name,
        &evidence,
        effective_pipeline_tag.as_deref(),
        &resolution,
    )?;
    let model_dir = library.build_model_path(
        resolution.model_type.as_str(),
        &owner,
        &normalize_name(&model_name),
    );
    fs::create_dir_all(&model_dir)?;
    library.save_metadata(&model_dir, &metadata).await?;
    library.index_model_dir(&model_dir).await?;

    let model_id = library
        .get_model_id(&model_dir)
        .ok_or("failed to derive model_id for audit metadata")?;
    let record = library
        .get_model(&model_id)
        .await?
        .ok_or("audit model was not indexed into SQLite")?;
    let search_kind = selection.search_kind.clone();

    let sqlite_model_type = record.model_type.clone();
    let sqlite_task_type_primary = record
        .metadata
        .get("task_type_primary")
        .and_then(|value| value.as_str())
        .unwrap_or("missing")
        .to_string();
    let sqlite_input_modalities = string_array_field(&record.metadata, "input_modalities");
    let sqlite_output_modalities = string_array_field(&record.metadata, "output_modalities");
    let task_classification_source = record
        .metadata
        .get("task_classification_source")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    let task_classification_confidence = record
        .metadata
        .get("task_classification_confidence")
        .and_then(|value| value.as_f64());
    let metadata_needs_review = record
        .metadata
        .get("metadata_needs_review")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let review_reasons = string_array_field(&record.metadata, "review_reasons");
    let issues = detect_issues(
        selection.search_kind.as_str(),
        effective_pipeline_tag.as_deref(),
        raw.pipeline_tag.as_deref(),
        sqlite_model_type.as_str(),
        sqlite_task_type_primary.as_str(),
        metadata_needs_review,
    );

    Ok(AuditSample {
        repo_id: raw.model_id,
        owner,
        model_name,
        sampled_via: selection,
        search_kind,
        effective_pipeline_tag,
        hf_pipeline_tag: raw.pipeline_tag,
        raw_config_model_type: raw
            .config
            .as_ref()
            .and_then(|config| config.model_type.clone()),
        raw_architectures: raw
            .config
            .as_ref()
            .map(|config| config.architectures.clone())
            .unwrap_or_default(),
        top_tags: raw.tags.into_iter().take(8).collect(),
        resolved_model_type: resolution.model_type.as_str().to_string(),
        model_type_resolution_source: resolution.source,
        model_type_resolution_confidence: resolution.confidence,
        sqlite_model_id: model_id,
        sqlite_model_type,
        sqlite_task_type_primary,
        sqlite_input_modalities,
        sqlite_output_modalities,
        task_classification_source,
        task_classification_confidence,
        metadata_needs_review,
        review_reasons,
        issues,
    })
}

fn derive_effective_pipeline_tag(
    hf_pipeline_tag: Option<&str>,
    search_kind: &str,
) -> Option<String> {
    hf_pipeline_tag
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            let normalized = search_kind.trim();
            if normalized.is_empty() || normalized.eq_ignore_ascii_case("unknown") {
                None
            } else {
                Some(normalized.to_string())
            }
        })
}

fn build_hf_evidence(
    raw: &RawHfModelResponse,
    effective_pipeline_tag: Option<&str>,
) -> HuggingFaceEvidence {
    let tags = (!raw.tags.is_empty()).then(|| raw.tags.clone());
    let architectures = raw
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
    let config_model_type = raw
        .config
        .as_ref()
        .and_then(|config| config.model_type.as_ref())
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());

    HuggingFaceEvidence {
        repo_id: Some(raw.model_id.clone()),
        captured_at: Some(chrono::Utc::now().to_rfc3339()),
        remote_kind: effective_pipeline_tag.map(str::to_string),
        pipeline_tag: effective_pipeline_tag.map(str::to_string),
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

fn build_metadata(
    library: &ModelLibrary,
    raw: &RawHfModelResponse,
    owner: &str,
    model_name: &str,
    evidence: &HuggingFaceEvidence,
    effective_pipeline_tag: Option<&str>,
    resolution: &pumas_library::model_library::ModelTypeResolution,
) -> Result<ModelMetadata, Box<dyn std::error::Error>> {
    let normalized_task =
        normalize_task_signature(effective_pipeline_tag.unwrap_or("unknown->unknown"));
    let mapping = library
        .index()
        .get_active_task_signature_mapping(&normalized_task.signature_key)?;
    let now = chrono::Utc::now().to_rfc3339();

    let mut metadata = ModelMetadata {
        schema_version: Some(2),
        family: Some(normalize_name(owner)),
        model_type: Some(resolution.model_type.as_str().to_string()),
        official_name: Some(model_name.to_string()),
        cleaned_name: Some(normalize_name(model_name)),
        tags: (!raw.tags.is_empty()).then(|| raw.tags.clone()),
        repo_id: Some(raw.model_id.clone()),
        download_url: Some(format!("https://huggingface.co/{}", raw.model_id)),
        pipeline_tag: effective_pipeline_tag.map(str::to_string),
        huggingface_evidence: Some(evidence.clone()),
        task_type_primary: Some(
            mapping
                .as_ref()
                .map(|mapping| mapping.task_type_primary.clone())
                .unwrap_or_else(|| "unknown".to_string()),
        ),
        task_type_secondary: None,
        input_modalities: Some(normalized_task.input_modalities.clone()),
        output_modalities: Some(normalized_task.output_modalities.clone()),
        task_classification_source: Some(match mapping {
            Some(_) => "task-signature-mapping".to_string(),
            None => "runtime-discovered-signature".to_string(),
        }),
        task_classification_confidence: Some(
            match (mapping.is_some(), normalized_task.normalization_status) {
                (true, TaskNormalizationStatus::Ok) => 1.0,
                (true, TaskNormalizationStatus::Warning) => 0.8,
                (true, TaskNormalizationStatus::Error) => 0.0,
                (false, _) => 0.0,
            },
        ),
        model_type_resolution_source: Some(resolution.source.clone()),
        model_type_resolution_confidence: Some(resolution.confidence),
        runtime_engine_hints: Some(Vec::new()),
        requires_custom_code: Some(false),
        metadata_needs_review: Some(false),
        review_reasons: Some(Vec::new()),
        review_status: Some("not_required".to_string()),
        match_source: Some("hf-metadata-audit".to_string()),
        match_method: Some("repo_id".to_string()),
        match_confidence: Some(1.0),
        added_date: Some(now.clone()),
        updated_date: Some(now),
        ..Default::default()
    };

    for warning in &normalized_task.normalization_warnings {
        push_review_reason(&mut metadata, warning);
    }
    for review_reason in &resolution.review_reasons {
        push_review_reason(&mut metadata, review_reason);
    }

    if mapping.is_none() && effective_pipeline_tag.is_none() {
        push_review_reason(&mut metadata, "unknown-task-signature");
    }

    if let Some(pipeline_tag) = effective_pipeline_tag {
        metadata.task_type_primary = Some(pipeline_tag.to_string());
        metadata.task_classification_source = Some("hf-pipeline-tag".to_string());
        metadata.task_classification_confidence =
            Some(match normalized_task.normalization_status {
                TaskNormalizationStatus::Ok => 1.0,
                TaskNormalizationStatus::Warning => 0.8,
                TaskNormalizationStatus::Error => 0.0,
            });
    } else if mapping.is_none() {
        metadata.metadata_needs_review = Some(true);
        metadata.review_status = Some("pending".to_string());
    }

    if normalized_task.normalization_status == TaskNormalizationStatus::Error {
        metadata.task_type_primary = Some("unknown".to_string());
        metadata.task_classification_source = Some("invalid-task-signature".to_string());
        metadata.task_classification_confidence = Some(0.0);
        metadata.metadata_needs_review = Some(true);
        metadata.review_status = Some("pending".to_string());
        push_review_reason(&mut metadata, "invalid-task-signature");
    } else if normalized_task.normalization_status == TaskNormalizationStatus::Warning {
        metadata.metadata_needs_review = Some(true);
        metadata.review_status = Some("pending".to_string());
    }

    if resolution.model_type.as_str() == "unknown" {
        metadata.metadata_needs_review = Some(true);
        metadata.review_status = Some("pending".to_string());
        push_review_reason(&mut metadata, "model-type-unresolved");
    }

    validate_metadata_v2_with_index(&metadata, library.index())?;
    Ok(metadata)
}

fn string_array_field(metadata: &serde_json::Value, field: &str) -> Vec<String> {
    metadata
        .get(field)
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn detect_issues(
    search_kind: &str,
    effective_pipeline_tag: Option<&str>,
    hf_pipeline_tag: Option<&str>,
    sqlite_model_type: &str,
    sqlite_task_type_primary: &str,
    metadata_needs_review: bool,
) -> Vec<String> {
    let mut issues = BTreeSet::new();

    if let Some(hf_pipeline_tag) = hf_pipeline_tag {
        if !hf_pipeline_tag.eq_ignore_ascii_case(search_kind) {
            issues.insert("search-kind-mismatch".to_string());
        }
    }

    if sqlite_model_type.eq_ignore_ascii_case("unknown") {
        issues.insert("model-type-unknown".to_string());
    }

    if let Some(pipeline_tag) = effective_pipeline_tag {
        let expected_model_type = ModelType::from_pipeline_tag(pipeline_tag);
        if expected_model_type != ModelType::Unknown
            && !expected_model_type
                .as_str()
                .eq_ignore_ascii_case(sqlite_model_type)
        {
            issues.insert("model-type-mismatch-with-task".to_string());
        }
    }

    if sqlite_task_type_primary.eq_ignore_ascii_case("unknown") {
        issues.insert("task-type-unknown".to_string());
    }

    if let Some(pipeline_tag) = effective_pipeline_tag {
        if !task_equivalent(pipeline_tag, sqlite_task_type_primary) {
            issues.insert("task-label-collapsed-or-misclassified".to_string());
        }
    }

    if metadata_needs_review {
        issues.insert("metadata-needs-review".to_string());
    }

    issues.into_iter().collect()
}

fn task_equivalent(pipeline_tag: &str, sqlite_task_type_primary: &str) -> bool {
    if pipeline_tag.eq_ignore_ascii_case(sqlite_task_type_primary) {
        return true;
    }

    matches!(
        (
            pipeline_tag.to_lowercase().as_str(),
            sqlite_task_type_primary.to_lowercase().as_str()
        ),
        ("automatic-speech-recognition", "audio-to-text")
            | ("speech-to-text", "audio-to-text")
            | ("text-to-speech", "text-to-audio")
            | ("feature-extraction", "text-embedding")
            | ("sentence-similarity", "text-embedding")
    )
}

fn build_report(seed: u64, samples: Vec<AuditSample>) -> AuditReport {
    let mut issue_counts = BTreeMap::new();
    let mut task_counts = BTreeMap::new();
    let mut search_kind_counts = BTreeMap::new();
    let mut sampled_repo_ids = Vec::with_capacity(samples.len());
    let mut review_count = 0;

    for sample in &samples {
        sampled_repo_ids.push(sample.repo_id.clone());
        *search_kind_counts
            .entry(sample.search_kind.clone())
            .or_insert(0) += 1;
        *task_counts
            .entry(sample.sqlite_task_type_primary.clone())
            .or_insert(0) += 1;
        if sample.metadata_needs_review {
            review_count += 1;
        }
        for issue in &sample.issues {
            *issue_counts.entry(issue.clone()).or_insert(0) += 1;
        }
    }

    AuditReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        summary: AuditSummary {
            sample_size: samples.len(),
            seed,
            sampled_repo_ids,
            issue_counts,
            task_counts,
            search_kind_counts,
            review_count,
        },
        samples,
    }
}

fn write_report(
    markdown_output: &Path,
    json_output: &Path,
    report: &AuditReport,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = markdown_output.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = json_output.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(json_output, serde_json::to_string_pretty(report)?)?;
    fs::write(markdown_output, render_markdown(report))?;
    Ok(())
}

fn render_markdown(report: &AuditReport) -> String {
    let mut out = String::new();
    out.push_str("# Hugging Face Metadata Audit\n\n");
    out.push_str(&format!(
        "- Generated: `{}`\n- Sample size: `{}`\n- Seed: `{}`\n- Models needing review after projection: `{}`\n\n",
        report.generated_at,
        report.summary.sample_size,
        report.summary.seed,
        report.summary.review_count
    ));

    out.push_str("## Issue Counts\n\n");
    if report.summary.issue_counts.is_empty() {
        out.push_str("- None detected.\n\n");
    } else {
        for (issue, count) in &report.summary.issue_counts {
            out.push_str(&format!("- `{}`: `{}`\n", issue, count));
        }
        out.push('\n');
    }

    out.push_str("## Samples\n\n");
    out.push_str("| Repo | Search Kind | HF Pipeline | SQLite Task | SQLite Type | Issues |\n");
    out.push_str("|------|-------------|-------------|-------------|-------------|--------|\n");
    for sample in &report.samples {
        let issues = if sample.issues.is_empty() {
            "none".to_string()
        } else {
            sample.issues.join(", ")
        };
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            sample.repo_id,
            sample.search_kind,
            sample
                .hf_pipeline_tag
                .as_deref()
                .or(sample.effective_pipeline_tag.as_deref())
                .unwrap_or("unknown"),
            sample.sqlite_task_type_primary,
            sample.sqlite_model_type,
            issues
        ));
    }

    out.push_str("\n## Detailed Findings\n\n");
    for sample in report
        .samples
        .iter()
        .filter(|sample| !sample.issues.is_empty())
    {
        out.push_str(&format!("### `{}`\n\n", sample.repo_id));
        out.push_str(&format!(
            "- Search plan: `{}` query=`{}` offset=`{}`\n",
            sample.sampled_via.plan_label, sample.sampled_via.query, sample.sampled_via.offset
        ));
        out.push_str(&format!(
            "- Search kind: `{}`\n- HF pipeline tag: `{}`\n- Effective pipeline tag: `{}`\n",
            sample.search_kind,
            sample.hf_pipeline_tag.as_deref().unwrap_or("unknown"),
            sample
                .effective_pipeline_tag
                .as_deref()
                .unwrap_or("unknown")
        ));
        out.push_str(&format!(
            "- SQLite task/type: `{}` / `{}`\n",
            sample.sqlite_task_type_primary, sample.sqlite_model_type
        ));
        out.push_str(&format!(
            "- Input/output modalities: `{:?}` -> `{:?}`\n",
            sample.sqlite_input_modalities, sample.sqlite_output_modalities
        ));
        out.push_str(&format!("- Issues: `{}`\n", sample.issues.join(", ")));
        if !sample.review_reasons.is_empty() {
            out.push_str(&format!(
                "- Review reasons: `{}`\n",
                sample.review_reasons.join(", ")
            ));
        }
        out.push('\n');
    }

    out
}
