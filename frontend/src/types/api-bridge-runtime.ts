import type { BaseResponse } from './api-common';
import type {
  LaunchResponse,
  RuntimeLaunchResponse,
  RuntimeStopResponse,
  OllamaCreateModelResponse,
  OllamaListModelsResponse,
  OllamaListRunningResponse,
  StopOllamaResponse,
  TorchComputeDevice,
  TorchConfigureResponse,
  TorchGetStatusResponse,
  TorchListDevicesResponse,
  TorchListSlotsResponse,
  TorchLoadModelResponse,
  TorchServerConfig,
  TorchUnloadModelResponse,
} from './api-processes';
import type {
  DiskSpaceResponse,
  StatusResponse,
  StatusTelemetrySnapshot,
  SystemResourcesResponse,
} from './api-system';
import type {
  ModelRuntimeRoute,
  RuntimeProfileConfig,
  RuntimeProfileMutationResponse,
  RuntimeProfileUpdateFeedResponse,
  RuntimeProviderId,
  RuntimeProfilesSnapshotResponse,
} from './api-runtime-profiles';
import type {
  ModelServeValidationResponse,
  ServeModelRequest,
  ServeModelResponse,
  ServingStatusResponse,
  ServingStatusUpdateFeedResponse,
  UnserveModelRequest,
  UnserveModelResponse,
} from './api-serving';
import type {
  CacheStatusResponse,
  CancelInstallationResponse,
  CheckVersionDependenciesResponse,
  GetActiveVersionResponse,
  GetAvailableVersionsResponse,
  GetDefaultVersionResponse,
  GetInstalledVersionsResponse,
  GetReleaseDependenciesResponse,
  GetVersionInfoResponse,
  InstallationProgressResponse,
  InstallVersionDependenciesResponse,
  RemoveVersionResponse,
  ResetBackgroundFetchFlagResponse,
  SetDefaultVersionResponse,
  SwitchVersionResponse,
  ValidateInstallationsResponse,
  VersionActionResponse,
  VersionStatusResponse,
} from './api-versions';

export interface DesktopBridgeRuntimeAPI {
  // ========================================
  // Status & System
  // ========================================
  get_status(): Promise<StatusResponse>;
  get_disk_space(): Promise<DiskSpaceResponse>;
  get_system_resources(): Promise<SystemResourcesResponse>;
  get_status_telemetry_snapshot(): Promise<StatusTelemetrySnapshot>;

  // ========================================
  // Version Management
  // ========================================
  get_available_versions(
    forceRefresh?: boolean,
    appId?: string
  ): Promise<GetAvailableVersionsResponse>;
  get_installed_versions(appId?: string): Promise<GetInstalledVersionsResponse>;
  get_active_version(appId?: string): Promise<GetActiveVersionResponse>;
  install_version(tag: string, appId?: string): Promise<VersionActionResponse>;
  remove_version(tag: string, appId?: string): Promise<RemoveVersionResponse>;
  switch_version(tag: string, appId?: string): Promise<SwitchVersionResponse>;
  validate_installations(appId?: string): Promise<ValidateInstallationsResponse>;
  get_version_info(tag: string, appId?: string): Promise<GetVersionInfoResponse>;
  get_default_version(appId?: string): Promise<GetDefaultVersionResponse>;
  set_default_version(tag?: string | null, appId?: string): Promise<SetDefaultVersionResponse>;
  get_version_status(appId?: string): Promise<VersionStatusResponse>;
  launch_version(tag: string, extraArgs?: string[], appId?: string): Promise<RuntimeLaunchResponse>;
  check_version_dependencies(tag: string, appId: string): Promise<CheckVersionDependenciesResponse>;
  get_release_dependencies(tag: string, appId: string): Promise<GetReleaseDependenciesResponse>;
  install_version_dependencies(tag: string, appId: string): Promise<InstallVersionDependenciesResponse>;

  // ========================================
  // Installation & Progress
  // ========================================
  get_installation_progress(appId?: string): Promise<InstallationProgressResponse | null>;
  cancel_installation(appId?: string): Promise<CancelInstallationResponse>;

