import React from 'react';
import { Star, ExternalLink } from 'lucide-react';
import type { ModelCategory } from '../types/apps';

interface ModelManagerProps {
  modelGroups: ModelCategory[];
  starredModels: Set<string>;
  linkedModels: Set<string>;
  onToggleStar: (modelId: string) => void;
  onToggleLink: (modelId: string) => void;
  selectedAppId: string | null;
}

export const ModelManager: React.FC<ModelManagerProps> = ({
  modelGroups,
  starredModels,
  linkedModels,
  onToggleStar,
  onToggleLink,
  selectedAppId,
}) => {
  return (
    <div className="flex-1 bg-[hsl(var(--launcher-bg-tertiary)/0.2)] overflow-hidden flex flex-col">
      <div className="flex-1 overflow-y-auto">
        <div className="p-3 space-y-4 px-0 py-0">
          {modelGroups.length === 0 ? (
            <div className="flex items-center justify-center h-64 text-[hsl(var(--launcher-text-muted))]">
              <p className="text-sm text-center">No models found. Add models to your library to get started.</p>
            </div>
          ) : (
            modelGroups.map((group) => (
              <div key={group.category}>
                <p className="text-xs font-semibold text-[hsl(var(--launcher-text-muted))] uppercase tracking-wider mb-2 px-2">
                  {group.category}
                </p>
                <div className="space-y-1.5">
                  {group.models.map((model) => {
                    const isStarred = starredModels.has(model.id);
                    const isLinked = linkedModels.has(model.id);
                    return (
                      <div
                        key={model.id}
                        className={`flex items-center justify-between p-2.5 rounded transition-colors group ${
                          isLinked
                            ? 'bg-[hsl(var(--launcher-bg-tertiary)/0.4)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.6)]'
                            : 'bg-[hsl(var(--launcher-bg-tertiary)/0.2)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.4)]'
                        }`}
                      >
                        <div className="flex items-center gap-2 flex-1">
                          <button
                            onClick={() => onToggleStar(model.id)}
                            className="text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-primary))] transition-colors"
                          >
                            <Star className="w-4 h-4" fill={isStarred ? 'currentColor' : 'none'} />
                          </button>
                          <span
                            className={`text-sm ${
                              isLinked
                                ? 'text-[hsl(var(--launcher-text-primary))]'
                                : 'text-[hsl(var(--launcher-text-secondary))]'
                            }`}
                          >
                            {model.name}
                          </span>
                        </div>
                        <button
                          onClick={() => onToggleLink(model.id)}
                          className={`transition-colors cursor-pointer ${
                            isLinked
                              ? 'text-[hsl(var(--launcher-accent-primary))] hover:text-[hsl(var(--launcher-accent-primary)/0.8)]'
                              : 'text-[hsl(var(--launcher-text-muted))] group-hover:text-[hsl(var(--launcher-accent-primary))]'
                          }`}
                          title={isLinked ? `Linked to ${selectedAppId || 'app'}` : 'Link to current app'}
                        >
                          <ExternalLink className="w-4 h-4" />
                        </button>
                      </div>
                    );
                  })}
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
};
