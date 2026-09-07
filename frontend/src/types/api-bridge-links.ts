import type { BaseResponse } from './api-common';
import type {
  CancelConversionResponse,
  ConversionEnvironmentResponse,
  ConversionDirection,
  ConversionBackendStatusResponse,
  ConversionSetupResponse,
  QuantBackend,
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
  DeleteModelMigrationReportResponse,
  ExecuteModelMigrationResponse,
  GenerateModelMigrationDryRunReportResponse,
  LinkExclusionsResponse,
  ListModelMigrationReportsResponse,
  PruneModelMigrationReportsResponse,
  SandboxInfoResponse,
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
    direction: ConversionDirection,
    targetQuant?: string | null,
    outputName?: string | null,
    imatrixCalibrationFile?: string | null,
    forceImatrix?: boolean | null
  ): Promise<StartConversionResponse>;
  get_conversion_progress(conversionId: string): Promise<GetConversionProgressResponse>;
  cancel_model_conversion(conversionId: string): Promise<CancelConversionResponse>;
  list_model_conversions(): Promise<ListConversionsResponse>;
  check_conversion_environment(): Promise<ConversionEnvironmentResponse>;
  setup_conversion_environment(): Promise<ConversionSetupResponse>;
  get_supported_quant_types(): Promise<SupportedQuantTypesResponse>;
  get_backend_status(): Promise<ConversionBackendStatusResponse>;
  setup_quantization_backend(backend: QuantBackend): Promise<ConversionSetupResponse>;
}