  // ========================================
  // Cache & Background Fetch
  // ========================================
  get_github_cache_status(appId?: string): Promise<CacheStatusResponse>;
  should_update_ui_from_background_fetch(): Promise<boolean>;
  reset_background_fetch_flag(): Promise<ResetBackgroundFetchFlagResponse>;

  // ========================================
  // Process Management
  // ========================================
  launch_ollama(): Promise<RuntimeLaunchResponse>;
  stop_ollama(): Promise<RuntimeStopResponse>;

  // Local Runtime Profiles
  get_runtime_profiles_snapshot(): Promise<RuntimeProfilesSnapshotResponse>;
  list_runtime_profile_updates_since(
    cursor?: string | null,
    limit?: number
  ): Promise<RuntimeProfileUpdateFeedResponse>;
  upsert_runtime_profile(
    profile: RuntimeProfileConfig
  ): Promise<RuntimeProfileMutationResponse>;
  delete_runtime_profile(profileId: string): Promise<RuntimeProfileMutationResponse>;
  set_model_runtime_route(route: ModelRuntimeRoute): Promise<RuntimeProfileMutationResponse>;
  clear_model_runtime_route(
    provider: RuntimeProviderId,
    modelId: string
  ): Promise<RuntimeProfileMutationResponse>;
  launch_runtime_profile(
    profileId: string,
    tag?: string | null,
    modelId?: string | null
  ): Promise<LaunchResponse>;
  stop_runtime_profile(profileId: string): Promise<StopOllamaResponse>;
  get_serving_status(): Promise<ServingStatusResponse>;
  list_serving_status_updates_since(
    cursor?: string | null
  ): Promise<ServingStatusUpdateFeedResponse>;
  validate_model_serving_config(
    request: ServeModelRequest
  ): Promise<ModelServeValidationResponse>;
  serve_model(request: ServeModelRequest): Promise<ServeModelResponse>;
  unserve_model(request: UnserveModelRequest): Promise<UnserveModelResponse>;

  // Ollama Model Management
  ollama_list_models(connectionUrl?: string): Promise<OllamaListModelsResponse>;
  ollama_list_models_for_profile(profileId?: string): Promise<OllamaListModelsResponse>;
  ollama_create_model(
    modelId: string,
    modelName?: string,
    connectionUrl?: string
  ): Promise<OllamaCreateModelResponse>;
  ollama_create_model_for_profile(
    modelId: string,
    modelName?: string,
    profileId?: string
  ): Promise<OllamaCreateModelResponse>;
  ollama_delete_model(
    modelName: string,
    connectionUrl?: string
  ): Promise<BaseResponse>;
  ollama_delete_model_for_profile(
    modelName: string,
    profileId?: string,
    modelId?: string
  ): Promise<BaseResponse>;
  ollama_load_model(
    modelName: string,
    connectionUrl?: string
  ): Promise<BaseResponse>;
  ollama_load_model_for_profile(
    modelName: string,
    profileId?: string,
    modelId?: string
  ): Promise<BaseResponse>;
  ollama_unload_model(
    modelName: string,
    connectionUrl?: string
  ): Promise<BaseResponse>;
  ollama_unload_model_for_profile(
    modelName: string,
    profileId?: string,
    modelId?: string
  ): Promise<BaseResponse>;
  ollama_list_running(
    connectionUrl?: string
  ): Promise<OllamaListRunningResponse>;

  // Torch Inference Server
  launch_torch(): Promise<RuntimeLaunchResponse>;
  stop_torch(): Promise<RuntimeStopResponse>;
  torch_list_slots(connectionUrl?: string): Promise<TorchListSlotsResponse>;
  torch_load_model(
    modelId: string,
    device?: TorchComputeDevice,
    connectionUrl?: string
  ): Promise<TorchLoadModelResponse>;
  torch_unload_model(
    slotId: string,
    connectionUrl?: string
  ): Promise<TorchUnloadModelResponse>;
  torch_get_status(connectionUrl?: string): Promise<TorchGetStatusResponse>;
  torch_list_devices(connectionUrl?: string): Promise<TorchListDevicesResponse>;
  torch_configure(config: Partial<TorchServerConfig>): Promise<TorchConfigureResponse>;
}
