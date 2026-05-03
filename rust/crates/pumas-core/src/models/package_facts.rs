//! Versioned model package-fact contracts.
//!
//! These DTOs describe bounded, serializable evidence about local model
//! packages. They intentionally preserve package facts without selecting a
//! runtime or executing Transformers/Python code.

use serde::{Deserialize, Serialize};

use super::{AssetValidationError, AssetValidationState, StorageKind};

/// Current producer contract version for resolved package facts.
pub const PACKAGE_FACTS_CONTRACT_VERSION: u32 = 1;

/// Stable reference to a Pumas model and optional selected artifact.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct PumasModelRef {
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageFactStatus {
    Present,
    Missing,
    Invalid,
    Unsupported,
    Uninspected,
}

impl Default for PackageFactStatus {
    fn default() -> Self {
        Self::Uninspected
    }
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
    pub diagnostic_codes: Vec<String>,
}

impl From<&ResolvedModelPackageFacts> for ResolvedModelPackageFactsSummary {
    fn from(facts: &ResolvedModelPackageFacts) -> Self {
        Self {
            package_facts_contract_version: facts.package_facts_contract_version,
            model_ref: facts.model_ref.clone(),
            artifact_kind: facts.artifact.artifact_kind,
            entry_path: facts.artifact.entry_path.clone(),
            storage_kind: facts.artifact.storage_kind,
            validation_state: facts.artifact.validation_state,
            task: facts.task.clone(),
            backend_hints: facts.backend_hints.clone(),
            requires_custom_code: facts.custom_code.requires_custom_code,
            config_status: facts
                .transformers
                .as_ref()
                .map(|evidence| evidence.config_status)
                .unwrap_or(PackageFactStatus::Uninspected),
            tokenizer_status: component_status(
                &facts.components,
                &[
                    ProcessorComponentKind::Tokenizer,
                    ProcessorComponentKind::TokenizerConfig,
                    ProcessorComponentKind::SpecialTokensMap,
                ],
            ),
            processor_status: component_status(
                &facts.components,
                &[
                    ProcessorComponentKind::Processor,
                    ProcessorComponentKind::Preprocessor,
                    ProcessorComponentKind::ImageProcessor,
                    ProcessorComponentKind::VideoProcessor,
                    ProcessorComponentKind::AudioFeatureExtractor,
                    ProcessorComponentKind::FeatureExtractor,
                ],
            ),
            generation_config_status: facts
                .transformers
                .as_ref()
                .map(|evidence| evidence.generation_config_status)
                .unwrap_or(PackageFactStatus::Uninspected),
            generation_defaults_status: facts.generation_defaults.status,
            diagnostic_codes: facts
                .diagnostics
                .iter()
                .chain(facts.generation_defaults.diagnostics.iter())
                .map(|diagnostic| diagnostic.code.clone())
                .collect(),
        }
    }
}

fn component_status(
    components: &[ProcessorComponentFacts],
    kinds: &[ProcessorComponentKind],
) -> PackageFactStatus {
    components
        .iter()
        .filter(|component| kinds.contains(&component.kind))
        .map(|component| component.status)
        .find(|status| *status != PackageFactStatus::Uninspected)
        .unwrap_or(PackageFactStatus::Uninspected)
}
