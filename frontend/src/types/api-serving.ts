import type { BaseResponse } from './api-common';
import type { RuntimeDeviceMode, RuntimeProviderId } from './api-runtime-profiles';

export type ServingEndpointMode = 'not_configured' | 'provider_endpoint' | 'pumas_gateway';

export type ServedModelLoadState =
  | 'requested'
  | 'loading'
  | 'loaded'
  | 'unloading'
  | 'unloaded'
  | 'failed';

export type ModelServeErrorSeverity = 'non_critical' | 'critical';

export type ModelServeErrorCode =
  | 'invalid_request'
  | 'model_not_found'
  | 'model_not_executable'
  | 'profile_not_found'
  | 'profile_stopped'
  | 'unsupported_provider'
  | 'unsupported_placement'
  | 'device_unavailable'
  | 'insufficient_memory'
  | 'provider_load_failed'
  | 'missing_runtime'
  | 'invalid_format'
  | 'endpoint_unavailable'
  | 'unknown';

export interface ModelServeError {
  code: ModelServeErrorCode;
  severity: ModelServeErrorSeverity;
  message: string;
  model_id?: string | null;
  profile_id?: string | null;
  provider?: RuntimeProviderId | null;
}

export interface ModelServingConfig {
  provider: RuntimeProviderId;
  profile_id: string;
  device_mode: RuntimeDeviceMode;
  device_id?: string | null;
  gpu_layers?: number | null;
  tensor_split?: number[] | null;
  context_size?: number | null;
  keep_loaded: boolean;
  model_alias?: string | null;
}

export interface ServeModelRequest {
  model_id: string;
  config: ModelServingConfig;
}

export interface UnserveModelRequest {
  model_id: string;
  profile_id?: string | null;
  model_alias?: string | null;
}

export interface ModelServeValidationResponse extends BaseResponse {
  valid: boolean;
  errors: ModelServeError[];
  warnings: ModelServeError[];
}

export interface ServingEndpointStatus {
  endpoint_mode: ServingEndpointMode;
  endpoint_url?: string | null;
  model_count: number;
  message?: string | null;
}

export interface ServedModelStatus {
  model_id: string;
  model_alias?: string | null;
  provider: RuntimeProviderId;
  profile_id: string;
  load_state: ServedModelLoadState;
  device_mode: RuntimeDeviceMode;
  device_id?: string | null;
  gpu_layers?: number | null;
  tensor_split?: number[] | null;
  context_size?: number | null;
  keep_loaded: boolean;
  endpoint_url?: string | null;
  memory_bytes?: number | null;
  loaded_at?: string | null;
  last_error?: ModelServeError | null;
}

export interface ServingStatusSnapshot {
  schema_version: number;
  cursor: string;
  endpoint: ServingEndpointStatus;
  served_models: ServedModelStatus[];
  last_errors: ModelServeError[];
}

export interface ServingStatusResponse extends BaseResponse {
  snapshot: ServingStatusSnapshot;
}

export interface ServeModelResponse extends BaseResponse {
  loaded: boolean;
  loaded_models_unchanged: boolean;
  status?: ServedModelStatus | null;
  load_error?: ModelServeError | null;
  snapshot?: ServingStatusSnapshot | null;
}

export interface UnserveModelResponse extends BaseResponse {
  unloaded: boolean;
  snapshot?: ServingStatusSnapshot | null;
}
