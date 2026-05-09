import type {
  RuntimeDeviceMode,
  RuntimeManagementMode,
  RuntimeProviderId,
  RuntimeProviderMode,
} from '../../../types/api-runtime-profiles';

export type RuntimeProfileDraft = {
  profile_id: string;
  provider: RuntimeProviderId;
  provider_mode: RuntimeProviderMode;
  management_mode: RuntimeManagementMode;
  name: string;
  enabled: boolean;
  endpoint_url: string;
  port: string;
  device_mode: RuntimeDeviceMode;
  device_id: string;
  gpu_layers: string;
};

export const providerModes: Record<RuntimeProviderId, RuntimeProviderMode[]> = {
  ollama: ['ollama_serve'],
  llama_cpp: ['llama_cpp_router', 'llama_cpp_dedicated'],
};

export const providerDeviceModes: Record<RuntimeProviderId, RuntimeDeviceMode[]> = {
  ollama: ['auto', 'cpu', 'gpu', 'hybrid'],
  llama_cpp: ['auto', 'cpu', 'gpu', 'specific_device'],
};

export function providerLabel(provider: RuntimeProviderId): string {
  return provider === 'llama_cpp' ? 'llama.cpp' : 'Ollama';
}

export function modeLabel(mode: RuntimeProviderMode): string {
  switch (mode) {
    case 'ollama_serve':
      return 'Serve';
    case 'llama_cpp_router':
      return 'Router';
    case 'llama_cpp_dedicated':
      return 'Dedicated';
  }
}

export function deviceModeLabel(mode: RuntimeDeviceMode): string {
  switch (mode) {
    case 'auto':
      return 'Auto';
    case 'cpu':
      return 'CPU';
    case 'gpu':
      return 'GPU';
    case 'hybrid':
      return 'Hybrid';
    case 'specific_device':
      return 'Specific device';
  }
}

export type RuntimeProfileDraftUpdater = <Key extends keyof RuntimeProfileDraft>(
  key: Key,
  value: RuntimeProfileDraft[Key]
) => void;
