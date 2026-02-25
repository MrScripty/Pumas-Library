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
  HardDrive,
  Calendar,
  Tag,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  Download,
  Pause,
  ArrowRightLeft,
  AlertTriangle,
  X,
} from 'lucide-react';
import type { ModelCategory, RelatedModelsState } from '../types/apps';
import type { InterruptedDownloadInfo } from '../types/api';
import { formatSize, formatDate } from '../utils/modelFormatters';
import { ModelKindIcon } from './ModelKindIcon';
import { EmptyState, IconButton, HoldToDeleteButton, ListItem, ListItemContent, MetadataRow, MetadataItem } from './ui';
import { ModelMetadataModal } from './ModelMetadataModal';

interface LocalModelsListProps {
  modelGroups: ModelCategory[];
  starredModels: Set<string>;
  linkedModels: Set<string>;
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
  onDeleteModel?: (modelId: string) => void;
  onConvertModel?: (modelId: string) => void;
  interruptedDownloads?: InterruptedDownloadInfo[];
  onRecoverDownload?: (repoId: string, destDir: string) => void;
  onDismissInterrupted?: (destDir: string) => void;
}

export function LocalModelsList({
  modelGroups,
  starredModels,
  linkedModels,
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
  onDeleteModel,
  onConvertModel,
  interruptedDownloads,
  onRecoverDownload,
  onDismissInterrupted,
}: LocalModelsListProps) {
  // State for metadata modal
  const [metadataModal, setMetadataModal] = useState<{
    modelId: string;
    modelName: string;
  } | null>(null);

  // State for interrupted download recovery (repo_id inputs per dest_dir)
  const [recoveryInputs, setRecoveryInputs] = useState<Record<string, string>>({});

  // Handle ctrl+click on model name to open metadata
  const handleModelNameClick = (
    e: React.MouseEvent,
    modelId: string,
    modelName: string
  ) => {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      e.stopPropagation();
      setMetadataModal({ modelId, modelName });
    }
  };

  if (modelGroups.length === 0) {
    return (
      <EmptyState
        icon={<HardDrive />}
        message={totalModels === 0
          ? 'No models found. Add models to your library to get started.'
          : 'No models match your filters.'}
        action={totalModels > 0 && hasFilters && onClearFilters ? {
          label: 'Clear filters',
          onClick: onClearFilters,
        } : undefined}
      />
    );
  }

  return (
    <>
      {/* Interrupted downloads recovery */}
      {interruptedDownloads && interruptedDownloads.length > 0 && (
        <div className="space-y-2 mb-4">
          <div className="flex items-center gap-2 px-1">
            <AlertTriangle className="w-3.5 h-3.5 text-[hsl(var(--launcher-accent-warning))]" />
            <p className="text-xs font-semibold text-[hsl(var(--launcher-accent-warning))] uppercase tracking-wider">
              Interrupted Downloads
            </p>
          </div>
          <div className="space-y-2">
            {interruptedDownloads.map((item) => {
              const suggestion = `${item.family}/${item.inferred_name.replace(/ /g, '-')}`;
              const inputValue = recoveryInputs[item.model_dir] ?? suggestion;
              return (
                <div
                  key={item.model_dir}
                  className="rounded-lg border border-[hsl(var(--launcher-accent-warning)/0.3)] bg-[hsl(var(--launcher-accent-warning)/0.05)] p-3"
                >
                  <div className="flex items-start justify-between gap-2">
                    <div className="flex-1 min-w-0">
                      <p className="text-sm font-medium text-[hsl(var(--text-primary))] truncate">
                        {item.family}/{item.inferred_name}
                      </p>
                      <p className="text-xs text-[hsl(var(--text-muted))] mt-0.5">
                        {item.part_files.map((f) => f.replace('.part', '')).join(', ')}
                        {item.completed_files.length > 0 && ` + ${item.completed_files.length} completed`}
                      </p>
                    </div>
                    {onDismissInterrupted && (
                      <button
                        className="p-1 rounded hover:bg-[hsl(var(--launcher-bg-tertiary))] text-[hsl(var(--text-muted))]"
                        onClick={() => onDismissInterrupted(item.model_dir)}
                        title="Dismiss"
                      >
                        <X className="w-3.5 h-3.5" />
                      </button>
                    )}
                  </div>
                  {onRecoverDownload && (
                    <div className="flex items-center gap-2 mt-2">
                      <input
                        type="text"
                        value={inputValue}
                        onChange={(e) =>
                          setRecoveryInputs((prev) => ({ ...prev, [item.model_dir]: e.target.value }))
                        }
                        placeholder="owner/repo-name"
                        className="flex-1 text-xs px-2 py-1.5 rounded border border-[hsl(var(--launcher-border))]
                          bg-[hsl(var(--launcher-bg-secondary))] text-[hsl(var(--text-primary))]
                          placeholder:text-[hsl(var(--text-muted))] focus:outline-none
                          focus:border-[hsl(var(--launcher-accent-primary))]"
                      />
                      <button
                        className="text-xs px-3 py-1.5 rounded font-medium
                          bg-[hsl(var(--launcher-accent-primary))] text-white
                          hover:bg-[hsl(var(--launcher-accent-primary)/0.8)]
                          disabled:opacity-50 disabled:cursor-not-allowed"
                        disabled={!inputValue.includes('/')}
                        onClick={() => onRecoverDownload(inputValue, item.model_dir)}
                      >
                        Resume
                      </button>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {modelGroups.map((group: ModelCategory) => (
        <div key={group.category} className="space-y-2">
          <div className="flex items-center gap-2 px-1">
            <Tag className="w-3.5 h-3.5 text-[hsl(var(--text-muted))]" />
            <p className="text-xs font-semibold text-[hsl(var(--text-muted))] uppercase tracking-wider">
              {group.category}
            </p>
            <span className="text-xs text-[hsl(var(--text-muted))]">
              ({group.models.length})
            </span>
          </div>
          <div className="space-y-1.5">
            {group.models.map((model) => {
              const isStarred = starredModels.has(model.id);
              const isLinked = linkedModels.has(model.id);
              const isDownloading = Boolean(model.isDownloading);
              const isConvertible = !isDownloading && Boolean(model.primaryFormat);
              const isExpanded = expandedRelated.has(model.id);
              const relatedState = relatedModelsById[model.id];
              const relatedModels = relatedState?.models ?? [];
              const relatedStatus = relatedState?.status ?? 'idle';
              const canShowRelated = Boolean(model.relatedAvailable) && !isDownloading;
              const isLoadingRelated = relatedStatus === 'loading' || relatedStatus === 'idle';
              const progressValue = Math.min(1, Math.max(0, model.downloadProgress ?? 0));
              const isQueued = model.downloadStatus === 'queued';
              const isPaused = model.downloadStatus === 'paused';
              const progressDegrees = Math.round(progressValue * 360);
              const ringDegrees = isQueued ? 60 : Math.min(360, Math.max(0, progressDegrees));
              const canPause = isDownloading && (model.downloadStatus === 'downloading' || model.downloadStatus === 'queued') && Boolean(onPauseDownload) && Boolean(model.downloadRepoId);
              const canResume = isDownloading && (isPaused || model.downloadStatus === 'error') && Boolean(onResumeDownload) && Boolean(model.downloadRepoId);
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
                        <span
                          className={`text-sm font-medium block truncate cursor-pointer ${
                            isDownloading
                              ? 'text-[hsl(var(--text-muted))]'
                              : isLinked
                              ? 'text-[hsl(var(--text-primary))]'
                              : 'text-[hsl(var(--text-secondary))]'
                          }`}
                          onClick={(e) => handleModelNameClick(e, model.id, model.name)}
                          onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleModelNameClick(e as unknown as React.MouseEvent, model.id, model.name); }}
                          role="button"
                          tabIndex={0}
                          title="Ctrl+click to view metadata"
                        >
                          {model.name}
                          {model.wasDequantized && (
                            <span
                              className="ml-1.5 inline-flex items-center px-1.5 py-0.5 text-[10px] font-medium rounded
                                bg-[hsl(var(--launcher-accent-warning)/0.15)]
                                text-[hsl(var(--launcher-accent-warning))]"
                              title="Dequantized from quantized GGUF - may have reduced precision"
                            >
                              DQ
                            </span>
                          )}
                        </span>
                        {/* Metadata row */}
                        <MetadataRow>
                          {model.size && (
                            <MetadataItem icon={<HardDrive />}>
                              {formatSize(model.size)}
                            </MetadataItem>
                          )}
                          {model.date && (
                            <MetadataItem icon={<Calendar />}>
                              {formatDate(model.date)}
                            </MetadataItem>
                          )}
                        </MetadataRow>
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
                          <span
                            className={`download-progress-ring ${isQueued ? 'is-waiting' : ''} ${isPaused ? 'is-paused' : ''}`}
                            style={
                              {
                                '--progress': `${ringDegrees}deg`,
                              } as CSSProperties
                            }
                          />
                          {!isQueued && !isPaused && <span className="download-scan-ring" />}
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
                            tooltip={isLinked ? `Linked to ${selectedAppId || 'app'}` : 'Link to app'}
                            onClick={() => onToggleLink(model.id)}
                            size="sm"
                            active={isLinked}
                          />
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
                    <div className="border-t border-[hsl(var(--launcher-border))] px-3 py-2 space-y-2">
                      <div className="flex items-center justify-between text-[11px] uppercase tracking-wider text-[hsl(var(--text-muted))]">
                        <span>Related models</span>
                        {relatedModels.length > 0 && (
                          <span>{relatedModels.length}</span>
                        )}
                      </div>
                      {isLoadingRelated && (
                        <div className="text-xs text-[hsl(var(--text-muted))]">
                          Looking up related models...
                        </div>
                      )}
                      {relatedStatus === 'error' && (
                        <div className="text-xs text-[hsl(var(--launcher-accent-error))]">
                          {relatedState?.error || 'Related models unavailable.'}
                        </div>
                      )}
                      {!isLoadingRelated && relatedStatus !== 'error' && relatedModels.length === 0 && (
                        <div className="text-xs text-[hsl(var(--text-muted))]">
                          No related models found.
                        </div>
                      )}
                      {relatedModels.length > 0 && (
                        <div className="space-y-1.5">
                          {relatedModels.map((related) => (
                            <div
                              key={related.repoId}
                              className="flex items-center justify-between rounded bg-[hsl(var(--launcher-bg-tertiary)/0.2)] px-2 py-1.5"
                            >
                              <div className="min-w-0">
                                <div className="flex items-center gap-2">
                                  <span className="text-xs font-semibold text-[hsl(var(--text-primary))] truncate">
                                    {related.name}
                                  </span>
                                  <span
                                    className="inline-flex items-center gap-1 text-[hsl(var(--text-muted))]"
                                    title={related.kind}
                                    aria-label={related.kind}
                                  >
                                    <ModelKindIcon kind={related.kind} />
                                  </span>
                                </div>
                                <span className="text-[11px] text-[hsl(var(--text-muted))] truncate">
                                  {related.developer}
                                </span>
                              </div>
                              <IconButton
                                icon={<ExternalLink />}
                                tooltip="Open"
                                onClick={() => onOpenRelatedUrl(related.url)}
                                size="sm"
                              />
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
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
