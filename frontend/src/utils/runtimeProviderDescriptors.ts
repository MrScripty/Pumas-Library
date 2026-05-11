import type {
  RuntimeDeviceMode,
  RuntimeProviderId,
  RuntimeProviderMode,
} from '../types/api-runtime-profiles';
import type { ModelCategory, ModelInfo } from '../types/apps';

export interface RuntimeProviderDescriptor {
  id: RuntimeProviderId;
  label: string;
  profileModes: RuntimeProviderMode[];
  deviceModes: RuntimeDeviceMode[];
  compatibleExecutableFormats: string[];
  dedicatedPlacementModes: RuntimeProviderMode[];
  supportsGpuLayers: boolean;
  supportsTensorSplit: boolean;
  supportsContextSize: boolean;
  defaultContextSize: string | null;
  canLaunchOnServe: boolean;
}

export const runtimeProviderDescriptors: Record<RuntimeProviderId, RuntimeProviderDescriptor> = {
  ollama: {
    id: 'ollama',
    label: 'Ollama',
    profileModes: ['ollama_serve'],
    deviceModes: ['auto', 'cpu', 'gpu', 'hybrid'],
    compatibleExecutableFormats: ['gguf'],
    dedicatedPlacementModes: [],
    supportsGpuLayers: false,
    supportsTensorSplit: false,
    supportsContextSize: false,
    defaultContextSize: null,
    canLaunchOnServe: false,
  },
  llama_cpp: {
    id: 'llama_cpp',
    label: 'llama.cpp',
    profileModes: ['llama_cpp_router', 'llama_cpp_dedicated'],
    deviceModes: ['auto', 'cpu', 'gpu', 'specific_device'],
    compatibleExecutableFormats: ['gguf'],
    dedicatedPlacementModes: ['llama_cpp_dedicated'],
    supportsGpuLayers: true,
    supportsTensorSplit: true,
    supportsContextSize: true,
    defaultContextSize: '4096',
    canLaunchOnServe: true,
  },
  onnx_runtime: {
    id: 'onnx_runtime',
    label: 'ONNX Runtime',
    profileModes: ['onnx_serve'],
    deviceModes: ['auto', 'cpu'],
    compatibleExecutableFormats: ['onnx'],
    dedicatedPlacementModes: [],
    supportsGpuLayers: false,
    supportsTensorSplit: false,
    supportsContextSize: false,
    defaultContextSize: null,
    canLaunchOnServe: false,
  },
};

const modeLabels: Record<RuntimeProviderMode, string> = {
  ollama_serve: 'Serve',
  llama_cpp_router: 'Router',
  llama_cpp_dedicated: 'Dedicated',
  onnx_serve: 'Serve',
};

const deviceModeLabels: Record<RuntimeDeviceMode, string> = {
  auto: 'Auto',
  cpu: 'CPU',
  gpu: 'GPU',
  hybrid: 'Hybrid',
  specific_device: 'Specific device',
};

export function getRuntimeProviderDescriptor(
  provider: RuntimeProviderId
): RuntimeProviderDescriptor {
  return runtimeProviderDescriptors[provider];
}

export function providerLabel(provider: RuntimeProviderId): string {
  return getRuntimeProviderDescriptor(provider).label;
}

export function modeLabel(mode: RuntimeProviderMode): string {
  return modeLabels[mode];
}

export function deviceModeLabel(mode: RuntimeDeviceMode): string {
  return deviceModeLabels[mode];
}

export function modelHasExecutableFormat(model: ModelInfo, formats: string[]): boolean {
  const normalizedFormats = new Set(formats.map((format) => format.toLowerCase()));
  const artifactNames = [
    model.path,
    model.selectedArtifactId,
    model.downloadArtifactId,
    model.downloadSelectedArtifactId,
    ...(model.selectedArtifactFiles ?? []),
  ];

  return (
    normalizedFormats.has(normalized(model.primaryFormat)) ||
    normalizedFormats.has(normalized(model.format)) ||
    artifactNames.some((artifactName) => {
      const normalizedArtifactName = normalized(artifactName);
      return [...normalizedFormats].some((format) => normalizedArtifactName.endsWith(`.${format}`));
    })
  );
}

export function isModelCompatibleWithProvider(
  model: ModelInfo,
  provider: RuntimeProviderId
): boolean {
  return modelHasExecutableFormat(
    model,
    getRuntimeProviderDescriptor(provider).compatibleExecutableFormats
  );
}

export function filterProviderCompatibleModelGroups(
  modelGroups: ModelCategory[],
  provider: RuntimeProviderId
): ModelCategory[] {
  return modelGroups
    .map((group) => ({
      ...group,
      models: group.models.filter((model) => isModelCompatibleWithProvider(model, provider)),
    }))
    .filter((group) => group.models.length > 0);
}

function normalized(value: string | undefined | null): string {
  return value?.trim().toLowerCase() ?? '';
}
