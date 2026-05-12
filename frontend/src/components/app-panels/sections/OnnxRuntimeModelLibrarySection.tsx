import { useState } from 'react';
import { Search } from 'lucide-react';
import { useRuntimeProfiles } from '../../../hooks/useRuntimeProfiles';
import type { ModelCategory } from '../../../types/apps';
import { ModelMetadataModal } from '../../ModelMetadataModal';
import {
  clearModelRuntimeRoute,
  saveModelRuntimeRoute,
} from '../../model-serve/runtimeRouteMutations';
import {
  buildOnnxRuntimeModelRows,
  type OnnxRuntimeModelRowViewModel,
} from './onnxRuntimeLibraryViewModels';
import { OnnxRuntimeModelRow } from './OnnxRuntimeModelRow';

const ONNX_RUNTIME_PROVIDER = 'onnx_runtime';

export interface OnnxRuntimeModelLibrarySectionProps {
  excludedModels: Set<string>;
  modelGroups: ModelCategory[];
  starredModels: Set<string>;
  onToggleLink: (modelId: string) => void;
  onToggleStar: (modelId: string) => void;
}

function filterRows(
  rows: OnnxRuntimeModelRowViewModel[],
  searchQuery: string
): OnnxRuntimeModelRowViewModel[] {
  const normalizedSearchQuery = searchQuery.trim().toLowerCase();
  if (!normalizedSearchQuery) {
    return rows;
  }

  return rows.filter((row) =>
    [row.model.name, row.model.id, row.model.category].join(' ').toLowerCase()
      .includes(normalizedSearchQuery)
  );
}

export function OnnxRuntimeModelLibrarySection({
  excludedModels,
  modelGroups,
  starredModels,
  onToggleLink,
  onToggleStar,
}: OnnxRuntimeModelLibrarySectionProps) {
  const { profiles, routes, refreshRuntimeProfiles } = useRuntimeProfiles();
  const [metadataModal, setMetadataModal] = useState<{
    modelId: string;
    modelName: string;
  } | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [savingRouteModelId, setSavingRouteModelId] = useState<string | null>(null);
  const [routeError, setRouteError] = useState<string | null>(null);
  const providerProfiles = profiles.filter((profile) => profile.provider === ONNX_RUNTIME_PROVIDER);
  const rows = buildOnnxRuntimeModelRows({ modelGroups, profiles, routes });
  const filteredRows = filterRows(rows, searchQuery);

  const handleSaveRoute = async (modelId: string, profileId: string) => {
    setSavingRouteModelId(modelId);
    setRouteError(null);
    try {
      if (profileId) {
        await saveModelRuntimeRoute({
          provider: ONNX_RUNTIME_PROVIDER,
          modelId,
          profileId,
          autoLoad: true,
        });
      } else {
        await clearModelRuntimeRoute(ONNX_RUNTIME_PROVIDER, modelId);
      }
      await refreshRuntimeProfiles();
    } catch (caught) {
      setRouteError(caught instanceof Error ? caught.message : 'Failed to save runtime route');
    } finally {
      setSavingRouteModelId(null);
    }
  };

  return (
    <section className="min-h-0 flex-1 overflow-hidden bg-[hsl(var(--launcher-bg-tertiary)/0.2)]">
      <div className="flex h-full flex-col">
        <div className="flex shrink-0 items-center justify-between border-b border-[hsl(var(--border-subtle))] px-4 py-3">
          <div className="min-w-0">
            <h2 className="text-sm font-semibold text-[hsl(var(--text-primary))]">
              ONNX Runtime Library
            </h2>
            <div className="text-xs text-[hsl(var(--text-muted))]">
              {rows.length} compatible local model{rows.length === 1 ? '' : 's'}
            </div>
          </div>
          <label className="relative min-w-44 max-w-64 flex-1">
            <span className="sr-only">Search ONNX Runtime models</span>
            <Search className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[hsl(var(--text-muted))]" />
            <input
              value={searchQuery}
              onChange={(event) => setSearchQuery(event.target.value)}
              placeholder="Search models"
              className="h-8 w-full rounded border border-[hsl(var(--border-subtle))] bg-[hsl(var(--surface-high))] pl-7 pr-2 text-xs text-[hsl(var(--text-primary))] placeholder:text-[hsl(var(--text-muted))]"
            />
          </label>
        </div>
        {routeError && (
          <div className="mx-4 mt-3 rounded border border-[hsl(var(--accent-error)/0.35)] bg-[hsl(var(--accent-error)/0.12)] px-3 py-2 text-xs text-[hsl(var(--accent-error))]">
            {routeError}
          </div>
        )}
        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          {rows.length === 0 ? (
            <div className="rounded border border-[hsl(var(--border-subtle))] bg-[hsl(var(--surface-low)/0.18)] px-4 py-6 text-sm text-[hsl(var(--text-muted))]">
              No local ONNX models are available for ONNX Runtime.
            </div>
          ) : filteredRows.length === 0 ? (
            <div className="rounded border border-[hsl(var(--border-subtle))] bg-[hsl(var(--surface-low)/0.18)] px-4 py-6 text-sm text-[hsl(var(--text-muted))]">
              No compatible ONNX Runtime models match the current search.
            </div>
          ) : (
            <div className="space-y-1.5">
              {filteredRows.map((row) => (
                <OnnxRuntimeModelRow
                  key={row.model.id}
                  excludedModels={excludedModels}
                  isSavingRoute={savingRouteModelId === row.model.id}
                  providerProfiles={providerProfiles}
                  row={row}
                  starredModels={starredModels}
                  onOpenMetadata={(modelId, modelName) => {
                    setMetadataModal({ modelId, modelName });
                  }}
                  onSaveRoute={handleSaveRoute}
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
