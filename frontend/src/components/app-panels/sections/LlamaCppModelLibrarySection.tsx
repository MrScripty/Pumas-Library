import { useEffect, useState } from 'react';
import { Link2, Play, Save, Search, SlidersHorizontal, Star } from 'lucide-react';
import { getElectronAPI } from '../../../api/adapter';
import { useRuntimeProfiles } from '../../../hooks/useRuntimeProfiles';
import type { ModelCategory, ModelInfo } from '../../../types/apps';
import type {
  ModelServeError,
  ModelServingConfig,
  ServedModelStatus,
} from '../../../types/api-serving';
import type { RuntimeProfileConfig } from '../../../types/api-runtime-profiles';
import { getRuntimeProviderDescriptor } from '../../../utils/runtimeProviderDescriptors';
import { IconButton, ListItem, ListItemContent } from '../../ui';
import { LocalModelMetadataSummary } from '../../LocalModelMetadataSummary';
import { LocalModelNameButton } from '../../LocalModelNameButton';
import { ModelMetadataModal } from '../../ModelMetadataModal';
import { ModelServeDialog } from '../../ModelServeDialog';
import {
  clearModelRuntimeRoute,
  saveModelRuntimeRoute,
} from '../../model-serve/runtimeRouteMutations';
import {
  DEFAULT_LLAMA_CPP_CONTEXT_SIZE,
  getPlacementControls,
} from '../../model-serve/modelServeHelpers';
import { serveModelWithValidation } from '../../model-serve/useModelServingActions';
import {
  buildLlamaCppModelRows,
  getLlamaCppPlacementLabel,
  type LlamaCppModelRowViewModel,
} from './llamaCppLibraryViewModels';

const LLAMA_CPP_PROVIDER = getRuntimeProviderDescriptor('llama_cpp').id;

interface ServingTarget {
  row: LlamaCppModelRowViewModel;
  profileId: string;
}

export interface LlamaCppModelLibrarySectionProps {
  excludedModels: Set<string>;
  modelGroups: ModelCategory[];
  servedModels?: ServedModelStatus[];
  starredModels: Set<string>;
  onToggleLink: (modelId: string) => void;
  onToggleStar: (modelId: string) => void;
}

function getModelFormatLabel(model: ModelInfo): string | undefined {
  return model.primaryFormat ?? model.format;
}

function getRouteLabel(row: LlamaCppModelRowViewModel): string {
  if (row.routeState === 'missing_profile') {
    return 'Missing profile';
  }
  if (row.selectedProfile) {
    return row.selectedProfile.name;
  }
  return 'No profile';
}

function getPlacementLabel(row: LlamaCppModelRowViewModel): string {
  return row.servedPlacement?.label ?? row.selectedProfilePlacement?.label ?? 'Auto';
}

function getPlacementBadge(row: LlamaCppModelRowViewModel): {
  className: string;
  label: string;
  title?: string;
} {
  const failedStatus = row.selectedServedStatus?.load_state === 'failed'
    ? row.selectedServedStatus
    : row.servedStatuses.find((status) => status.load_state === 'failed');
  if (failedStatus?.last_error) {
    return {
      className: 'bg-[hsl(var(--accent-error)/0.14)] text-[hsl(var(--accent-error))]',
      label: 'Failed',
      title: failedStatus.last_error.message,
    };
  }

  if (row.servedPlacement?.source === 'served_status') {
    return {
      className: 'bg-[hsl(var(--accent-success)/0.14)] text-[hsl(var(--accent-success))]',
      label: getPlacementLabel(row),
    };
  }

  return {
    className: 'bg-[hsl(var(--surface-low)/0.55)] text-[hsl(var(--text-secondary))]',
    label: getPlacementLabel(row),
  };
}

function getProfileOptionLabel(profile: RuntimeProfileConfig): string {
  return `${profile.name} - ${
    getLlamaCppPlacementLabel({ profile })?.label ?? 'Auto'
  }`;
}

function formatQuickServeError(error: ModelServeError | null | undefined): string | null {
  if (!error) {
    return null;
  }
  return error.message || error.code.replace(/_/g, ' ');
}

