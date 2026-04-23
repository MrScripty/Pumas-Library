import type { BaseResponse } from './api-common';

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
  /** True when the request was rate limited by the API provider (e.g., GitHub) */
  rate_limited?: boolean;
  /** Seconds until the rate limit resets (if known) */
  retry_after_secs?: number | null;
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
    releaseTag: string;
    pythonVersion?: string;
    downloadUrl?: string;
    size?: number;
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
