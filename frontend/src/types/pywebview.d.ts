/**
 * PyWebView API Type Definitions
 *
 * Complete type definitions for the PyWebView bridge API.
 * This file provides type safety for all backend API interactions.
 */

// ============================================================================
// Response Base Types
// ============================================================================

export interface BaseResponse {
  success: boolean;
  error?: string;
}

// ============================================================================
// System & Status Types
// ============================================================================

export interface DiskSpaceResponse extends BaseResponse {
  total: number;
  used: number;
  free: number;
  percent: number;
}

export interface StatusResponse extends BaseResponse {
  version: string;
  deps_ready: boolean;
  patched: boolean;
  menu_shortcut: boolean;
  desktop_shortcut: boolean;
  shortcut_version: string | null;
  message: string;
  comfyui_running: boolean;
  last_launch_error: string | null;
  last_launch_log: string | null;
  app_resources?: {
    comfyui?: {
      gpu_memory?: number;
      ram_memory?: number;
    };
  };
}

export interface SystemResourcesResponse extends BaseResponse {
  resources: {
    cpu: {
      usage: number;
      temp?: number;
    };
    gpu: {
      usage: number;
      memory: number;
      memory_total: number;
      temp?: number;
    };
    ram: {
      usage: number;
      total: number;
    };
    disk: {
      usage: number;
      total: number;
      free: number;
    };
  };
}

// ============================================================================
// Model Types
// ============================================================================

export interface ModelData {
  modelType: string;
  officialName?: string;
  cleanedName?: string;
  size?: number;
  addedDate?: string;
}

export interface ModelsResponse extends BaseResponse {
  models: Record<string, ModelData>;
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
  downloads?: number | null;
  totalSizeBytes?: number | null;
  quantSizes?: Record<string, number>;
}

export interface SearchHFModelsResponse extends BaseResponse {
  models: HuggingFaceModel[];
}

export interface ModelDownloadResponse extends BaseResponse {
  download_id?: string;
  total_bytes?: number;
  model_path?: string;
}

export interface ModelDownloadStatusResponse extends BaseResponse {
  download_id?: string;
  repo_id?: string;
  status?: string;
  progress?: number;
  downloaded_bytes?: number;
  total_bytes?: number;
}

export interface ScanSharedStorageResponse extends BaseResponse {
  result: {
    modelsFound?: number;
    [key: string]: unknown;
  };
}

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
  metadata?: {
    repo_id: string;
    official_name: string;
    family: string;
    model_type?: string;
    subtype?: string;
    tags?: string[];
    description?: string;
  };
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

/**
 * Individual model import result
 */
export interface ModelImportResult {
  path: string;
  success: boolean;
  model_path?: string;
  error?: string;
  security_tier?: SecurityTier;
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
}

/**
 * FTS5 search response for local model library
 */
