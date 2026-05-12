import { useEffect, useState } from 'react';
import { Link2, Save, Star } from 'lucide-react';
import type { RuntimeProfileConfig } from '../../../types/api-runtime-profiles';
import { LocalModelMetadataSummary } from '../../LocalModelMetadataSummary';
import { LocalModelNameButton } from '../../LocalModelNameButton';
import { IconButton, ListItem, ListItemContent } from '../../ui';
import type { OnnxRuntimeModelRowViewModel } from './onnxRuntimeLibraryViewModels';

interface OnnxRuntimeModelRowProps {
  excludedModels: Set<string>;
  isSavingRoute: boolean;
  providerProfiles: RuntimeProfileConfig[];
  row: OnnxRuntimeModelRowViewModel;
  starredModels: Set<string>;
  onOpenMetadata: (modelId: string, modelName: string) => void;
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
  isSavingRoute,
  providerProfiles,
  row,
  starredModels,
  onOpenMetadata,
  onSaveRoute,
  onToggleLink,
  onToggleStar,
}: OnnxRuntimeModelRowProps) {
  const isStarred = starredModels.has(row.model.id) || Boolean(row.model.starred);
  const isLinked = row.model.linkedApps?.includes('onnx-runtime') ?? false;
  const isExcluded = excludedModels.has(row.model.id);
  const [draftProfileId, setDraftProfileId] = useState(row.route?.profile_id ?? '');
  const hasDraftChange = draftProfileId !== (row.route?.profile_id ?? '');

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
            </div>
            <LocalModelMetadataSummary
              format={row.model.primaryFormat ?? row.model.format}
              quant={row.model.quant}
              size={row.model.size}
              hasDependencies={row.model.hasDependencies}
              dependencyCount={row.model.dependencyCount}
            />
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
