import type {
  RuntimeProfileConfig,
  RuntimeProviderId,
} from '../../../types/api-runtime-profiles';
import {
  providerLabel,
  providerModes,
  type RuntimeProfileDraft,
} from './RuntimeProfileSettingsShared';

export function profileToDraft(profile: RuntimeProfileConfig): RuntimeProfileDraft {
  return {
    profile_id: profile.profile_id,
    provider: profile.provider,
    provider_mode: profile.provider_mode,
    management_mode: profile.management_mode,
    name: profile.name,
    enabled: profile.enabled,
    endpoint_url: profile.endpoint_url ?? '',
    port: profile.port?.toString() ?? '',
    device_mode: profile.device.mode,
    device_id: profile.device.device_id ?? '',
    gpu_layers: profile.device.gpu_layers?.toString() ?? '',
  };
}

export function newProfileDraft(provider: RuntimeProviderId): RuntimeProfileDraft {
  return {
    profile_id: `runtime-${Date.now()}`,
    provider,
    provider_mode: providerModes[provider][0] ?? 'ollama_serve',
    management_mode: 'managed',
    name: `New ${providerLabel(provider)} Profile`,
    enabled: true,
    endpoint_url: '',
    port: '',
    device_mode: 'auto',
    device_id: '',
    gpu_layers: '',
  };
}

export function draftToProfile(draft: RuntimeProfileDraft): RuntimeProfileConfig {
  const keepsDeviceId = draft.device_mode === 'gpu' || draft.device_mode === 'specific_device';
  const keepsGpuLayers =
    draft.provider === 'llama_cpp' &&
    (draft.device_mode === 'gpu' ||
      draft.device_mode === 'hybrid' ||
      draft.device_mode === 'specific_device');

  return {
    profile_id: draft.profile_id.trim(),
    provider: draft.provider,
    provider_mode: draft.provider_mode,
    management_mode: draft.management_mode,
    name: draft.name.trim(),
    enabled: draft.enabled,
    endpoint_url: draft.endpoint_url.trim() || null,
    port: draft.port.trim() ? Number.parseInt(draft.port, 10) : null,
    device: {
      mode: draft.device_mode,
      device_id: keepsDeviceId ? draft.device_id.trim() || null : null,
      gpu_layers: keepsGpuLayers ? parseLlamaCppGpuLayers(draft) : null,
      tensor_split: null,
    },
    scheduler: {
      auto_load: true,
      max_concurrent_models: null,
      keep_alive_seconds: null,
    },
  };
}

function parseLlamaCppGpuLayers(draft: RuntimeProfileDraft): number | null {
  if (draft.gpu_layers.trim()) {
    return Number.parseInt(draft.gpu_layers, 10);
  }
  return draft.device_mode === 'gpu' || draft.device_mode === 'specific_device' ? -1 : null;
}