export interface FTSSearchResponse extends BaseResponse {
  models: Array<{
    model_id: string;
    repo_id?: string;
    official_name: string;
    family: string;
    model_type?: string;
    subtype?: string;
    tags?: string[];
    description?: string;
    file_path: string;
    size_bytes?: number;
    security_tier?: SecurityTier;
    added_date?: string;
    last_used?: string;
  }>;
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
 * HuggingFace metadata lookup response
 */
export interface HFMetadataLookupResponse extends BaseResponse {
  found: boolean;
  metadata?: HFMetadataLookupResult;
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

// ============================================================================
// Link Registry Types (Phase 1B)
// ============================================================================

/**
 * Health status levels for link registry
 */
export type HealthStatus = 'healthy' | 'warnings' | 'errors';

/**
 * Link types supported by the registry
 */
export type LinkType = 'symlink' | 'hardlink' | 'copy';

/**
 * Information about a broken link
 */
export interface BrokenLinkInfo {
  link_id: number;
  target_path: string;
  expected_source: string;
  model_id: string;
  reason: string;
}

/**
 * Information about a registered link
 */
export interface LinkInfo {
  link_id: number;
  model_id: string;
  source_path: string;
  target_path: string;
  link_type: LinkType;
  app_id: string;
  app_version: string;
  is_external: boolean;
  created_at: string;
}

/**
 * Link health check response
 */
export interface LinkHealthResponse extends BaseResponse {
  status: HealthStatus;
  total_links: number;
  healthy_links: number;
  broken_links: BrokenLinkInfo[];
  orphaned_links: string[];
  warnings: string[];
  errors: string[];
}

/**
 * Clean broken links response
 */
export interface CleanBrokenLinksResponse extends BaseResponse {
  cleaned: number;
}

/**
 * Remove orphaned links response
 */
export interface RemoveOrphanedLinksResponse extends BaseResponse {
  removed: number;
}

/**
 * Get links for model response
 */
export interface GetLinksForModelResponse extends BaseResponse {
  links: LinkInfo[];
}

/**
 * Cascade delete model response
 */
export interface DeleteModelCascadeResponse extends BaseResponse {
  links_removed: number;
}

// ============================================================================
// Mapping Preview Types (Phase 1C)
// ============================================================================

/**
 * A single mapping action to be performed
 */
export interface MappingAction {
  model_id: string;
  model_name: string;
  source_path: string;
  target_path: string;
  link_type?: string;
  reason: string;
  existing_target?: string;
}

/**
 * Broken link to be removed
 */
export interface BrokenLinkToRemove {
  target_path: string;
  existing_target: string;
  reason: string;
}

/**
 * Mapping preview response
 */
export interface MappingPreviewResponse extends BaseResponse {
  to_create: MappingAction[];
  to_skip_exists: MappingAction[];
  conflicts: MappingAction[];
  broken_to_remove: BrokenLinkToRemove[];
  total_actions: number;
  warnings: string[];
  errors: string[];
}

/**
 * Incremental sync response
 */
export interface IncrementalSyncResponse extends BaseResponse {
  links_created: number;
  links_updated: number;
  links_skipped: number;
}

/**
 * Conflict resolution action types
 */
export type ConflictResolutionAction = 'skip' | 'overwrite' | 'rename';

/**
 * Conflict resolutions map
 */
export type ConflictResolutions = Record<string, ConflictResolutionAction>;

/**
 * Sync with resolutions response
 */
export interface SyncWithResolutionsResponse extends BaseResponse {
  links_created: number;
  links_skipped: number;
  links_renamed: number;
  overwrites: number;
  errors: string[];
}

/**
 * Apply model mapping response
 */
export interface ApplyModelMappingResponse extends BaseResponse {
  links_created: number;
  links_removed: number;
  total_links: number;
}

/**
 * Sandbox type enumeration
 */
export type SandboxType = 'flatpak' | 'snap' | 'docker' | 'appimage' | 'none' | 'unknown';

/**
 * Sandbox environment info response
 */
export interface SandboxInfoResponse extends BaseResponse {
  is_sandboxed: boolean;
  sandbox_type: SandboxType;
  limitations: string[];
}

/**
 * Cross-filesystem warning response
 */
export interface CrossFilesystemWarningResponse extends BaseResponse {
  cross_filesystem: boolean;
  library_path?: string;
  app_path?: string;
  warning?: string;
  recommendation?: string;
}

// ============================================================================
// Version Management Types
// ============================================================================

export interface VersionReleaseAsset {
  name: string;
  size: number;
  download_url: string;
}

export interface VersionReleaseInfo {
  tag_name: string;
  name: string;
  published_at: string;
  prerelease: boolean;
  body: string;
  html_url: string;
  assets: VersionReleaseAsset[];
  total_size?: number | null;
  archive_size?: number | null;
  dependencies_size?: number | null;
  installing?: boolean;
}

export interface GetAvailableVersionsResponse extends BaseResponse {
  versions: VersionReleaseInfo[];
}

export interface GetInstalledVersionsResponse extends BaseResponse {
  versions: string[];
}

export interface GetActiveVersionResponse extends BaseResponse {
  version: string | null;
}

export interface VersionActionResponse extends BaseResponse {
  // Used for install, remove, switch operations
}

export interface ValidateInstallationsResponse extends BaseResponse {
  result: {
    had_invalid: boolean;
    removed: string[];
    valid: string[];
  };
}

export interface GetVersionInfoResponse extends BaseResponse {
  info: {
    path: string;
    installedDate: string;
    pythonVersion: string;
    releaseTag: string;
  } | null;
}

export interface GetDefaultVersionResponse extends BaseResponse {
  version: string | null;
}

export interface SetDefaultVersionResponse extends BaseResponse {
  // Empty body on success
}

export interface VersionStatusResponse extends BaseResponse {
  status: {
    installedCount: number;
    activeVersion: string | null;
    defaultVersion?: string | null;
    versions: {
      [tag: string]: {
        isActive: boolean;
        dependencies: {
          installed: string[];
          missing: string[];
        };
      };
    };
  } | null;
}

// ============================================================================
// Installation & Progress Types
// ============================================================================

export interface InstallationProgressItem {
  name: string;
  type: string;
  size: number | null;
  completed_at: string;
}

export interface InstallationProgressResponse {
  tag?: string;
  started_at?: string;
  stage?: 'download' | 'extract' | 'venv' | 'dependencies' | 'setup';
  stage_progress?: number;
  overall_progress?: number;
  current_item?: string | null;
  download_speed?: number | null;
  eta_seconds?: number | null;
  total_size?: number | null;
  downloaded_bytes?: number;
  dependency_count?: number | null;
  completed_dependencies?: number;
  completed_items?: InstallationProgressItem[];
  error?: string | null;
  completed_at?: string;
  success?: boolean;
  log_path?: string | null;
}

export interface CancelInstallationResponse extends BaseResponse {
  // Empty body on success
}

// ============================================================================
// Cache & Background Fetch Types
// ============================================================================

export interface CacheStatusResponse {
  has_cache: boolean;
  is_valid: boolean;
  is_fetching: boolean;
  age_seconds?: number;
  last_fetched?: string;
  releases_count?: number;
}

export interface BackgroundFetchCompletedResponse extends BaseResponse {
  completed: boolean;
}

export interface ResetBackgroundFetchFlagResponse extends BaseResponse {
  // Empty body on success
}

// ============================================================================
// Process & Launch Types
// ============================================================================

export interface LaunchResponse extends BaseResponse {
  log_path?: string;
  ready?: boolean;
}

export interface StopComfyUIResponse extends BaseResponse {
  // Empty body on success
}

// ============================================================================
// Shortcuts Types
// ============================================================================

export interface ShortcutState {
  menu: boolean;
  desktop: boolean;
  tag: string;
}

export interface GetVersionShortcutsResponse extends BaseResponse {
  state: ShortcutState;
}

export interface GetAllShortcutStatesResponse extends BaseResponse {
  states: {
    active: string | null;
    states: Record<string, ShortcutState>;
  };
}

export interface SetVersionShortcutsResponse extends BaseResponse {
  state: ShortcutState;
}

export interface ToggleShortcutResponse extends BaseResponse {
  state: ShortcutState;
}

// ============================================================================
// Launcher Update Types
// ============================================================================

export interface LauncherVersionResponse extends BaseResponse {
  version: string;
  branch: string;
  isGitRepo: boolean;
}

export interface CheckLauncherUpdatesResponse extends BaseResponse {
  hasUpdate: boolean;
  currentCommit: string;
  latestCommit: string;
  commitsBehind: number;
  commits: Array<{
    hash: string;
    message: string;
    author: string;
    date: string;
  }>;
}

export interface ApplyLauncherUpdateResponse extends BaseResponse {
  message: string;
  newCommit?: string;
}

export interface RestartLauncherResponse extends BaseResponse {
  message: string;
}

// ============================================================================
// Utility Types
// ============================================================================

export interface OpenPathResponse extends BaseResponse {
  // Empty body on success
}

export interface OpenActiveInstallResponse extends BaseResponse {
  // Empty body on success
}

export interface OpenUrlResponse extends BaseResponse {
  // Empty body on success
}

export interface CloseWindowResponse extends BaseResponse {
  // Empty body on success
}

// ============================================================================
// Main PyWebView API Interface
// ============================================================================

export interface PyWebViewAPI {
  // ========================================
  // Status & System
  // ========================================
  get_status(): Promise<StatusResponse>;
  get_disk_space(): Promise<DiskSpaceResponse>;
  get_system_resources(): Promise<SystemResourcesResponse>;

