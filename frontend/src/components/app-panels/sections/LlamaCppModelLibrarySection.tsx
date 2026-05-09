import { useState } from 'react';
import { Link2, Star } from 'lucide-react';
import type { ModelCategory, ModelInfo } from '../../../types/apps';
import type { ServedModelStatus } from '../../../types/api-serving';
import { IconButton, ListItem, ListItemContent } from '../../ui';
import { LocalModelMetadataSummary } from '../../LocalModelMetadataSummary';
import { LocalModelNameButton } from '../../LocalModelNameButton';
import { ModelMetadataModal } from '../../ModelMetadataModal';
import {
  buildLlamaCppModelRows,
  type LlamaCppModelRowViewModel,
} from './llamaCppLibraryViewModels';

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

function LlamaCppModelRow({
  excludedModels,
  row,
  starredModels,
  onOpenMetadata,
  onToggleLink,
  onToggleStar,
}: {
  excludedModels: Set<string>;
  row: LlamaCppModelRowViewModel;
  starredModels: Set<string>;
  onOpenMetadata: (modelId: string, modelName: string) => void;
  onToggleLink: (modelId: string) => void;
  onToggleStar: (modelId: string) => void;
}) {
  const isStarred = starredModels.has(row.model.id) || Boolean(row.model.starred);
  const isLinked = row.model.linkedApps?.includes('llama-cpp') ?? false;
  const isExcluded = excludedModels.has(row.model.id);
  const servedCount = row.servedStatuses.filter((status) => status.load_state === 'loaded').length;

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
              <span className="rounded bg-[hsl(var(--surface-low)/0.55)] px-1.5 py-0.5 text-[10px] font-medium uppercase text-[hsl(var(--text-secondary))]">
                {getPlacementLabel(row)}
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
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2 pt-0.5">
          <span className="max-w-32 truncate text-xs text-[hsl(var(--text-muted))]">
            {getRouteLabel(row)}
          </span>
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
  const [metadataModal, setMetadataModal] = useState<{
    modelId: string;
    modelName: string;
  } | null>(null);
  const rows = buildLlamaCppModelRows({
    modelGroups,
    profiles: [],
    routes: [],
    servedStatuses: servedModels,
  });

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
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          {rows.length === 0 ? (
            <div className="rounded border border-[hsl(var(--border-subtle))] bg-[hsl(var(--surface-low)/0.18)] px-4 py-6 text-sm text-[hsl(var(--text-muted))]">
              No local GGUF models are available for llama.cpp.
            </div>
          ) : (
            <div className="space-y-1.5">
              {rows.map((row) => (
                <LlamaCppModelRow
                  key={row.model.id}
                  excludedModels={excludedModels}
                  row={row}
                  starredModels={starredModels}
                  onOpenMetadata={(modelId, modelName) => {
                    setMetadataModal({ modelId, modelName });
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
