/**
 * Local Models List Component
 *
 * Displays locally installed models grouped by category.
 * Extracted from ModelManager.tsx
 */

import type { CSSProperties } from 'react';
import {
  Star,
  HardDrive,
  Calendar,
  Tag,
  ChevronDown,
  ChevronRight,
  ExternalLink,
  Download,
} from 'lucide-react';
import type { ModelCategory, RelatedModelsState } from '../types/apps';
import { formatSize, formatDate } from '../utils/modelFormatters';
import { ModelKindIcon } from './ModelKindIcon';

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
}: LocalModelsListProps) {
  if (modelGroups.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-64 text-[hsl(var(--launcher-text-muted))]">
        <HardDrive className="w-12 h-12 mb-3 opacity-50" />
        <p className="text-sm text-center">
          {totalModels === 0
            ? 'No models found. Add models to your library to get started.'
            : 'No models match your filters.'}
        </p>
        {totalModels > 0 && hasFilters && onClearFilters && (
          <button
            onClick={onClearFilters}
            className="mt-2 text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline"
          >
            Clear filters
          </button>
        )}
      </div>
    );
  }

  return (
    <>
      {modelGroups.map((group: ModelCategory) => (
        <div key={group.category} className="space-y-2">
          <div className="flex items-center gap-2 px-1">
            <Tag className="w-3.5 h-3.5 text-[hsl(var(--launcher-text-muted))]" />
            <p className="text-xs font-semibold text-[hsl(var(--launcher-text-muted))] uppercase tracking-wider">
              {group.category}
            </p>
            <span className="text-xs text-[hsl(var(--launcher-text-muted))]">
              ({group.models.length})
            </span>
          </div>
          <div className="space-y-1.5">
            {group.models.map((model) => {
              const isStarred = starredModels.has(model.id);
              const isLinked = linkedModels.has(model.id);
              const isDownloading = Boolean(model.isDownloading);
              const isExpanded = expandedRelated.has(model.id);
              const relatedState = relatedModelsById[model.id];
              const relatedModels = relatedState?.models ?? [];
              const relatedStatus = relatedState?.status ?? 'idle';
              const canShowRelated = Boolean(model.relatedAvailable) && !isDownloading;
              const isLoadingRelated = relatedStatus === 'loading' || relatedStatus === 'idle';
              const progressValue = Math.min(1, Math.max(0, model.downloadProgress ?? 0));
              const isQueued = model.downloadStatus === 'queued';
              const progressDegrees = Math.round(progressValue * 360);
              const ringDegrees = isQueued ? 60 : Math.min(360, Math.max(0, progressDegrees));
              return (
                <div
                  key={model.id}
                  className={`rounded transition-colors group ${
                    isLinked
                      ? 'bg-[hsl(var(--launcher-bg-tertiary)/0.4)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.6)]'
                      : 'bg-[hsl(var(--launcher-bg-tertiary)/0.2)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.4)]'
                  }`}
                >
                  {/* Main row */}
                  <div className="flex items-center justify-between p-2.5">
                    <div className="flex items-center gap-2 flex-1 min-w-0">
                      <button
                        onClick={() => onToggleStar(model.id)}
                        disabled={isDownloading}
                        className={`flex-shrink-0 transition-colors ${
                          isDownloading
                            ? 'text-[hsl(var(--launcher-text-muted))] opacity-50 cursor-not-allowed'
                            : 'text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-accent-primary))]'
                        }`}
                      >
                        <Star className="w-4 h-4" fill={isStarred ? 'currentColor' : 'none'} />
                      </button>
                      <div className="flex-1 min-w-0">
                        <span
                          className={`text-sm font-medium block truncate ${
                            isDownloading
                              ? 'text-[hsl(var(--launcher-text-muted))]'
                              : isLinked
                              ? 'text-[hsl(var(--launcher-text-primary))]'
                              : 'text-[hsl(var(--launcher-text-secondary))]'
                          }`}
                        >
                          {model.name}
                        </span>
                        {/* Metadata row */}
                        <div className="flex items-center gap-3 mt-1 text-xs text-[hsl(var(--launcher-text-muted))]">
                          {model.size && (
                            <span className="flex items-center gap-1">
                              <HardDrive className="w-3 h-3" />
                              {formatSize(model.size)}
                            </span>
                          )}
                          {model.date && (
                            <span className="flex items-center gap-1">
                              <Calendar className="w-3 h-3" />
                              {formatDate(model.date)}
                            </span>
                          )}
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      {canShowRelated && (
                        <button
                          onClick={() => onToggleRelated(model.id)}
                          className={`flex-shrink-0 transition-colors ${
                            isExpanded
                              ? 'text-[hsl(var(--launcher-accent-primary))]'
                              : 'text-[hsl(var(--launcher-text-muted))] group-hover:text-[hsl(var(--launcher-accent-primary))]'
                          }`}
                          title={isExpanded ? 'Hide related models' : 'Show related models'}
                          aria-label={isExpanded ? 'Hide related models' : 'Show related models'}
                          aria-expanded={isExpanded}
                        >
                          {isExpanded ? (
                            <ChevronDown className="w-4 h-4" />
                          ) : (
                            <ChevronRight className="w-4 h-4" />
                          )}
                        </button>
                      )}
                      {isDownloading ? (
                        <div className="relative flex h-4 w-4 items-center justify-center text-[hsl(var(--launcher-text-muted))]">
                          <span
                            className={`download-progress-ring ${isQueued ? 'is-waiting' : ''}`}
                            style={
                              {
                                '--progress': `${ringDegrees}deg`,
                              } as CSSProperties
                            }
                          />
                          {!isQueued && <span className="download-scan-ring" />}
                          <Download className="h-3.5 w-3.5" />
                        </div>
                      ) : (
                        <button
                          onClick={() => onToggleLink(model.id)}
                          className={`flex-shrink-0 transition-colors cursor-pointer ${
                            isLinked
                              ? 'text-[hsl(var(--launcher-accent-primary))] hover:text-[hsl(var(--launcher-accent-primary)/0.8)]'
                              : 'text-[hsl(var(--launcher-text-muted))] group-hover:text-[hsl(var(--launcher-accent-primary))]'
                          }`}
                          title={isLinked ? `Linked to ${selectedAppId || 'app'}` : 'Link to current app'}
                        >
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
                        </button>
                      )}
                    </div>
                  </div>
                  {canShowRelated && isExpanded && (
                    <div className="border-t border-[hsl(var(--launcher-border))] px-3 py-2 space-y-2">
                      <div className="flex items-center justify-between text-[11px] uppercase tracking-wider text-[hsl(var(--launcher-text-muted))]">
                        <span>Related models</span>
                        {relatedModels.length > 0 && (
                          <span>{relatedModels.length}</span>
                        )}
                      </div>
                      {isLoadingRelated && (
                        <div className="text-xs text-[hsl(var(--launcher-text-muted))]">
                          Looking up related models...
                        </div>
                      )}
                      {relatedStatus === 'error' && (
                        <div className="text-xs text-[hsl(var(--launcher-accent-error))]">
                          {relatedState?.error || 'Related models unavailable.'}
                        </div>
                      )}
                      {!isLoadingRelated && relatedStatus !== 'error' && relatedModels.length === 0 && (
                        <div className="text-xs text-[hsl(var(--launcher-text-muted))]">
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
                                  <span className="text-xs font-semibold text-[hsl(var(--launcher-text-primary))] truncate">
                                    {related.name}
                                  </span>
                                  <span
                                    className="inline-flex items-center gap-1 text-[hsl(var(--launcher-text-muted))]"
                                    title={related.kind}
                                    aria-label={related.kind}
                                  >
                                    <ModelKindIcon kind={related.kind} />
                                  </span>
                                </div>
                                <span className="text-[11px] text-[hsl(var(--launcher-text-muted))] truncate">
                                  {related.developer}
                                </span>
                              </div>
                              <button
                                onClick={() => onOpenRelatedUrl(related.url)}
                                className="flex-shrink-0 text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-accent-primary))] transition-colors"
                                title={`Open ${related.url}`}
                                aria-label={`Open ${related.url}`}
                              >
                                <ExternalLink className="w-3.5 h-3.5" />
                              </button>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </>
  );
}
