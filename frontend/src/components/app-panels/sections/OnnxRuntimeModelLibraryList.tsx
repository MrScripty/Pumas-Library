import { useState } from 'react';
import { Search } from 'lucide-react';
import type { RuntimeProfileConfig } from '../../../types/api-runtime-profiles';
import type { ServingEndpointStatus } from '../../../types/api-serving';
import { OnnxRuntimeModelRow } from './OnnxRuntimeModelRow';
import type { OnnxRuntimeModelRowViewModel } from './onnxRuntimeLibraryViewModels';

export interface OnnxRuntimeModelLibraryListProps {
  excludedModels: Set<string>;
  providerProfiles: RuntimeProfileConfig[];
  quickServeFeedback: {
    kind: 'error' | 'success';
    message: string;
    modelId: string;
  } | null;
  quickServeModelId: string | null;
  routeError: string | null;
  rows: OnnxRuntimeModelRowViewModel[];
  savingRouteModelId: string | null;
  servingEndpoint: ServingEndpointStatus | null;
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

function endpointModeLabel(endpoint: ServingEndpointStatus | null): string | null {
  if (!endpoint) {
    return null;
  }

  switch (endpoint.endpoint_mode) {
    case 'pumas_gateway':
      return 'Pumas gateway';
    case 'provider_endpoint':
      return 'Provider endpoint';
    case 'not_configured':
      return 'Not configured';
  }
  return null;
}

export function OnnxRuntimeModelLibraryList({
  excludedModels,
  providerProfiles,
  quickServeFeedback,
  quickServeModelId,
  routeError,
  rows,
  savingRouteModelId,
  servingEndpoint,
  starredModels,
  onOpenMetadata,
  onOpenServeOptions,
  onQuickServe,
  onSaveRoute,
  onToggleLink,
  onToggleStar,
}: OnnxRuntimeModelLibraryListProps) {
  const [searchQuery, setSearchQuery] = useState('');
  const filteredRows = filterRows(rows, searchQuery);
  const endpointLabel = endpointModeLabel(servingEndpoint);

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center justify-between border-b border-[hsl(var(--border-subtle))] px-4 py-3">
        <div className="min-w-0">
          <h2 className="text-sm font-semibold text-[hsl(var(--text-primary))]">
            ONNX Runtime Library
          </h2>
          <div className="text-xs text-[hsl(var(--text-muted))]">
            {rows.length} compatible local model{rows.length === 1 ? '' : 's'}
            {endpointLabel ? ` - ${endpointLabel}` : ''}
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
                isQuickServing={quickServeModelId === row.model.id}
                isSavingRoute={savingRouteModelId === row.model.id}
                providerProfiles={providerProfiles}
                quickServeFeedback={
                  quickServeFeedback?.modelId === row.model.id ? quickServeFeedback : null
                }
                row={row}
                starredModels={starredModels}
                onOpenMetadata={onOpenMetadata}
                onOpenServeOptions={onOpenServeOptions}
                onQuickServe={onQuickServe}
                onSaveRoute={onSaveRoute}
                onToggleLink={onToggleLink}
                onToggleStar={onToggleStar}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
