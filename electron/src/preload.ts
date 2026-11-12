/**
 * Electron Preload Script
 *
 * Exposes a secure API to the renderer process via contextBridge.
 * Implements the canonical desktop bridge contract for the renderer process.
 */

import { contextBridge, ipcRenderer, webUtils, type IpcRendererEvent } from 'electron';
import { decodeModelImportSelection, type ModelImportSelection } from './model-import-picker';
import type {
  LauncherRootSelectionResult,
  LauncherRootStartupState,
} from './launcher-root-recovery';
import type { LauncherRootCommittedPresentation } from './window-presentation';
import type { LibraryUpgradeConfirmation, LibraryUpgradeResult } from './library-upgrade-contract';
import {
  decodeS3TransferRetryParams, decodeS3TransferRetryState, type S3TransferRetryParams,
  decodeS3PersistedImportsWire,
  decodeS3DiscoveryParams, decodeS3AuthenticatedDiscoveryParams, decodeS3DiscoveryOutcome,
  type S3DiscoveryParams, type S3AuthenticatedDiscoveryParams,
  decodeS3ImportParams,
  decodeS3BundleImportParams,
  decodeS3AuthenticatedBundleImportParams,
  decodeS3BundleImportObservation,
  type S3BundleImportParams,
  type S3AuthenticatedBundleImportParams,
  decodeS3AuthenticatedImportParams,
  type S3AuthenticatedImportParams,
  decodeS3ImportOutcome,
  decodeS3ImportStatusParams,
  decodeS3ImportCancelParams,
  decodeS3ImportCancelOutcome,
  type S3ImportParams,
  decodeCatalogSearchOutcome,
  decodeConversionProgressResponse,
  decodeConversionSetupStartedOutcome,
  decodeConversionSetupStatusOutcome,
  decodeStartBackendSetupParams,
  decodeGetBackendSetupParams,
  decodeConversionListOutcome,
  decodeConversionStartedOutcome,
  decodeConversionCancelledOutcome,
  decodeConversionEnvironmentOutcome,
  decodeSupportedQuantTypesOutcome,
  decodeBackendStatusOutcome,
  decodeSuccessOutcome,
  type ConversionDirection,
  type QuantBackend,
  decodeDownloadListOutcome,
  decodeModelDownloadUpdateOutcome,
  decodeModelLibraryUpdateNotification,
  decodeRuntimeProfileUpdateFeed,
  decodeServingStatusUpdateFeed,
  decodeStatusTelemetryUpdateNotification,
  decodeUpdateStreamError,
  decodeDownloadMutationOutcome,
  decodeDownloadLocationReport,
  decodeDownloadResolutionRequest,
  type DownloadResolutionRequest,
  decodeDownloadStartedOutcome,
  decodeDownloadStatusOutcome,
  decodeModelIndexRefreshOutcome,
  decodeModelsOutcome,
  decodeLinkHealthOutcome,
  decodeHfDownloadDetailsOutcome,
  decodeInferenceSettingsOutcome,
  decodeLibraryModelMetadataOutcome,
  decodeUpdateInferenceSettingsParams,
  decodeUpdateModelNotesParams,
  decodeUpdateModelNotesOutcome,
  decodeModelCategoryOutcome,
  decodeUpdateInferenceSettingsOutcome,
  decodeAvailableVersionsOutcome,
  decodeGithubCacheStatusOutcome,
  decodeInstalledVersionsOutcome,
  decodeSelectedVersionOutcome,
  decodeVersionStatusOutcome,
  decodeVersionInfoOutcome,
  decodeCheckVersionDependenciesOutcome,
  decodeCheckVersionDependenciesParams,
  decodeGetReleaseDependenciesOutcome,
  decodeGetReleaseDependenciesParams,
  decodeInstallVersionDependenciesOutcome,
  decodeInstallVersionDependenciesParams,
  decodeValidateInstallationsOutcome,
  decodeInstallationProgressOutcome,
  decodeCancelInstallationOutcome,
  decodeRemoveVersionOutcome,
  decodeSwitchVersionOutcome,
  decodeSwitchVersionParams,
  decodeRuntimeLaunchOutcome,
  decodeRuntimeLaunchParams,
  decodeRuntimeStopOutcome,
  decodeRuntimeProfilesSnapshotResponse,
  decodeRuntimeProvidersOutcome,
  decodeServingStatusResponse,
  decodeHfModelSearchOutcome,
  decodeSetDefaultVersionOutcome,
  decodeSetDefaultVersionParams,
  decodeInstallVersionOutcome,
  decodeInstallVersionParams,
  decodeGetTorchReleaseOptionsParams,
  decodeTorchReleaseOptionsOutcome,
  decodePreviewTorchRuntimeParams,
  decodeTorchRuntimePreviewOutcome,
  decodeGetTorchRuntimeProbeParams,
  decodeTrialTorchRuntimeParams,
  decodeTrialTorchRuntimeOutcome,
  decodeFindTorchAlternativesParams,
  decodeTorchAlternativesOutcome,
  decodeTorchRuntimeOptionsOutcome,
  decodeTorchRuntimeProbeOutcome,
  decodeStopRuntimeProfileGenerationParams,
  decodeStopRuntimeProfileGenerationOutcome,
  decodeGetHfDownloadDetailsParams,
  decodePartialDownloadOutcome,
  decodeRecoverDownloadParams,
  decodeSetHfTokenParams,
  decodeModelIdParams,
  decodeImportModelParams,
  decodeImportBatchParams,
  decodeStatusTelemetrySnapshot,
  decodeNetworkStatusResponse,
  decodeLibraryStatusResponse,
  decodeLibraryUpgradeConfirmation,
  decodeLauncherRootSelectionResult as decodeGeneratedLauncherRootSelectionResult,
  decodeLauncherRootStartupState as decodeGeneratedLauncherRootStartupState,
  decodeLibraryUpgradeResult as decodeGeneratedLibraryUpgradeResult,
  type DecodeOutcome,
  type ModelDownloadUpdateOutcome,
  type ModelLibraryUpdateNotification,
  type RuntimeProfileUpdateFeed,
  type ServingStatusUpdateFeed,
  type StatusTelemetryUpdateNotification,
  type RuntimeLaunchOutcome,
} from './desktop-contract';

