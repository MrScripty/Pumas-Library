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

// Launcher-root results are declared once in electron/contracts and generated.
export type {
  LauncherRootSelectionAction,
  LauncherRootSelectionResult,
  LauncherRootStartupState,
} from '../generated/desktop-contract';

// ============================================================================
