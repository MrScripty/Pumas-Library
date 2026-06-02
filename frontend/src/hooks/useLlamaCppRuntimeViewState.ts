import { useMemo } from 'react';
import type {
  RuntimeProfileConfig,
  RuntimeProfileStatus,
} from '../types/api-runtime-profiles';
import type { ServedModelStatus, ServingEndpointStatus } from '../types/api-serving';
import { getLlamaCppConnectionUrl } from '../utils/llamaCppConnectionUrl';

export interface LlamaCppRuntimeViewState {
  isRunning: boolean;
  isStarting: boolean;
  isStopping: boolean;
  launchError: string | null;
}

export function useLlamaCppRuntimeViewState({
  profiles,
  runtimeStatuses,
  servedModels,
  servingEndpoint,
}: {
  profiles: RuntimeProfileConfig[];
  runtimeStatuses: RuntimeProfileStatus[];
  servedModels: ServedModelStatus[];
  servingEndpoint: ServingEndpointStatus | null;
}) {
  const llamaCppProfileIds = useMemo(() => {
    return new Set(
      profiles
        .filter((profile) => profile.provider === 'llama_cpp')
        .map((profile) => profile.profile_id)
    );
  }, [profiles]);

  const runtimeState = useMemo((): LlamaCppRuntimeViewState => {
    const statuses = runtimeStatuses.filter((status) =>
      llamaCppProfileIds.has(status.profile_id)
    );
    const hasServedModel = servedModels.some(
      (model) => model.provider === 'llama_cpp' && model.load_state === 'loaded'
    );
    return {
      isRunning:
        hasServedModel ||
        statuses.some((status) => status.state === 'running' || status.state === 'external'),
      isStarting: statuses.some((status) => status.state === 'starting'),
      isStopping: statuses.some((status) => status.state === 'stopping'),
      launchError: statuses.find((status) => status.state === 'failed')?.last_error ?? null,
    };
  }, [llamaCppProfileIds, runtimeStatuses, servedModels]);

  const connectionUrl = useMemo(
    () =>
      getLlamaCppConnectionUrl({
        servingEndpoint,
        servedModels,
        runtimeStatuses,
        llamaCppProfileIds,
      }),
    [llamaCppProfileIds, runtimeStatuses, servedModels, servingEndpoint]
  );

  return { connectionUrl, runtimeState };
}