const launcherRootPresentationTimeoutListeners = new Set<() => void>();
let launcherRootPresentationTimeoutLatched = false;

ipcRenderer.on('launcher-root:presentation-timeout', () => {
  launcherRootPresentationTimeoutLatched = true;
  for (const listener of launcherRootPresentationTimeoutListeners) {
    listener();
  }
});

class LauncherRootRecoveryContractError extends Error {
  readonly code = 'launcher_root_contract_invalid';

  constructor(message: string) {
    super(message);
    this.name = 'LauncherRootRecoveryContractError';
  }
}

/** Decode a renderer-facing result with its generated validator, or throw the stable contract error. */
function decodeOrThrow<T>(outcome: DecodeOutcome<T>, message: string): T {
  if (outcome.status === 'valid') return outcome.value;
  throw new LauncherRootRecoveryContractError(message);
}

function decodeLauncherRootStartupState(value: unknown): LauncherRootStartupState {
  return decodeOrThrow(decodeGeneratedLauncherRootStartupState(value), 'Invalid launcher-root startup state.');
}

function decodeLibraryUpgradeResult(value: unknown): LibraryUpgradeResult {
  return decodeOrThrow(decodeGeneratedLibraryUpgradeResult(value), 'Invalid library upgrade result.');
}

function decodeLauncherRootSelectionResult(value: unknown): LauncherRootSelectionResult {
  return decodeOrThrow(decodeGeneratedLauncherRootSelectionResult(value), 'Invalid launcher-root selection result.');
}

/**
 * Generic RPC call wrapper
 * Converts method names from snake_case to match Python API
 */
async function apiCall<T>(method: string, params: Record<string, unknown> = {}): Promise<T> {
  return await ipcRenderer.invoke('api:call', method, params);
}

class DesktopContractError extends Error {
  readonly status: Exclude<DecodeOutcome<never>, { status: 'valid' }>['status'];

  constructor(method: string, outcome: Exclude<DecodeOutcome<never>, { status: 'valid' }>) {
    super(`Desktop contract ${outcome.status} for ${method}: ${outcome.message}`);
    this.name = 'DesktopContractError';
    this.status = outcome.status;
  }
}

function requireDecoded<T>(result: DecodeOutcome<T>, method: string): T {
  if (result.status === 'valid') return result.value;
  throw new DesktopContractError(method, result);
}

async function validatedApiCall<T>(
  method: string,
  decode: (value: unknown) => DecodeOutcome<T>,
  params: Record<string, unknown> = {}
): Promise<T> {
  return requireDecoded(decode(await apiCall<unknown>(method, params)), method);
}

/** JSON omits undefined; the generated decoders accept only JSON values. */
function withoutUndefined<T extends Record<string, unknown>>(value: T): T {
  return Object.fromEntries(Object.entries(value).filter(([, field]) => field !== undefined)) as T;
}

type BaseRpcResponse = {
  success: boolean;
  error?: string;
};

type UpdateCallbacks<T> = {
  callback: (notification: T) => void;
  onError: (message: string) => void;
};

/**
 * Deliver decoded update-stream payloads. A payload that fails its contract is
 * never dropped silently: its reason goes to `onError`, and the renderer
 * resynchronizes from its RPC snapshot.
 */
function updateListener<T>(
  stream: string,
  decode: (value: unknown) => DecodeOutcome<T>,
  { callback, onError }: UpdateCallbacks<T>
): (event: IpcRendererEvent, payload: unknown) => void {
  return (_event, payload) => {
    const decoded = decode(payload);
    if (decoded.status === 'valid') {
      callback(decoded.value);
      return;
    }
    onError(`Desktop contract ${decoded.status} for ${stream} update: ${decoded.message}`);
  };
}

/**
 * Subscribe to one update stream: decoded payloads go to `callback`; an
 * undeliverable payload, a transport failure reported by main on
 * `<stream>:error`, or a failed subscription goes to `onError`.
 */