  // ========================================
  // Dependencies
  // ========================================
  install_deps(): Promise<BaseResponse>;

  // ========================================
  // Shortcuts
  // ========================================
  toggle_menu(tag?: string): Promise<BaseResponse>;
  toggle_desktop(tag?: string): Promise<BaseResponse>;
  get_version_shortcuts(tag: string): Promise<GetVersionShortcutsResponse>;
  get_all_shortcut_states(): Promise<GetAllShortcutStatesResponse>;
  set_version_shortcuts(tag: string, enabled: boolean): Promise<SetVersionShortcutsResponse>;
  toggle_version_menu(tag: string): Promise<ToggleShortcutResponse>;
  toggle_version_desktop(tag: string): Promise<ToggleShortcutResponse>;

  // ========================================
  // Version Management
  // ========================================
  get_available_versions(forceRefresh?: boolean): Promise<GetAvailableVersionsResponse>;
  get_installed_versions(): Promise<GetInstalledVersionsResponse>;
  get_active_version(): Promise<GetActiveVersionResponse>;
  install_version(tag: string): Promise<VersionActionResponse>;
  remove_version(tag: string): Promise<VersionActionResponse>;
  switch_version(tag: string): Promise<VersionActionResponse>;
  validate_installations(): Promise<ValidateInstallationsResponse>;
  get_version_info(tag: string): Promise<GetVersionInfoResponse>;
  get_default_version(): Promise<GetDefaultVersionResponse>;
  set_default_version(tag?: string | null): Promise<SetDefaultVersionResponse>;
  get_version_status(): Promise<VersionStatusResponse>;
  launch_version(tag: string, extraArgs?: string[]): Promise<LaunchResponse>;
  check_version_dependencies(tag: string): Promise<BaseResponse>;
  install_version_dependencies(tag: string): Promise<BaseResponse>;

