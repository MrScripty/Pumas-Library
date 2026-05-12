import type {
  ModelRuntimeRoute,
  RuntimeProfileConfig,
} from '../../../types/api-runtime-profiles';
import type { ServedModelStatus } from '../../../types/api-serving';
import type { ModelCategory, ModelInfo } from '../../../types/apps';
import {
  filterProviderCompatibleModelGroups,
  getRuntimeProviderDescriptor,
} from '../../../utils/runtimeProviderDescriptors';

export type OnnxRuntimeRouteState = 'unrouted' | 'routed' | 'missing_profile';

export interface OnnxRuntimeModelRowViewModel {
  model: ModelInfo;
  route: ModelRuntimeRoute | null;
  routeState: OnnxRuntimeRouteState;
  selectedProfile: RuntimeProfileConfig | null;
  selectedServedStatus: ServedModelStatus | null;
  servedStatuses: ServedModelStatus[];
}

export interface BuildOnnxRuntimeModelRowsInput {
  modelGroups: ModelCategory[];
  profiles: RuntimeProfileConfig[];
  routes: ModelRuntimeRoute[];
  servedStatuses: ServedModelStatus[];
}

const ONNX_RUNTIME_PROVIDER = getRuntimeProviderDescriptor('onnx_runtime').id;

export function buildOnnxRuntimeModelRows({
  modelGroups,
  profiles,
  routes,
  servedStatuses,
}: BuildOnnxRuntimeModelRowsInput): OnnxRuntimeModelRowViewModel[] {
  const profileById = new Map(
    profiles
      .filter((profile) => profile.provider === ONNX_RUNTIME_PROVIDER)
      .map((profile) => [profile.profile_id, profile])
  );
  const routeByModelId = new Map(
    routes
      .filter((route) => route.provider === ONNX_RUNTIME_PROVIDER)
      .map((route) => [route.model_id, route])
  );
  const servedStatusesByModelId = new Map<string, ServedModelStatus[]>();
  for (const status of servedStatuses.filter(
    (status) => status.provider === ONNX_RUNTIME_PROVIDER
  )) {
    const modelStatuses = servedStatusesByModelId.get(status.model_id);
    if (modelStatuses) {
      modelStatuses.push(status);
    } else {
      servedStatusesByModelId.set(status.model_id, [status]);
    }
  }

  return filterProviderCompatibleModelGroups(modelGroups, ONNX_RUNTIME_PROVIDER)
    .flatMap((group) => group.models)
    .map((model) => {
      const route = routeByModelId.get(model.id);
      const selectedProfile = route?.profile_id ? profileById.get(route.profile_id) ?? null : null;
      const routeState: OnnxRuntimeRouteState = route?.profile_id
        ? selectedProfile
          ? 'routed'
          : 'missing_profile'
        : 'unrouted';
      const modelServedStatuses = servedStatusesByModelId.get(model.id) ?? [];
      const selectedServedStatus =
        route?.profile_id
          ? modelServedStatuses.find((status) => status.profile_id === route.profile_id) ?? null
          : null;

      return {
        model,
        route: route ?? null,
        routeState,
        selectedProfile,
        selectedServedStatus,
        servedStatuses: modelServedStatuses,
      };
    });
}
