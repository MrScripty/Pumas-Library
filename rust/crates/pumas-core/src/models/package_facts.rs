//! Versioned model package-fact contracts.
//!
//! These DTOs describe bounded, serializable evidence about local model
//! packages. They intentionally preserve package facts without selecting a
//! runtime or executing Transformers/Python code.

use serde::{Deserialize, Serialize};

use super::{AssetValidationError, AssetValidationState, StorageKind};

/// Current producer contract version for resolved package facts.
pub const PACKAGE_FACTS_CONTRACT_VERSION: u32 = 2;

/// Current stable contract version for `PumasModelRef`.
pub const PUMAS_MODEL_REF_CONTRACT_VERSION: u32 = 1;

pub(crate) fn default_pumas_model_ref_contract_version() -> u32 {
    PUMAS_MODEL_REF_CONTRACT_VERSION
}

/// Stable reference to a Pumas model and optional selected artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct PumasModelRef {
    #[serde(default = "default_pumas_model_ref_contract_version")]
    pub model_ref_contract_version: u32,
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_artifact_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub migration_diagnostics: Vec<ModelRefMigrationDiagnostic>,
}

impl Default for PumasModelRef {
    fn default() -> Self {
        Self {
            model_ref_contract_version: PUMAS_MODEL_REF_CONTRACT_VERSION,
            model_id: String::new(),
            revision: None,
            selected_artifact_id: None,
            selected_artifact_path: None,
            migration_diagnostics: Vec::new(),
        }
    }
}

/// Diagnostic produced while converting legacy references to Pumas refs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelRefMigrationDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
}

/// Artifact family understood by Pumas package-fact producers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageArtifactKind {
    Gguf,
    HfCompatibleDirectory,
    Safetensors,
    DiffusersBundle,
    Onnx,
    Adapter,
    Shard,
    Unknown,
}

/// Stable backend hint labels Pumas may expose as advisory package facts.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BackendHintLabel {
    Transformers,
    #[serde(rename = "llama.cpp")]
    LlamaCpp,
    Vllm,
    Mlx,
    Candle,
    Diffusers,
    OnnxRuntime,
}

/// Normalized state for package-file inspection.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageFactStatus {
    Present,
    Missing,
    Invalid,
    Unsupported,
    #[default]
    Uninspected,
}

/// Package component kind with stable labels for consumer diagnostics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessorComponentKind {
    Config,
    Tokenizer,
    TokenizerConfig,
    SpecialTokensMap,
    Processor,
    Preprocessor,
    ImageProcessor,
    VideoProcessor,
    AudioFeatureExtractor,
    FeatureExtractor,
    ChatTemplate,
    GenerationConfig,
    ModelIndex,
    WeightIndex,
    Shard,
    Weights,
    Adapter,
    Quantization,
    Other,
}

/// Source strength for a package fact value.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageFactValueSource {
    Header,
    Config,
    UpstreamMetadata,
    ComponentLayout,
    FilenameWeak,
    Ambiguous,
    Unavailable,
}

/// Image-generation family labels produced only from package evidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageGenerationFamilyLabel {
    StableDiffusion,
    StableDiffusionXl,
    Flux,
    Flux2,
    QwenImage,
    LuminaImage,
    GlmImage,
    ZImage,
    Unknown,
    Ambiguous,
}

/// Package evidence source used for image-generation family labels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageGenerationFamilyEvidenceSource {
    PipelineClass,
    ModelIndexComponent,
    ComponentConfig,
    RepoMetadata,
    Ambiguous,
}

/// Source-tagged image-generation family evidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ImageGenerationFamilyEvidence {
    pub family: ImageGenerationFamilyLabel,
    pub source: ImageGenerationFamilyEvidenceSource,
    pub value_source: PackageFactValueSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Stable roles for Diffusers-style package components.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffusersComponentRole {
    PipelineIndex,
    Scheduler,
    Tokenizer,
    Tokenizer2,
    TextEncoder,
    TextEncoder2,
    TextEncoder3,
    ImageProcessor,
    Processor,
    Unet,
    Transformer,
    Vae,
    Controlnet,
    Adapter,
    Weights,
    GenerationConfig,
}

/// Component facts from a Diffusers bundle without importing Python classes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct DiffusersComponentFacts {
    pub role: DiffusersComponentRole,
    pub status: PackageFactStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_library: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_model_type: Option<String>,
}

/// Diffusers bundle evidence from `model_index.json` and bounded component configs.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct DiffusersPackageEvidence {
    pub status: PackageFactStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusers_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_or_path: Option<String>,
    pub task: TaskEvidence,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub family_evidence: Vec<ImageGenerationFamilyEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<DiffusersComponentFacts>,
}

