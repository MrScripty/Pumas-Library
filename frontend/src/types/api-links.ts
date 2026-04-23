import type { BaseResponse } from './api-common';

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
