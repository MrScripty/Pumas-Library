import type {
  RuntimeProfileEvent as DesktopRuntimeProfileEvent,
  RuntimeProfileEventKind as DesktopRuntimeProfileEventKind,
  RuntimeProfileUpdateFeed as DesktopRuntimeProfileUpdateFeed,
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

export interface RuntimeProfileConfig {
  profile_id: string;
  provider: RuntimeProviderId;
  provider_mode: RuntimeProviderMode;
  management_mode: RuntimeManagementMode;
  name: string;
  enabled: boolean;
  endpoint_url?: string | null;
  port?: number | null;
  device: RuntimeDeviceSettings;
  scheduler: RuntimeSchedulerSettings;
}

export interface ModelRuntimeRoute {
  provider: RuntimeProviderId;
  model_id: string;
  profile_id?: string | null;
  auto_load: boolean;
}

export interface RuntimeProfileStatus {
  profile_id: string;
  state: RuntimeLifecycleState;
  endpoint_url?: string | null;
  pid?: number | null;
  log_path?: string | null;
  last_error?: string | null;
}

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
