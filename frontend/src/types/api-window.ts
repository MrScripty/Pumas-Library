import type { BaseResponse } from './api-common';

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

export interface SelectLauncherRootResponse extends BaseResponse {
  cancelled?: boolean;
  restarting?: boolean;
  selectedPath?: string;
  launcherRoot?: string;
}

// ============================================================================
