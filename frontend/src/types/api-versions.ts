import type { BaseResponse } from './api-common';
import type { AvailableVersionsOutcome, CancelInstallationOutcome, GithubCacheStatusOutcome, InstallationProgressOutcome, InstalledVersionsOutcome, RemoveVersionOutcome, SelectedVersionOutcome, SwitchVersionOutcome, ValidateInstallationsOutcome, VersionInfoOutcome, VersionStatusOutcome } from '../generated/desktop-contract';
export type { VersionReleaseAsset, VersionReleaseInfo } from '../generated/desktop-contract';

// ============================================================================
// Version Management Types
// ============================================================================

export type GetAvailableVersionsResponse = AvailableVersionsOutcome;

export type GetInstalledVersionsResponse = InstalledVersionsOutcome;

export type GetActiveVersionResponse = SelectedVersionOutcome;

export interface VersionActionResponse extends BaseResponse {
  // Used for install operations
}

export type RemoveVersionResponse = RemoveVersionOutcome;

export type SwitchVersionResponse = SwitchVersionOutcome;

export type ValidateInstallationsResponse = ValidateInstallationsOutcome;

export type GetVersionInfoResponse = VersionInfoOutcome;

export type GetDefaultVersionResponse = SelectedVersionOutcome;

export interface SetDefaultVersionResponse extends BaseResponse {
  // Empty body on success
}

export type VersionStatusResponse = VersionStatusOutcome;

// ============================================================================
// Installation & Progress Types
// ============================================================================

export type InstallationProgressResponse = InstallationProgressOutcome;

export type CancelInstallationResponse = CancelInstallationOutcome;

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
