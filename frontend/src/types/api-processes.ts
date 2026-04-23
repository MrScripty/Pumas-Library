import type { BaseResponse } from './api-common';

// ============================================================================

export interface LaunchResponse extends BaseResponse {
  log_path?: string;
  ready?: boolean;
}

export interface StopComfyUIResponse extends BaseResponse {
  // Empty body on success
}

export interface StopOllamaResponse extends BaseResponse {
  // Empty body on success
}

export interface OllamaModelInfo {
  name: string;
  size: number;
  digest: string;
  modified_at: string;
}

export interface OllamaListModelsResponse extends BaseResponse {
  models: OllamaModelInfo[];
}

export interface OllamaCreateModelResponse extends BaseResponse {
  model_name?: string;
}

export interface OllamaRunningModel {
  name: string;
  size: number;
  digest: string;
  size_vram: number;
  expires_at: string;
}

export interface OllamaListRunningResponse extends BaseResponse {
  models: OllamaRunningModel[];
}

// ============================================================================
// Torch Inference Server Types
// ============================================================================

export type TorchComputeDevice = 'cpu' | 'cuda:0' | 'cuda:1' | 'cuda:2' | 'cuda:3' | 'mps' | 'auto' | string;

export type TorchSlotState = 'unloaded' | 'loading' | 'ready' | 'unloading' | 'error';

export interface TorchModelSlot {
  slot_id: string;
  model_name: string;
  model_path: string;
  device: TorchComputeDevice;
  state: TorchSlotState;
  gpu_memory_bytes?: number;
  ram_memory_bytes?: number;
  model_type?: string;
}

export interface TorchDeviceInfo {
  device_id: string;
  name: string;
  memory_total: number;
  memory_available: number;
  is_available: boolean;
}

export interface TorchServerConfig {
  api_port: number;
  host: string;
  max_loaded_models: number;
  lan_access: boolean;
}

export interface TorchListSlotsResponse extends BaseResponse {
  slots: TorchModelSlot[];
}

export interface TorchLoadModelResponse extends BaseResponse {
  slot?: TorchModelSlot;
}

export interface TorchUnloadModelResponse extends BaseResponse {}

export interface TorchGetStatusResponse extends BaseResponse {
  running: boolean;
  slots?: TorchModelSlot[];
  devices?: TorchDeviceInfo[];
  api_url?: string;
  config?: TorchServerConfig;
}

export interface TorchListDevicesResponse extends BaseResponse {
  devices: TorchDeviceInfo[];
}

export interface TorchConfigureResponse extends BaseResponse {}

export interface StopTorchResponse extends BaseResponse {}

// ============================================================================
// Shortcuts Types
// ============================================================================

export interface ShortcutState {
  menu: boolean;
  desktop: boolean;
  tag: string;
}

export interface GetVersionShortcutsResponse extends BaseResponse {
  state: ShortcutState;
}

export interface GetAllShortcutStatesResponse extends BaseResponse {
  states: {
    active: string | null;
    states: Record<string, ShortcutState>;
  };
}

export interface SetVersionShortcutsResponse extends BaseResponse {
  state: ShortcutState;
}

export interface ToggleShortcutResponse extends BaseResponse {
  state: ShortcutState;
}

// ============================================================================
// Launcher Update Types
// ============================================================================

export interface LauncherVersionResponse extends BaseResponse {
  version: string;
  branch: string;
  isGitRepo: boolean;
}

export interface CheckLauncherUpdatesResponse extends BaseResponse {
  hasUpdate: boolean;
  currentCommit: string;
  latestCommit: string;
  commitsBehind: number;
  currentVersion?: string;
  latestVersion?: string;
  releaseName?: string;
  releaseUrl?: string;
  downloadUrl?: string;
  publishedAt?: string;
  commits: Array<{
    hash: string;
    message: string;
    author: string;
    date: string;
  }>;
}

export interface ApplyLauncherUpdateResponse extends BaseResponse {
  message: string;
  newCommit?: string;
}

export interface RestartLauncherResponse extends BaseResponse {
  message: string;
}
