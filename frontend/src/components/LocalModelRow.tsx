import { Star } from 'lucide-react';
import type { ModelInfo, RelatedModelsState } from '../types/apps';
import { IconButton, ListItem, ListItemContent } from './ui';
import { LocalModelMetadataSummary } from './LocalModelMetadataSummary';
import { LocalModelNameButton } from './LocalModelNameButton';
import { LocalModelRowActions } from './LocalModelRowActions';
import { getLocalModelRowState } from './LocalModelRowState';
import { RelatedModelsPanel } from './RelatedModelsPanel';

interface LocalModelRowProps {
  downloadErrors?: Record<string, string>;
  excludedModels: Set<string>;
  expandedRelated: Set<string>;
  model: ModelInfo;
  recoveringPartialRepoIds?: Set<string>;
  relatedModelsById: Record<string, RelatedModelsState>;
  selectedAppId: string | null;
  starredModels: Set<string>;
  onCancelDownload?: (repoId: string) => void;
  onConvertModel?: (modelId: string) => void;
  onDeleteModel?: (modelId: string) => void;
  onOpenMetadata: (modelId: string, modelName: string) => void;
  onOpenRelatedUrl: (url: string) => void;
  onPauseDownload?: (repoId: string) => void;
  onRecoverPartialDownload?: (model: ModelInfo) => void;
  onResumeDownload?: (repoId: string) => void;
  onServeModel?: (model: ModelInfo) => void;
  onToggleLink: (modelId: string) => void;
  onToggleRelated: (modelId: string) => void;
  onToggleStar: (modelId: string) => void;
}

export function LocalModelRow({
  downloadErrors,
  excludedModels,
  expandedRelated,
  model,
  recoveringPartialRepoIds,
  relatedModelsById,
  selectedAppId,
  starredModels,
  onCancelDownload,
  onConvertModel,
  onDeleteModel,
  onOpenMetadata,
  onOpenRelatedUrl,
  onPauseDownload,
  onRecoverPartialDownload,
  onResumeDownload,
  onServeModel,
  onToggleLink,
  onToggleRelated,
  onToggleStar,
}: LocalModelRowProps) {
  const rowState = getLocalModelRowState({
    canConvertModel: Boolean(onConvertModel),
    canPauseDownload: Boolean(onPauseDownload),
    canRecoverDownload: Boolean(onRecoverPartialDownload),
    canResumeDownload: Boolean(onResumeDownload),
    downloadErrors,
    excludedModels,
    expandedRelated,
    model,
    recoveringPartialRepoIds,
    relatedModelsById,
    starredModels,
  });

  return (
    <ListItem highlighted={rowState.isLinked}>
      <ListItemContent>
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <IconButton
            icon={<Star fill={rowState.isStarred ? 'currentColor' : 'none'} />}
            tooltip={rowState.isStarred ? 'Unstar' : 'Star'}
            onClick={() => onToggleStar(model.id)}
            disabled={rowState.isDownloading}
            size="sm"
          />
          <div className="flex-1 min-w-0">
            <LocalModelNameButton
              modelId={model.id}
              modelName={model.name}
              isDownloading={rowState.isDownloading}
              isPartialDownload={rowState.isPartialDownload}
              isLinked={rowState.isLinked}
              wasDequantized={model.wasDequantized}
              hasIntegrityIssue={Boolean(model.hasIntegrityIssue)}
              integrityIssueMessage={model.integrityIssueMessage}
              onOpenMetadata={onOpenMetadata}
            />
            <LocalModelMetadataSummary
              format={model.format}
              quant={model.quant}
              size={model.size}
              hasDependencies={model.hasDependencies}
              dependencyCount={model.dependencyCount}
              partialError={rowState.partialError}
            />
          </div>
        </div>
        <LocalModelRowActions
          model={model}
          rowState={rowState}
          selectedAppId={selectedAppId}
          onCancelDownload={onCancelDownload}
          onConvertModel={onConvertModel}
          onDeleteModel={onDeleteModel}
          onPauseDownload={onPauseDownload}
          onRecoverPartialDownload={onRecoverPartialDownload}
          onResumeDownload={onResumeDownload}
          onServeModel={onServeModel}
          onToggleLink={onToggleLink}
          onToggleRelated={onToggleRelated}
        />
      </ListItemContent>
      {rowState.canShowRelated && rowState.isExpanded && (
        <RelatedModelsPanel
          error={rowState.relatedState?.error}
          relatedModels={rowState.relatedModels}
          relatedStatus={rowState.relatedStatus}
          onOpenRelatedUrl={onOpenRelatedUrl}
        />
      )}
    </ListItem>
  );
}
