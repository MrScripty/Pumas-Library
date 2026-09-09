import type { BaseResponse } from './api-common';
import type {
  DownloadListOutcome, DownloadProgressOutcome, DownloadStartedOutcome, DownloadStatusOutcome, HfDownloadDetailsOutcome, InferenceSettingsOutcome, ModelsOutcome, PartialDownloadOutcome, UpdateInferenceSettingsOutcome, UpdateModelNotesOutcome,
} from '../generated/desktop-contract';

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
  downloaded_size_bytes?: number;
  download_progress?: number;
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

export type ModelsResponse = ModelsOutcome;

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
  min?: number | null;
  max?: number | null;
  allowed_values?: unknown[] | null;
}

/**
 * Mutable editor/update draft. Reads use the generated InferenceSettingsResponse;
 * new drafts may omit fields that the backend serializes explicitly as null.
 */
export interface InferenceParamSchema {
  key: string;
  label: string;
  param_type: InferenceSettingsOutcome['inference_settings'][number]['param_type'];
  default: unknown;
  description?: string | null;
  constraints?: ParamConstraints | null;
}

/**
 * Response containing the inference settings schema for a model.
 */
export type InferenceSettingsResponse = InferenceSettingsOutcome;

/**
 * Response after updating inference settings.
 */
export type UpdateInferenceSettingsResponse = UpdateInferenceSettingsOutcome;

export type UpdateModelNotesResponse = UpdateModelNotesOutcome;

export interface SearchHFModelsResponse extends BaseResponse {
  models: HuggingFaceModel[];
}

export interface RelatedModelsResponse extends BaseResponse {
  models: HuggingFaceModel[];
}

export type HFDownloadDetails = Extract<HfDownloadDetailsOutcome, { success: true }>['details'];

export type GetHFDownloadDetailsResponse = HfDownloadDetailsOutcome;

export type ModelDownloadResponse = DownloadStartedOutcome;
export type ModelDownloadStatusResponse = DownloadStatusOutcome;
export type ListModelDownloadsResponse = DownloadListOutcome;

/** Local presentation projection of canonical download progress. */
export interface ModelDownloadSnapshotEntry {
  success?: boolean;
  downloadId?: string;
  libraryModelId?: string | null;
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

export interface ModelDownloadSnapshot {
  cursor: string;
  revision: number;
  downloads: DownloadProgressOutcome[];
}

export interface ModelDownloadUpdateNotification {
  cursor: string;
  snapshot: ModelDownloadSnapshot;
  stale_cursor: boolean;
  snapshot_required: boolean;
}

export type ResumePartialDownloadResponse = PartialDownloadOutcome;

export interface ScanSharedStorageResponse extends BaseResponse {
  result: {
    modelsFound?: number;
    [key: string]: unknown;
  };
}
