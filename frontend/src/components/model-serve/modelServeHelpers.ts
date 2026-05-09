import type {
  RuntimeDeviceMode,
  RuntimeProfileConfig,
  RuntimeProfileStatus,
} from '../../types/api-runtime-profiles';
import type { ModelInfo } from '../../types/apps';
import type { ModelServeError, ModelServingConfig } from '../../types/api-serving';

export const DEFAULT_LLAMA_CPP_CONTEXT_SIZE = '4096';

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
}

export function formatServeError(error: ModelServeError | null): string | null {
  if (!error) {
    return null;
  }
  return error.message || error.code.replace(/_/g, ' ');
}

export function isGgufModel(model: ModelInfo): boolean {
  return model.primaryFormat === 'gguf' || model.format?.toLowerCase() === 'gguf';
}

export function isDedicatedLlamaCppProfile(profile: RuntimeProfileConfig | undefined): boolean {
  return profile?.provider === 'llama_cpp' && profile.provider_mode === 'llama_cpp_dedicated';
}

export function isLlamaCppProfile(profile: RuntimeProfileConfig | undefined): boolean {
  return profile?.provider === 'llama_cpp';
}

export function isManagedLlamaCppProfile(profile: RuntimeProfileConfig | undefined): boolean {
  return isLlamaCppProfile(profile) && profile?.management_mode === 'managed';
}

export function getPlacementControls(
  profile: RuntimeProfileConfig | undefined,
  deviceMode: RuntimeDeviceMode
): ModelServeControls {
  const supportsModelPlacement = isDedicatedLlamaCppProfile(profile);
  const canUseGpuPlacement = supportsModelPlacement && deviceMode !== 'cpu';

  return {
    showDeviceControls: supportsModelPlacement,
    showDeviceId: supportsModelPlacement && deviceMode === 'specific_device',
    showGpuLayers: canUseGpuPlacement,
    showTensorSplit:
      canUseGpuPlacement &&
      (deviceMode === 'gpu' || deviceMode === 'hybrid' || deviceMode === 'specific_device'),
    showContextSize: supportsModelPlacement,
  };
}

export function getProfileStateBlockReason(
  profile: RuntimeProfileConfig | undefined,
  status: RuntimeProfileStatus | null
): string | null {
  if (!profile) {
    return null;
  }

  const canLaunchOnServe = isManagedLlamaCppProfile(profile);
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
  return isGgufModel(model) ? null : 'Only GGUF models can be served locally in this flow.';
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
    model_alias: null,
  };
}
