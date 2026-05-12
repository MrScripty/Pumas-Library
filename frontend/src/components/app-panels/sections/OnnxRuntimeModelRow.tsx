import { useEffect, useState } from 'react';
import { Link2, Play, Save, SlidersHorizontal, Star } from 'lucide-react';
import type { RuntimeProfileConfig } from '../../../types/api-runtime-profiles';
import type { ServedModelStatus } from '../../../types/api-serving';
import { LocalModelMetadataSummary } from '../../LocalModelMetadataSummary';
import { LocalModelNameButton } from '../../LocalModelNameButton';
import { IconButton, ListItem, ListItemContent } from '../../ui';
import type { OnnxRuntimeModelRowViewModel } from './onnxRuntimeLibraryViewModels';

interface OnnxRuntimeModelRowProps {
  excludedModels: Set<string>;
  isQuickServing: boolean;
  isSavingRoute: boolean;
  providerProfiles: RuntimeProfileConfig[];
  quickServeFeedback: {
    kind: 'error' | 'success';
    message: string;
  } | null;
  row: OnnxRuntimeModelRowViewModel;
  starredModels: Set<string>;
  onOpenMetadata: (modelId: string, modelName: string) => void;
  onOpenServeOptions: (
    row: OnnxRuntimeModelRowViewModel,
    profile: RuntimeProfileConfig,
    shouldPersistRoute: boolean
  ) => void;
  onQuickServe: (
    row: OnnxRuntimeModelRowViewModel,
    profile: RuntimeProfileConfig,
    shouldPersistRoute: boolean
  ) => void;
  onSaveRoute: (modelId: string, profileId: string) => void;
  onToggleLink: (modelId: string) => void;
  onToggleStar: (modelId: string) => void;
}

function routeLabel(row: OnnxRuntimeModelRowViewModel): string {
  if (row.routeState === 'missing_profile') {
    return 'Missing profile';
  }
  return row.selectedProfile?.name ?? 'No profile';
}

export function OnnxRuntimeModelRow({
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
}: OnnxRuntimeModelRowProps) {
  const isStarred = starredModels.has(row.model.id) || Boolean(row.model.starred);
  const isLinked = row.model.linkedApps?.includes('onnx-runtime') ?? false;
  const isExcluded = excludedModels.has(row.model.id);
  const [draftProfileId, setDraftProfileId] = useState(row.route?.profile_id ?? '');
  const hasDraftChange = draftProfileId !== (row.route?.profile_id ?? '');
  const draftProfile = providerProfiles.find((profile) => profile.profile_id === draftProfileId);
  const loadedStatuses = row.servedStatuses.filter(
    (status: ServedModelStatus) => status.load_state === 'loaded'
  );
  const failedStatus =
    row.selectedServedStatus?.load_state === 'failed'
      ? row.selectedServedStatus
      : row.servedStatuses.find((status) => status.load_state === 'failed') ?? null;
  const isDraftProfileLoaded = row.servedStatuses.some(
    (status) => status.profile_id === draftProfileId && status.load_state === 'loaded'
  );

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
                onOpenMetadata={() => onOpenMetadata(row.model.id, row.model.name)}
              />
              <span className="rounded bg-[hsl(var(--surface-low)/0.55)] px-1.5 py-0.5 text-[10px] font-medium uppercase text-[hsl(var(--text-secondary))]">
                ONNX
              </span>
              {row.routeState === 'missing_profile' && (
                <span className="rounded bg-[hsl(var(--accent-error)/0.14)] px-1.5 py-0.5 text-[10px] font-medium uppercase text-[hsl(var(--accent-error))]">
                  Missing profile
                </span>
              )}
              {loadedStatuses.length > 0 && (
                <span
                  className="rounded bg-[hsl(var(--accent-success)/0.14)] px-1.5 py-0.5 text-[10px] font-medium uppercase text-[hsl(var(--accent-success))]"
                  title={loadedStatuses[0]?.endpoint_url ?? undefined}
                >
                  Loaded {loadedStatuses.length}
                </span>
              )}
              {failedStatus && (
                <span
                  className="rounded bg-[hsl(var(--accent-error)/0.14)] px-1.5 py-0.5 text-[10px] font-medium uppercase text-[hsl(var(--accent-error))]"
                  title={failedStatus.last_error?.message ?? undefined}
                >
                  Failed
                </span>
              )}
            </div>
            <LocalModelMetadataSummary
              format={row.model.primaryFormat ?? row.model.format}
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
          <label className="sr-only" htmlFor={`onnx-profile-${row.model.id}`}>
            ONNX Runtime profile for {row.model.name}
          </label>
          <select
            id={`onnx-profile-${row.model.id}`}
            value={draftProfileId}
            onChange={(event) => setDraftProfileId(event.target.value)}
            disabled={providerProfiles.length === 0}
            className="h-8 max-w-44 rounded border border-[hsl(var(--border-subtle))] bg-[hsl(var(--surface-high))] px-2 text-xs text-[hsl(var(--text-primary))]"
            aria-label={`ONNX Runtime profile for ${row.model.name}`}
          >
            <option value="">
              {providerProfiles.length === 0 ? 'No ONNX Runtime profiles' : routeLabel(row)}
            </option>
            {providerProfiles.map((profile) => (
              <option key={profile.profile_id} value={profile.profile_id}>
                {profile.name}
              </option>
            ))}
          </select>
          <IconButton
            icon={<Save />}
            tooltip="Save ONNX Runtime route"
            onClick={() => onSaveRoute(row.model.id, draftProfileId)}
            disabled={!hasDraftChange || isSavingRoute}
            size="sm"
          />
          <IconButton
            icon={<Play />}
            tooltip={
              isDraftProfileLoaded
                ? 'Already loaded on selected profile'
                : isQuickServing
                  ? 'Starting ONNX Runtime serving'
                  : 'Quick serve with selected ONNX Runtime profile'
            }
            onClick={() => {
              if (draftProfile) {
                onQuickServe(row, draftProfile, hasDraftChange);
              }
            }}
            disabled={!draftProfile || isQuickServing || isSavingRoute || isDraftProfileLoaded}
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
            disabled={!draftProfile || isSavingRoute}
            size="sm"
          />
          <IconButton
            icon={<Link2 />}
            tooltip={isLinked ? 'Unlink from ONNX Runtime' : 'Link to ONNX Runtime'}
            onClick={() => onToggleLink(row.model.id)}
            size="sm"
          />
        </div>
      </ListItemContent>
    </ListItem>
  );
}
