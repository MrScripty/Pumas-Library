import type { BaseResponse } from './api-common';

export type RuntimeProviderId = 'ollama' | 'llama_cpp';

export type RuntimeProviderMode =
  | 'ollama_serve'
  | 'llama_cpp_router'
  | 'llama_cpp_dedicated';

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

export type RuntimeProfileEventKind =
  | 'profile_created'
  | 'profile_updated'
  | 'profile_deleted'
  | 'route_updated'
  | 'route_deleted'
  | 'status_changed'
  | 'snapshot_required';

export interface RuntimeProfileEvent {
  cursor: string;
  event_kind: RuntimeProfileEventKind;
  profile_id?: string | null;
  provider?: RuntimeProviderId | null;
  model_id?: string | null;
  producer_revision?: string | null;
}

export interface RuntimeProfileUpdateFeed {
  cursor: string;
  events: RuntimeProfileEvent[];
  stale_cursor: boolean;
  snapshot_required: boolean;
}

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

export function isRuntimeProfileUpdateFeed(value: unknown): value is RuntimeProfileUpdateFeed {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const candidate = value as Partial<RuntimeProfileUpdateFeed>;
  return (
    typeof candidate.cursor === 'string' &&
    Array.isArray(candidate.events) &&
    typeof candidate.stale_cursor === 'boolean' &&
    typeof candidate.snapshot_required === 'boolean'
  );
}
