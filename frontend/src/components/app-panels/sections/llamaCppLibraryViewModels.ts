import type {
  ModelRuntimeRoute,
  RuntimeDeviceMode,
  RuntimeProfileConfig,
} from '../../../types/api-runtime-profiles';
import type { ServedModelStatus } from '../../../types/api-serving';
import type { ModelCategory, ModelInfo } from '../../../types/apps';
import {
  filterProviderCompatibleModelGroups,
  getRuntimeProviderDescriptor,
  isModelCompatibleWithProvider,
} from '../../../utils/runtimeProviderDescriptors';

export type LlamaCppModelTypeLabel = 'Chat' | 'Embedding' | 'Reranker' | 'Model';
export type LlamaCppRouteState = 'unrouted' | 'routed' | 'missing_profile';
export type LlamaCppPlacementSource = 'served_status' | 'profile';

export interface ServedInstanceIdentity {
  modelId: string;
  profileId: string;
  modelAlias?: string | null;
}

export interface LlamaCppPlacementLabel {
  label: 'CPU' | 'GPU' | 'iGPU' | 'Hybrid' | 'Auto';
  source: LlamaCppPlacementSource;
}

export interface LlamaCppServedStateViewModel {
  servedStatusesByModelId: Map<string, ServedModelStatus[]>;
  servedStatusByInstanceKey: Map<string, ServedModelStatus>;
}

export interface LlamaCppModelRowViewModel {
  model: ModelInfo;
  modelTypeLabel: LlamaCppModelTypeLabel;
  route: ModelRuntimeRoute | null;
  routeState: LlamaCppRouteState;
  selectedProfile: RuntimeProfileConfig | null;
  selectedProfilePlacement: LlamaCppPlacementLabel | null;
  selectedServedStatus: ServedModelStatus | null;
  servedStatuses: ServedModelStatus[];
  servedInstanceKeys: string[];
  servedPlacement: LlamaCppPlacementLabel | null;
}

export interface BuildLlamaCppModelRowsInput {
  modelGroups: ModelCategory[];
  profiles: RuntimeProfileConfig[];
  routes: ModelRuntimeRoute[];
  servedStatuses: ServedModelStatus[];
}

const LLAMA_CPP_PROVIDER = getRuntimeProviderDescriptor('llama_cpp').id;

function normalized(value: string | undefined | null): string {
  return value?.trim().toLowerCase() ?? '';
}

export function isLlamaCppCompatibleModel(model: ModelInfo): boolean {
  return isModelCompatibleWithProvider(model, LLAMA_CPP_PROVIDER);
}

export function filterLlamaCppCompatibleModelGroups(modelGroups: ModelCategory[]): ModelCategory[] {
  return filterProviderCompatibleModelGroups(modelGroups, LLAMA_CPP_PROVIDER);
}

export function getLlamaCppModelTypeLabel(model: ModelInfo): LlamaCppModelTypeLabel {
  const category = normalized(model.category);
  const name = normalized(model.name);
  const id = normalized(model.id);
  const searchable = `${category} ${name} ${id}`;

  if (searchable.includes('embedding') || searchable.includes('embed')) {
    return 'Embedding';
  }
  if (searchable.includes('reranker') || searchable.includes('rerank')) {
    return 'Reranker';
  }
  if (searchable.includes('chat') || searchable.includes('instruct')) {
    return 'Chat';
  }
  return 'Model';
}

export function buildServedInstanceKey(identity: ServedInstanceIdentity): string {
  return JSON.stringify([
    identity.modelId,
    identity.profileId,
    identity.modelAlias?.trim() ?? '',
  ]);
}

export function buildServedInstanceKeyFromStatus(status: ServedModelStatus): string {
  return buildServedInstanceKey({
    modelId: status.model_id,
    profileId: status.profile_id,
    modelAlias: status.model_alias,
  });
}

