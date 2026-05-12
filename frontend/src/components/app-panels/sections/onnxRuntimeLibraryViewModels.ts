import type {
  ModelRuntimeRoute,
  RuntimeProfileConfig,
} from '../../../types/api-runtime-profiles';
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
}

export interface BuildOnnxRuntimeModelRowsInput {
  modelGroups: ModelCategory[];
  profiles: RuntimeProfileConfig[];
  routes: ModelRuntimeRoute[];
}

const ONNX_RUNTIME_PROVIDER = getRuntimeProviderDescriptor('onnx_runtime').id;

export function buildOnnxRuntimeModelRows({
  modelGroups,
  profiles,
  routes,
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

      return {
        model,
        route: route ?? null,
        routeState,
        selectedProfile,
      };
    });
}
