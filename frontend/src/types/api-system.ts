import type { BaseResponse } from './api-common';

// ============================================================================
// System & Status Types
// ============================================================================

export interface DiskSpaceResponse extends BaseResponse {
  total: number;
  used: number;
  free: number;
  percent: number;
}

export interface StatusResponse extends BaseResponse {
  version: string;
  deps_ready: boolean;
  patched: boolean;
  menu_shortcut: boolean;
  desktop_shortcut: boolean;
  shortcut_version: string | null;
  message: string;
  comfyui_running: boolean;
  ollama_running: boolean;
  torch_running: boolean;
  last_launch_error: string | null;
  last_launch_log: string | null;
  app_resources?: {
    comfyui?: {
      gpu_memory?: number;
      ram_memory?: number;
    };
    ollama?: {
      gpu_memory?: number;
      ram_memory?: number;
    };
  };
}

export interface SystemResourcesResponse extends BaseResponse {
  resources: {
    cpu: {
      usage: number;
      temp?: number;
    };
    gpu: {
      usage: number;
      memory: number;
      memory_total: number;
      temp?: number;
    };
    ram: {
      usage: number;
      total: number;
    };
    disk: {
      usage: number;
      total: number;
      free: number;
    };
  };
}