/// Bounded GGUF metadata evidence from the selected artifact header.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct GgufPackageEvidence {
    pub status: PackageFactStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template_present: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_length: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attention_head_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_source: Option<PackageFactValueSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metadata_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub companion_artifacts: Vec<String>,
}

/// Serializable package inspection manifest used for cache/debug contracts.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct PackageInspectionManifest {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<PackageInspectionManifestEntry>,
}

/// One bounded manifest entry that may affect package facts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct PackageInspectionManifestEntry {
    pub relative_path: String,
    pub value_source: PackageFactValueSource,
}

/// Component-level package-file evidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ProcessorComponentFacts {
    pub kind: ProcessorComponentKind,
    pub status: PackageFactStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Transformers/Hugging Face package layout evidence.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct TransformersPackageEvidence {
    pub config_status: PackageFactStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_model_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub architectures: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dtype: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub torch_dtype: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto_map: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processor_class: Option<String>,
    pub generation_config_status: PackageFactStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_repo_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_files: Vec<String>,
}

/// Raw and normalized task evidence for routing and validation.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct TaskEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type_primary: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_modalities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_modalities: Vec<String>,
}

/// Model-provided generation defaults, distinct from Pumas inference settings.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GenerationDefaultFacts {
    pub status: PackageFactStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub defaults: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ModelPackageDiagnostic>,
}

/// Custom-code and trust evidence for package consumers.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct CustomCodeFacts {
    pub requires_custom_code: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_code_sources: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auto_map_sources: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub class_references: Vec<PackageClassReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependency_manifests: Vec<String>,
}

/// Class reference discovered from package metadata without importing code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct PackageClassReference {
    pub kind: ProcessorComponentKind,
    pub class_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

/// Backend hints as advisory facts, not runtime decisions.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct BackendHintFacts {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accepted: Vec<BackendHintLabel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unsupported: Vec<String>,
}

/// Artifact-specific package evidence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ResolvedArtifactFacts {
    pub artifact_kind: PackageArtifactKind,
    pub entry_path: String,
    pub storage_kind: StorageKind,
    pub validation_state: AssetValidationState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_errors: Vec<AssetValidationError>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub companion_artifacts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sibling_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_files: Vec<String>,
}

/// Generic package-fact diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelPackageDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Top-level fact family that changed or needs refresh.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelFactFamily {
    ModelRecord,
    Metadata,
    PackageFacts,
    DependencyBindings,
    Validation,
    SearchIndex,
}

/// Model-library change kind for host cache invalidation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelLibraryChangeKind {
    ModelAdded,
    ModelRemoved,
    MetadataModified,
    PackageFactsModified,
    StaleFactsInvalidated,
    DependencyBindingModified,
}

/// Consumer refresh scope implied by a model-library change event.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelLibraryRefreshScope {
    Summary,
    Detail,
    SummaryAndDetail,
}

/// Host-agnostic model-library update event for cache invalidation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelLibraryUpdateEvent {
    pub cursor: String,
    pub model_id: String,
    pub change_kind: ModelLibraryChangeKind,
    pub fact_family: ModelFactFamily,
    pub refresh_scope: ModelLibraryRefreshScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_revision: Option<String>,
}

/// Ordered page of model-library updates after a consumer cursor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelLibraryUpdateFeed {
    pub cursor: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<ModelLibraryUpdateEvent>,
    pub stale_cursor: bool,
    pub snapshot_required: bool,
}

/// Realtime notification that the durable model-library update feed advanced.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelLibraryUpdateNotification {
    pub cursor: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<ModelLibraryUpdateEvent>,
    pub stale_cursor: bool,
    pub snapshot_required: bool,
}

/// Startup result for a cursor-based model-library update subscription.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelLibraryUpdateSubscription {
    pub requested_cursor: String,
    pub cursor_after_recovery: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recovered_events: Vec<ModelLibraryUpdateEvent>,
    pub stale_cursor: bool,
    pub snapshot_required: bool,
    pub live_stream_ready: bool,
}

impl From<ModelLibraryUpdateFeed> for ModelLibraryUpdateNotification {
    fn from(feed: ModelLibraryUpdateFeed) -> Self {
        Self {
            cursor: feed.cursor,
            events: feed.events,
            stale_cursor: feed.stale_cursor,
            snapshot_required: feed.snapshot_required,
        }
    }
}

/// Consumer-visible freshness/source state for a package-facts summary row.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelPackageFactsSummaryStatus {
    Cached,
    Missing,
    Invalid,
    StaleContract,
    StaleFingerprint,
    WrongSelectedArtifact,
    Error,
    Fresh,
    DetailDerived,
    Regenerated,
}

