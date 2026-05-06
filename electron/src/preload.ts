/**
 * Electron Preload Script
 *
 * Exposes a secure API to the renderer process via contextBridge.
 * Implements the canonical desktop bridge contract for the renderer process.
 */

import { contextBridge, ipcRenderer, webUtils, type IpcRendererEvent } from 'electron';

/**
 * Generic RPC call wrapper
 * Converts method names from snake_case to match Python API
 */
async function apiCall<T>(method: string, params: Record<string, unknown> = {}): Promise<T> {
  return await ipcRenderer.invoke('api:call', method, params);
}

type BaseRpcResponse = {
  success: boolean;
  error?: string;
};

type ActiveVersionResponse = BaseRpcResponse & {
  version?: string | null;
};

type LaunchRpcResponse = BaseRpcResponse & {
  log_path?: string | null;
  ready?: boolean | null;
};

type VersionShortcutState = {
  tag?: string;
  menu: boolean;
  desktop: boolean;
};

type VersionShortcutResponse = BaseRpcResponse & {
  state?: VersionShortcutState;
};

type ModelLibraryUpdateNotificationPayload = {
  cursor: string;
  events?: unknown[];
  stale_cursor: boolean;
  snapshot_required: boolean;
};

type RuntimeProfileUpdateNotificationPayload = {
  cursor: string;
  events?: unknown[];
  stale_cursor: boolean;
  snapshot_required: boolean;
};

type StatusTelemetryUpdateNotificationPayload = {
  cursor: string;
  snapshot: unknown;
  stale_cursor: boolean;
  snapshot_required: boolean;
};

const MODEL_LIBRARY_CHANGE_KINDS = new Set([
  'model_added',
  'model_removed',
  'metadata_modified',
  'package_facts_modified',
  'stale_facts_invalidated',
  'dependency_binding_modified',
]);

const MODEL_FACT_FAMILIES = new Set([
  'model_record',
  'metadata',
  'package_facts',
  'dependency_bindings',
  'validation',
  'search_index',
]);

const MODEL_LIBRARY_REFRESH_SCOPES = new Set(['summary', 'detail', 'summary_and_detail']);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNullableString(value: unknown): boolean {
  return value === undefined || value === null || typeof value === 'string';
}

function isModelLibraryUpdateEventPayload(value: unknown): boolean {
  if (!isRecord(value)) {
    return false;
  }

  return (
    typeof value['cursor'] === 'string' &&
    typeof value['model_id'] === 'string' &&
    MODEL_LIBRARY_CHANGE_KINDS.has(String(value['change_kind'])) &&
    MODEL_FACT_FAMILIES.has(String(value['fact_family'])) &&
    MODEL_LIBRARY_REFRESH_SCOPES.has(String(value['refresh_scope'])) &&
    isNullableString(value['selected_artifact_id']) &&
    isNullableString(value['producer_revision'])
  );
}

function isModelLibraryUpdateNotificationPayload(
  value: unknown
): value is ModelLibraryUpdateNotificationPayload {
  if (!isRecord(value)) {
    return false;
  }

  const events = value['events'];
  return (
    typeof value['cursor'] === 'string' &&
    typeof value['stale_cursor'] === 'boolean' &&
    typeof value['snapshot_required'] === 'boolean' &&
    (events === undefined ||
      (Array.isArray(events) && events.every(isModelLibraryUpdateEventPayload)))
  );
}

function isRuntimeProfileUpdateNotificationPayload(
  value: unknown
): value is RuntimeProfileUpdateNotificationPayload {
  if (!isRecord(value)) {
    return false;
  }

  const events = value['events'];
  return (
    typeof value['cursor'] === 'string' &&
    typeof value['stale_cursor'] === 'boolean' &&
    typeof value['snapshot_required'] === 'boolean' &&
    (events === undefined || Array.isArray(events))
  );
}

