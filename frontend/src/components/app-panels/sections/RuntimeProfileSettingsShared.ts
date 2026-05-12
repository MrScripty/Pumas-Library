import type {
  RuntimeDeviceMode,
  RuntimeManagementMode,
  RuntimeProviderId,
  RuntimeProviderMode,
} from '../../../types/api-runtime-profiles';
import {
  deviceModeLabel,
  modeLabel,
  providerLabel,
  runtimeProviderDescriptors,
} from '../../../utils/runtimeProviderDescriptors';

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
  ollama: runtimeProviderDescriptors.ollama.profileModes,
  llama_cpp: runtimeProviderDescriptors.llama_cpp.profileModes,
  onnx_runtime: runtimeProviderDescriptors.onnx_runtime.profileModes,
};

export const providerManagementModes: Record<RuntimeProviderId, RuntimeManagementMode[]> = {
  ollama: runtimeProviderDescriptors.ollama.managementModes,
  llama_cpp: runtimeProviderDescriptors.llama_cpp.managementModes,
  onnx_runtime: runtimeProviderDescriptors.onnx_runtime.managementModes,
};

export const providerDeviceModes: Record<RuntimeProviderId, RuntimeDeviceMode[]> = {
  ollama: runtimeProviderDescriptors.ollama.deviceModes,
  llama_cpp: runtimeProviderDescriptors.llama_cpp.deviceModes,
  onnx_runtime: runtimeProviderDescriptors.onnx_runtime.deviceModes,
};

export { deviceModeLabel, modeLabel, providerLabel };

export type RuntimeProfileDraftUpdater = <Key extends keyof RuntimeProfileDraft>(
  key: Key,
  value: RuntimeProfileDraft[Key]
) => void;
