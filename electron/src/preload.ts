/**
 * Electron Preload Script
 *
 * Exposes a secure API to the renderer process via contextBridge.
 * Mirrors the PyWebView API interface for seamless migration.
 */

import { contextBridge, ipcRenderer, webUtils } from 'electron';

/**
 * Generic RPC call wrapper
 * Converts method names from snake_case to match Python API
 */
async function apiCall<T>(method: string, params: Record<string, unknown> = {}): Promise<T> {
  return await ipcRenderer.invoke('api:call', method, params);
}

/**
 * Electron API exposed to the renderer
 * Matches PyWebViewAPI interface for compatibility
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
  install_deps: () => apiCall('install_deps'),

  // ========================================
  // Shortcuts
  // ========================================
  toggle_menu: (tag?: string) => apiCall('toggle_menu', { tag }),
  toggle_desktop: (tag?: string) => apiCall('toggle_desktop', { tag }),
  get_version_shortcuts: (tag: string) => apiCall('get_version_shortcuts', { tag }),
  get_all_shortcut_states: () => apiCall('get_all_shortcut_states'),
  set_version_shortcuts: (tag: string, enabled: boolean) =>
    apiCall('set_version_shortcuts', { tag, enabled }),
  toggle_version_menu: (tag: string) => apiCall('toggle_version_menu', { tag }),
  toggle_version_desktop: (tag: string) => apiCall('toggle_version_desktop', { tag }),

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
    apiCall('launch_version', { tag, extra_args: extraArgs, app_id: appId }),
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

  // ========================================
  // Model Management
  // ========================================
  get_models: () => apiCall('get_models'),
  refresh_model_index: () => apiCall('refresh_model_index'),
  refresh_model_mappings: (appId?: string) => apiCall('refresh_model_mappings', { app_id: appId }),
  scan_shared_storage: () => apiCall('scan_shared_storage'),
  search_hf_models: (query: string, kind?: string | null, limit?: number) =>
    apiCall('search_hf_models', { query, kind, limit }),
  get_related_models: (modelId: string, limit?: number) =>
    apiCall('get_related_models', { model_id: modelId, limit }),
  start_model_download_from_hf: (
    repoId: string,
    family: string,
    officialName: string,
    modelType?: string | null,
    subtype?: string | null,
    quant?: string | null
  ) =>
    apiCall('start_model_download_from_hf', {
      repo_id: repoId,
      family,
      official_name: officialName,
      model_type: modelType,
      subtype,
      quant,
    }),
  download_model_from_hf: (
    repoId: string,
    family: string,
    officialName: string,
    modelType?: string | null,
    subtype?: string | null,
    quant?: string | null
  ) =>
    apiCall('download_model_from_hf', {
      repo_id: repoId,
      family,
      official_name: officialName,
      model_type: modelType,
      subtype,
      quant,
    }),
  get_model_download_status: (downloadId: string) =>
    apiCall('get_model_download_status', { download_id: downloadId }),
  cancel_model_download: (downloadId: string) =>
    apiCall('cancel_model_download', { download_id: downloadId }),
  get_model_overrides: (relPath: string) => apiCall('get_model_overrides', { rel_path: relPath }),
  update_model_overrides: (relPath: string, overrides: Record<string, unknown>) =>
    apiCall('update_model_overrides', { rel_path: relPath, overrides }),

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
    apiCall('import_batch', { import_specs: importSpecs }),
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
  detect_sharded_sets: (filePaths: string[]) =>
    apiCall('detect_sharded_sets', { file_paths: filePaths }),
  validate_file_type: (filePath: string) => apiCall('validate_file_type', { file_path: filePath }),
  mark_metadata_as_manual: (modelId: string) =>
    apiCall('mark_metadata_as_manual', { model_id: modelId }),
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

  open_active_install: (appId?: string) => apiCall('open_active_install', { app_id: appId }),

  close_window: async () => {
    await ipcRenderer.invoke('window:close');
    return { success: true };
  },

  // Model Import Dialog - uses Electron's native dialog
  open_model_import_dialog: async () => {
    const result = await ipcRenderer.invoke('dialog:openFile', {
      title: 'Select Model Files',
      properties: ['openFile', 'multiSelections'],
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
  // Window Controls (Electron-specific)
  // ========================================
  minimizeWindow: () => ipcRenderer.invoke('window:minimize'),
  maximizeWindow: () => ipcRenderer.invoke('window:maximize'),
  getTheme: () => ipcRenderer.invoke('theme:get'),

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

// Also expose as pywebview.api for backwards compatibility during migration
contextBridge.exposeInMainWorld('pywebview', {
  api: electronAPI,
});

// Type declaration for the exposed API
export type ElectronAPI = typeof electronAPI;
