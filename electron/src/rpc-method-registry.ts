export const RPC_METHOD_REGISTRY = {
  version: 1,
  defaultPolicy: {
    owner: 'rust-json-rpc',
    stability: 'internal',
    paramsValidation: 'record',
    requestSchema: 'deferred',
    responseSchema: 'deferred',
  },
  methods: [
    'health_check',
    'shutdown',
    'get_status',
    'get_status_telemetry_snapshot',
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
    'get_runtime_profiles_snapshot',
    'list_runtime_profile_updates_since',
    'upsert_runtime_profile',
    'delete_runtime_profile',
    'set_model_runtime_route',
    'clear_model_runtime_route',
    'launch_runtime_profile',
    'stop_runtime_profile',
    'get_serving_status',
    'list_serving_status_updates_since',
    'validate_model_serving_config',
    'serve_model',
    'unserve_model',
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
    'resolve_model_package_facts',
    'list_model_library_updates_since',
    'resolve_model_package_facts_summary',
    'model_package_facts_summary_snapshot',
    'resolve_pumas_model_ref',
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
    'ollama_list_models_for_profile',
    'ollama_create_model',
    'ollama_create_model_for_profile',
    'ollama_delete_model',
    'ollama_delete_model_for_profile',
    'ollama_load_model',
    'ollama_load_model_for_profile',
    'ollama_unload_model',
    'ollama_unload_model_for_profile',
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
    'call_plugin_endpoint',
    'check_plugin_health',
    'get_custom_nodes',
    'install_custom_node',
    'update_custom_node',
    'remove_custom_node',
  ] as const,
} as const;

export type RpcMethodName = (typeof RPC_METHOD_REGISTRY.methods)[number];

export type RpcParamsValidationPolicy = 'record' | 'empty-record';

export type RpcParamFieldType =
  | 'boolean'
  | 'number'
  | 'string'
  | 'string-array'
  | 'string-record'
  | 'unknown-record'
  | 'unknown-array';

export type RpcRequestSchema = {
  required?: Readonly<Record<string, RpcParamFieldType>>;
  optional?: Readonly<Record<string, RpcParamFieldType>>;
  nullable?: Readonly<Record<string, RpcParamFieldType>>;
  allowUnknown?: boolean;
};

export const RPC_METHOD_PARAM_VALIDATION = {
  get_status: 'empty-record',
  get_status_telemetry_snapshot: 'empty-record',
  get_disk_space: 'empty-record',
  get_system_resources: 'empty-record',
  get_all_shortcut_states: 'empty-record',
  has_background_fetch_completed: 'empty-record',
  reset_background_fetch_flag: 'empty-record',
  get_models: 'empty-record',
  refresh_model_index: 'empty-record',
  scan_shared_storage: 'empty-record',
  list_model_downloads: 'empty-record',
  list_interrupted_downloads: 'empty-record',
  get_hf_auth_status: 'empty-record',
  launch_comfyui: 'empty-record',
  stop_comfyui: 'empty-record',
  launch_ollama: 'empty-record',
  stop_ollama: 'empty-record',
  launch_torch: 'empty-record',
  stop_torch: 'empty-record',
  get_plugins: 'empty-record',
  get_runtime_profiles_snapshot: 'empty-record',
  get_serving_status: 'empty-record',
} as const satisfies Partial<Record<RpcMethodName, RpcParamsValidationPolicy>>;

const RPC_METHOD_PARAM_VALIDATION_BY_METHOD: Partial<
  Record<RpcMethodName, RpcParamsValidationPolicy>
> = RPC_METHOD_PARAM_VALIDATION;

const OPTIONAL_APP_ID_SCHEMA = {
  optional: {
    app_id: 'string',
  },
} as const satisfies RpcRequestSchema;

const REQUIRED_TAG_OPTIONAL_APP_ID_SCHEMA = {
  required: {
    tag: 'string',
  },
  optional: {
    app_id: 'string',
  },
} as const satisfies RpcRequestSchema;

const REQUIRED_DOWNLOAD_ID_SCHEMA = {
  required: {
    download_id: 'string',
  },
} as const satisfies RpcRequestSchema;