function subscribeUpdateStream<T>(
  stream: string,
  decode: (value: unknown) => DecodeOutcome<T>,
  callbacks: UpdateCallbacks<T>,
  { refcounted }: { refcounted: boolean }
): () => void {
  const listener = updateListener(stream, decode, callbacks);
  const errorListener = updateListener(`${stream} error`, decodeUpdateStreamError, {
    callback: (error) => callbacks.onError(error.message),
    onError: callbacks.onError,
  });
  ipcRenderer.on(`${stream}:update`, listener);
  ipcRenderer.on(`${stream}:error`, errorListener);
  if (refcounted) {
    void ipcRenderer.invoke(`${stream}:subscribe`).catch((error: unknown) => {
      const message = (error as { message?: unknown } | null)?.message;
      callbacks.onError(typeof message === 'string' && message.length > 0
        ? message
        : `Failed to subscribe to ${stream} updates`);
    });
  }
  return () => {
    ipcRenderer.removeListener(`${stream}:update`, listener);
    ipcRenderer.removeListener(`${stream}:error`, errorListener);
    if (refcounted) void ipcRenderer.invoke(`${stream}:unsubscribe`).catch(() => {});
  };
}

async function launchAppVersion(
  appId: string | undefined,
  versionTag: string
): Promise<RuntimeLaunchOutcome> {
  if (!appId) {
    return { success: false, error: 'An inference plugin app id is required' };
  }
  const resolvedAppId = appId;
  const switchParams = requireDecoded(
    decodeSwitchVersionParams({ tag: versionTag, app_id: resolvedAppId }),
    'switch_version request'
  );
  const switchResult = await validatedApiCall(
    'switch_version', decodeSwitchVersionOutcome, switchParams
  );

  if (!switchResult.success) {
    return {
      success: false,
      error: `Failed to switch ${resolvedAppId} to ${versionTag}`,
    };
  }

  switch (resolvedAppId) {
    case 'ollama':
      return await validatedApiCall(
        'launch_ollama', decodeRuntimeLaunchOutcome,
        requireDecoded(decodeRuntimeLaunchParams({}), 'launch_ollama request')
      );
    case 'torch':
      return await validatedApiCall(
        'launch_torch', decodeRuntimeLaunchOutcome,
        requireDecoded(decodeRuntimeLaunchParams({}), 'launch_torch request')
      );
    default:
      return {
        success: false,
        error: `Unsupported app launch target: ${resolvedAppId}`,
      };
  }
}

