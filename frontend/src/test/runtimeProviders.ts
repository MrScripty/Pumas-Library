import { ValidationError } from '../errors';
import type {
  RuntimeProviderId,
  RuntimeProviderInfo,
  RuntimeProvidersResponse,
} from '../types/api-runtime-profiles';

/**
 * Test double of the backend's `get_runtime_providers` answer for the built-in
 * registry. Production code never reads this; it asks the backend.
 */
export const TEST_RUNTIME_PROVIDERS: RuntimeProviderInfo[] = [
  {
    provider: 'ollama',
    app_id: 'ollama',
    version_managed: true,
    provider_modes: ['ollama_serve'],
    device_modes: ['auto', 'cpu', 'gpu', 'hybrid'],
    supports_managed_profiles: true,
    supports_external_profiles: true,
  },
  {
    provider: 'llama_cpp',
    app_id: 'llama-cpp',
    version_managed: true,
    provider_modes: ['llama_cpp_router', 'llama_cpp_dedicated'],
    device_modes: ['auto', 'cpu', 'gpu', 'specific_device'],
    supports_managed_profiles: true,
    supports_external_profiles: true,
  },
  {
    provider: 'onnx_runtime',
    app_id: 'onnx-runtime',
    version_managed: true,
    provider_modes: ['onnx_serve'],
    device_modes: ['auto', 'cpu', 'gpu'],
    supports_managed_profiles: true,
    supports_external_profiles: false,
  },
  {
    provider: 'torch',
    app_id: 'torch',
    version_managed: true,
    provider_modes: ['torch_serve'],
    device_modes: ['gpu', 'hybrid'],
    supports_managed_profiles: true,
    supports_external_profiles: false,
  },
];

export function testRuntimeProvider(provider: RuntimeProviderId): RuntimeProviderInfo {
  const info = TEST_RUNTIME_PROVIDERS.find((entry) => entry.provider === provider);
  if (!info) {
    throw new ValidationError(`No test runtime provider ${provider}`, 'provider');
  }
  return info;
}

export function testRuntimeProvidersResponse(
  providers: RuntimeProviderInfo[] = TEST_RUNTIME_PROVIDERS
): RuntimeProvidersResponse {
  return { success: true, providers };
}
