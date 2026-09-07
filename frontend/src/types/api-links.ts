import type { BaseResponse } from './api-common';
import type { LinkHealthOutcome } from '../generated/desktop-contract';

// ============================================================================
// Link Registry Types (Phase 1B)
// ============================================================================

/**
 * Health status levels returned by the backend link registry health check.
 */
export type HealthStatus = LinkHealthOutcome['status'];

/**
 * Link types supported by the registry
 */
export type LinkType = 'symlink' | 'hardlink' | 'copy';

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
export type LinkHealthResponse = LinkHealthOutcome;

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