async function stopApp(appId: string): Promise<BaseRpcResponse> {
  switch (appId) {
    case 'ollama':
      return await validatedApiCall(
        'stop_ollama', decodeRuntimeStopOutcome,
        requireDecoded(decodeRuntimeLaunchParams({}), 'stop_ollama request')
      );
    case 'torch':
      return await validatedApiCall(
        'stop_torch', decodeRuntimeStopOutcome,
        requireDecoded(decodeRuntimeLaunchParams({}), 'stop_torch request')
      );
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
  // Pull snapshots decode like their push notifications (FE-02).
  get_status_telemetry_snapshot: () =>
    validatedApiCall('get_status_telemetry_snapshot', decodeStatusTelemetrySnapshot),
  get_disk_space: () => apiCall('get_disk_space'),
  get_system_resources: () => apiCall('get_system_resources'),

  // ========================================
  // Version Management
  // ========================================
  get_available_versions: (forceRefresh?: boolean, appId?: string) =>
    validatedApiCall('get_available_versions', decodeAvailableVersionsOutcome, { force_refresh: forceRefresh, app_id: appId }),
  get_installed_versions: (appId?: string) => validatedApiCall('get_installed_versions', decodeInstalledVersionsOutcome, { app_id: appId }),
  get_active_version: (appId?: string) => validatedApiCall('get_active_version', decodeSelectedVersionOutcome, { app_id: appId }),
  install_version: (tag: string, appId?: string, previewId?: string) => {
    const params = requireDecoded(
      decodeInstallVersionParams({ tag, app_id: appId }),
      'install_version request'
    );
    if (previewId !== undefined) {
      if (appId !== 'torch' || typeof previewId !== 'string' || !/^[a-f0-9]{48}$/.test(previewId)) {
        throw new TypeError('Invalid Torch preview ID');
      }
      return validatedApiCall('install_version', decodeInstallVersionOutcome, { ...params, preview_id: previewId });
    }
    return validatedApiCall('install_version', decodeInstallVersionOutcome, params);
  },
  get_torch_runtime_options: () => validatedApiCall('get_torch_runtime_options', decodeTorchRuntimeOptionsOutcome),
  get_torch_release_options: (tag: string) => {
    const params = requireDecoded(decodeGetTorchReleaseOptionsParams({ tag }), 'get_torch_release_options request');
    return validatedApiCall('get_torch_release_options', decodeTorchReleaseOptionsOutcome, params);
  },
  preview_torch_runtime: async (request: {
    tag: string; build: string; python: string; adapter: string; verified_versions?: boolean;
  }) => {
    const params = requireDecoded(
      decodePreviewTorchRuntimeParams(withoutUndefined(request)), 'preview_torch_runtime request');
    return validatedApiCall('preview_torch_runtime', decodeTorchRuntimePreviewOutcome, params);
  },
  get_torch_runtime_probe: async (tag: string) => {
    const params = requireDecoded(decodeGetTorchRuntimeProbeParams({ tag }), 'get_torch_runtime_probe request');
    return validatedApiCall('get_torch_runtime_probe', decodeTorchRuntimeProbeOutcome, params);
  },
  trial_torch_runtime: async (tag: string, profileId: string) => {
    const params = requireDecoded(decodeTrialTorchRuntimeParams({ tag, profileId }), 'trial_torch_runtime request');
    return validatedApiCall('trial_torch_runtime', decodeTrialTorchRuntimeOutcome, params);
  },
  stop_runtime_profile_if_generation: (profileId: string, generation: string) => {
    if (!profileId || !/^[1-9]\d*$/.test(generation)) {
      throw new TypeError('Conditional profile stop requires a profile ID and valid generation');
    }
    const params = requireDecoded(decodeStopRuntimeProfileGenerationParams({ profileId, generation }), 'stop_runtime_profile_if_generation request');
    return validatedApiCall('stop_runtime_profile_if_generation', decodeStopRuntimeProfileGenerationOutcome, params);
  },
  find_torch_alternatives: async (tag: string, build: string, python: string) => {
    const params = requireDecoded(decodeFindTorchAlternativesParams({ tag, build, python }), 'find_torch_alternatives request');
    return validatedApiCall('find_torch_alternatives', decodeTorchAlternativesOutcome, params);
  },
  remove_version: (tag: string, appId?: string) =>
    validatedApiCall('remove_version', decodeRemoveVersionOutcome, { tag, app_id: appId }),
  switch_version: (tag: string, appId?: string) => {
    const params = requireDecoded(
      decodeSwitchVersionParams({ tag, app_id: appId }),
      'switch_version request'
    );
    return validatedApiCall('switch_version', decodeSwitchVersionOutcome, params);
  },
  validate_installations: (appId?: string) =>
    validatedApiCall('validate_installations', decodeValidateInstallationsOutcome, { app_id: appId }),
  get_version_info: (tag: string, appId?: string) =>
    validatedApiCall('get_version_info', decodeVersionInfoOutcome, { tag, app_id: appId }),
  get_default_version: (appId?: string) => validatedApiCall('get_default_version', decodeSelectedVersionOutcome, { app_id: appId }),
  set_default_version: (tag?: string | null, appId?: string) => {
    const params = requireDecoded(decodeSetDefaultVersionParams(
      tag === undefined ? { app_id: appId } : { tag, app_id: appId }
    ), 'set_default_version request');
    return validatedApiCall('set_default_version', decodeSetDefaultVersionOutcome, params);
  },
  get_version_status: (appId?: string) => validatedApiCall('get_version_status', decodeVersionStatusOutcome, { app_id: appId }),
  launch_version: (tag: string, extraArgs?: string[], appId?: string) =>
    extraArgs && extraArgs.length > 0
      ? Promise.resolve({
          success: false,
          error: 'Extra launch arguments are not supported by the desktop RPC bridge',
        })
      : launchAppVersion(appId, tag),
  check_version_dependencies: (tag: string, appId: string) => {
    const params = requireDecoded(
      decodeCheckVersionDependenciesParams({ tag, app_id: appId }),
      'check_version_dependencies request'
    );
    return validatedApiCall(
      'check_version_dependencies',
      decodeCheckVersionDependenciesOutcome,
      params
    );
  },
  install_version_dependencies: (tag: string, appId: string) => {
    const params = requireDecoded(
      decodeInstallVersionDependenciesParams({ tag, app_id: appId }),
      'install_version_dependencies request'
    );
    return validatedApiCall(
      'install_version_dependencies',
      decodeInstallVersionDependenciesOutcome,
      params
    );
  },

  // ========================================
  // Installation & Progress
  // ========================================
  get_installation_progress: (appId?: string) =>
    validatedApiCall('get_installation_progress', decodeInstallationProgressOutcome, { app_id: appId }),
  cancel_installation: (appId?: string) =>
    validatedApiCall('cancel_installation', decodeCancelInstallationOutcome, { app_id: appId }),

  // ========================================
  // Cache & Background Fetch
  // ========================================
  get_github_cache_status: (appId?: string) =>
    validatedApiCall('get_github_cache_status', decodeGithubCacheStatusOutcome, { app_id: appId }),
  should_update_ui_from_background_fetch: () => apiCall('has_background_fetch_completed'),
  reset_background_fetch_flag: () => apiCall('reset_background_fetch_flag'),

  // ========================================
  // Process Management
  // ========================================
  launch_ollama: () => validatedApiCall(
    'launch_ollama', decodeRuntimeLaunchOutcome,
    requireDecoded(decodeRuntimeLaunchParams({}), 'launch_ollama request')
  ),
  stop_ollama: () => validatedApiCall(
    'stop_ollama', decodeRuntimeStopOutcome,
    requireDecoded(decodeRuntimeLaunchParams({}), 'stop_ollama request')
  ),
  get_runtime_providers: () =>
    validatedApiCall('get_runtime_providers', decodeRuntimeProvidersOutcome),
  get_runtime_profiles_snapshot: () =>
    validatedApiCall('get_runtime_profiles_snapshot', decodeRuntimeProfilesSnapshotResponse),
  list_runtime_profile_updates_since: (cursor?: string | null, limit?: number) =>
    apiCall('list_runtime_profile_updates_since', { cursor, limit }),
  upsert_runtime_profile: (profile: Record<string, unknown>) =>
    apiCall('upsert_runtime_profile', { profile }),
  delete_runtime_profile: (profileId: string) =>
    apiCall('delete_runtime_profile', { profile_id: profileId }),
  set_model_runtime_route: (route: Record<string, unknown>) =>
    apiCall('set_model_runtime_route', { route }),
  clear_model_runtime_route: (provider: string, modelId: string) =>
    apiCall('clear_model_runtime_route', { provider, model_id: modelId }),
  launch_runtime_profile: (profileId: string, tag?: string | null, modelId?: string | null) =>
    apiCall('launch_runtime_profile', { profile_id: profileId, tag, model_id: modelId }),
  stop_runtime_profile: (profileId: string) =>
    apiCall('stop_runtime_profile', { profile_id: profileId }),
  get_serving_status: () => validatedApiCall('get_serving_status', decodeServingStatusResponse),
  list_serving_status_updates_since: (cursor?: string | null) =>
    apiCall('list_serving_status_updates_since', { cursor }),
  validate_model_serving_config: (request: Record<string, unknown>) =>
    apiCall('validate_model_serving_config', { request }),
  serve_model: (request: Record<string, unknown>) =>
    apiCall('serve_model', { request }),
  unserve_model: (request: Record<string, unknown>) =>
    apiCall('unserve_model', { request }),

  // Torch Inference Server
  launch_torch: () => validatedApiCall(
    'launch_torch', decodeRuntimeLaunchOutcome,
    requireDecoded(decodeRuntimeLaunchParams({}), 'launch_torch request')
  ),
  stop_torch: () => validatedApiCall(
    'stop_torch', decodeRuntimeStopOutcome,
    requireDecoded(decodeRuntimeLaunchParams({}), 'stop_torch request')
  ),
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
  // Rust reads `{ connection_url?, config }`; main validates the full shape.
  torch_configure: (config: Record<string, unknown>, connectionUrl?: string) =>
    apiCall('torch_configure', withoutUndefined({ config, connection_url: connectionUrl })),

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
  get_models: () => validatedApiCall('get_models', decodeModelsOutcome),
  refresh_model_index: () => validatedApiCall('refresh_model_index', decodeModelIndexRefreshOutcome),
  scan_shared_storage: () => apiCall('scan_shared_storage'),
  search_hf_models: (
    query: string,
    kind?: string | null,
    limit?: number,
    hydrateLimit?: number
  ) =>
    validatedApiCall('search_hf_models', decodeHfModelSearchOutcome,
      withoutUndefined({ query, kind, limit, hydrate_limit: hydrateLimit })),
  get_hf_download_details: async (repoId: string, quants?: string[] | null) => {
    const params = requireDecoded(decodeGetHfDownloadDetailsParams({
      repo_id: repoId, ...(quants === undefined ? {} : { quants }),
    }), 'get_hf_download_details');
    return validatedApiCall('get_hf_download_details', decodeHfDownloadDetailsOutcome, params);
  },
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
    validatedApiCall('start_model_download_from_hf', decodeDownloadStartedOutcome, {
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
    validatedApiCall('download_model_from_hf', decodeDownloadStartedOutcome, {
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
    validatedApiCall('get_model_download_status', decodeDownloadStatusOutcome, { download_id: downloadId }),
  cancel_model_download: (downloadId: string) =>
    validatedApiCall('cancel_model_download', decodeDownloadMutationOutcome, { download_id: downloadId }),
  pause_model_download: (downloadId: string) =>
    validatedApiCall('pause_model_download', decodeDownloadMutationOutcome, { download_id: downloadId }),
  resume_model_download: (downloadId: string) =>
    validatedApiCall('resume_model_download', decodeDownloadMutationOutcome, { download_id: downloadId }),
  inspect_retained_download: (downloadId:string) =>
    validatedApiCall('inspect_retained_download',decodeDownloadLocationReport,{download_id:downloadId}),
  resolve_retained_download: (request:DownloadResolutionRequest) =>
    validatedApiCall('resolve_retained_download',decodeDownloadLocationReport,
      requireDecoded(decodeDownloadResolutionRequest(request),'resolve_retained_download request')),
  list_model_downloads: () =>
    validatedApiCall('list_model_downloads', decodeDownloadListOutcome),
  resume_partial_download: (modelId: string, recoveryToken: string) =>
    validatedApiCall('resume_partial_download', decodePartialDownloadOutcome,
      requireDecoded(decodeRecoverDownloadParams({ modelId, recoveryToken }), 'resume_partial_download request')),
  get_library_model_metadata: (modelId: string) =>
    validatedApiCall('get_library_model_metadata', decodeLibraryModelMetadataOutcome, { model_id: modelId }),
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
    validatedApiCall('get_inference_settings', decodeInferenceSettingsOutcome, { model_id: modelId }),
  update_inference_settings: async (modelId: string, inferenceSettings: Record<string, unknown>[]) => {
    const params = requireDecoded(decodeUpdateInferenceSettingsParams({
      model_id: modelId, settings: inferenceSettings,
    }), 'update_inference_settings');
    const result = await validatedApiCall('update_inference_settings', decodeUpdateInferenceSettingsOutcome, params);
    if (result.model_id !== modelId) {
      throw new DesktopContractError('update_inference_settings', { status: 'invalid', message: 'Response model identity does not match the request.' });
    }
    return result;
  },
  update_model_notes: async (modelId: string, notes?: string | null) => {
    const params = requireDecoded(decodeUpdateModelNotesParams({
      model_id: modelId, ...(notes === undefined ? {} : { notes }),
    }), 'update_model_notes');
    const result = await validatedApiCall('update_model_notes', decodeUpdateModelNotesOutcome, params);
    if (result.model_id !== modelId) {
      throw new DesktopContractError('update_model_notes', { status: 'invalid', message: 'Response model identity does not match the request.' });
    }
    return result;
  },
  /** Choose a model's category (`null` clears the choice and re-detects it). */
  set_model_category: async (modelId: string, category: string | null) => {
    const result = await validatedApiCall('set_model_category', decodeModelCategoryOutcome, {
      model_id: modelId, category,
    });
    if (result.model_id !== modelId) {
      throw new DesktopContractError('set_model_category', { status: 'invalid', message: 'Response model identity does not match the request.' });
    }
    return result;
  },

  // HuggingFace Authentication
  set_hf_token: (token: string) =>
    apiCall('set_hf_token', requireDecoded(decodeSetHfTokenParams({ token }), 'set_hf_token request')),
  clear_hf_token: () => apiCall('clear_hf_token'),
  get_hf_auth_status: () => apiCall('get_hf_auth_status'),

  // ========================================
  // Model Library Import (Phase 1A)
  // ========================================
  search_models_fts: (
    query: string,
    limit?: number,
    offset?: number
  ) =>
    validatedApiCall('search_models_fts', decodeCatalogSearchOutcome, {
      query,
      limit,
      offset,
    }),
  import_batch: (importSpecs: Array<Record<string, unknown>>) =>
    apiCall('import_batch', requireDecoded(decodeImportBatchParams({
      imports: Array.isArray(importSpecs)
        ? importSpecs.map((spec) => (spec && typeof spec === 'object' ? withoutUndefined(spec) : spec))
        : importSpecs,
    }), 'import_batch request')),
  import_external_diffusers_directory: (spec: Record<string, unknown>) =>
    apiCall('import_external_diffusers_directory', spec),
  classify_model_import_paths: (paths: string[]) =>
    apiCall('classify_model_import_paths', { paths }),
  get_network_status: () => validatedApiCall('get_network_status', decodeNetworkStatusResponse),

  // ========================================
  // Model Import (Phase 2)
  // ========================================
  get_s3_transfer_retry: (operationId?: string) => validatedApiCall('get_s3_transfer_retry', decodeS3TransferRetryState, requireDecoded(decodeS3ImportStatusParams({operation_id:operationId ?? null}), 'get_s3_transfer_retry')),
  retry_s3_model_transfer: (request: S3TransferRetryParams) => validatedApiCall('retry_s3_model_transfer', decodeS3ImportOutcome, requireDecoded(decodeS3TransferRetryParams(request), 'retry_s3_model_transfer')),
  inspect_persisted_s3_imports: () => validatedApiCall('inspect_persisted_s3_imports', decodeS3PersistedImportsWire, {}),
  start_s3_prefix_discovery: (request: S3DiscoveryParams) =>
    validatedApiCall('start_s3_prefix_discovery', decodeS3DiscoveryOutcome, requireDecoded(decodeS3DiscoveryParams(request), 'start_s3_prefix_discovery')),
  start_authenticated_s3_prefix_discovery: (request: S3AuthenticatedDiscoveryParams) =>
    validatedApiCall('start_authenticated_s3_prefix_discovery', decodeS3DiscoveryOutcome, requireDecoded(decodeS3AuthenticatedDiscoveryParams(request), 'start_authenticated_s3_prefix_discovery')),
  get_s3_prefix_discovery: (operationId?: string) =>
    validatedApiCall('get_s3_prefix_discovery', decodeS3DiscoveryOutcome, requireDecoded(decodeS3ImportStatusParams({operation_id: operationId ?? null}), 'get_s3_prefix_discovery')),
  cancel_s3_prefix_discovery: (operationId: string) =>
    validatedApiCall('cancel_s3_prefix_discovery', decodeS3DiscoveryOutcome, requireDecoded(decodeS3ImportCancelParams({operation_id: operationId}), 'cancel_s3_prefix_discovery')),
  start_s3_model_bundle_import: (request: S3BundleImportParams) =>
    validatedApiCall('start_s3_model_bundle_import', decodeS3ImportOutcome,
      requireDecoded(decodeS3BundleImportParams(request), 'start_s3_model_bundle_import')),
  start_authenticated_s3_model_bundle_import: (request: S3AuthenticatedBundleImportParams) =>
    validatedApiCall('start_authenticated_s3_model_bundle_import', decodeS3ImportOutcome,
      requireDecoded(decodeS3AuthenticatedBundleImportParams(request), 'start_authenticated_s3_model_bundle_import')),
  get_s3_model_bundle_import: (operationId?: string) =>
    validatedApiCall('get_s3_model_bundle_import', decodeS3BundleImportObservation,
      requireDecoded(decodeS3ImportStatusParams({operation_id: operationId ?? null}), 'get_s3_model_bundle_import')),
  start_s3_model_import: (request: S3ImportParams) =>
    validatedApiCall('start_s3_model_import', decodeS3ImportOutcome,
      requireDecoded(decodeS3ImportParams(request), 'start_s3_model_import')),
  start_authenticated_s3_model_import: (request: S3AuthenticatedImportParams) =>
    validatedApiCall('start_authenticated_s3_model_import', decodeS3ImportOutcome,
      requireDecoded(decodeS3AuthenticatedImportParams(request), 'start_authenticated_s3_model_import')),
  get_s3_model_import: (operationId?: string) =>
    validatedApiCall('get_s3_model_import', decodeS3ImportOutcome,
      requireDecoded(decodeS3ImportStatusParams({ operation_id: operationId ?? null }), 'get_s3_model_import')),
  cancel_s3_model_import: (operationId: string) =>
    validatedApiCall('cancel_s3_model_import', decodeS3ImportCancelOutcome,
      requireDecoded(decodeS3ImportCancelParams({ operation_id: operationId }), 'cancel_s3_model_import')),
  import_model: (localPath: string, family: string, officialName: string, repoId?: string) =>
    apiCall('import_model', requireDecoded(decodeImportModelParams(withoutUndefined({
      local_path: localPath,
      family,
      official_name: officialName,
      repo_id: repoId,
    })), 'import_model request')),
  lookup_hf_metadata_for_file: (filename: string, filePath?: string | null) =>
    apiCall('lookup_hf_metadata_for_file', { filename, file_path: filePath }),
  lookup_hf_metadata_for_bundle_directory: (directoryPath: string) =>
    apiCall('lookup_hf_metadata_for_bundle_directory', { directory_path: directoryPath }),
  detect_sharded_sets: (filePaths: string[]) =>
    apiCall('detect_sharded_sets', { files: filePaths }),
  validate_file_type: (filePath: string) => apiCall('validate_file_type', { file_path: filePath }),
  get_library_status: () => validatedApiCall('get_library_status', decodeLibraryStatusResponse),
  get_file_link_count: (filePath: string) => apiCall('get_file_link_count', { file_path: filePath }),
  check_files_writable: (filePaths: string[]) =>
    apiCall('check_files_writable', { file_paths: filePaths }),
  get_embedded_metadata: (filePath: string) =>
    apiCall('get_embedded_metadata', { file_path: filePath }),

  // ========================================
  // Link Health (Phase 1B)
  // ========================================
  get_link_health: (versionTag?: string | null) =>
    validatedApiCall('get_link_health', decodeLinkHealthOutcome, { version_tag: versionTag }),
  clean_broken_links: () => apiCall('clean_broken_links'),
  remove_orphaned_links: (versionTag: string) =>
    apiCall('remove_orphaned_links', { version_tag: versionTag }),
  get_links_for_model: (modelId: string) => apiCall('get_links_for_model',
    requireDecoded(decodeModelIdParams({ model_id: modelId }), 'get_links_for_model request')),
  delete_model_with_cascade: (modelId: string) =>
    apiCall('delete_model_with_cascade',
      requireDecoded(decodeModelIdParams({ model_id: modelId }), 'delete_model_with_cascade request')),

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
  // Model Format Conversion
  // ========================================
  start_model_conversion: (
    modelId: string,
    direction: ConversionDirection,
    targetQuant?: string | null,
    outputName?: string | null,
    imatrixCalibrationFile?: string | null,
    forceImatrix?: boolean | null
  ) =>
    validatedApiCall('start_model_conversion', decodeConversionStartedOutcome, {
      model_id: modelId,
      direction,
      target_quant: targetQuant,
      output_name: outputName,
      imatrix_calibration_file: imatrixCalibrationFile,
      force_imatrix: forceImatrix,
    }),
  get_conversion_progress: (conversionId: string) =>
    validatedApiCall('get_conversion_progress', decodeConversionProgressResponse, { conversion_id: conversionId }),
  cancel_model_conversion: (conversionId: string) =>
    validatedApiCall('cancel_model_conversion', decodeConversionCancelledOutcome, { conversion_id: conversionId }),
  list_model_conversions: () => validatedApiCall('list_model_conversions', decodeConversionListOutcome),
  check_conversion_environment: () => validatedApiCall('check_conversion_environment', decodeConversionEnvironmentOutcome),
  setup_conversion_environment: () => validatedApiCall('setup_conversion_environment', decodeSuccessOutcome),
  start_conversion_setup: (expectedPreviousOperationId?: string | null) => validatedApiCall('start_conversion_setup', decodeConversionSetupStartedOutcome, { expected_previous_operation_id: expectedPreviousOperationId ?? null }),
  get_conversion_setup: () => validatedApiCall('get_conversion_setup', decodeConversionSetupStatusOutcome),
  start_backend_setup: (backend: QuantBackend, expectedPreviousOperationId?: string | null) =>
    validatedApiCall('start_backend_setup', decodeConversionSetupStartedOutcome,
      requireDecoded(decodeStartBackendSetupParams({ backend, expected_previous_operation_id: expectedPreviousOperationId ?? null }), 'start_backend_setup request')),
  get_backend_setup: (backend: QuantBackend) =>
    validatedApiCall('get_backend_setup', decodeConversionSetupStatusOutcome,
      requireDecoded(decodeGetBackendSetupParams({ backend }), 'get_backend_setup request')),
  get_supported_quant_types: () => validatedApiCall('get_supported_quant_types', decodeSupportedQuantTypesOutcome),
  get_backend_status: () => validatedApiCall('get_backend_status', decodeBackendStatusOutcome),
  setup_quantization_backend: (backend: QuantBackend) =>
    validatedApiCall('setup_quantization_backend', decodeSuccessOutcome, { backend }),

  // ========================================
  // Size Calculation
  // ========================================
  calculate_release_size: (tag: string, forceRefresh?: boolean, appId?: string) =>
    apiCall('calculate_release_size', { tag, force_refresh: forceRefresh, app_id: appId }),
  calculate_all_release_sizes: () => apiCall('calculate_all_release_sizes'),
  get_release_size_info: (tag: string, archiveSize: number) =>
    apiCall('get_release_size_info', { tag, archive_size: archiveSize }),
  get_release_size_breakdown: (tag: string) => apiCall('get_release_size_breakdown', { tag }),
  get_release_dependencies: (tag: string, appId: string) => {
    const params = requireDecoded(
      decodeGetReleaseDependenciesParams({ tag, app_id: appId }),
      'get_release_dependencies request'
    );
    return validatedApiCall(
      'get_release_dependencies',
      decodeGetReleaseDependenciesOutcome,
      params
    );
  },

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

  get_launcher_root_state: async () => {
    return decodeLauncherRootStartupState(
      await ipcRenderer.invoke('launcher:getRootState')
    );
  },

  get_launcher_root_bootstrap: () => {
    return decodeLauncherRootStartupState(
      ipcRenderer.sendSync('launcher:getRootBootstrap')
    );
  },

  notify_launcher_root_presentation_committed: async (
    presentation: LauncherRootCommittedPresentation
  ) => {
    await ipcRenderer.invoke('launcher-root:presentation-committed', presentation);
  },

  onLauncherRootPresentationTimeout: (callback: () => void): (() => void) => {
    launcherRootPresentationTimeoutListeners.add(callback);
    if (launcherRootPresentationTimeoutLatched) {
      callback();
    }
    return () => {
      launcherRootPresentationTimeoutListeners.delete(callback);
    };
  },

  upgrade_launcher_library: async (confirmation: LibraryUpgradeConfirmation) => {
    const confirmed = decodeOrThrow(
      decodeLibraryUpgradeConfirmation(confirmation),
      'Library upgrade requires both confirmations.'
    );
    return decodeLibraryUpgradeResult(await ipcRenderer.invoke('launcher:upgradeLibrary', confirmed));
  },

  select_launcher_root: async () => {
    return decodeLauncherRootSelectionResult(
      await ipcRenderer.invoke('launcher:chooseLibraryRoot')
    );
  },

  open_active_install: (appId?: string) => apiCall('open_active_install', { app_id: appId }),

  close_window: async () => {
    await ipcRenderer.invoke('window:close');
    return { success: true };
  },

  // Model Import Dialog - uses Electron's native dialog
  open_model_import_dialog: async (): Promise<ModelImportSelection> => {
    try {
      const result: unknown = await ipcRenderer.invoke('dialog:openFile', {
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
      return decodeModelImportSelection(result);
    } catch {
      return { status: 'unavailable' };
    }
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
    callback: (notification: ModelLibraryUpdateNotification) => void,
    onError: (message: string) => void
  ): (() => void) => subscribeUpdateStream('model-library', decodeModelLibraryUpdateNotification,
    { callback, onError }, { refcounted: false }),
  onModelDownloadUpdate: (
    callback: (notification: ModelDownloadUpdateOutcome) => void,
    onError: (message: string) => void
  ): (() => void) => subscribeUpdateStream('model-download', decodeModelDownloadUpdateOutcome,
    { callback, onError }, { refcounted: true }),
  onRuntimeProfileUpdate: (
    callback: (notification: RuntimeProfileUpdateFeed) => void,
    onError: (message: string) => void
  ): (() => void) => subscribeUpdateStream('runtime-profile', decodeRuntimeProfileUpdateFeed,
    { callback, onError }, { refcounted: true }),
  onServingStatusUpdate: (
    callback: (notification: ServingStatusUpdateFeed) => void,
    onError: (message: string) => void
  ): (() => void) => subscribeUpdateStream('serving-status', decodeServingStatusUpdateFeed,
    { callback, onError }, { refcounted: true }),
  onStatusTelemetryUpdate: (
    callback: (notification: StatusTelemetryUpdateNotification) => void,
    onError: (message: string) => void
  ): (() => void) => subscribeUpdateStream('status-telemetry', decodeStatusTelemetryUpdateNotification,
    { callback, onError }, { refcounted: true }),

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
