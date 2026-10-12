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

export type LauncherRootSelectionAction = 'select-library' | 'correct-launch-input';

export type LauncherRootStartupState =
  | { status: 'initializing' }
  | { status: 'ready'; selectionAction: LauncherRootSelectionAction; libraryScopeId: string | null }
  | {
      status: 'recovery-required';
      reason: 'invalid' | 'unavailable' | 'migration-required' | 'backend-unavailable';
      authoritySource: 'persisted' | 'environment' | 'argument' | 'default';
      action: LauncherRootSelectionAction;
    };

export type LauncherRootSelectionResult =
  | { status: 'cancelled' }
  | { status: 'restarting' }
  | { status: 'not-selectable'; action: 'correct-launch-input' }
  | {
      status: 'recovery-required';
      reason: 'invalid-selection' | 'chooser-unavailable';
      authorityState: 'unchanged';
    }
  | {
      status: 'recovery-required';
      reason: 'persistence-unavailable';
      authorityState:
        | 'unchanged'
        | 'replacement-visibility-unknown'
        | 'published-durability-unavailable';
    }
  | {
      status: 'recovery-required';
      reason: 'restart-unavailable';
      authorityState: 'published';
    };

// ============================================================================
