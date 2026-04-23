import type { BaseResponse } from './api-common';
import type { ModelRecord } from './api-models';

// ============================================================================
// Model Library Types (Phase 1A - Part 6)
// ============================================================================

/**
 * Match methods indicating how metadata was matched
 */
export type MatchMethod = 'hash' | 'filename_exact' | 'filename_fuzzy' | 'manual' | 'none';

/**
 * Import stages for progress tracking
 */
export type ImportStage = 'copying' | 'hashing' | 'writing_metadata' | 'indexing' | 'syncing' | 'complete';

/**
 * Security tier for pickle scanning
 */
export type SecurityTier = 'safe' | 'unknown' | 'pickle';

/**
 * Library status including indexing state
 */
export interface LibraryStatusResponse extends BaseResponse {
  indexing: boolean;
  deep_scan_in_progress: boolean;
  deep_scan_progress?: {
    current: number;
    total: number;
    stage: string;
  };
  model_count: number;
  last_scan?: string;
}

/**
 * HuggingFace metadata lookup result
 */
export interface HFMetadataLookupResponse extends BaseResponse {
  found: boolean;
  match_method?: MatchMethod;
  confidence?: number;
  metadata?: HFMetadataLookupResult;
  related_files?: Array<{
    filename: string;
    size_bytes: number;
    quant?: string;
  }>;
}

/**
 * Individual model import specification
 */
export interface ModelImportSpec {
  path: string;
  family: string;
  official_name: string;
  repo_id?: string;
  model_type?: string;
  subtype?: string;
  tags?: string[];
  security_acknowledged?: boolean;
}

export interface ExternalDiffusersImportSpec {
  source_path: string;
  family: string;
  official_name: string;
  repo_id?: string | null;
  tags?: string[] | null;
}

/**
 * Individual model import result
 */
export interface ModelImportResult {
  path: string;
  success: boolean;
  model_id?: string;
  model_path?: string;
  error?: string;
  security_tier?: SecurityTier;
}

export type ImportPathClassificationKind =
  | 'single_file'
  | 'single_bundle'
  | 'single_model_directory'
  | 'multi_model_container'
  | 'ambiguous'
  | 'unsupported';

export type ImportPathCandidateKind =
  | 'file_model'
  | 'directory_model'
  | 'external_diffusers_bundle';

export interface ImportPathCandidate {
  path: string;
  kind: ImportPathCandidateKind;
  display_name: string;
  model_type?: string | null;
  bundle_format?: BundleFormat | null;
  pipeline_class?: string | null;
  component_manifest?: BundleComponentManifestEntry[] | null;
  reasons: string[];
}

export interface ImportPathClassification {
  path: string;
  kind: ImportPathClassificationKind;
  suggested_family?: string | null;
  suggested_official_name?: string | null;
  model_type?: string | null;
  bundle_format?: BundleFormat | null;
  pipeline_class?: string | null;
  component_manifest?: BundleComponentManifestEntry[] | null;
  reasons: string[];
  candidates: ImportPathCandidate[];
}

export type StorageKind = 'library_owned' | 'external_reference';

export type BundleFormat = 'diffusers_directory';

export type ImportState = 'pending' | 'ready' | 'failed';

export type AssetValidationState = 'valid' | 'degraded' | 'invalid';

export interface AssetValidationError {
  code: string;
  message: string;
  path?: string | null;
}

export type BundleComponentState =
  | 'present'
  | 'missing'
  | 'unreadable'
  | 'path_escape';

export interface BundleComponentManifestEntry {
  name: string;
  relative_path: string;
  source_library?: string | null;
  class_name?: string | null;
  state: BundleComponentState;
}

export interface LibraryEmbeddedMetadataResponse {
  file_type: string;
  metadata: Record<string, unknown>;
}

export interface LibraryModelMetadataResponse {
  success: boolean;
  model_id: string;
  stored_metadata: Record<string, unknown> | null;
  effective_metadata?: Record<string, unknown> | null;
  embedded_metadata: LibraryEmbeddedMetadataResponse | null;
  primary_file: string | null;
  component_manifest?: BundleComponentManifestEntry[] | null;
}