function isStatusTelemetryUpdateNotificationPayload(
  value: unknown
): value is StatusTelemetryUpdateNotificationPayload {
  if (!isRecord(value) || !isRecord(value['snapshot'])) {
    return false;
  }

  const snapshot = value['snapshot'];
  return (
    typeof value['cursor'] === 'string' &&
    typeof value['stale_cursor'] === 'boolean' &&
    typeof value['snapshot_required'] === 'boolean' &&
    typeof snapshot['cursor'] === 'string' &&
    typeof snapshot['revision'] === 'number'
  );
}

async function installActiveVersionDependencies(): Promise<BaseRpcResponse> {
  const activeVersion = await apiCall<ActiveVersionResponse>('get_active_version', {
    app_id: 'comfyui',
  });

  if (!activeVersion.success || !activeVersion.version) {
    return {
      success: false,
      error: activeVersion.error ?? 'No active ComfyUI version set',
    };
  }

  return await apiCall('install_version_dependencies', {
    tag: activeVersion.version,
    app_id: 'comfyui',
  });
}

async function setVersionShortcuts(
  tag: string,
  enabled: boolean
): Promise<VersionShortcutResponse> {
  const current = await apiCall<VersionShortcutResponse>('get_version_shortcuts', { tag });
  if (!current.success) {
    return current;
  }

  const currentState = current.state ?? { tag, menu: false, desktop: false };
  if (currentState.menu !== enabled) {
    const menuResult = await apiCall<BaseRpcResponse>('toggle_menu', { tag });
    if (!menuResult.success) {
      return {
        success: false,
        state: currentState,
        error: menuResult.error ?? 'Failed to update menu shortcut',
      };
    }
  }

  if (currentState.desktop !== enabled) {
    const desktopResult = await apiCall<BaseRpcResponse>('toggle_desktop', { tag });
    if (!desktopResult.success) {
      return {
        success: false,
        state: currentState,
        error: desktopResult.error ?? 'Failed to update desktop shortcut',
      };
    }
  }

  const refreshed = await apiCall<VersionShortcutResponse>('get_version_shortcuts', { tag });
  return {
    success: refreshed.success,
    state: refreshed.state ?? { tag, menu: enabled, desktop: enabled },
    error: refreshed.error,
  };
}

async function launchAppVersion(
  appId: string | undefined,
  versionTag: string
): Promise<LaunchRpcResponse> {
  const resolvedAppId = appId ?? 'comfyui';
  const switchResult = await apiCall<BaseRpcResponse>('switch_version', {
    tag: versionTag,
    app_id: resolvedAppId,
  });

  if (!switchResult.success) {
    return {
      success: false,
      error: switchResult.error ?? `Failed to switch ${resolvedAppId} to ${versionTag}`,
    };
  }

  switch (resolvedAppId) {
    case 'comfyui':
      return await apiCall('launch_comfyui');
    case 'ollama':
      return await apiCall('launch_ollama');
    case 'torch':
      return await apiCall('launch_torch');
    default:
      return {
        success: false,
        error: `Unsupported app launch target: ${resolvedAppId}`,
      };
  }
}

async function stopApp(appId: string): Promise<BaseRpcResponse> {
  switch (appId) {
    case 'comfyui':
      return await apiCall('stop_comfyui');
    case 'ollama':
      return await apiCall('stop_ollama');
    case 'torch':
      return await apiCall('stop_torch');
    default:
      return {
        success: false,
        error: `Unsupported app stop target: ${appId}`,
      };
  }
}

/**
 * Electron API exposed to the renderer
 * Matches the canonical desktop bridge contract
 */
