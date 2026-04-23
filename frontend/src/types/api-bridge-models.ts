import type { BaseResponse } from './api-common';
import type {
  CheckFilesWritableResponse,
  DetectShardedSetsResponse,
  EmbeddedMetadataResponse,
  ExternalDiffusersImportSpec,
  FileLinkCountResponse,
  FileTypeValidationResponse,
  FTSSearchResponse,
  GetLibraryStatusResponse,
  HFMetadataLookupResponse,
  ImportBatchResponse,
  ImportPathClassification,
  LibraryModelMetadataResponse,
  ModelExecutionDescriptor,
  ModelImportResult,
  ModelImportSpec,
  NetworkStatusResponse,
} from './api-import';
import type {
  GetHFDownloadDetailsResponse,
  HfAuthStatusResponse,
  InferenceParamSchema,
  InferenceSettingsResponse,
  ListInterruptedDownloadsResponse,
  ListModelDownloadsResponse,
  ModelDownloadResponse,
  ModelDownloadStatusResponse,
  ModelsResponse,
  RecoverDownloadResponse,
  RelatedModelsResponse,
  ResumePartialDownloadResponse,
  ScanSharedStorageResponse,
  SearchHFModelsResponse,
  UpdateInferenceSettingsResponse,
  UpdateModelNotesResponse,
} from './api-models';

export interface DesktopBridgeModelAPI {
  // ========================================
  // Model Management
  // ========================================
  get_models(): Promise<ModelsResponse>;
  scan_shared_storage(): Promise<ScanSharedStorageResponse>;
  search_hf_models(
    query: string,
    kind?: string | null,
    limit?: number,
    hydrateLimit?: number
  ): Promise<SearchHFModelsResponse>;
  get_hf_download_details(
    repoId: string,
    quants?: string[] | null
  ): Promise<GetHFDownloadDetailsResponse>;
  get_related_models(modelId: string, limit?: number): Promise<RelatedModelsResponse>;
  start_model_download_from_hf(
    repoId: string,
    family: string,
    officialName: string,
    modelType?: string | null,
    pipelineTag?: string | null,
    releaseDate?: string | null,
    downloadUrl?: string | null,
    quant?: string | null,
    filenames?: string[] | null
  ): Promise<ModelDownloadResponse>;
  download_model_from_hf(
    repoId: string,
    family: string,
    officialName: string,
    modelType?: string | null,
    pipelineTag?: string | null,
    releaseDate?: string | null,
    downloadUrl?: string | null,
    quant?: string | null,
    filenames?: string[] | null
  ): Promise<ModelDownloadResponse>;
  get_model_download_status(downloadId: string): Promise<ModelDownloadStatusResponse>;
  cancel_model_download(downloadId: string): Promise<BaseResponse>;
  pause_model_download(downloadId: string): Promise<BaseResponse>;
  resume_model_download(downloadId: string): Promise<BaseResponse>;
  list_model_downloads(): Promise<ListModelDownloadsResponse>;
  list_interrupted_downloads(): Promise<ListInterruptedDownloadsResponse>;
  recover_download(repoId: string, destDir: string): Promise<RecoverDownloadResponse>;
  resume_partial_download(repoId: string, destDir: string): Promise<ResumePartialDownloadResponse>;

  // HuggingFace Authentication
  set_hf_token(token: string): Promise<BaseResponse>;
  clear_hf_token(): Promise<BaseResponse>;
  get_hf_auth_status(): Promise<HfAuthStatusResponse>;

  // Inference Settings
  /**
   * Get inference settings schema for a model.
   * Returns persisted settings, or lazy defaults based on model type/format.
   */
  get_inference_settings(modelId: string): Promise<InferenceSettingsResponse>;

  /**
   * Update (replace) inference settings schema for a model.
   * Pass an empty array to clear and revert to lazy defaults.
   */
  update_inference_settings(
    modelId: string,
    inferenceSettings: InferenceParamSchema[]
  ): Promise<UpdateInferenceSettingsResponse>;

  /**
   * Update user-authored markdown notes for a model.
   */
  update_model_notes(
    modelId: string,
    notes?: string | null
  ): Promise<UpdateModelNotesResponse>;

  /**
   * Get metadata for a library model (both stored and embedded)
   */
  get_library_model_metadata(modelId: string): Promise<LibraryModelMetadataResponse>;

  /**
   * Resolve a runtime execution descriptor for a model.
   */
  resolve_model_execution_descriptor(modelId: string): Promise<ModelExecutionDescriptor>;

  /**
   * Refetch model metadata from HuggingFace
   */
  refetch_model_metadata_from_hf(modelId: string): Promise<{
    success: boolean;
    model_id: string;
    metadata: Record<string, unknown> | null;
    error?: string;
  }>;

  // ========================================
  // Model Library Import (Phase 1A - Part 6)
  // ========================================
  /**
   * Search local model library using FTS5 full-text search
   */
  search_models_fts(
    query: string,
    limit?: number,
    offset?: number,
    modelType?: string | null,
    tags?: string[] | null
  ): Promise<FTSSearchResponse>;

  /**
   * Import multiple models in a batch operation
   */
  import_batch(importSpecs: ModelImportSpec[]): Promise<ImportBatchResponse>;

  /**
   * Register an external diffusers directory without copying its contents.
   */
  import_external_diffusers_directory(
    spec: ExternalDiffusersImportSpec
  ): Promise<ModelImportResult>;

  /**
   * Classify proposed import paths before any persistence side effects occur.
   */
  classify_model_import_paths(paths: string[]): Promise<ImportPathClassification[]>;

  /**
   * Get network status including circuit breaker state
   */
  get_network_status(): Promise<NetworkStatusResponse>;

  // ========================================
  // Model Import (Phase 2)
  // ========================================
  /**
   * Look up HuggingFace metadata for a file using hybrid filename + hash matching
   */
  lookup_hf_metadata_for_file(
    filename: string,
    filePath?: string | null
  ): Promise<HFMetadataLookupResponse>;

  /**
   * Look up HuggingFace metadata for a diffusers bundle directory.
   */
  lookup_hf_metadata_for_bundle_directory(
    directoryPath: string
  ): Promise<HFMetadataLookupResponse>;

  /**
   * Detect and group sharded model files
   */
  detect_sharded_sets(filePaths: string[]): Promise<DetectShardedSetsResponse>;

  /**
   * Validate file type using magic bytes
   */
  validate_file_type(filePath: string): Promise<FileTypeValidationResponse>;

  /**
   * Get embedded metadata from a model file (GGUF or safetensors)
   */
  get_embedded_metadata(filePath: string): Promise<EmbeddedMetadataResponse>;

  /**
   * Get current library status including indexing state
   */
  get_library_status(): Promise<GetLibraryStatusResponse>;

  /**
   * Get number of hard links for a file (NTFS detection)
   */
  get_file_link_count(filePath: string): Promise<FileLinkCountResponse>;

  /**
   * Check if files can be safely deleted
   */
  check_files_writable(filePaths: string[]): Promise<CheckFilesWritableResponse>;
}
