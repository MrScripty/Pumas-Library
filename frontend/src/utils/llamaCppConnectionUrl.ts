import type { RuntimeProfileStatus } from '../types/api-runtime-profiles';
import type { ServedModelStatus, ServingEndpointStatus } from '../types/api-serving';

export function getLlamaCppConnectionUrl({
  llamaCppProfileIds,
  runtimeStatuses,
  servedModels,
  servingEndpoint,
}: {
  llamaCppProfileIds: Set<string>;
  runtimeStatuses: RuntimeProfileStatus[];
  servedModels: ServedModelStatus[];
  servingEndpoint: ServingEndpointStatus | null | undefined;
}): string | undefined {
  const loadedLlamaCppModels = servedModels.filter(
    (model) => model.provider === 'llama_cpp' && model.load_state === 'loaded'
  );

  if (
    servingEndpoint?.endpoint_mode === 'pumas_gateway' &&
    servingEndpoint.endpoint_url &&
    loadedLlamaCppModels.length > 0
  ) {
    return servingEndpoint.endpoint_url;
  }

  const loadedProviderUrls = uniqueUrls(
    loadedLlamaCppModels.map((model) => model.endpoint_url)
  );
  if (loadedProviderUrls.length === 1) {
    return loadedProviderUrls[0];
  }

  const runningProfileUrls = uniqueUrls(
    runtimeStatuses
      .filter(
        (status) =>
          llamaCppProfileIds.has(status.profile_id) &&
          (status.state === 'running' || status.state === 'external')
      )
      .map((status) => status.endpoint_url)
  );
  return runningProfileUrls.length === 1 ? runningProfileUrls[0] : undefined;
}

function uniqueUrls(urls: Array<string | null | undefined>): string[] {
  return Array.from(new Set(urls.filter((url): url is string => Boolean(url))));
}