const electronAPI = {
  // ========================================
  // Status & System
  // ========================================
  get_status: () => apiCall('get_status'),
  get_disk_space: () => apiCall('get_disk_space'),
  get_system_resources: () => apiCall('get_system_resources'),

  // ========================================
  // Dependencies
  // ========================================
  install_deps: () => installActiveVersionDependencies(),

  // ========================================
  // Shortcuts
  // ========================================
  toggle_menu: (tag?: string) => apiCall('toggle_menu', { tag }),
  toggle_desktop: (tag?: string) => apiCall('toggle_desktop', { tag }),
  get_version_shortcuts: (tag: string) => apiCall('get_version_shortcuts', { tag }),
  get_all_shortcut_states: () => apiCall('get_all_shortcut_states'),
  set_version_shortcuts: (tag: string, enabled: boolean) =>
    setVersionShortcuts(tag, enabled),
  toggle_version_menu: (tag: string) => apiCall('toggle_menu', { tag }),
  toggle_version_desktop: (tag: string) => apiCall('toggle_desktop', { tag }),

  // ========================================
  // Version Management
  // ========================================
  get_available_versions: (forceRefresh?: boolean, appId?: string) =>
    apiCall('get_available_versions', { force_refresh: forceRefresh, app_id: appId }),
  get_installed_versions: (appId?: string) => apiCall('get_installed_versions', { app_id: appId }),
  get_active_version: (appId?: string) => apiCall('get_active_version', { app_id: appId }),
  install_version: (tag: string, appId?: string) =>
    apiCall('install_version', { tag, app_id: appId }),
  remove_version: (tag: string, appId?: string) =>
    apiCall('remove_version', { tag, app_id: appId }),
  switch_version: (tag: string, appId?: string) =>
    apiCall('switch_version', { tag, app_id: appId }),
  validate_installations: (appId?: string) =>
    apiCall('validate_installations', { app_id: appId }),
  get_version_info: (tag: string, appId?: string) =>
    apiCall('get_version_info', { tag, app_id: appId }),
  get_default_version: (appId?: string) => apiCall('get_default_version', { app_id: appId }),
  set_default_version: (tag?: string | null, appId?: string) =>
    apiCall('set_default_version', { tag, app_id: appId }),
  get_version_status: (appId?: string) => apiCall('get_version_status', { app_id: appId }),
  launch_version: (tag: string, extraArgs?: string[], appId?: string) =>
    extraArgs && extraArgs.length > 0
      ? Promise.resolve({
          success: false,
          error: 'Extra launch arguments are not supported by the desktop RPC bridge',
        })
      : launchAppVersion(appId, tag),
  check_version_dependencies: (tag: string, appId?: string) =>
    apiCall('check_version_dependencies', { tag, app_id: appId }),
  install_version_dependencies: (tag: string, appId?: string) =>
    apiCall('install_version_dependencies', { tag, app_id: appId }),

  // ========================================
  // Installation & Progress
  // ========================================
  get_installation_progress: (appId?: string) =>
    apiCall('get_installation_progress', { app_id: appId }),
  cancel_installation: (appId?: string) => apiCall('cancel_installation', { app_id: appId }),

  // ========================================
  // Cache & Background Fetch
  // ========================================
  get_github_cache_status: (appId?: string) =>
    apiCall('get_github_cache_status', { app_id: appId }),
  should_update_ui_from_background_fetch: () => apiCall('has_background_fetch_completed'),
  reset_background_fetch_flag: () => apiCall('reset_background_fetch_flag'),

  // ========================================
  // Process Management
  // ========================================
  launch_comfyui: () => apiCall('launch_comfyui'),
  stop_comfyui: () => apiCall('stop_comfyui'),
  launch_ollama: () => apiCall('launch_ollama'),
  stop_ollama: () => apiCall('stop_ollama'),
  get_runtime_profiles_snapshot: () =>
    apiCall('get_runtime_profiles_snapshot'),
  list_runtime_profile_updates_since: (cursor?: string | null, limit?: number) =>
    apiCall('list_runtime_profile_updates_since', { cursor, limit }),
  upsert_runtime_profile: (profile: Record<string, unknown>) =>
    apiCall('upsert_runtime_profile', { profile }),
  delete_runtime_profile: (profileId: string) =>
    apiCall('delete_runtime_profile', { profile_id: profileId }),
  set_model_runtime_route: (route: Record<string, unknown>) =>
    apiCall('set_model_runtime_route', { route }),
  clear_model_runtime_route: (modelId: string) =>
    apiCall('clear_model_runtime_route', { model_id: modelId }),
  launch_runtime_profile: (profileId: string, tag?: string | null, modelId?: string | null) =>
    apiCall('launch_runtime_profile', { profile_id: profileId, tag, model_id: modelId }),
  stop_runtime_profile: (profileId: string) =>
    apiCall('stop_runtime_profile', { profile_id: profileId }),

  // Torch Inference Server
  launch_torch: () => apiCall('launch_torch'),
  stop_torch: () => apiCall('stop_torch'),
  torch_list_slots: (connectionUrl?: string) =>
    apiCall('torch_list_slots', { connection_url: connectionUrl }),
  torch_load_model: (modelId: string, device?: string, connectionUrl?: string) =>
    apiCall('torch_load_model', {
      model_id: modelId,
      device,
      connection_url: connectionUrl,
    }),
  torch_unload_model: (slotId: string, connectionUrl?: string) =>
    apiCall('torch_unload_model', {
      slot_id: slotId,
      connection_url: connectionUrl,
    }),
  torch_get_status: (connectionUrl?: string) =>
    apiCall('torch_get_status', { connection_url: connectionUrl }),
  torch_list_devices: (connectionUrl?: string) =>
    apiCall('torch_list_devices', { connection_url: connectionUrl }),
  torch_configure: (config: Record<string, unknown>) =>
    apiCall('torch_configure', config),

  // Ollama Model Management
  ollama_list_models: (connectionUrl?: string) =>
    apiCall('ollama_list_models', { connection_url: connectionUrl }),
  ollama_list_models_for_profile: (profileId?: string) =>
    apiCall('ollama_list_models_for_profile', { profile_id: profileId }),
  ollama_create_model: (modelId: string, modelName?: string, connectionUrl?: string) =>
    apiCall('ollama_create_model', {
      model_id: modelId,
      model_name: modelName,
      connection_url: connectionUrl,
    }),
  ollama_create_model_for_profile: (modelId: string, modelName?: string, profileId?: string) =>
    apiCall('ollama_create_model_for_profile', {
      model_id: modelId,
      model_name: modelName,
      profile_id: profileId,
    }),
  ollama_delete_model: (modelName: string, connectionUrl?: string) =>
    apiCall('ollama_delete_model', {
      model_name: modelName,
      connection_url: connectionUrl,
    }),
  ollama_delete_model_for_profile: (modelName: string, profileId?: string, modelId?: string) =>
    apiCall('ollama_delete_model_for_profile', {
      model_name: modelName,
      model_id: modelId,
      profile_id: profileId,
    }),
  ollama_load_model: (modelName: string, connectionUrl?: string) =>
    apiCall('ollama_load_model', {
      model_name: modelName,
      connection_url: connectionUrl,
    }),
  ollama_load_model_for_profile: (modelName: string, profileId?: string, modelId?: string) =>
    apiCall('ollama_load_model_for_profile', {
      model_name: modelName,
      model_id: modelId,
      profile_id: profileId,
    }),
  ollama_unload_model: (modelName: string, connectionUrl?: string) =>
    apiCall('ollama_unload_model', {
      model_name: modelName,
      connection_url: connectionUrl,
    }),
  ollama_unload_model_for_profile: (modelName: string, profileId?: string, modelId?: string) =>
    apiCall('ollama_unload_model_for_profile', {
      model_name: modelName,
      model_id: modelId,
      profile_id: profileId,
    }),
  ollama_list_running: (connectionUrl?: string) =>
    apiCall('ollama_list_running', { connection_url: connectionUrl }),

  // ========================================
  // Model Management
  // ========================================
  get_models: () => apiCall('get_models'),
  refresh_model_index: () => apiCall('refresh_model_index'),
  refresh_model_mappings: (appId?: string) => apiCall('refresh_model_mappings', { app_id: appId }),
  scan_shared_storage: () => apiCall('scan_shared_storage'),
  search_hf_models: (
    query: string,
    kind?: string | null,
    limit?: number,
    hydrateLimit?: number
  ) =>
    apiCall('search_hf_models', { query, kind, limit, hydrate_limit: hydrateLimit }),
  get_hf_download_details: (repoId: string, quants?: string[] | null) =>
    apiCall('get_hf_download_details', { repo_id: repoId, quants }),
  get_related_models: (modelId: string, limit?: number) =>
    apiCall('get_related_models', { model_id: modelId, limit }),
  start_model_download_from_hf: (
    repoId: string,
    family: string,
    officialName: string,
    modelType?: string | null,
    pipelineTag?: string | null,
    releaseDate?: string | null,
    downloadUrl?: string | null,
    quant?: string | null,
    filenames?: string[] | null
  ) =>
    apiCall('start_model_download_from_hf', {
      repo_id: repoId,
      family,
      official_name: officialName,
      model_type: modelType,
      pipeline_tag: pipelineTag,
      release_date: releaseDate,
      download_url: downloadUrl,
      quant,
      filenames,
    }),
  download_model_from_hf: (
    repoId: string,
    family: string,
    officialName: string,
    modelType?: string | null,
    pipelineTag?: string | null,
    releaseDate?: string | null,
    downloadUrl?: string | null,
    quant?: string | null,
    filenames?: string[] | null
  ) =>
    apiCall('download_model_from_hf', {
      repo_id: repoId,
      family,
      official_name: officialName,
      model_type: modelType,
      pipeline_tag: pipelineTag,
      release_date: releaseDate,
      download_url: downloadUrl,
      quant,
      filenames,
    }),
  get_model_download_status: (downloadId: string) =>
    apiCall('get_model_download_status', { download_id: downloadId }),
  cancel_model_download: (downloadId: string) =>
    apiCall('cancel_model_download', { download_id: downloadId }),
  pause_model_download: (downloadId: string) =>
    apiCall('pause_model_download', { download_id: downloadId }),
  resume_model_download: (downloadId: string) =>
    apiCall('resume_model_download', { download_id: downloadId }),
  list_model_downloads: () =>
    apiCall('list_model_downloads'),
  list_interrupted_downloads: () =>
    apiCall('list_interrupted_downloads'),
  recover_download: (repoId: string, destDir: string) =>
    apiCall('recover_download', { repo_id: repoId, dest_dir: destDir }),
  resume_partial_download: (repoId: string, destDir: string) =>
    apiCall('resume_partial_download', { repo_id: repoId, dest_dir: destDir }),
  get_library_model_metadata: (modelId: string) =>
    apiCall('get_library_model_metadata', { model_id: modelId }),
  refetch_model_metadata_from_hf: (modelId: string) =>
    apiCall('refetch_model_metadata_from_hf', { model_id: modelId }),
  resolve_model_package_facts: (modelId: string) =>
    apiCall('resolve_model_package_facts', { model_id: modelId }),
  list_model_library_updates_since: (cursor?: string | null, limit?: number) =>
    apiCall('list_model_library_updates_since', { cursor, limit }),
  resolve_model_package_facts_summary: (modelId: string) =>
    apiCall('resolve_model_package_facts_summary', { model_id: modelId }),
  model_package_facts_summary_snapshot: (limit?: number, offset?: number) =>
    apiCall('model_package_facts_summary_snapshot', { limit, offset }),
  resolve_pumas_model_ref: (input: string) =>
    apiCall('resolve_pumas_model_ref', { input }),

  // Inference Settings
  get_inference_settings: (modelId: string) =>
    apiCall('get_inference_settings', { model_id: modelId }),
  update_inference_settings: (modelId: string, inferenceSettings: Record<string, unknown>[]) =>
    apiCall('update_inference_settings', { model_id: modelId, settings: inferenceSettings }),
  update_model_notes: (modelId: string, notes?: string | null) =>
    apiCall('update_model_notes', { model_id: modelId, notes }),

  // HuggingFace Authentication
  set_hf_token: (token: string) => apiCall('set_hf_token', { token }),
  clear_hf_token: () => apiCall('clear_hf_token'),
  get_hf_auth_status: () => apiCall('get_hf_auth_status'),

  // ========================================
  // Model Library Import (Phase 1A)
  // ========================================
  search_models_fts: (
    query: string,
    limit?: number,
    offset?: number,
    modelType?: string | null,
    tags?: string[] | null
  ) =>
    apiCall('search_models_fts', {
      query,
      limit,
      offset,
      model_type: modelType,
      tags,
    }),
  import_batch: (importSpecs: Array<Record<string, unknown>>) =>
    apiCall('import_batch', { imports: importSpecs }),
  import_external_diffusers_directory: (spec: Record<string, unknown>) =>
    apiCall('import_external_diffusers_directory', spec),
  classify_model_import_paths: (paths: string[]) =>
    apiCall('classify_model_import_paths', { paths }),
  get_network_status: () => apiCall('get_network_status'),

  // ========================================
  // Model Import (Phase 2)
  // ========================================
  import_model: (localPath: string, family: string, officialName: string, repoId?: string) =>
    apiCall('import_model', {
      local_path: localPath,
      family,
      official_name: officialName,
      repo_id: repoId,
    }),
  lookup_hf_metadata_for_file: (filename: string, filePath?: string | null) =>
    apiCall('lookup_hf_metadata_for_file', { filename, file_path: filePath }),
  lookup_hf_metadata_for_bundle_directory: (directoryPath: string) =>
    apiCall('lookup_hf_metadata_for_bundle_directory', { directory_path: directoryPath }),
  detect_sharded_sets: (filePaths: string[]) =>
    apiCall('detect_sharded_sets', { files: filePaths }),
  validate_file_type: (filePath: string) => apiCall('validate_file_type', { file_path: filePath }),
  get_library_status: () => apiCall('get_library_status'),
  get_file_link_count: (filePath: string) => apiCall('get_file_link_count', { file_path: filePath }),
  check_files_writable: (filePaths: string[]) =>
    apiCall('check_files_writable', { file_paths: filePaths }),
  get_embedded_metadata: (filePath: string) =>
    apiCall('get_embedded_metadata', { file_path: filePath }),

  // ========================================
  // Link Health (Phase 1B)
  // ========================================
  get_link_health: (versionTag?: string | null) =>
    apiCall('get_link_health', { version_tag: versionTag }),
  clean_broken_links: () => apiCall('clean_broken_links'),
  remove_orphaned_links: (versionTag: string) =>
    apiCall('remove_orphaned_links', { version_tag: versionTag }),
  get_links_for_model: (modelId: string) => apiCall('get_links_for_model', { model_id: modelId }),
  delete_model_with_cascade: (modelId: string) =>
    apiCall('delete_model_with_cascade', { model_id: modelId }),

  // ========================================
  // Mapping Preview (Phase 1C)
  // ========================================
  preview_model_mapping: (versionTag: string) =>
    apiCall('preview_model_mapping', { version_tag: versionTag }),
  sync_models_incremental: (versionTag: string, modelIds: string[]) =>
    apiCall('sync_models_incremental', { version_tag: versionTag, model_ids: modelIds }),
  get_cross_filesystem_warning: (versionTag: string) =>
    apiCall('get_cross_filesystem_warning', { version_tag: versionTag }),
  apply_model_mapping: (versionTag: string) =>
    apiCall('apply_model_mapping', { version_tag: versionTag }),
  sync_with_resolutions: (versionTag: string, resolutions: Record<string, string>) =>
    apiCall('sync_with_resolutions', { version_tag: versionTag, resolutions }),
  get_sandbox_info: () => apiCall('get_sandbox_info'),
  set_model_link_exclusion: (modelId: string, appId: string, excluded: boolean) =>
    apiCall('set_model_link_exclusion', { model_id: modelId, app_id: appId, excluded }),
  get_link_exclusions: (appId: string) =>
    apiCall('get_link_exclusions', { app_id: appId }),
  generate_model_migration_dry_run_report: () =>
    apiCall('generate_model_migration_dry_run_report'),
  execute_model_migration: () => apiCall('execute_model_migration'),
  list_model_migration_reports: () => apiCall('list_model_migration_reports'),
  delete_model_migration_report: (reportPath: string) =>
    apiCall('delete_model_migration_report', { report_path: reportPath }),
  prune_model_migration_reports: (keepLatest: number) =>
    apiCall('prune_model_migration_reports', { keep_latest: keepLatest }),

  // ========================================
  // Custom Nodes
  // ========================================
  get_custom_nodes: (versionTag: string) => apiCall('get_custom_nodes', { version_tag: versionTag }),
  install_custom_node: (gitUrl: string, versionTag: string, nodeName?: string) =>
    apiCall('install_custom_node', { git_url: gitUrl, version_tag: versionTag, node_name: nodeName }),
  update_custom_node: (nodeName: string, versionTag: string) =>
    apiCall('update_custom_node', { node_name: nodeName, version_tag: versionTag }),
  remove_custom_node: (nodeName: string, versionTag: string) =>
    apiCall('remove_custom_node', { node_name: nodeName, version_tag: versionTag }),

  // ========================================
  // Model Format Conversion
  // ========================================
  start_model_conversion: (
    modelId: string,
    direction: string,
    targetQuant?: string | null,
    outputName?: string | null
  ) =>
    apiCall('start_model_conversion', {
      model_id: modelId,
      direction,
      target_quant: targetQuant,
      output_name: outputName,
    }),
  get_conversion_progress: (conversionId: string) =>
    apiCall('get_conversion_progress', { conversion_id: conversionId }),
  cancel_model_conversion: (conversionId: string) =>
    apiCall('cancel_model_conversion', { conversion_id: conversionId }),
  list_model_conversions: () => apiCall('list_model_conversions'),
  check_conversion_environment: () => apiCall('check_conversion_environment'),
  setup_conversion_environment: () => apiCall('setup_conversion_environment'),
  get_supported_quant_types: () => apiCall('get_supported_quant_types'),

  // ========================================
  // Size Calculation
  // ========================================
  calculate_release_size: (tag: string, forceRefresh?: boolean, appId?: string) =>
    apiCall('calculate_release_size', { tag, force_refresh: forceRefresh, app_id: appId }),
  calculate_all_release_sizes: () => apiCall('calculate_all_release_sizes'),
  get_release_size_info: (tag: string, archiveSize: number) =>
    apiCall('get_release_size_info', { tag, archive_size: archiveSize }),
  get_release_size_breakdown: (tag: string) => apiCall('get_release_size_breakdown', { tag }),
  get_release_dependencies: (tag: string, topN?: number) =>
    apiCall('get_release_dependencies', { tag, top_n: topN }),

  // ========================================
  // Launcher Updates
  // ========================================
  get_launcher_version: () => apiCall('get_launcher_version'),
  check_launcher_updates: (forceRefresh?: boolean) =>
    apiCall('check_launcher_updates', { force_refresh: forceRefresh }),
  apply_launcher_update: () => apiCall('apply_launcher_update'),
  restart_launcher: () => apiCall('restart_launcher'),

  // ========================================
  // Utility - Electron-native implementations
  // ========================================
  open_url: async (url: string) => {
    await ipcRenderer.invoke('shell:openExternal', url);
    return { success: true };
  },

  open_path: async (path: string) => {
    return await apiCall('open_path', { path });
  },

  select_launcher_root: async () => {
    return await ipcRenderer.invoke('launcher:chooseLibraryRoot');
  },

  open_active_install: (appId?: string) => apiCall('open_active_install', { app_id: appId }),

  close_window: async () => {
    await ipcRenderer.invoke('window:close');
    return { success: true };
  },

  // Model Import Dialog - uses Electron's native dialog
  open_model_import_dialog: async () => {
    const result = await ipcRenderer.invoke('dialog:openFile', {
      title: 'Select Model Files or Folders',
      properties: ['openFile', 'openDirectory', 'multiSelections'],
      filters: [
        {
          name: 'Model Files',
          extensions: ['safetensors', 'ckpt', 'gguf', 'pt', 'bin', 'pth', 'onnx'],
        },
        { name: 'All Files', extensions: ['*'] },
      ],
    });

    if (result.canceled) {
      return { success: true, paths: [] };
    }

    return { success: true, paths: result.filePaths };
  },

  // ========================================
  // Plugin System
  // ========================================
  get_plugins: () => apiCall('get_plugins'),
  get_plugin: (appId: string) => apiCall('get_plugin', { app_id: appId }),
  call_plugin_endpoint: (appId: string, endpointName: string, params: Record<string, string>) =>
    apiCall('call_plugin_endpoint', { app_id: appId, endpoint_name: endpointName, params }),
  check_plugin_health: (appId: string) => apiCall('check_plugin_health', { app_id: appId }),
  launch_app: (appId: string, versionTag: string) =>
    launchAppVersion(appId, versionTag),
  stop_app: (appId: string) => stopApp(appId),
  get_app_status: (appId: string) => apiCall('get_app_status', { app_id: appId }),

  // ========================================
  // Window Controls (Electron-specific)
  // ========================================
  minimizeWindow: () => ipcRenderer.invoke('window:minimize'),
  maximizeWindow: () => ipcRenderer.invoke('window:maximize'),
  getTheme: () => ipcRenderer.invoke('theme:get'),
  onModelLibraryUpdate: (
    callback: (notification: ModelLibraryUpdateNotificationPayload) => void
  ): (() => void) => {
    const listener = (_event: IpcRendererEvent, payload: unknown) => {
      if (isModelLibraryUpdateNotificationPayload(payload)) {
        callback(payload);
      }
    };

    ipcRenderer.on('model-library:update', listener);
    return () => {
      ipcRenderer.removeListener('model-library:update', listener);
    };
  },
  onRuntimeProfileUpdate: (
    callback: (notification: RuntimeProfileUpdateNotificationPayload) => void
  ): (() => void) => {
    const listener = (_event: IpcRendererEvent, payload: unknown) => {
      if (isRuntimeProfileUpdateNotificationPayload(payload)) {
        callback(payload);
      }
    };

    ipcRenderer.on('runtime-profile:update', listener);
    return () => {
      ipcRenderer.removeListener('runtime-profile:update', listener);
    };
  },
  onStatusTelemetryUpdate: (
    callback: (notification: StatusTelemetryUpdateNotificationPayload) => void
  ): (() => void) => {
    const listener = (_event: IpcRendererEvent, payload: unknown) => {
      if (isStatusTelemetryUpdateNotificationPayload(payload)) {
        callback(payload);
      }
    };

    ipcRenderer.on('status-telemetry:update', listener);
    void ipcRenderer.invoke('status-telemetry:subscribe');
    return () => {
      ipcRenderer.removeListener('status-telemetry:update', listener);
      void ipcRenderer.invoke('status-telemetry:unsubscribe');
    };
  },

  // ========================================
  // File Utilities (for drag-and-drop)
  // ========================================
  /**
   * Get the filesystem path for a dropped file.
   * Required for sandboxed renderer to access file paths.
   */
  getPathForFile: (file: File): string => {
    return webUtils.getPathForFile(file);
  },
};

// Expose the API to the renderer process
contextBridge.exposeInMainWorld('electronAPI', electronAPI);


// Type declaration for the exposed API
export type ElectronAPI = typeof electronAPI;
