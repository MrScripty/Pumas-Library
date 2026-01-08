/**
 * Local Models List Component
 *
 * Displays locally installed models grouped by category.
 * Extracted from ModelManager.tsx
 */

import { Star, HardDrive, Calendar, Tag } from 'lucide-react';
import type { ModelCategory } from '../types/apps';
import { formatSize, formatDate } from '../utils/modelFormatters';

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
                        className="flex-shrink-0 text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-accent-primary))] transition-colors"
                      >
                        <Star className="w-4 h-4" fill={isStarred ? 'currentColor' : 'none'} />
                      </button>
                      <div className="flex-1 min-w-0">
                        <span
                          className={`text-sm font-medium block truncate ${
                            isLinked
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
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </>
  );
}
