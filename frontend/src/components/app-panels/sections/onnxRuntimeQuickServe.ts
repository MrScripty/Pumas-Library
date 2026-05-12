import type { RuntimeProfileConfig } from '../../../types/api-runtime-profiles';
import type { ModelServeError, ModelServingConfig } from '../../../types/api-serving';
import {
  buildModelServingConfig,
  defaultContextSizeForProfile,
  formatServeError,
  getPlacementControls,
} from '../../model-serve/modelServeHelpers';

export function buildOnnxQuickServeConfig(profile: RuntimeProfileConfig): ModelServingConfig | null {
  return buildModelServingConfig({
    selectedProfile: profile,
    formState: {
      deviceMode: profile.device.mode,
      deviceId: profile.device.device_id ?? '',
      gpuLayers: '',
      tensorSplit: '',
      contextSize: defaultContextSizeForProfile(profile),
      keepLoaded: true,
      modelAlias: '',
    },
    controls: getPlacementControls(profile, profile.device.mode),
  });
}

export function formatOnnxQuickServeError(error: ModelServeError): string {
  return formatServeError(error) ?? 'The selected ONNX Runtime profile cannot serve this model.';
}