  // ========================================
  // Installation & Progress
  // ========================================
  get_installation_progress(): Promise<InstallationProgressResponse | null>;
  cancel_installation(): Promise<CancelInstallationResponse>;

  // ========================================
  // Cache & Background Fetch
  // ========================================
  get_github_cache_status(): Promise<CacheStatusResponse>;
  should_update_ui_from_background_fetch(): Promise<boolean>;
  reset_background_fetch_flag(): Promise<ResetBackgroundFetchFlagResponse>;

  // ========================================
  // Process Management
  // ========================================
  launch_comfyui(): Promise<LaunchResponse>;
  stop_comfyui(): Promise<StopComfyUIResponse>;

  // ========================================
  // Model Management
  // ========================================
  get_models(): Promise<ModelsResponse>;
  scan_shared_storage(): Promise<ScanSharedStorageResponse>;
  search_hf_models(
    query: string,
    kind?: string | null,
    limit?: number
  ): Promise<SearchHFModelsResponse>;
  start_model_download_from_hf(
    repoId: string,
    family: string,
    officialName: string,
    modelType?: string | null,
    subtype?: string | null,
    quant?: string | null
  ): Promise<ModelDownloadResponse>;
  download_model_from_hf(
    repoId: string,
    family: string,
    officialName: string,
    modelType?: string | null,
    subtype?: string | null,
    quant?: string | null
  ): Promise<ModelDownloadResponse>;
  get_model_download_status(downloadId: string): Promise<ModelDownloadStatusResponse>;
  cancel_model_download(downloadId: string): Promise<BaseResponse>;

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
   * Detect and group sharded model files
   */
  detect_sharded_sets(filePaths: string[]): Promise<DetectShardedSetsResponse>;