export const RPC_METHOD_REQUEST_SCHEMAS = {
  get_available_versions: {
    optional: {
      force_refresh: 'boolean',
      app_id: 'string',
    },
  },
  get_installed_versions: OPTIONAL_APP_ID_SCHEMA,
  get_active_version: OPTIONAL_APP_ID_SCHEMA,
  get_default_version: OPTIONAL_APP_ID_SCHEMA,
  get_version_status: OPTIONAL_APP_ID_SCHEMA,
  get_installation_progress: OPTIONAL_APP_ID_SCHEMA,
  cancel_installation: OPTIONAL_APP_ID_SCHEMA,
  validate_installations: OPTIONAL_APP_ID_SCHEMA,
  get_github_cache_status: OPTIONAL_APP_ID_SCHEMA,
  open_active_install: OPTIONAL_APP_ID_SCHEMA,
  install_version: REQUIRED_TAG_OPTIONAL_APP_ID_SCHEMA,
  remove_version: REQUIRED_TAG_OPTIONAL_APP_ID_SCHEMA,
  switch_version: REQUIRED_TAG_OPTIONAL_APP_ID_SCHEMA,
  get_version_info: REQUIRED_TAG_OPTIONAL_APP_ID_SCHEMA,
  check_version_dependencies: REQUIRED_TAG_OPTIONAL_APP_ID_SCHEMA,
  install_version_dependencies: REQUIRED_TAG_OPTIONAL_APP_ID_SCHEMA,
  set_default_version: {
    optional: {
      app_id: 'string',
    },
    nullable: {
      tag: 'string',
    },
  },
  get_version_shortcuts: {
    required: {
      tag: 'string',
    },
  },
  toggle_menu: {
    optional: {
      tag: 'string',
    },
  },
  toggle_desktop: {
    optional: {
      tag: 'string',
    },
  },
  get_model_download_status: REQUIRED_DOWNLOAD_ID_SCHEMA,
  resolve_model_execution_descriptor: {
    required: {
      model_id: 'string',
    },
  },
  resolve_model_package_facts: {
    required: {
      model_id: 'string',
    },
  },
  resolve_model_package_facts_summary: {
    required: {
      model_id: 'string',
    },
  },
  model_package_facts_summary_snapshot: {
    optional: {
      limit: 'number',
      offset: 'number',
    },
  },
  list_model_library_updates_since: {
    optional: {
      cursor: 'string',
      limit: 'number',
    },
  },
  list_runtime_profile_updates_since: {
    optional: {
      cursor: 'string',
      limit: 'number',
    },
  },
  upsert_runtime_profile: {
    required: {
      profile: 'unknown-record',
    },
  },
  delete_runtime_profile: {
    required: {
      profile_id: 'string',
    },
  },
  set_model_runtime_route: {
    required: {
      route: 'unknown-record',
    },
  },
  clear_model_runtime_route: {
    required: {
      provider: 'string',
      model_id: 'string',
    },
  },
  launch_runtime_profile: {
    required: {
      profile_id: 'string',
    },
    optional: {
      tag: 'string',
      model_id: 'string',
    },
  },
  stop_runtime_profile: {
    required: {
      profile_id: 'string',
    },
  },
  validate_model_serving_config: {
    required: {
      request: 'unknown-record',
    },
  },
  list_serving_status_updates_since: {
    nullable: {
      cursor: 'string',
    },
  },
  serve_model: {
    required: {
      request: 'unknown-record',
    },
  },
  unserve_model: {
    required: {
      request: 'unknown-record',
    },
  },
  ollama_list_models_for_profile: {
    optional: {
      profile_id: 'string',
    },
  },
  ollama_create_model_for_profile: {
    required: {
      model_id: 'string',
    },
    optional: {
      model_name: 'string',
      profile_id: 'string',
    },
  },
  ollama_delete_model_for_profile: {
    required: {
      model_name: 'string',
    },
    optional: {
      model_id: 'string',
      profile_id: 'string',
    },
  },
  ollama_load_model_for_profile: {
    required: {
      model_name: 'string',
    },
    optional: {
      model_id: 'string',
      profile_id: 'string',
    },
  },
  ollama_unload_model_for_profile: {
    required: {
      model_name: 'string',
    },
    optional: {
      model_id: 'string',
      profile_id: 'string',
    },
  },
  resolve_pumas_model_ref: {
    required: {
      input: 'string',
    },
  },
  cancel_model_download: REQUIRED_DOWNLOAD_ID_SCHEMA,
  pause_model_download: REQUIRED_DOWNLOAD_ID_SCHEMA,
  resume_model_download: REQUIRED_DOWNLOAD_ID_SCHEMA,
  recover_download: {
    required: {
      repo_id: 'string',
      dest_dir: 'string',
    },
  },
  resume_partial_download: {
    required: {
      repo_id: 'string',
      dest_dir: 'string',
    },
  },
  open_path: {
    required: {
      path: 'string',
    },
  },
  open_url: {
    required: {
      url: 'string',
    },
  },
  get_plugin: {
    required: {
      app_id: 'string',
    },
  },
  check_plugin_health: {
    required: {
      app_id: 'string',
    },
  },
  call_plugin_endpoint: {
    required: {
      app_id: 'string',
      endpoint_name: 'string',
    },
    optional: {
      params: 'string-record',
    },
  },
  get_app_status: {
    required: {
      app_id: 'string',
    },
  },
  get_custom_nodes: {
    required: {
      version_tag: 'string',
    },
  },
  install_custom_node: {
    required: {
      git_url: 'string',
      version_tag: 'string',
    },
    optional: {
      node_name: 'string',
    },
  },
  update_custom_node: {
    required: {
      node_name: 'string',
      version_tag: 'string',
    },
  },
  remove_custom_node: {
    required: {
      node_name: 'string',
      version_tag: 'string',
    },
  },
} as const satisfies Partial<Record<RpcMethodName, RpcRequestSchema>>;

const RPC_METHOD_REQUEST_SCHEMAS_BY_METHOD: Partial<Record<RpcMethodName, RpcRequestSchema>> =
  RPC_METHOD_REQUEST_SCHEMAS;

export function getRpcParamsValidationPolicy(method: RpcMethodName): RpcParamsValidationPolicy {
  return (
    RPC_METHOD_PARAM_VALIDATION_BY_METHOD[method] ??
    RPC_METHOD_REGISTRY.defaultPolicy.paramsValidation
  );
}

export function getRpcRequestSchema(method: RpcMethodName): RpcRequestSchema | undefined {
  return RPC_METHOD_REQUEST_SCHEMAS_BY_METHOD[method];
}