export interface ModelExecutionDescriptor {
  execution_contract_version: number;
  model_id: string;
  entry_path: string;
  model_type: string;
  task_type_primary: string;
  recommended_backend?: string | null;
  runtime_engine_hints: string[];
  storage_kind: StorageKind;
  validation_state: AssetValidationState;
  dependency_resolution?: Record<string, unknown> | null;
}

/**
 * Batch import response
 */
export interface ImportBatchResponse extends BaseResponse {
  imported: number;
  failed: number;
  results: ModelImportResult[];
}

/**
 * Network status including circuit breaker state
 */
export interface NetworkStatusResponse extends BaseResponse {
  total_requests: number;
  successful_requests: number;
  failed_requests: number;
  circuit_breaker_rejections: number;
  retries: number;
  success_rate: number;
  /** Map of domain to circuit state (CLOSED, OPEN, HALF_OPEN) */
  circuit_states: Record<string, string>;
  /** Whether any circuit breaker is currently open (offline indicator) */
  is_offline: boolean;
}

/**
 * FTS5 search response for local model library
 */
export interface FTSSearchResponse extends BaseResponse {
  models: ModelRecord[];
  total_count: number;
  query_time_ms: number;
  query: string;
}

/**
 * File writability check response
 */
export interface FileWritableResponse extends BaseResponse {
  writable: boolean;
  reason?: string;
}

/**
 * File link count response (for NTFS hard link detection)
 */
export interface FileLinkCountResponse extends BaseResponse {
  link_count: number;
  is_hard_linked: boolean;
}

/**
 * HuggingFace metadata lookup result (Phase 2 - Model Import)
 */
export interface HFMetadataLookupResult {
  repo_id: string;
  official_name: string;
  family: string;
  model_type?: string;
  subtype?: string;
  variant?: string;
  precision?: string;
  tags?: string[];
  base_model?: string;
  download_url?: string;
  description?: string;
  match_confidence?: number;
  match_method?: 'hash' | 'filename_exact' | 'filename_fuzzy';
  requires_confirmation?: boolean;
  hash_mismatch?: boolean;
  matched_filename?: string;
  pending_full_verification?: boolean;
  fast_hash?: string;
  expected_sha256?: string;
}

/**
 * Shard validation result
 */
export interface ShardValidation {
  complete: boolean;
  missing_shards: number[];
  total_expected: number;
  total_found: number;
  error?: string;
}

/**
 * Sharded set group info
 */
export interface ShardedSetGroup {
  files: string[];
  validation: ShardValidation;
}

/**
 * Detect sharded sets response
 */
export interface DetectShardedSetsResponse extends BaseResponse {
  groups: Record<string, ShardedSetGroup>;
}

/**
 * File type validation response
 */
export interface FileTypeValidationResponse extends BaseResponse {
  valid: boolean;
  detected_type: 'safetensors' | 'gguf' | 'ggml' | 'pickle' | 'onnx' | 'unknown' | 'error';
  error?: string;
}

/**
 * Embedded metadata response (GGUF or safetensors)
 */
export interface EmbeddedMetadataResponse extends BaseResponse {
  file_type: 'gguf' | 'safetensors' | 'unsupported' | 'unknown';
  metadata: Record<string, unknown> | null;
}

/**
 * Library status response
 */
export interface GetLibraryStatusResponse extends BaseResponse {
  indexing: boolean;
  deep_scan_in_progress: boolean;
  model_count: number;
  pending_lookups?: number;
  deep_scan_progress?: {
    current: number;
    total: number;
    stage: string;
  };
}

/**
 * Check files writable response
 */
export interface CheckFilesWritableResponse extends BaseResponse {
  all_writable: boolean;
  details: Array<{
    path: string;
    writable: boolean;
    reason?: string;
  }>;
}
