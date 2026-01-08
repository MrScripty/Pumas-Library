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