/// Single model package-facts summary lookup result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelPackageFactsSummaryResult {
    pub model_id: String,
    pub status: ModelPackageFactsSummaryStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<ResolvedModelPackageFactsSummary>,
}

/// Per-model result for batch package-facts summary hydration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelPackageFactsSummaryBatchItem {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ModelPackageFactsSummaryResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Startup/list snapshot item for host cache population.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelPackageFactsSummarySnapshotItem {
    pub model_id: String,
    pub status: ModelPackageFactsSummaryStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<ResolvedModelPackageFactsSummary>,
}

/// Bounded startup snapshot of cached package-facts summaries plus update cursor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ModelPackageFactsSummarySnapshot {
    pub cursor: String,
    pub items: Vec<ModelPackageFactsSummarySnapshotItem>,
}

/// Versioned inference-facing model package facts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ResolvedModelPackageFacts {
    pub package_facts_contract_version: u32,
    pub model_ref: PumasModelRef,
    pub artifact: ResolvedArtifactFacts,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub components: Vec<ProcessorComponentFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transformers: Option<TransformersPackageEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusers: Option<DiffusersPackageEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gguf: Option<GgufPackageEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection_manifest: Option<PackageInspectionManifest>,
    pub task: TaskEvidence,
    pub generation_defaults: GenerationDefaultFacts,
    pub custom_code: CustomCodeFacts,
    pub backend_hints: BackendHintFacts,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ModelPackageDiagnostic>,
}

/// Compact package-fact summary intended for indexing, list views, and stale checks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ResolvedModelPackageFactsSummary {
    pub package_facts_contract_version: u32,
    pub model_ref: PumasModelRef,
    pub artifact_kind: PackageArtifactKind,
    pub entry_path: String,
    pub storage_kind: StorageKind,
    pub validation_state: AssetValidationState,
    pub task: TaskEvidence,
    pub backend_hints: BackendHintFacts,
    pub requires_custom_code: bool,
    pub config_status: PackageFactStatus,
    pub tokenizer_status: PackageFactStatus,
    pub processor_status: PackageFactStatus,
    pub generation_config_status: PackageFactStatus,
    pub generation_defaults_status: PackageFactStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_generation_family_evidence: Vec<ImageGenerationFamilyEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diffusers_pipeline_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gguf_architecture: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostic_codes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_library_update_notification_serializes_contract_shape() {
        let notification = ModelLibraryUpdateNotification {
            cursor: "model-library-updates:42".to_string(),
            events: vec![ModelLibraryUpdateEvent {
                cursor: "model-library-updates:42".to_string(),
                model_id: "llm/llama/test".to_string(),
                change_kind: ModelLibraryChangeKind::ModelAdded,
                fact_family: ModelFactFamily::ModelRecord,
                refresh_scope: ModelLibraryRefreshScope::SummaryAndDetail,
                selected_artifact_id: Some("artifact".to_string()),
                producer_revision: Some("2026-05-04T00:00:00Z".to_string()),
            }],
            stale_cursor: false,
            snapshot_required: false,
        };

        let value = serde_json::to_value(&notification).unwrap();
        assert_eq!(value["cursor"], "model-library-updates:42");
        assert_eq!(value["stale_cursor"], false);
        assert_eq!(value["snapshot_required"], false);
        assert_eq!(value["events"][0]["change_kind"], "model_added");
        assert_eq!(value["events"][0]["fact_family"], "model_record");
        assert_eq!(value["events"][0]["refresh_scope"], "summary_and_detail");

        let parsed: ModelLibraryUpdateNotification = serde_json::from_value(value).unwrap();
        assert_eq!(parsed, notification);
    }

    #[test]
    fn model_library_update_notification_can_be_built_from_feed() {
        let feed = ModelLibraryUpdateFeed {
            cursor: "model-library-updates:7".to_string(),
            events: Vec::new(),
            stale_cursor: true,
            snapshot_required: true,
        };

        let notification = ModelLibraryUpdateNotification::from(feed);
        assert_eq!(notification.cursor, "model-library-updates:7");
        assert!(notification.events.is_empty());
        assert!(notification.stale_cursor);
        assert!(notification.snapshot_required);
    }

    #[test]
    fn package_facts_summary_status_serializes_stale_states() {
        assert_eq!(
            serde_json::to_value(ModelPackageFactsSummaryStatus::StaleContract).unwrap(),
            serde_json::json!("stale_contract")
        );
        assert_eq!(
            serde_json::to_value(ModelPackageFactsSummaryStatus::StaleFingerprint).unwrap(),
            serde_json::json!("stale_fingerprint")
        );
        assert_eq!(
            serde_json::to_value(ModelPackageFactsSummaryStatus::WrongSelectedArtifact).unwrap(),
            serde_json::json!("wrong_selected_artifact")
        );
    }
}