export function deriveLlamaCppServedState(
  servedStatuses: ServedModelStatus[]
): LlamaCppServedStateViewModel {
  const servedStatusesByModelId = new Map<string, ServedModelStatus[]>();
  const servedStatusByInstanceKey = new Map<string, ServedModelStatus>();

  for (const status of servedStatuses.filter((status) => status.provider === LLAMA_CPP_PROVIDER)) {
    const modelStatuses = servedStatusesByModelId.get(status.model_id);
    if (modelStatuses) {
      modelStatuses.push(status);
    } else {
      servedStatusesByModelId.set(status.model_id, [status]);
    }

    servedStatusByInstanceKey.set(buildServedInstanceKeyFromStatus(status), status);
  }

  return {
    servedStatusesByModelId,
    servedStatusByInstanceKey,
  };
}

function deviceModeToPlacementLabel(
  deviceMode: RuntimeDeviceMode,
  deviceId: string | null | undefined
): LlamaCppPlacementLabel['label'] {
  if (deviceMode === 'cpu') {
    return 'CPU';
  }
  if (deviceMode === 'hybrid') {
    return 'Hybrid';
  }
  if (deviceMode === 'auto') {
    return 'Auto';
  }

  const normalizedDeviceId = normalized(deviceId);
  if (
    normalizedDeviceId.includes('igpu') ||
    normalizedDeviceId.includes('integrated')
  ) {
    return 'iGPU';
  }

  return 'GPU';
}

export function getLlamaCppPlacementLabel({
  profile,
  status,
}: {
  profile?: RuntimeProfileConfig | null;
  status?: ServedModelStatus | null;
}): LlamaCppPlacementLabel | null {
  if (status) {
    return {
      label: deviceModeToPlacementLabel(status.device_mode, status.device_id),
      source: 'served_status',
    };
  }

  if (!profile) {
    return null;
  }

  return {
    label: deviceModeToPlacementLabel(profile.device.mode, profile.device.device_id),
    source: 'profile',
  };
}

function findSelectedServedStatus(
  modelId: string,
  route: ModelRuntimeRoute | undefined,
  servedStatuses: ServedModelStatus[]
): ServedModelStatus | null {
  if (!route?.profile_id) {
    return null;
  }

  return (
    servedStatuses.find(
      (status) => status.model_id === modelId && status.profile_id === route.profile_id
    ) ?? null
  );
}

export function buildLlamaCppModelRows({
  modelGroups,
  profiles,
  routes,
  servedStatuses,
}: BuildLlamaCppModelRowsInput): LlamaCppModelRowViewModel[] {
  const compatibleGroups = filterLlamaCppCompatibleModelGroups(modelGroups);
  const profileById = new Map(
    profiles
      .filter((profile) => profile.provider === LLAMA_CPP_PROVIDER)
      .map((profile) => [profile.profile_id, profile])
  );
  const routeByModelId = new Map(
    routes
      .filter((route) => route.provider === LLAMA_CPP_PROVIDER)
      .map((route) => [route.model_id, route])
  );
  const servedState = deriveLlamaCppServedState(servedStatuses);

  return compatibleGroups.flatMap((group) =>
    group.models.map((model) => {
      const route = routeByModelId.get(model.id);
      const selectedProfile = route?.profile_id ? profileById.get(route.profile_id) ?? null : null;
      const routeState: LlamaCppRouteState = route?.profile_id
        ? selectedProfile
          ? 'routed'
          : 'missing_profile'
        : 'unrouted';
      const modelServedStatuses = servedState.servedStatusesByModelId.get(model.id) ?? [];
      const selectedServedStatus = findSelectedServedStatus(model.id, route, modelServedStatuses);

      return {
        model,
        modelTypeLabel: getLlamaCppModelTypeLabel(model),
        route: route ?? null,
        routeState,
        selectedProfile,
        selectedProfilePlacement: getLlamaCppPlacementLabel({ profile: selectedProfile }),
        selectedServedStatus,
        servedStatuses: modelServedStatuses,
        servedInstanceKeys: modelServedStatuses.map(buildServedInstanceKeyFromStatus),
        servedPlacement: getLlamaCppPlacementLabel({
          profile: selectedProfile,
          status: selectedServedStatus,
        }),
      };
    })
  );
}
