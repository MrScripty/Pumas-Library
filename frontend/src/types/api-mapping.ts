import type { BaseResponse } from './api-common';

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
 * Link exclusions response
 */
export interface LinkExclusionsResponse extends BaseResponse {
  excluded_model_ids: string[];
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

/**
 * Per-model migration dry-run row
 */
export interface MigrationDryRunItem {
  model_id: string;
  target_model_id?: string | null;
  current_path: string;
  target_path?: string | null;
  action: string;
  current_model_type?: string | null;
  resolved_model_type?: string | null;
  resolver_source?: string | null;
  resolver_confidence?: number | null;
  resolver_review_reasons: string[];
  metadata_needs_review: boolean;
  review_reasons: string[];
  license_status?: string | null;
  declared_dependency_binding_count: number;
  active_dependency_binding_count: number;
  dependency_binding_history_count?: number;
  package_facts_cache_row_count?: number;
  package_facts_without_selected_artifact_count?: number;
  conversion_source_ref_count?: number;
  link_exclusion_count?: number;
  findings: string[];
  error?: string | null;
}

/**
 * Migration dry-run aggregate report
 */
export interface MigrationDryRunReport {
  generated_at: string;
  total_models: number;
  move_candidates: number;
  keep_candidates: number;
  collision_count: number;
  blocked_partial_count: number;
  blocked_reference_count?: number;
  error_count: number;
  models_with_findings: number;
  machine_readable_report_path?: string | null;
  human_readable_report_path?: string | null;
  items: MigrationDryRunItem[];
}

/**
 * Per-model migration execution result row
 */
export interface MigrationExecutionItem {
  model_id: string;
  target_model_id: string;
  action: string;
  error?: string | null;
}

/**
 * Migration execution aggregate report
 */
export interface MigrationExecutionReport {
  generated_at: string;
  completed_at?: string | null;
  resumed_from_checkpoint: boolean;
  checkpoint_path: string;
  planned_move_count: number;
  completed_move_count: number;
  skipped_move_count: number;
  error_count: number;
  reindexed_model_count: number;
  metadata_dir_count: number;
  index_model_count: number;
  index_metadata_model_count: number;
  index_partial_download_count: number;
  index_stale_model_count: number;
  referential_integrity_ok: boolean;
  referential_integrity_errors: string[];
  machine_readable_report_path?: string | null;
  human_readable_report_path?: string | null;
  results: MigrationExecutionItem[];
}

/**
 * Migration report artifact row from report index
 */
export interface MigrationReportArtifact {
  generated_at: string;
  report_kind: string;
  json_report_path: string;
  markdown_report_path: string;
}

/**
 * Generate migration dry-run report response
 */
export interface GenerateModelMigrationDryRunReportResponse extends BaseResponse {
  report: MigrationDryRunReport;
}

/**
 * Execute migration response
 */
export interface ExecuteModelMigrationResponse extends BaseResponse {
  report: MigrationExecutionReport;
}

/**
 * List migration report artifacts response
 */
export interface ListModelMigrationReportsResponse extends BaseResponse {
  reports: MigrationReportArtifact[];
}

/**
 * Delete one migration report artifact pair response
 */
export interface DeleteModelMigrationReportResponse extends BaseResponse {
  removed: boolean;
}

/**
 * Prune migration report history response
 */
export interface PruneModelMigrationReportsResponse extends BaseResponse {
  removed: number;
  kept: number;
}