function requiresAliasBeforeQuickServe(
  row: LlamaCppModelRowViewModel,
  profile: RuntimeProfileConfig
): boolean {
  if (!profile) {
    return false;
  }

  return row.servedStatuses.some(
    (status) =>
      status.load_state === 'loaded' &&
      status.profile_id !== profile.profile_id
  );
}

function buildQuickServeConfig(profile: RuntimeProfileConfig): ModelServingConfig {
  const deviceMode = profile.device.mode;
  const controls = getPlacementControls(profile, deviceMode);

  return {
    provider: 'llama_cpp',
    profile_id: profile.profile_id,
    device_mode: deviceMode,
    device_id:
      controls.showDeviceId && profile.device.device_id?.trim()
        ? profile.device.device_id.trim()
        : null,
    gpu_layers: controls.showGpuLayers ? profile.device.gpu_layers ?? null : null,
    tensor_split: controls.showTensorSplit ? profile.device.tensor_split ?? null : null,
    context_size: Number(DEFAULT_LLAMA_CPP_CONTEXT_SIZE),
    keep_loaded: true,
    model_alias: null,
  };
}

function LlamaCppModelRow({
  excludedModels,
  isQuickServing,
  isSavingRoute,
  providerProfiles,
  quickServeFeedback,
  row,
  starredModels,
  onOpenMetadata,
  onOpenServeOptions,
  onQuickServe,
  onSaveRoute,
  onToggleLink,
  onToggleStar,
}: {
  excludedModels: Set<string>;
  isQuickServing: boolean;
  isSavingRoute: boolean;
  providerProfiles: RuntimeProfileConfig[];
  quickServeFeedback?: { kind: 'error' | 'success'; message: string } | null;
  row: LlamaCppModelRowViewModel;
  starredModels: Set<string>;
  onOpenMetadata: (modelId: string, modelName: string) => void;
  onOpenServeOptions: (
    row: LlamaCppModelRowViewModel,
    profile: RuntimeProfileConfig,
    shouldPersistRoute: boolean
  ) => void;
  onQuickServe: (
    row: LlamaCppModelRowViewModel,
    profile: RuntimeProfileConfig,
    shouldPersistRoute: boolean
  ) => void;
  onSaveRoute: (modelId: string, profileId: string) => void;
  onToggleLink: (modelId: string) => void;
  onToggleStar: (modelId: string) => void;
}) {
  const isStarred = starredModels.has(row.model.id) || Boolean(row.model.starred);
  const isLinked = row.model.linkedApps?.includes('llama-cpp') ?? false;
  const isExcluded = excludedModels.has(row.model.id);
  const servedCount = row.servedStatuses.filter((status) => status.load_state === 'loaded').length;
  const placementBadge = getPlacementBadge(row);
  const [draftProfileId, setDraftProfileId] = useState(row.route?.profile_id ?? '');
  const hasDraftChange = draftProfileId !== (row.route?.profile_id ?? '');
  const draftProfile = providerProfiles.find((profile) => profile.profile_id === draftProfileId);
  const isDraftProfileLoaded = row.servedStatuses.some(
    (status) => status.profile_id === draftProfileId && status.load_state === 'loaded'
  );
  const hasRunnableProfile = Boolean(draftProfile);

  useEffect(() => {
    setDraftProfileId(row.route?.profile_id ?? '');
  }, [row.route?.profile_id]);

  return (
    <ListItem highlighted={isLinked} className={isExcluded ? 'opacity-60' : ''}>
      <ListItemContent className="items-start">
        <div className="flex min-w-0 flex-1 items-start gap-2">
          <IconButton
            icon={<Star fill={isStarred ? 'currentColor' : 'none'} />}
            tooltip={isStarred ? 'Unstar' : 'Star'}
            onClick={() => onToggleStar(row.model.id)}
            size="sm"
          />
          <div className="min-w-0 flex-1">
            <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
              <LocalModelNameButton
                modelId={row.model.id}
                modelName={row.model.name}
                isDownloading={false}
                isPartialDownload={Boolean(row.model.isPartialDownload)}
                isLinked={isLinked}
                wasDequantized={row.model.wasDequantized}
                hasIntegrityIssue={Boolean(row.model.hasIntegrityIssue)}
                integrityIssueMessage={row.model.integrityIssueMessage}
                onOpenMetadata={onOpenMetadata}
              />
              <span className="rounded bg-[hsl(var(--surface-low)/0.55)] px-1.5 py-0.5 text-[10px] font-medium uppercase text-[hsl(var(--text-secondary))]">
                {row.modelTypeLabel}
              </span>
              <span
                className={`rounded px-1.5 py-0.5 text-[10px] font-medium uppercase ${placementBadge.className}`}
                title={placementBadge.title}
              >
                {placementBadge.label}
              </span>
              {servedCount > 0 && (
                <span className="rounded bg-[hsl(var(--accent-success)/0.14)] px-1.5 py-0.5 text-[10px] font-medium uppercase text-[hsl(var(--accent-success))]">
                  Loaded {servedCount}
                </span>
              )}
            </div>
            <LocalModelMetadataSummary
              format={getModelFormatLabel(row.model)}
              quant={row.model.quant}
              size={row.model.size}
              hasDependencies={row.model.hasDependencies}
              dependencyCount={row.model.dependencyCount}
            />
            {quickServeFeedback && (
              <div
                className={
                  quickServeFeedback.kind === 'error'
                    ? 'mt-1 text-xs text-[hsl(var(--accent-error))]'
                    : 'mt-1 text-xs text-[hsl(var(--accent-success))]'
                }
              >
                {quickServeFeedback.message}
              </div>
            )}
          </div>
        </div>
        <div className="flex shrink-0 flex-wrap items-center justify-end gap-2 pt-0.5">
          <label className="sr-only" htmlFor={`llamacpp-profile-${row.model.id}`}>
            llama.cpp profile for {row.model.name}
          </label>
          <select
            id={`llamacpp-profile-${row.model.id}`}
            value={draftProfileId}
            onChange={(event) => setDraftProfileId(event.target.value)}
            disabled={providerProfiles.length === 0}
            className="h-8 max-w-44 rounded border border-[hsl(var(--border-subtle))] bg-[hsl(var(--surface-high))] px-2 text-xs text-[hsl(var(--text-primary))]"
            aria-label={`llama.cpp profile for ${row.model.name}`}
          >
            <option value="">
              {providerProfiles.length === 0 ? 'No llama.cpp profiles' : getRouteLabel(row)}
            </option>
            {providerProfiles.map((profile) => (
              <option key={profile.profile_id} value={profile.profile_id}>
                {getProfileOptionLabel(profile)}
              </option>
            ))}
          </select>
          <IconButton
            icon={<Save />}
            tooltip="Save llama.cpp route"
            onClick={() => onSaveRoute(row.model.id, draftProfileId)}
            disabled={!hasDraftChange || isSavingRoute}
            size="sm"
          />
          <IconButton
            icon={<Play />}
            tooltip={
              isDraftProfileLoaded
                ? 'Already loaded on selected profile'
                : 'Quick serve with selected llama.cpp profile'
            }
            onClick={() => {
              if (draftProfile) {
                onQuickServe(row, draftProfile, hasDraftChange);
              }
            }}
            disabled={!hasRunnableProfile || isQuickServing || isSavingRoute || isDraftProfileLoaded}
            size="sm"
          />
          <IconButton
            icon={<SlidersHorizontal />}
            tooltip="Serving options"
            onClick={() => {
              if (draftProfile) {
                onOpenServeOptions(row, draftProfile, hasDraftChange);
              }
            }}
            disabled={!hasRunnableProfile || isSavingRoute}
            size="sm"
          />
          <IconButton
            icon={<Link2 />}
            tooltip={isLinked ? 'Unlink from llama.cpp' : 'Link to llama.cpp'}
            onClick={() => onToggleLink(row.model.id)}
            size="sm"
          />
        </div>
      </ListItemContent>
    </ListItem>
  );
}

