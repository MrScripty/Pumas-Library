import { getElectronAPI } from '../../api/adapter';
import type { RuntimeProviderId } from '../../types/api-runtime-profiles';

export async function saveModelRuntimeRoute({
  autoLoad,
  modelId,
  profileId,
  provider,
}: {
  autoLoad: boolean;
  modelId: string;
  profileId: string | null;
  provider: RuntimeProviderId;
}): Promise<void> {
  const electronAPI = getElectronAPI();
  if (!electronAPI?.set_model_runtime_route) {
    throw new Error('Runtime route API is unavailable');
  }

  const response = await electronAPI.set_model_runtime_route({
    provider,
    model_id: modelId,
    profile_id: profileId,
    auto_load: autoLoad,
  });
  if (!response.success) {
    throw new Error(response.error ?? 'Failed to save runtime route');
  }
}

export async function clearModelRuntimeRoute(
  provider: RuntimeProviderId,
  modelId: string
): Promise<void> {
  const electronAPI = getElectronAPI();
  if (!electronAPI?.clear_model_runtime_route) {
    throw new Error('Runtime route API is unavailable');
  }

  const response = await electronAPI.clear_model_runtime_route(provider, modelId);
  if (!response.success) {
    throw new Error(response.error ?? 'Failed to clear runtime route');
  }
}
