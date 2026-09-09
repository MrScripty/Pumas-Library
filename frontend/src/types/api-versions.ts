import type { BaseResponse } from './api-common';
import type { AvailableVersionsOutcome, GithubCacheStatusOutcome, InstalledVersionsOutcome, SelectedVersionOutcome, VersionInfoOutcome, VersionStatusOutcome } from '../generated/desktop-contract';
export type { VersionReleaseAsset, VersionReleaseInfo } from '../generated/desktop-contract';

// ============================================================================
// Version Management Types
// ============================================================================

export type GetAvailableVersionsResponse = AvailableVersionsOutcome;

export type GetInstalledVersionsResponse = InstalledVersionsOutcome;

export type GetActiveVersionResponse = SelectedVersionOutcome;

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

export type GetVersionInfoResponse = VersionInfoOutcome;

export type GetDefaultVersionResponse = SelectedVersionOutcome;

export interface SetDefaultVersionResponse extends BaseResponse {
  // Empty body on success
}

export type VersionStatusResponse = VersionStatusOutcome;

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

export type CacheStatusResponse = GithubCacheStatusOutcome;

export interface BackgroundFetchCompletedResponse extends BaseResponse {
  completed: boolean;
}

export interface ResetBackgroundFetchFlagResponse extends BaseResponse {
  // Empty body on success
}
