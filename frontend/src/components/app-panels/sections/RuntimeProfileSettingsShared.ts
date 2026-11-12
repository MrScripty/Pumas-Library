import type {
  RuntimeDeviceMode,
  RuntimeManagementMode,
  RuntimeProfileStatus,
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
  /** Installed runtime version to launch; empty follows the active version. */
  runtime_version: string;
};

export const providerModes: Record<RuntimeProviderId, RuntimeProviderMode[]> = {
  ollama: runtimeProviderDescriptors.ollama.profileModes,
  llama_cpp: runtimeProviderDescriptors.llama_cpp.profileModes,
  onnx_runtime: runtimeProviderDescriptors.onnx_runtime.profileModes,
  torch: runtimeProviderDescriptors.torch.profileModes,
};

export const providerManagementModes: Record<RuntimeProviderId, RuntimeManagementMode[]> = {
  ollama: runtimeProviderDescriptors.ollama.managementModes,
  llama_cpp: runtimeProviderDescriptors.llama_cpp.managementModes,
  onnx_runtime: runtimeProviderDescriptors.onnx_runtime.managementModes,
  torch: runtimeProviderDescriptors.torch.managementModes,
};

export const providerDeviceModes: Record<RuntimeProviderId, RuntimeDeviceMode[]> = {
  ollama: runtimeProviderDescriptors.ollama.deviceModes,
  llama_cpp: runtimeProviderDescriptors.llama_cpp.deviceModes,
  onnx_runtime: runtimeProviderDescriptors.onnx_runtime.deviceModes,
  torch: runtimeProviderDescriptors.torch.deviceModes,
};

export { deviceModeLabel, modeLabel, providerLabel };

export type RuntimeProfileDraftUpdater = <Key extends keyof RuntimeProfileDraft>(
  key: Key,
  value: RuntimeProfileDraft[Key]
) => void;

/** Lifecycle state, with the version a running instance launched from when known. */
export function runtimeStatusLabel(status: RuntimeProfileStatus | null | undefined): string {
  if (!status) {
    return 'unknown';
  }
  return status.runtime_version ? `${status.state} ${status.runtime_version}` : status.state;
}
