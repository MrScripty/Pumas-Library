import type { BaseResponse } from './api-common';
import type { AssetValidationError, AssetValidationState, StorageKind } from './api-import';

// ============================================================================
// Model Types
// ============================================================================

export interface ModelRecordMetadata {
  family?: string;
  added_date?: string;
  updated_date?: string;
  size_bytes?: number;
  expected_files?: string[];
  files?: Array<{
    name: string;
    original_name?: string | null;
    size?: number | null;
    sha256?: string | null;
    blake3?: string | null;
  }>;
  repo_id?: string;
  download_incomplete?: boolean;
  download_has_part_files?: boolean;
  download_missing_expected_files?: number;
  integrity_issue_duplicate_repo_id?: boolean;
  integrity_issue_duplicate_repo_id_count?: number;
  integrity_issue_duplicate_repo_id_others?: string[];
  dependency_bindings?: Array<Record<string, unknown>>;
  requires_custom_code?: boolean;
  recommended_backend?: string | null;
  primary_format?: string | null;
  quantization?: string | null;
  related_available?: boolean;
  [key: string]: unknown;
}

export interface ModelRecord {
  id: string;
  path: string;
  modelType: string;
  officialName?: string;
  cleanedName?: string;
  tags: string[];
  hashes: Record<string, string>;
  metadata: ModelRecordMetadata;
  updatedAt: string;
}

export interface ModelsResponse extends BaseResponse {
  models: Record<string, ModelRecord>;
}

export interface HuggingFaceModel {
  repoId: string;
  name: string;
  developer: string;
  kind: string;
  formats: string[];
  quants: string[];
  downloadOptions?: Array<{
    quant: string;
    sizeBytes?: number | null;
  }>;
  url: string;
  releaseDate?: string;
  modelCard?: Record<string, unknown> | null;
  license?: string | null;
  downloads?: number | null;
  totalSizeBytes?: number | null;
  quantSizes?: Record<string, number>;
  /** Compatible inference engines based on model formats */
  compatibleEngines?: string[];
}

export interface HfAuthStatusResponse extends BaseResponse {
  authenticated: boolean;
  username?: string;
  token_source?: string;
}

// ============================================================================
// Inference Settings Types
// ============================================================================

/**
 * Constraints on an inference parameter value.
 */
export interface ParamConstraints {
  min?: number;
  max?: number;
  allowed_values?: unknown[];
}

/**
 * Describes a single configurable inference parameter with its type,
 * default value, and optional constraints.
 */
export interface InferenceParamSchema {
  key: string;
  label: string;
  param_type: 'Number' | 'Integer' | 'String' | 'Boolean';
  default: unknown;
  description?: string;
  constraints?: ParamConstraints;
}

/**
 * Response containing the inference settings schema for a model.
 */
export interface InferenceSettingsResponse extends BaseResponse {
  model_id: string;
  inference_settings: InferenceParamSchema[];
}

/**
 * Response after updating inference settings.
 */
export interface UpdateInferenceSettingsResponse extends BaseResponse {
  model_id: string;
}

export interface UpdateModelNotesResponse extends BaseResponse {
  model_id: string;
  notes?: string | null;
}

export type PackageArtifactKind =
  | 'gguf'
  | 'hf_compatible_directory'
  | 'safetensors'
  | 'diffusers_bundle'
  | 'onnx'
  | 'adapter'
  | 'shard'
  | 'unknown';

export type PackageFactStatus = 'present' | 'missing' | 'invalid' | 'unsupported' | 'uninspected';

export type ProcessorComponentKind =
  | 'config'
  | 'tokenizer'
  | 'tokenizer_config'
  | 'processor'
  | 'preprocessor'
  | 'image_processor'
  | 'video_processor'
  | 'audio_feature_extractor'
  | 'feature_extractor'
  | 'chat_template'
  | 'generation_config'
  | 'model_index'
  | 'weight_index'
  | 'weights'
  | 'adapter'
  | 'quantization'
  | 'other';

export type BackendHintLabel =
  | 'transformers'
  | 'llama.cpp'
  | 'vllm'
  | 'mlx'
  | 'candle'
  | 'diffusers'
  | 'onnx-runtime';

export interface ModelPackageDiagnostic {
  code: string;
  message: string;
  path?: string | null;
}

export interface ModelRefMigrationDiagnostic {
  code: string;
  message: string;
  input?: string | null;
}

export interface PumasModelRef {
  model_id: string;
  revision?: string | null;
  selected_artifact_id?: string | null;
  selected_artifact_path?: string | null;
  migration_diagnostics?: ModelRefMigrationDiagnostic[];
}

export interface ResolvedArtifactFacts {
  artifact_kind: PackageArtifactKind;
  entry_path: string;
  storage_kind: StorageKind;
  validation_state: AssetValidationState;
  validation_errors?: AssetValidationError[];
  companion_artifacts?: string[];
  sibling_files?: string[];
  selected_files?: string[];
}

