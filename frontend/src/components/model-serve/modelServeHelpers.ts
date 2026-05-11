import type {
  RuntimeDeviceMode,
  RuntimeProfileConfig,
  RuntimeProfileStatus,
} from '../../types/api-runtime-profiles';
import type { ModelInfo } from '../../types/apps';
import type { ModelServeError, ModelServingConfig } from '../../types/api-serving';
import {
  getRuntimeProviderDescriptor,
  isModelCompatibleWithProvider,
} from '../../utils/runtimeProviderDescriptors';

export const DEVICE_OPTIONS: Array<{ value: RuntimeDeviceMode; label: string }> = [
  { value: 'auto', label: 'Auto' },
  { value: 'cpu', label: 'CPU' },
  { value: 'gpu', label: 'GPU' },
  { value: 'hybrid', label: 'Hybrid' },
  { value: 'specific_device', label: 'Device' },
];

export interface ModelServeControls {
  showDeviceControls: boolean;
  showDeviceId: boolean;
  showGpuLayers: boolean;
  showTensorSplit: boolean;
  showContextSize: boolean;
}

export interface ModelServeFormState {
  deviceMode: RuntimeDeviceMode;
  deviceId: string;
  gpuLayers: string;
  tensorSplit: string;
  contextSize: string;
  keepLoaded: boolean;
  modelAlias: string;
}

export function formatServeError(error: ModelServeError | null): string | null {
  if (!error) {
    return null;
  }
  return error.message || error.code.replace(/_/g, ' ');
}

export function profileUsesDedicatedPlacement(profile: RuntimeProfileConfig | undefined): boolean {
  if (!profile) {
    return false;
  }

  return getRuntimeProviderDescriptor(profile.provider).dedicatedPlacementModes.includes(
    profile.provider_mode
  );
}

export function profileCanLaunchOnServe(profile: RuntimeProfileConfig | undefined): boolean {
  return Boolean(
    profile &&
      getRuntimeProviderDescriptor(profile.provider).canLaunchOnServe &&
      profile.management_mode === 'managed'
  );
}

export function defaultContextSizeForProfile(profile: RuntimeProfileConfig | undefined): string {
  return profile ? (getRuntimeProviderDescriptor(profile.provider).defaultContextSize ?? '') : '';
}

export function getPlacementControls(
  profile: RuntimeProfileConfig | undefined,
  deviceMode: RuntimeDeviceMode
): ModelServeControls {
  const supportsModelPlacement = profileUsesDedicatedPlacement(profile);
  const canUseGpuPlacement = supportsModelPlacement && deviceMode !== 'cpu';
  const supportsContextSize = profile
    ? getRuntimeProviderDescriptor(profile.provider).supportsContextSize
    : false;

  return {
    showDeviceControls: supportsModelPlacement,
    showDeviceId: supportsModelPlacement && deviceMode === 'specific_device',
    showGpuLayers: canUseGpuPlacement,
    showTensorSplit:
      canUseGpuPlacement &&
      (deviceMode === 'gpu' || deviceMode === 'hybrid' || deviceMode === 'specific_device'),
    showContextSize: supportsContextSize,
  };
}

export function getProfileStateBlockReason(
  profile: RuntimeProfileConfig | undefined,
  status: RuntimeProfileStatus | null
): string | null {
  if (!profile) {
    return null;
  }

  const canLaunchOnServe = profileCanLaunchOnServe(profile);
  const isAvailable = status?.state === 'running' || status?.state === 'external';

  return canLaunchOnServe || isAvailable
    ? null
    : 'Start the selected runtime target before serving models with this mode.';
}

export function buildServeBlockReason({
  profileError,
  isLoading,
  servingProfileCount,
  selectedProfile,
  profileStateBlockReason,
  model,
}: {
  profileError: string | null;
  isLoading: boolean;
  servingProfileCount: number;
  selectedProfile: RuntimeProfileConfig | undefined;
  profileStateBlockReason: string | null;
  model: ModelInfo;
}): string | null {
  if (profileError) {
    return profileError;
  }
  if (isLoading) {
    return 'Loading runtime profiles.';
  }
  if (servingProfileCount === 0) {
    return 'Create a runtime profile before serving a model.';
  }
  if (!selectedProfile) {
    return 'Select a runtime target before serving.';
  }
  if (profileStateBlockReason) {
    return profileStateBlockReason;
  }
  if (isModelCompatibleWithProvider(model, selectedProfile.provider)) {
    return null;
  }

  const descriptor = getRuntimeProviderDescriptor(selectedProfile.provider);
  const formats = descriptor.compatibleExecutableFormats
    .map((format) => format.toUpperCase())
    .join('/');
  return `Only ${formats} models can be served with the selected provider.`;
}

export function buildModelServingConfig({
  selectedProfile,
  formState,
  controls,
}: {
  selectedProfile: RuntimeProfileConfig | undefined;
  formState: ModelServeFormState;
  controls: ModelServeControls;
}): ModelServingConfig | null {
  if (!selectedProfile) {
    return null;
  }

  return {
    provider: selectedProfile.provider,
    profile_id: selectedProfile.profile_id,
    device_mode: formState.deviceMode,
    device_id: controls.showDeviceId && formState.deviceId.trim() ? formState.deviceId.trim() : null,
    gpu_layers:
      controls.showGpuLayers && formState.gpuLayers.trim() ? Number(formState.gpuLayers) : null,
    tensor_split:
      controls.showTensorSplit && formState.tensorSplit.trim()
        ? formState.tensorSplit.split(',').map((value) => Number(value.trim()))
        : null,
    context_size:
      controls.showContextSize && formState.contextSize.trim()
        ? Number(formState.contextSize)
        : null,
    keep_loaded: formState.keepLoaded,
    model_alias: formState.modelAlias.trim() ? formState.modelAlias.trim() : null,
  };
}
