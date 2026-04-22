import type { OpenDialogOptions } from 'electron';

export type ApiCallPayload = {
  method: RpcMethodName;
  params: Record<string, unknown>;
};

export type RpcMethodName = (typeof ALLOWED_RPC_METHODS)[number];
type DialogProperty = NonNullable<OpenDialogOptions['properties']>[number];

export const ALLOWED_RPC_METHODS = [
  'health_check',
  'shutdown',
  'get_status',
  'get_disk_space',
  'get_system_resources',
  'get_launcher_version',
  'check_launcher_updates',
  'apply_launcher_update',
  'restart_launcher',
  'get_sandbox_info',
  'check_git',
  'check_brave',
  'check_setproctitle',
  'get_network_status',
  'get_library_status',
  'get_app_status',
  'get_available_versions',
  'get_installed_versions',
  'get_active_version',
  'get_default_version',
  'set_default_version',
  'switch_version',
  'install_version',
  'remove_version',
  'cancel_installation',
  'get_installation_progress',
  'validate_installations',
  'get_version_status',
  'get_version_info',
  'get_release_size_info',
  'get_release_size_breakdown',
  'calculate_release_size',
  'calculate_all_release_sizes',
  'has_background_fetch_completed',
  'reset_background_fetch_flag',
  'get_github_cache_status',
  'check_version_dependencies',
  'install_version_dependencies',
  'get_release_dependencies',
  'is_patched',
  'toggle_patch',
  'get_models',
  'refresh_model_index',
  'refresh_model_mappings',
  'import_model',
  'download_model_from_hf',
  'start_model_download_from_hf',
  'get_model_download_status',
  'cancel_model_download',
  'pause_model_download',
  'resume_model_download',
  'list_model_downloads',
  'list_interrupted_downloads',
  'recover_download',
  'resume_partial_download',
  'search_hf_models',
  'get_hf_download_details',
  'get_related_models',
  'search_models_fts',
  'import_batch',
  'import_external_diffusers_directory',
  'classify_model_import_paths',
  'lookup_hf_metadata_for_file',
  'lookup_hf_metadata_for_bundle_directory',
  'detect_sharded_sets',
  'validate_file_type',
  'get_embedded_metadata',
  'get_library_model_metadata',
  'resolve_model_execution_descriptor',
  'refetch_model_metadata_from_hf',
  'adopt_orphan_models',
  'import_model_in_place',
  'scan_shared_storage',
  'get_inference_settings',
  'update_inference_settings',
  'update_model_notes',
  'resolve_model_dependency_requirements',
  'audit_dependency_pin_compliance',
  'list_models_needing_review',
  'submit_model_review',
  'reset_model_review',
  'generate_model_migration_dry_run_report',
  'execute_model_migration',
  'list_model_migration_reports',
  'delete_model_migration_report',
  'prune_model_migration_reports',
  'set_hf_token',
  'clear_hf_token',
  'get_hf_auth_status',
  'is_comfyui_running',
  'stop_comfyui',
  'launch_comfyui',
  'launch_ollama',
  'stop_ollama',
  'is_ollama_running',
  'launch_torch',
  'stop_torch',
  'is_torch_running',
  'open_path',
  'open_url',
  'open_active_install',
  'ollama_list_models',
  'ollama_create_model',
  'ollama_delete_model',
  'ollama_load_model',
  'ollama_unload_model',
  'ollama_list_running',
  'torch_list_slots',
  'torch_load_model',
  'torch_unload_model',
  'torch_get_status',
  'torch_list_devices',
  'torch_configure',
  'get_link_health',
  'clean_broken_links',
  'remove_orphaned_links',
  'get_links_for_model',
  'delete_model_with_cascade',
  'preview_model_mapping',
  'apply_model_mapping',
  'sync_models_incremental',
  'sync_with_resolutions',
  'get_cross_filesystem_warning',
  'get_file_link_count',
  'check_files_writable',
  'set_model_link_exclusion',
  'get_link_exclusions',
  'get_version_shortcuts',
  'get_all_shortcut_states',
  'toggle_menu',
  'toggle_desktop',
  'menu_exists',
  'desktop_exists',
  'install_icon',
  'create_menu_shortcut',
  'create_desktop_shortcut',
  'remove_menu_shortcut',
  'remove_desktop_shortcut',
  'start_model_conversion',
  'get_conversion_progress',
  'cancel_model_conversion',
  'list_model_conversions',
  'check_conversion_environment',
  'setup_conversion_environment',
  'get_supported_quant_types',
  'get_backend_status',
  'setup_quantization_backend',
  'get_plugins',
  'get_plugin',
  'check_plugin_health',
  'get_custom_nodes',
  'install_custom_node',
  'update_custom_node',
  'remove_custom_node',
] as const;

