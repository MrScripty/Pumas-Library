/**
 * Local Models List Component
 *
 * Displays locally installed models grouped by category.
 * Extracted from ModelManager.tsx
 *
 * Ctrl+click on a model name opens its metadata modal.
 */

import { useMemo, useState } from 'react';
import type { ModelCategory, ModelInfo, RelatedModelsState } from '../types/apps';
import type { RouterProfileSyncStatus, ServedModelStatus } from '../types/api-serving';
import type { ServingControlObservation } from '../hooks/useServingStatus';
import { LocalModelGroupHeader } from './LocalModelGroupHeader';
import { LocalModelRow } from './LocalModelRow';
import { LocalModelsEmptyState } from './LocalModelsEmptyState';
import { ModelMetadataModal } from './ModelMetadataModal';

interface LocalModelsListProps {
  modelGroups: ModelCategory[];
  starredModels: Set<string>;
  excludedModels: Set<string>;
  onToggleStar: (modelId: string) => void;
  onToggleLink: (modelId: string) => void;
  selectedAppId: string | null;
  servedModels?: ServedModelStatus[];
  routerProfiles?: RouterProfileSyncStatus[];
  servingControlObservation?: ServingControlObservation;
  totalModels: number;
  hasFilters: boolean;
  onClearFilters?: () => void;
  relatedModelsById: Record<string, RelatedModelsState>;
  expandedRelated: Set<string>;
  onToggleRelated: (modelId: string) => void;
  onOpenRelatedUrl: (url: string) => void;
  onPauseDownload?: (downloadKey: string) => void;
  onResumeDownload?: (downloadKey: string) => void;
  onCancelDownload?: (downloadKey: string) => void;
  onRecoverPartialDownload?: (model: ModelInfo) => void;
  recoveringPartialModelIds?: Set<string>;
  downloadErrors?: Record<string, string>;
  onDeleteModel?: (modelId: string) => void;
  onConvertModel?: (model: ModelInfo) => void;
  onServeModel?: (model: ModelInfo) => void;
  onChooseExistingLibrary?: () => Promise<void> | void;
  isChoosingExistingLibrary?: boolean;
}

export function LocalModelsList({
  modelGroups,
  starredModels,
  excludedModels,
  onToggleStar,
  onToggleLink,
  selectedAppId,
  servedModels = [],
  routerProfiles = [],
  servingControlObservation = { kind: 'known', rows: [] },
  totalModels,
  hasFilters,
  onClearFilters,
  relatedModelsById,
  expandedRelated,
  onToggleRelated,
  onOpenRelatedUrl,
  onPauseDownload,
  onResumeDownload,
  onCancelDownload,
  onRecoverPartialDownload,
  recoveringPartialModelIds,
  downloadErrors,
  onDeleteModel,
  onConvertModel,
  onServeModel,
  onChooseExistingLibrary,
  isChoosingExistingLibrary = false,
}: LocalModelsListProps) {
  // State for metadata modal
  const [metadataModal, setMetadataModal] = useState<{
    modelId: string;
    modelName: string;
  } | null>(null);
  const servingStateByModel = useMemo(() => {
    const loadedById = new Map<string, ServedModelStatus>();
    const loadingIds = new Set<string>();
    const unavailableIds = new Set<string>();
    const unavailableProfiles = new Set(routerProfiles
      .filter((profile) => profile.observation_state !== 'current')
      .map((profile) => profile.profile_id));
    for (const status of servedModels) {
      if (servingControlObservation.kind === 'unavailable' || unavailableProfiles.has(status.profile_id)) {
        unavailableIds.add(status.model_id);
        continue;
      }
      if (status.load_state === 'loaded' && !loadedById.has(status.model_id)) {
        loadedById.set(status.model_id, status);
      }
      if (status.load_state === 'requested' || status.load_state === 'loading') {
        loadingIds.add(status.model_id);
      }
    }
    return { loadedById, loadingIds, unavailableIds };
  }, [routerProfiles, servedModels, servingControlObservation.kind]);

  const pendingProfiles = routerProfiles.filter((profile) => profile.catalog_state === 'pending');
  const unavailableProfiles = routerProfiles.filter((profile) => profile.observation_state !== 'current');
  const profileNotices = (
    <>
      {servingControlObservation.kind === 'unavailable' && (
        <div role="status" className="rounded border border-[hsl(var(--accent-warning)/0.35)] px-3 py-2 text-xs text-[hsl(var(--text-secondary))]">
          Serving status unavailable. The last observed model state is retained, but its load state cannot be confirmed.
        </div>
      )}
      {unavailableProfiles.map((profile) => (
        <div key={`unavailable:${profile.profile_id}`} role="status" className="rounded border border-[hsl(var(--accent-warning)/0.35)] px-3 py-2 text-xs text-[hsl(var(--text-secondary))]">
          Router status unavailable for profile {profile.profile_id}.
        </div>
      ))}
      {pendingProfiles.map((profile) => (
        <div key={`pending:${profile.profile_id}`} role="status" className="rounded border border-[hsl(var(--accent-warning)/0.35)] px-3 py-2 text-xs text-[hsl(var(--text-secondary))]">
          Library changes pending profile restart for {profile.profile_id}.
        </div>
      ))}
    </>
  );

  if (modelGroups.length === 0) {
    return (
      <>
        {profileNotices}
        <LocalModelsEmptyState
          totalModels={totalModels}
          hasFilters={hasFilters}
          onClearFilters={onClearFilters}
          onChooseExistingLibrary={onChooseExistingLibrary}
          isChoosingExistingLibrary={isChoosingExistingLibrary}
        />
      </>
    );
  }

  return (
    <>
      {profileNotices}
      {modelGroups.map((group: ModelCategory) => (
        <div key={group.category} className="space-y-2">
          <LocalModelGroupHeader
            category={group.category}
            modelCount={group.models.length}
          />
          <div className="space-y-1.5">
            {group.models.map((model) => (
              <LocalModelRow
                key={model.id}
                downloadErrors={downloadErrors}
                excludedModels={excludedModels}
                expandedRelated={expandedRelated}
                model={model}
                isLoading={servingStateByModel.loadingIds.has(model.id)}
                isServingUnavailable={servingStateByModel.unavailableIds.has(model.id)}
                recoveringPartialModelIds={recoveringPartialModelIds}
                relatedModelsById={relatedModelsById}
                selectedAppId={selectedAppId}
                servedStatus={servingStateByModel.loadedById.get(model.id) ?? null}
                starredModels={starredModels}
                onCancelDownload={onCancelDownload}
                onDeleteModel={onDeleteModel}
                onConvertModel={onConvertModel}
                onOpenMetadata={(modelId, modelName) => {
                  setMetadataModal({ modelId, modelName });
                }}
                onOpenRelatedUrl={onOpenRelatedUrl}
                onPauseDownload={onPauseDownload}
                onRecoverPartialDownload={onRecoverPartialDownload}
                onResumeDownload={onResumeDownload}
                onServeModel={onServeModel}
                onToggleLink={onToggleLink}
                onToggleRelated={onToggleRelated}
                onToggleStar={onToggleStar}
              />
            ))}
          </div>
        </div>
      ))}

      {/* Metadata Modal */}
      {metadataModal && (
        <ModelMetadataModal
          modelId={metadataModal.modelId}
          modelName={metadataModal.modelName}
          onClose={() => setMetadataModal(null)}
        />
      )}
    </>
  );
}
