/**
 * Local Models List Component
 *
 * Displays locally installed models grouped by category.
 * Extracted from ModelManager.tsx
 *
 * Ctrl+click on a model name opens its metadata modal.
 */

import { useState } from 'react';
import type { ModelCategory, ModelInfo, RelatedModelsState } from '../types/apps';
import { LocalModelGroupHeader } from './LocalModelGroupHeader';
import { LocalModelRow } from './LocalModelRow';
import { LocalModelsEmptyState } from './LocalModelsEmptyState';
import { ModelMetadataModal } from './ModelMetadataModal';
import { ModelServeDialog } from './ModelServeDialog';

interface LocalModelsListProps {
  modelGroups: ModelCategory[];
  starredModels: Set<string>;
  excludedModels: Set<string>;
  onToggleStar: (modelId: string) => void;
  onToggleLink: (modelId: string) => void;
  selectedAppId: string | null;
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
  recoveringPartialRepoIds?: Set<string>;
  downloadErrors?: Record<string, string>;
  onDeleteModel?: (modelId: string) => void;
  onConvertModel?: (modelId: string) => void;
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
  recoveringPartialRepoIds,
  downloadErrors,
  onDeleteModel,
  onConvertModel,
  onChooseExistingLibrary,
  isChoosingExistingLibrary = false,
}: LocalModelsListProps) {
  // State for metadata modal
  const [metadataModal, setMetadataModal] = useState<{
    modelId: string;
    modelName: string;
  } | null>(null);
  const [servingModel, setServingModel] = useState<ModelInfo | null>(null);

  if (modelGroups.length === 0) {
    return (
      <LocalModelsEmptyState
        totalModels={totalModels}
        hasFilters={hasFilters}
        onClearFilters={onClearFilters}
        onChooseExistingLibrary={onChooseExistingLibrary}
        isChoosingExistingLibrary={isChoosingExistingLibrary}
      />
    );
  }

  return (
    <>
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
                recoveringPartialRepoIds={recoveringPartialRepoIds}
                relatedModelsById={relatedModelsById}
                selectedAppId={selectedAppId}
                starredModels={starredModels}
                onCancelDownload={onCancelDownload}
                onConvertModel={onConvertModel}
                onDeleteModel={onDeleteModel}
                onOpenMetadata={(modelId, modelName) => {
                  setMetadataModal({ modelId, modelName });
                }}
                onOpenRelatedUrl={onOpenRelatedUrl}
                onPauseDownload={onPauseDownload}
                onRecoverPartialDownload={onRecoverPartialDownload}
                onResumeDownload={onResumeDownload}
                onServeModel={setServingModel}
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
      {servingModel && (
        <ModelServeDialog
          model={servingModel}
          onClose={() => setServingModel(null)}
        />
      )}
    </>
  );
}
