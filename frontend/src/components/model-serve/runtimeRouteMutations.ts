import { getElectronAPI } from '../../api/adapter';

export async function saveModelRuntimeRoute({
  autoLoad,
  modelId,
  profileId,
}: {
  autoLoad: boolean;
  modelId: string;
  profileId: string | null;
}): Promise<void> {
  const electronAPI = getElectronAPI();
  if (!electronAPI?.set_model_runtime_route) {
    throw new Error('Runtime route API is unavailable');
  }

  const response = await electronAPI.set_model_runtime_route({
    model_id: modelId,
    profile_id: profileId,
    auto_load: autoLoad,
  });
  if (!response.success) {
    throw new Error(response.error ?? 'Failed to save runtime route');
  }
}

export async function clearModelRuntimeRoute(modelId: string): Promise<void> {
  const electronAPI = getElectronAPI();
  if (!electronAPI?.clear_model_runtime_route) {
    throw new Error('Runtime route API is unavailable');
  }

  const response = await electronAPI.clear_model_runtime_route(modelId);
  if (!response.success) {
    throw new Error(response.error ?? 'Failed to clear runtime route');
  }
}
