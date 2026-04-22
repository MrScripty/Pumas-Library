/**
 * Local Models List Component
 *
 * Displays locally installed models grouped by category.
 * Extracted from ModelManager.tsx
 *
 * Ctrl+click on a model name opens its metadata modal.
 */

import { useState, type CSSProperties } from 'react';
import {
  Star,
  ChevronDown,
  ChevronRight,
  Download,
  Pause,
  ArrowRightLeft,
} from 'lucide-react';
import type { ModelCategory, ModelInfo, RelatedModelsState } from '../types/apps';
import { IconButton, HoldToDeleteButton, ListItem, ListItemContent } from './ui';
import { LocalModelGroupHeader } from './LocalModelGroupHeader';
import { LocalModelsEmptyState } from './LocalModelsEmptyState';
import { LocalModelMetadataSummary } from './LocalModelMetadataSummary';
import { LocalModelNameButton } from './LocalModelNameButton';
import { ModelMetadataModal } from './ModelMetadataModal';
import { RelatedModelsPanel } from './RelatedModelsPanel';

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
  onPauseDownload?: (repoId: string) => void;
  onResumeDownload?: (repoId: string) => void;
  onCancelDownload?: (repoId: string) => void;
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
            {group.models.map((model) => {
              const isStarred = starredModels.has(model.id);
              const isPartialDownload = Boolean(model.isPartialDownload);
              const isLinked = !excludedModels.has(model.id);
              const isDownloading = Boolean(model.isDownloading);
              const isConvertible = !isDownloading && !isPartialDownload && Boolean(model.primaryFormat);
              const isExpanded = expandedRelated.has(model.id);
              const relatedState = relatedModelsById[model.id];
              const relatedModels = relatedState?.models ?? [];
              const relatedStatus = relatedState?.status ?? 'idle';
              const canShowRelated = Boolean(model.relatedAvailable) && !isDownloading && !isPartialDownload;
              const progressValue = Math.min(1, Math.max(0, model.downloadProgress ?? 0));
              const isQueued = model.downloadStatus === 'queued';
              const isPaused = model.downloadStatus === 'paused';
              const isActiveDownload = ['queued', 'downloading', 'pausing', 'cancelling'].includes(model.downloadStatus ?? '');
              const progressDegrees = Math.round(progressValue * 360);
              const ringDegrees = isQueued ? 60 : Math.min(360, Math.max(0, progressDegrees));
              const partialRepoId = model.repoId ?? model.downloadRepoId;
              const partialError = partialRepoId ? downloadErrors?.[partialRepoId] : undefined;
              const isRecoveringPartial = Boolean(partialRepoId && recoveringPartialRepoIds?.has(partialRepoId));
              const canPause = isDownloading && (model.downloadStatus === 'downloading' || model.downloadStatus === 'queued') && Boolean(onPauseDownload) && Boolean(model.downloadRepoId);
              const canResume = isDownloading && (isPaused || model.downloadStatus === 'error') && Boolean(onResumeDownload) && Boolean(model.downloadRepoId);
              const canRecoverPartial = !isDownloading
                && isPartialDownload
                && Boolean(onRecoverPartialDownload)
                && Boolean(model.repoId)
                && Boolean(model.modelDir);
              return (
                <ListItem key={model.id} highlighted={isLinked}>
                  {/* Main row */}
                  <ListItemContent>
                    <div className="flex items-center gap-2 flex-1 min-w-0">
                      <IconButton
                        icon={<Star fill={isStarred ? 'currentColor' : 'none'} />}
                        tooltip={isStarred ? 'Unstar' : 'Star'}
                        onClick={() => onToggleStar(model.id)}
                        disabled={isDownloading}
                        size="sm"
                      />
                      <div className="flex-1 min-w-0">
                        <LocalModelNameButton
                          modelId={model.id}
                          modelName={model.name}
                          isDownloading={isDownloading}
                          isPartialDownload={isPartialDownload}
                          isLinked={isLinked}
                          wasDequantized={model.wasDequantized}
                          hasIntegrityIssue={Boolean(model.hasIntegrityIssue)}
                          integrityIssueMessage={model.integrityIssueMessage}
                          onOpenMetadata={(modelId, modelName) => {
                            setMetadataModal({ modelId, modelName });
                          }}
                        />
                        <LocalModelMetadataSummary
                          format={model.format}
                          quant={model.quant}
                          size={model.size}
                          hasDependencies={model.hasDependencies}
                          dependencyCount={model.dependencyCount}
                          partialError={partialError}
                        />
                      </div>
                    </div>
                    <div className="flex items-center gap-1">
                      {canShowRelated && (
                        <IconButton
                          icon={isExpanded ? <ChevronDown /> : <ChevronRight />}
                          tooltip={isExpanded ? 'Hide related' : 'Show related'}
                          onClick={() => onToggleRelated(model.id)}
                          size="sm"
                          active={isExpanded}
                        />
                      )}
                      {isDownloading ? (
                        <>
                        <button
                          className={`relative flex h-6 w-6 items-center justify-center rounded-md border-0 bg-transparent ${
                            canResume || canPause
                              ? 'cursor-pointer'
                              : 'cursor-default'
                          } ${canResume ? 'download-resume-btn' : ''} text-[hsl(var(--text-muted))]`}
                          title={canPause ? 'Pause download' : isPaused ? 'Resume download' : model.downloadStatus === 'error' ? 'Retry download' : undefined}
                          onClick={
                            canPause
                              ? () => onPauseDownload!(model.downloadRepoId!)
                              : canResume
                              ? () => onResumeDownload!(model.downloadRepoId!)
                              : undefined
                          }
                        >
                          {isActiveDownload && (
                            <>
                              <span
                                className={`download-progress-ring ${isQueued ? 'is-waiting' : ''} ${isPaused ? 'is-paused' : ''}`}
                                style={
                                  {
                                    '--progress': `${ringDegrees}deg`,
                                  } as CSSProperties
                                }
                              />
                              {!isQueued && !isPaused && <span className="download-scan-ring" />}
                            </>
                          )}
                          {canPause ? (
                            <>
                              <Download className="h-3.5 w-3.5 group-hover:hidden" />
                              <Pause className="h-3.5 w-3.5 hidden group-hover:block" />
                            </>
                          ) : (
                            <Download className="h-3.5 w-3.5" />
                          )}
                        </button>
                        {onCancelDownload && model.downloadRepoId && (
                          <HoldToDeleteButton
                            onDelete={() => onCancelDownload(model.downloadRepoId!)}
                            tooltip="Hold to remove download"
                          />
                        )}
                        </>
                      ) : (
                        <>
                          <IconButton
                            icon={
                              <svg
                                width="16"
                                height="16"
                                viewBox="0 0 16 16"
                                fill="none"
                                stroke="currentColor"
                                strokeWidth="1.5"
                                strokeLinecap="round"
                                strokeLinejoin="round"
                              >
                                <path d="M7 3.5L9 1.5C10.1 0.4 11.9 0.4 13 1.5C14.1 2.6 14.1 4.4 13 5.5L11 7.5" />
                                <path d="M9 12.5L7 14.5C5.9 15.6 4.1 15.6 3 14.5C1.9 13.4 1.9 11.6 3 10.5L5 8.5" />
                                <path d="M10 6L6 10" />
                              </svg>
                            }
                            tooltip={isLinked ? `Linked to ${selectedAppId || 'app'}` : `Excluded from ${selectedAppId || 'app'}`}
                            onClick={() => onToggleLink(model.id)}
                            disabled={isPartialDownload}
                            size="sm"
                            active={isLinked}
                            className={isLinked ? 'text-[hsl(var(--accent-success))]' : 'opacity-40'}
                          />
                          {canRecoverPartial && (
                            <IconButton
                              icon={<Download />}
                              tooltip={isRecoveringPartial ? 'Resuming partial download...' : 'Resume partial download'}
                              onClick={isRecoveringPartial ? undefined : () => onRecoverPartialDownload!(model)}
                              disabled={isRecoveringPartial}
                              size="sm"
                            />
                          )}
                          {isConvertible && onConvertModel && (
                            <IconButton
                              icon={<ArrowRightLeft />}
                              tooltip={
                                model.primaryFormat === 'safetensors'
                                  ? 'Convert / Quantize'
                                  : 'Convert / Re-quantize'
                              }
                              onClick={() => onConvertModel(model.id)}
                              size="sm"
                            />
                          )}
                          {onDeleteModel && (
                            <HoldToDeleteButton
                              onDelete={() => onDeleteModel(model.id)}
                            />
                          )}
                        </>
                      )}
                    </div>
                  </ListItemContent>
                  {canShowRelated && isExpanded && (
                    <RelatedModelsPanel
                      error={relatedState?.error}
                      relatedModels={relatedModels}
                      relatedStatus={relatedStatus}
                      onOpenRelatedUrl={onOpenRelatedUrl}
                    />
                  )}
                </ListItem>
              );
            })}
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
