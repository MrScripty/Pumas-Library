import { useEffect, useState } from 'react';
import { Link2, Play, Save, SlidersHorizontal, Star } from 'lucide-react';
import type { RuntimeProfileConfig } from '../../../types/api-runtime-profiles';
import type { ServedModelStatus } from '../../../types/api-serving';
import type { ModelInfo } from '../../../types/apps';
import { IconButton, ListItem, ListItemContent } from '../../ui';
import { LocalModelMetadataSummary } from '../../LocalModelMetadataSummary';
import { LocalModelNameButton } from '../../LocalModelNameButton';
import {
  getLlamaCppPlacementLabel,
  type LlamaCppModelRowViewModel,
} from './llamaCppLibraryViewModels';

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

export interface LlamaCppModelRowProps {
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
}

export function LlamaCppModelRow({
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
}: LlamaCppModelRowProps) {
  const isStarred = starredModels.has(row.model.id) || Boolean(row.model.starred);
  const isLinked = row.model.linkedApps?.includes('llama-cpp') ?? false;
  const isExcluded = excludedModels.has(row.model.id);
  const servedCount = row.servedStatuses.filter((status: ServedModelStatus) => (
    status.load_state === 'loaded'
  )).length;
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