  /**
   * Validate file type using magic bytes
   */
  validate_file_type(filePath: string): Promise<FileTypeValidationResponse>;

  /**
   * Mark model metadata as manually corrected to protect from auto-updates
   */
  mark_metadata_as_manual(modelId: string): Promise<BaseResponse>;

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

  // ========================================
  // Link Health (Phase 1B)
  // ========================================
  /**
   * Get health status of model symlinks
   */
  get_link_health(versionTag?: string | null): Promise<LinkHealthResponse>;

  /**
   * Remove broken links from the registry and filesystem
   */
  clean_broken_links(): Promise<CleanBrokenLinksResponse>;

  /**
   * Remove orphaned symlinks from a version's models directory
   */
  remove_orphaned_links(versionTag: string): Promise<RemoveOrphanedLinksResponse>;

  /**
   * Get all links for a specific model
   */
  get_links_for_model(modelId: string): Promise<GetLinksForModelResponse>;

  /**
   * Delete a model and all its symlinks
   */
  delete_model_with_cascade(modelId: string): Promise<DeleteModelCascadeResponse>;

  // ========================================
  // Mapping Preview (Phase 1C)
  // ========================================
  /**
   * Preview model mapping operations without making changes
   */
  preview_model_mapping(versionTag: string): Promise<MappingPreviewResponse>;

  /**
   * Incrementally sync specific models to a version
   */
  sync_models_incremental(
    versionTag: string,
    modelIds: string[]
  ): Promise<IncrementalSyncResponse>;

  /**
   * Check if library and app version are on different filesystems
   */
  get_cross_filesystem_warning(versionTag: string): Promise<CrossFilesystemWarningResponse>;

  /**
   * Apply model mapping for a specific version
   * Cleans broken links and creates/updates symlinks for all mapped models
   */
  apply_model_mapping(versionTag: string): Promise<ApplyModelMappingResponse>;

  /**
   * Apply model mapping with user-provided conflict resolutions
   * Allows user to choose skip/overwrite/rename for each conflict
   */
  sync_with_resolutions(
    versionTag: string,
    resolutions: ConflictResolutions
  ): Promise<SyncWithResolutionsResponse>;

  /**
   * Get sandbox environment information
   * Detects Flatpak, Snap, Docker, AppImage environments
   */
  get_sandbox_info(): Promise<SandboxInfoResponse>;

  // ========================================
  // Custom Nodes
  // ========================================
  get_custom_nodes(versionTag: string): Promise<{
    success: boolean;
    nodes: string[];
    error?: string;
  }>;
  install_custom_node(
    gitUrl: string,
    versionTag: string,
    nodeName?: string
  ): Promise<BaseResponse>;
  update_custom_node(nodeName: string, versionTag: string): Promise<BaseResponse>;
  remove_custom_node(nodeName: string, versionTag: string): Promise<BaseResponse>;

  // ========================================
  // Size Calculation
  // ========================================
  calculate_release_size(
    tag: string,
    forceRefresh?: boolean
  ): Promise<{
    success: boolean;
    total_bytes?: number;
    error?: string;
  }>;
  calculate_all_release_sizes(): Promise<{
    success: boolean;
    sizes?: Record<string, number>;
    error?: string;
  }>;

  // ========================================
  // Launcher Updates
  // ========================================
  get_launcher_version(): Promise<LauncherVersionResponse>;
  check_launcher_updates(forceRefresh?: boolean): Promise<CheckLauncherUpdatesResponse>;
  apply_launcher_update(): Promise<ApplyLauncherUpdateResponse>;
  restart_launcher(): Promise<RestartLauncherResponse>;

  // ========================================
  // Utility
  // ========================================
  open_url(url: string): Promise<OpenUrlResponse>;
  open_path(path: string): Promise<OpenPathResponse>;
  open_active_install(): Promise<OpenActiveInstallResponse>;
  close_window(): Promise<CloseWindowResponse>;
}

// ============================================================================
// Global Window Extension
// ============================================================================

declare global {
  interface Window {
    pywebview?: {
      api: PyWebViewAPI;
    };
  }
}

export {};