const ALLOWED_RPC_METHOD_SET = new Set<string>(ALLOWED_RPC_METHODS);

const ALLOWED_DIALOG_PROPERTIES = new Set<DialogProperty>([
  'openFile',
  'openDirectory',
  'multiSelections',
  'showHiddenFiles',
  'createDirectory',
  'promptToCreate',
  'noResolveAliases',
  'treatPackageAsDirectory',
  'dontAddToRecent',
]);

export function validateApiCallPayload(rawMethod: unknown, rawParams: unknown): ApiCallPayload {
  if (typeof rawMethod !== 'string' || rawMethod.length === 0) {
    throw new Error('Invalid API method payload');
  }

  if (!ALLOWED_RPC_METHOD_SET.has(rawMethod)) {
    throw new Error(`Unknown API method: ${rawMethod}`);
  }

  if (rawParams === undefined || rawParams === null) {
    return { method: rawMethod as RpcMethodName, params: {} };
  }

  if (!isPlainRecord(rawParams)) {
    throw new Error('Invalid API params payload');
  }

  return { method: rawMethod as RpcMethodName, params: rawParams };
}

export function sanitizeOpenDialogOptions(rawOptions: unknown): OpenDialogOptions {
  if (!isPlainRecord(rawOptions)) {
    throw new Error('Invalid dialog options payload');
  }

  const options: OpenDialogOptions = {};

  if (typeof rawOptions.title === 'string') {
    options.title = rawOptions.title;
  }

  if (typeof rawOptions.defaultPath === 'string') {
    options.defaultPath = rawOptions.defaultPath;
  }

  if (typeof rawOptions.buttonLabel === 'string') {
    options.buttonLabel = rawOptions.buttonLabel;
  }

  if (typeof rawOptions.message === 'string') {
    options.message = rawOptions.message;
  }

  if (Array.isArray(rawOptions.properties)) {
    const properties = rawOptions.properties.filter(isDialogProperty);
    if (properties.length > 0) {
      options.properties = properties;
    }
  }

  if (Array.isArray(rawOptions.filters)) {
    const filters = rawOptions.filters
      .map((filter) => sanitizeDialogFilter(filter))
      .filter((filter): filter is NonNullable<OpenDialogOptions['filters']>[number] =>
        filter !== null
      );
    if (filters.length > 0) {
      options.filters = filters;
    }
  }

  return options;
}

export function validateExternalUrl(rawUrl: unknown): string {
  if (typeof rawUrl !== 'string') {
    throw new Error('Invalid URL payload');
  }

  let parsedUrl: URL;
  try {
    parsedUrl = new URL(rawUrl);
  } catch {
    throw new Error('Invalid URL');
  }

  if (parsedUrl.protocol !== 'http:' && parsedUrl.protocol !== 'https:') {
    throw new Error('Only http/https URLs are allowed');
  }

  return parsedUrl.toString();
}

function sanitizeDialogFilter(
  rawFilter: unknown
): NonNullable<OpenDialogOptions['filters']>[number] | null {
  if (!isPlainRecord(rawFilter)) {
    return null;
  }

  if (typeof rawFilter.name !== 'string' || !Array.isArray(rawFilter.extensions)) {
    return null;
  }

  const extensions = rawFilter.extensions.filter(
    (extension): extension is string => typeof extension === 'string' && extension.length > 0
  );

  if (extensions.length === 0) {
    return null;
  }

  return {
    name: rawFilter.name,
    extensions,
  };
}

function isDialogProperty(value: unknown): value is DialogProperty {
  return typeof value === 'string' && ALLOWED_DIALOG_PROPERTIES.has(value as DialogProperty);
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
