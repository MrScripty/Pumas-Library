import type { RuntimeProfileConfig } from '../../../types/api-runtime-profiles';
import type { ModelServeError, ModelServingConfig } from '../../../types/api-serving';
import {
  DEFAULT_LLAMA_CPP_CONTEXT_SIZE,
  getPlacementControls,
} from '../../model-serve/modelServeHelpers';
import type { LlamaCppModelRowViewModel } from './llamaCppLibraryViewModels';

export function formatQuickServeError(
  error: ModelServeError | null | undefined
): string | null {
  if (!error) {
    return null;
  }
  return error.message || error.code.replace(/_/g, ' ');
}

export function requiresAliasBeforeQuickServe(
  row: LlamaCppModelRowViewModel,
  profile: RuntimeProfileConfig
): boolean {
  return row.servedStatuses.some(
    (status) =>
      status.load_state === 'loaded' &&
      status.profile_id !== profile.profile_id
  );
}

export function buildQuickServeConfig(profile: RuntimeProfileConfig): ModelServingConfig {
  const deviceMode = profile.device.mode;
  const controls = getPlacementControls(profile, deviceMode);

  return {
    provider: 'llama_cpp',
    profile_id: profile.profile_id,
    device_mode: deviceMode,
    device_id:
      controls.showDeviceId && profile.device.device_id?.trim()
        ? profile.device.device_id.trim()
        : null,
    gpu_layers: controls.showGpuLayers ? profile.device.gpu_layers ?? null : null,
    tensor_split: controls.showTensorSplit ? profile.device.tensor_split ?? null : null,
    context_size: Number(DEFAULT_LLAMA_CPP_CONTEXT_SIZE),
    keep_loaded: true,
    model_alias: null,
  };
}
