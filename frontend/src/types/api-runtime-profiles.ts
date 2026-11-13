import type {
  RuntimeProfileConfig as DesktopRuntimeProfileConfig,
  RuntimeProfileStatus as DesktopRuntimeProfileStatus,
  RuntimeProfileEvent as DesktopRuntimeProfileEvent,
  RuntimeProfileEventKind as DesktopRuntimeProfileEventKind,
  RuntimeProfileUpdateFeed as DesktopRuntimeProfileUpdateFeed,
  RuntimeProviderDescriptor as DesktopRuntimeProviderDescriptor,
  RuntimeProvidersOutcome as DesktopRuntimeProvidersOutcome,
} from '@pumas/desktop-contract';
import type { BaseResponse } from './api-common';

export type RuntimeProviderId = 'ollama' | 'llama_cpp' | 'onnx_runtime' | 'torch';

export type RuntimeProviderMode =
  | 'ollama_serve'
  | 'llama_cpp_router'
  | 'llama_cpp_dedicated'
  | 'onnx_serve'
  | 'torch_serve';

export type RuntimeManagementMode = 'managed' | 'external';

export type RuntimeDeviceMode = 'auto' | 'cpu' | 'gpu' | 'hybrid' | 'specific_device';

export type LlamaCppProfileMode = 'router' | 'dedicated';

// Declared once by the backend provider registry (`get_runtime_providers`) and
// generated: each provider's runtime app id, whether that app is version
// managed, and the modes its profiles may use.
export type RuntimeProviderInfo = DesktopRuntimeProviderDescriptor;
export type RuntimeProvidersResponse = DesktopRuntimeProvidersOutcome;

export type RuntimeLifecycleState =
  | 'unknown'
  | 'stopped'
  | 'starting'
  | 'running'
  | 'stopping'
  | 'failed'
  | 'external';

export interface RuntimeDeviceSettings {
  mode: RuntimeDeviceMode;
  device_id?: string | null;
  gpu_layers?: number | null;
  tensor_split?: number[] | null;
}

export interface RuntimeSchedulerSettings {
  auto_load: boolean;
  max_concurrent_models?: number | null;
  keep_alive_seconds?: number | null;
}

// Declared once by the backend (`RuntimeProfileConfig`) and generated. A managed
// profile's `runtime_version` pins the installed runtime version it launches;
// absent or null follows the runtime's active version.
export type RuntimeProfileConfig = DesktopRuntimeProfileConfig;

export interface ModelRuntimeRoute {
  provider: RuntimeProviderId;
  model_id: string;
  profile_id?: string | null;
  auto_load: boolean;
}

// Declared once by the backend (`RuntimeProfileStatus`) and generated.
// `runtime_version` is the installed version a running instance launched from.
export type RuntimeProfileStatus = DesktopRuntimeProfileStatus;

export interface RuntimeProfilesSnapshot {
  schema_version: number;
  cursor: string;
  profiles: RuntimeProfileConfig[];
  routes: ModelRuntimeRoute[];
  statuses: RuntimeProfileStatus[];
  default_profile_id?: string | null;
}

// Declared once by the backend (`/events/runtime-profile-updates`) and generated.
export type RuntimeProfileEventKind = DesktopRuntimeProfileEventKind;
export type RuntimeProfileEvent = DesktopRuntimeProfileEvent;
export type RuntimeProfileUpdateFeed = DesktopRuntimeProfileUpdateFeed;

export interface RuntimeProfilesSnapshotResponse extends BaseResponse {
  snapshot: RuntimeProfilesSnapshot;
}

export interface RuntimeProfileUpdateFeedResponse extends BaseResponse {
  feed: RuntimeProfileUpdateFeed;
}

export interface RuntimeProfileMutationResponse extends BaseResponse {
  profile_id?: string | null;
  snapshot_required: boolean;
}