export interface ProcessorComponentFacts {
  kind: ProcessorComponentKind;
  status: PackageFactStatus;
  relative_path?: string | null;
  class_name?: string | null;
  message?: string | null;
}

export interface TransformersPackageEvidence {
  config_status: PackageFactStatus;
  config_model_type?: string | null;
  architectures?: string[];
  dtype?: string | null;
  torch_dtype?: string | null;
  auto_map?: string[];
  processor_class?: string | null;
  generation_config_status: PackageFactStatus;
  source_repo_id?: string | null;
  source_revision?: string | null;
  selected_files?: string[];
}

export interface TaskEvidence {
  pipeline_tag?: string | null;
  task_type_primary?: string | null;
  input_modalities?: string[];
  output_modalities?: string[];
}

export interface GenerationDefaultFacts {
  status: PackageFactStatus;
  source_path?: string | null;
  defaults?: unknown;
  diagnostics?: ModelPackageDiagnostic[];
}

export interface CustomCodeFacts {
  requires_custom_code: boolean;
  custom_code_sources?: string[];
  auto_map_sources?: string[];
  class_references?: PackageClassReference[];
  dependency_manifests?: string[];
}

export interface PackageClassReference {
  kind: ProcessorComponentKind;
  class_name: string;
  source_path?: string | null;
}

export interface BackendHintFacts {
  accepted?: BackendHintLabel[];
  raw?: string[];
  unsupported?: string[];
}

export interface ResolvedModelPackageFacts {
  package_facts_contract_version: number;
  model_ref: PumasModelRef;
  artifact: ResolvedArtifactFacts;
  components: ProcessorComponentFacts[];
  transformers?: TransformersPackageEvidence | null;
  task: TaskEvidence;
  generation_defaults: GenerationDefaultFacts;
  custom_code: CustomCodeFacts;
  backend_hints: BackendHintFacts;
  diagnostics?: ModelPackageDiagnostic[];
}

export type ModelFactFamily =
  | 'model_record'
  | 'metadata'
  | 'package_facts'
  | 'dependency_bindings'
  | 'validation'
  | 'search_index';

export type ModelLibraryChangeKind =
  | 'model_added'
  | 'model_removed'
  | 'metadata_modified'
  | 'package_facts_modified'
  | 'stale_facts_invalidated'
  | 'dependency_binding_modified';

export type ModelLibraryRefreshScope = 'summary' | 'detail' | 'summary_and_detail';

export interface ModelLibraryUpdateEvent {
  cursor: string;
  model_id: string;
  change_kind: ModelLibraryChangeKind;
  fact_family: ModelFactFamily;
  refresh_scope: ModelLibraryRefreshScope;
  selected_artifact_id?: string | null;
  producer_revision?: string | null;
}

export interface ModelLibraryUpdateFeed {
  cursor: string;
  events?: ModelLibraryUpdateEvent[];
  stale_cursor: boolean;
  snapshot_required: boolean;
}

export interface SearchHFModelsResponse extends BaseResponse {
  models: HuggingFaceModel[];
}

export interface RelatedModelsResponse extends BaseResponse {
  models: HuggingFaceModel[];
}

export interface HFDownloadDetails {
  repoId: string;
  downloadOptions: HuggingFaceModel['downloadOptions'];
  totalSizeBytes?: number | null;
}

export interface GetHFDownloadDetailsResponse extends BaseResponse {
  details?: HFDownloadDetails;
}

export interface ModelDownloadResponse extends BaseResponse {
  download_id?: string;
  total_bytes?: number;
  model_path?: string;
}

export interface ModelDownloadStatusResponse extends BaseResponse {
  downloadId?: string;
  repoId?: string;
  modelName?: string;
  modelType?: string;
  status?: string;
  progress?: number;
  downloadedBytes?: number;
  totalBytes?: number;
  speed?: number;
  etaSeconds?: number;
  retryAttempt?: number;
  retryLimit?: number;
  retrying?: boolean;
  nextRetryDelaySeconds?: number;
  error?: string;
}

export interface ListModelDownloadsResponse extends BaseResponse {
  downloads: ModelDownloadStatusResponse[];
}

export interface InterruptedDownloadInfo {
  model_dir: string;
  model_type?: string;
  family: string;
  inferred_name: string;
  part_files: string[];
  completed_files: string[];
}

export interface ListInterruptedDownloadsResponse extends BaseResponse {
  interrupted: InterruptedDownloadInfo[];
}

export interface RecoverDownloadResponse extends BaseResponse {
  download_id?: string;
}

export interface ResumePartialDownloadResponse extends BaseResponse {
  action?: 'resume' | 'recover' | 'attach' | 'none';
  download_id?: string;
  status?: string;
  reason_code?: string;
}

export interface ScanSharedStorageResponse extends BaseResponse {
  result: {
    modelsFound?: number;
    [key: string]: unknown;
  };
}
