import { runtimeProviderManagementModes } from '../../../hooks/useRuntimeProviders';
import type {
  RuntimeProfileConfig,
  RuntimeProviderInfo,
} from '../../../types/api-runtime-profiles';
import { getRuntimeProviderDescriptor } from '../../../utils/runtimeProviderDescriptors';
import {
  providerLabel,
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
    runtime_version: profile.runtime_version ?? '',
  };
}

/** A new profile's defaults: the first mode of each kind the backend offers. */
export function newProfileDraft(providerInfo: RuntimeProviderInfo): RuntimeProfileDraft {
  const { provider } = providerInfo;

  return {
    profile_id: `runtime-${Date.now()}`,
    provider,
    provider_mode: providerInfo.provider_modes[0] ?? 'ollama_serve',
    management_mode: runtimeProviderManagementModes(providerInfo)[0] ?? 'managed',
    name: `New ${providerLabel(provider)} Profile`,
    enabled: true,
    endpoint_url: '',
    port: '',
    device_mode: providerInfo.device_modes[0] ?? 'auto',
    device_id: '',
    gpu_layers: '',
    runtime_version: '',
  };
}

export function draftToProfile(draft: RuntimeProfileDraft): RuntimeProfileConfig {
  const descriptor = getRuntimeProviderDescriptor(draft.provider);
  const keepsDeviceId = draft.device_mode === 'gpu' || draft.device_mode === 'specific_device';
  const keepsGpuLayers =
    descriptor.supportsGpuLayers &&
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
    runtime_version: draftRuntimeVersion(draft),
  };
}

// A pin applies only to profiles Pumas launches; external profiles and an
// empty selection follow the runtime's active version.
function draftRuntimeVersion(draft: RuntimeProfileDraft): string | null {
  if (draft.management_mode !== 'managed') {
    return null;
  }
  return draft.runtime_version.trim() || null;
}

function parseLlamaCppGpuLayers(draft: RuntimeProfileDraft): number | null {
  if (draft.gpu_layers.trim()) {
    return Number.parseInt(draft.gpu_layers, 10);
  }
  return draft.device_mode === 'gpu' || draft.device_mode === 'specific_device' ? -1 : null;
}
