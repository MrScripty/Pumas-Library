import type { BaseResponse } from './api-common';

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
  selected_artifact_id?: string | null;
  selected_artifact_files?: string[];
  selected_artifact_quant?: string | null;
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
    selectedArtifactId?: string | null;
    artifactId?: string | null;
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
  selectedArtifactId?: string | null;
  artifactId?: string | null;
  total_bytes?: number;
  model_path?: string;
}

export interface ModelDownloadStatusResponse extends BaseResponse {
  downloadId?: string;
  repoId?: string;
  selectedArtifactId?: string | null;
  artifactId?: string | null;
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

export type ModelDownloadSnapshotEntry = Omit<ModelDownloadStatusResponse, 'success'> & {
  success?: boolean;
};

export interface ModelDownloadSnapshot {
  cursor: string;
  revision: number;
  downloads: ModelDownloadSnapshotEntry[];
}

export interface ModelDownloadUpdateNotification {
  cursor: string;
  snapshot: ModelDownloadSnapshot;
  stale_cursor: boolean;
  snapshot_required: boolean;
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