export function LlamaCppModelLibrarySection({
  excludedModels,
  modelGroups,
  servedModels = [],
  starredModels,
  onToggleLink,
  onToggleStar,
}: LlamaCppModelLibrarySectionProps) {
  const {
    profiles,
    routes,
    refreshRuntimeProfiles,
  } = useRuntimeProfiles();
  const [metadataModal, setMetadataModal] = useState<{
    modelId: string;
    modelName: string;
  } | null>(null);
  const [servingTarget, setServingTarget] = useState<ServingTarget | null>(null);
  const [quickServeModelId, setQuickServeModelId] = useState<string | null>(null);
  const [quickServeFeedback, setQuickServeFeedback] = useState<{
    kind: 'error' | 'success';
    message: string;
    modelId: string;
  } | null>(null);
  const [savingRouteModelId, setSavingRouteModelId] = useState<string | null>(null);
  const [routeError, setRouteError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const providerProfiles = profiles.filter((profile) => profile.provider === LLAMA_CPP_PROVIDER);
  const rows = buildLlamaCppModelRows({
    modelGroups,
    profiles,
    routes,
    servedStatuses: servedModels,
  });
  const normalizedSearchQuery = searchQuery.trim().toLowerCase();
  const filteredRows = normalizedSearchQuery
    ? rows.filter((row) => {
        const searchable = [
          row.model.name,
          row.model.id,
          row.model.category,
          row.modelTypeLabel,
        ].join(' ').toLowerCase();
        return searchable.includes(normalizedSearchQuery);
      })
    : rows;

  const persistRouteSelection = async (modelId: string, profileId: string): Promise<boolean> => {
    setSavingRouteModelId(modelId);
    setRouteError(null);
    try {
      if (profileId) {
        await saveModelRuntimeRoute({
          provider: LLAMA_CPP_PROVIDER,
          modelId,
          profileId,
          autoLoad: true,
        });
      } else {
        await clearModelRuntimeRoute(LLAMA_CPP_PROVIDER, modelId);
      }
      await refreshRuntimeProfiles();
      return true;
    } catch (caught) {
      setRouteError(caught instanceof Error ? caught.message : 'Failed to save runtime route');
      return false;
    } finally {
      setSavingRouteModelId(null);
    }
  };

  const handleSaveRoute = async (modelId: string, profileId: string) => {
    await persistRouteSelection(modelId, profileId);
  };

  const handleOpenServeOptions = async (
    row: LlamaCppModelRowViewModel,
    profile: RuntimeProfileConfig,
    shouldPersistRoute: boolean
  ) => {
    if (shouldPersistRoute) {
      const saved = await persistRouteSelection(row.model.id, profile.profile_id);
      if (!saved) {
        return;
      }
    }
    setServingTarget({
      row,
      profileId: profile.profile_id,
    });
  };

  const handleQuickServe = async (
    row: LlamaCppModelRowViewModel,
    profile: RuntimeProfileConfig,
    shouldPersistRoute: boolean
  ) => {
    if (!profile) {
      setQuickServeFeedback({
        kind: 'error',
        message: 'Select a llama.cpp profile before serving this model.',
        modelId: row.model.id,
      });
      return;
    }

    if (shouldPersistRoute) {
      const saved = await persistRouteSelection(row.model.id, profile.profile_id);
      if (!saved) {
        return;
      }
    }

    if (requiresAliasBeforeQuickServe(row, profile)) {
      setServingTarget({
        row,
        profileId: profile.profile_id,
      });
      return;
    }

    const api = getElectronAPI();
    if (!api?.validate_model_serving_config || !api.serve_model) {
      setQuickServeFeedback({
        kind: 'error',
        message: 'Serving API is not available in this app session.',
        modelId: row.model.id,
      });
      return;
    }

    setQuickServeModelId(row.model.id);
    setQuickServeFeedback(null);
    try {
      const result = await serveModelWithValidation({
        api,
        config: buildQuickServeConfig(profile),
        modelId: row.model.id,
      });

      if (result.kind === 'validation_failed' && result.error.code === 'duplicate_model_alias') {
        setServingTarget({
          row,
          profileId: profile.profile_id,
        });
        return;
      }
      if (result.kind === 'loaded') {
        setQuickServeFeedback({
          kind: 'success',
          message: 'Loaded',
          modelId: row.model.id,
        });
        return;
      }
      if (result.kind === 'validation_failed' || result.kind === 'load_failed') {
        setQuickServeFeedback({
          kind: 'error',
          message:
            formatQuickServeError(result.error) ??
            'The selected llama.cpp profile cannot serve this model.',
          modelId: row.model.id,
        });
        return;
      }
      setQuickServeFeedback({
        kind: 'error',
        message: result.message,
        modelId: row.model.id,
      });
    } catch (caught) {
      setQuickServeFeedback({
        kind: 'error',
        message: caught instanceof Error ? caught.message : 'Serving request failed',
        modelId: row.model.id,
      });
    } finally {
      setQuickServeModelId(null);
    }
  };

  if (servingTarget) {
    return (
      <section className="min-h-0 flex-1 overflow-hidden bg-[hsl(var(--launcher-bg-tertiary)/0.2)]">
        <ModelServeDialog
          model={servingTarget.row.model}
          displayMode="page"
          initialProfileId={servingTarget.profileId}
          providerFilter="llama_cpp"
          onBack={() => setServingTarget(null)}
          onClose={() => setServingTarget(null)}
        />
      </section>
    );
  }

  return (
    <section className="min-h-0 flex-1 overflow-hidden bg-[hsl(var(--launcher-bg-tertiary)/0.2)]">
      <div className="flex h-full flex-col">
        <div className="flex shrink-0 items-center justify-between border-b border-[hsl(var(--border-subtle))] px-4 py-3">
          <div className="min-w-0">
            <h2 className="text-sm font-semibold text-[hsl(var(--text-primary))]">
              llama.cpp Library
            </h2>
            <div className="text-xs text-[hsl(var(--text-muted))]">
              {rows.length} compatible local model{rows.length === 1 ? '' : 's'}
            </div>
          </div>
          <label className="relative min-w-44 max-w-64 flex-1">
            <span className="sr-only">Search llama.cpp models</span>
            <Search className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[hsl(var(--text-muted))]" />
            <input
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="Search models"
              className="h-8 w-full rounded border border-[hsl(var(--border-subtle))] bg-[hsl(var(--surface-high))] pl-7 pr-2 text-xs text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-muted))]"
            />
          </label>
          {routeError && (
            <div className="mt-2 rounded border border-[hsl(var(--accent-error)/0.35)] bg-[hsl(var(--accent-error)/0.12)] px-3 py-2 text-xs text-[hsl(var(--accent-error))]">
              {routeError}
            </div>
          )}
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          {rows.length === 0 ? (
            <div className="rounded border border-[hsl(var(--border-subtle))] bg-[hsl(var(--surface-low)/0.18)] px-4 py-6 text-sm text-[hsl(var(--text-muted))]">
              No local GGUF models are available for llama.cpp.
            </div>
          ) : filteredRows.length === 0 ? (
            <div className="rounded border border-[hsl(var(--border-subtle))] bg-[hsl(var(--surface-low)/0.18)] px-4 py-6 text-sm text-[hsl(var(--text-muted))]">
              No compatible llama.cpp models match the current search.
            </div>
          ) : (
            <div className="space-y-1.5">
              {filteredRows.map((row) => (
                <LlamaCppModelRow
                  key={row.model.id}
                  excludedModels={excludedModels}
                  isQuickServing={quickServeModelId === row.model.id}
                  isSavingRoute={savingRouteModelId === row.model.id}
                  providerProfiles={providerProfiles}
                  quickServeFeedback={
                    quickServeFeedback?.modelId === row.model.id ? quickServeFeedback : null
                  }
                  row={row}
                  starredModels={starredModels}
                  onOpenMetadata={(modelId, modelName) => {
                    setMetadataModal({ modelId, modelName });
                  }}
                  onOpenServeOptions={(selectedRow, profile, shouldPersistRoute) => {
                    void handleOpenServeOptions(selectedRow, profile, shouldPersistRoute);
                  }}
                  onQuickServe={(selectedRow, profile, shouldPersistRoute) => {
                    void handleQuickServe(selectedRow, profile, shouldPersistRoute);
                  }}
                  onSaveRoute={(modelId, profileId) => {
                    void handleSaveRoute(modelId, profileId);
                  }}
                  onToggleLink={onToggleLink}
                  onToggleStar={onToggleStar}
                />
              ))}
            </div>
          )}
        </div>
      </div>
      {metadataModal && (
        <ModelMetadataModal
          modelId={metadataModal.modelId}
          modelName={metadataModal.modelName}
          onClose={() => setMetadataModal(null)}
        />
      )}
    </section>
  );
}
