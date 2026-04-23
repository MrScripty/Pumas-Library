import type { BaseResponse } from './api-common';
import type {
  CancelConversionResponse,
  ConversionEnvironmentResponse,
  GetConversionProgressResponse,
  ListConversionsResponse,
  StartConversionResponse,
  SupportedQuantTypesResponse,
} from './api-conversion';
import type {
  CleanBrokenLinksResponse,
  DeleteModelCascadeResponse,
  GetLinksForModelResponse,
  LinkHealthResponse,
  RemoveOrphanedLinksResponse,
} from './api-links';
import type {
  ApplyModelMappingResponse,
  ConflictResolutions,
  CrossFilesystemWarningResponse,
  DeleteModelMigrationReportResponse,
  ExecuteModelMigrationResponse,
  GenerateModelMigrationDryRunReportResponse,
  IncrementalSyncResponse,
  LinkExclusionsResponse,
  ListModelMigrationReportsResponse,
  MappingPreviewResponse,
  PruneModelMigrationReportsResponse,
  SandboxInfoResponse,
  SyncWithResolutionsResponse,
} from './api-mapping';

export interface DesktopBridgeLinkMappingAPI {
  // ========================================
  // Link Health (Phase 1B)
  // ========================================
  /**
   * Get health status of model symlinks
   */
  get_link_health(versionTag?: string | null): Promise<LinkHealthResponse>;

  /**
   * Remove broken links from the registry and filesystem
   */
  clean_broken_links(): Promise<CleanBrokenLinksResponse>;

  /**
   * Remove orphaned symlinks from a version's models directory
   */
  remove_orphaned_links(versionTag: string): Promise<RemoveOrphanedLinksResponse>;

  /**
   * Get all links for a specific model
   */
  get_links_for_model(modelId: string): Promise<GetLinksForModelResponse>;

  /**
   * Delete a model and all its symlinks
   */
  delete_model_with_cascade(modelId: string): Promise<DeleteModelCascadeResponse>;

  // ========================================
  // Mapping Preview (Phase 1C)
  // ========================================
  /**
   * Preview model mapping operations without making changes
   */
  preview_model_mapping(versionTag: string): Promise<MappingPreviewResponse>;

  /**
   * Incrementally sync specific models to a version
   */
  sync_models_incremental(
    versionTag: string,
    modelIds: string[]
  ): Promise<IncrementalSyncResponse>;

  /**
   * Check if library and app version are on different filesystems
   */
  get_cross_filesystem_warning(versionTag: string): Promise<CrossFilesystemWarningResponse>;

  /**
   * Apply model mapping for a specific version
   * Cleans broken links and creates/updates symlinks for all mapped models
   */
  apply_model_mapping(versionTag: string): Promise<ApplyModelMappingResponse>;

  /**
   * Apply model mapping with user-provided conflict resolutions
   * Allows user to choose skip/overwrite/rename for each conflict
   */
  sync_with_resolutions(
    versionTag: string,
    resolutions: ConflictResolutions
  ): Promise<SyncWithResolutionsResponse>;

  /**
   * Get sandbox environment information
   * Detects Flatpak, Snap, Docker, AppImage environments
   */
  get_sandbox_info(): Promise<SandboxInfoResponse>;

  /**
   * Set whether a model is excluded from app linking
   */
  set_model_link_exclusion(
    modelId: string,
    appId: string,
    excluded: boolean
  ): Promise<BaseResponse>;

  /**
   * Get all model IDs excluded from linking for a given app
   */
  get_link_exclusions(appId: string): Promise<LinkExclusionsResponse>;

  /**
   * Generate metadata v2 migration dry-run report without mutating library paths
   */
  generate_model_migration_dry_run_report(): Promise<GenerateModelMigrationDryRunReportResponse>;

  /**
   * Execute metadata v2 migration with checkpoint/resume safety
   */
  execute_model_migration(): Promise<ExecuteModelMigrationResponse>;

  /**
   * List generated migration report artifacts from index
   */
  list_model_migration_reports(): Promise<ListModelMigrationReportsResponse>;

  /**
   * Delete one migration report artifact pair by indexed report path
   */
  delete_model_migration_report(reportPath: string): Promise<DeleteModelMigrationReportResponse>;

  /**
   * Prune migration report history to newest N entries
   */
  prune_model_migration_reports(keepLatest: number): Promise<PruneModelMigrationReportsResponse>;

  // ========================================
  // Model Format Conversion
  // ========================================
  start_model_conversion(
    modelId: string,
    direction: string,
    targetQuant?: string | null,
    outputName?: string | null
  ): Promise<StartConversionResponse>;
  get_conversion_progress(conversionId: string): Promise<GetConversionProgressResponse>;
  cancel_model_conversion(conversionId: string): Promise<CancelConversionResponse>;
  list_model_conversions(): Promise<ListConversionsResponse>;
  check_conversion_environment(): Promise<ConversionEnvironmentResponse>;
  setup_conversion_environment(): Promise<BaseResponse>;
  get_supported_quant_types(): Promise<SupportedQuantTypesResponse>;
}
