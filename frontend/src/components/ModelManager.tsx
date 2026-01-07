import React, { useState, useMemo } from 'react';
import { Star, ExternalLink, Search, Folder, FolderPlus, RefreshCw, Filter, HardDrive, Calendar, Tag } from 'lucide-react';
import type { ModelCategory } from '../types/apps';

interface ModelManagerProps {
  modelGroups: ModelCategory[];
  starredModels: Set<string>;
  linkedModels: Set<string>;
  onToggleStar: (modelId: string) => void;
  onToggleLink: (modelId: string) => void;
  selectedAppId: string | null;
  onScanModels?: () => void;
  onAddModels?: () => void;
  onOpenModelsRoot?: () => void;
}

export const ModelManager: React.FC<ModelManagerProps> = ({
  modelGroups,
  starredModels,
  linkedModels,
  onToggleStar,
  onToggleLink,
  selectedAppId,
  onScanModels,
  onAddModels,
  onOpenModelsRoot,
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('all');
  const [showStarredOnly, setShowStarredOnly] = useState(false);

  // Get all unique categories
  const categories = useMemo(() => {
    const cats = modelGroups.map((g: ModelCategory) => g.category);
    return ['all', ...cats];
  }, [modelGroups]);

  // Filter models based on search and filters
  const filteredGroups = useMemo(() => {
    let groups = modelGroups;

    // Filter by category
    if (selectedCategory !== 'all') {
      groups = groups.filter((g: ModelCategory) => g.category === selectedCategory);
    }

    // Filter by search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      groups = groups.map((group: ModelCategory) => ({
        ...group,
        models: group.models.filter(model =>
          model.name.toLowerCase().includes(query) ||
          model.path?.toLowerCase().includes(query)
        ),
      })).filter((group: ModelCategory) => group.models.length > 0);
    }

    // Filter by starred
    if (showStarredOnly) {
      groups = groups.map((group: ModelCategory) => ({
        ...group,
        models: group.models.filter(model => starredModels.has(model.id)),
      })).filter((group: ModelCategory) => group.models.length > 0);
    }

    return groups;
  }, [modelGroups, searchQuery, selectedCategory, showStarredOnly, starredModels]);

  // Count total models
  const totalModels = useMemo(() => {
    return modelGroups.reduce((sum: number, group: ModelCategory) => sum + group.models.length, 0);
  }, [modelGroups]);

  // Format file size
  const formatSize = (bytes?: number): string => {
    if (!bytes) return 'Unknown';
    const gb = bytes / (1024 ** 3);
    if (gb >= 1) return `${gb.toFixed(2)} GB`;
    const mb = bytes / (1024 ** 2);
    return `${mb.toFixed(2)} MB`;
  };

  // Format date
  const formatDate = (dateStr?: string): string => {
    if (!dateStr) return 'Unknown';
    try {
      return new Date(dateStr).toLocaleDateString();
    } catch {
      return 'Unknown';
    }
  };

  return (
    <div className="flex-1 bg-[hsl(var(--launcher-bg-tertiary)/0.2)] overflow-hidden flex flex-col">
      {/* Header */}
      <div className="border-b border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary))]">
        <div className="p-4 pb-3">
          <div className="flex items-center justify-between mb-3">
            <div>
              <h2 className="text-lg font-semibold text-[hsl(var(--launcher-text-primary))]">Model Library</h2>
              <p className="text-xs text-[hsl(var(--launcher-text-muted))] mt-0.5">
                {totalModels} {totalModels === 1 ? 'model' : 'models'} in your collection
              </p>
            </div>
            <div className="flex gap-2">
              {onOpenModelsRoot && (
                <button
                  onClick={onOpenModelsRoot}
                  className="p-2 text-xs rounded bg-[hsl(var(--launcher-bg-tertiary))] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.8)] text-[hsl(var(--launcher-text-secondary))] transition-colors"
                  title="Open models folder"
                  aria-label="Open models folder"
                >
                  <Folder className="w-3.5 h-3.5" />
                </button>
              )}
              <button
                onClick={onScanModels}
                className="px-3 py-1.5 text-xs rounded bg-[hsl(var(--launcher-bg-tertiary))] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.8)] text-[hsl(var(--launcher-text-secondary))] transition-colors flex items-center gap-1.5"
                title="Scan for new models"
              >
                <RefreshCw className="w-3.5 h-3.5" />
                Scan
              </button>
              <button
                onClick={onAddModels}
                className="px-3 py-1.5 text-xs rounded bg-[hsl(var(--launcher-accent-primary))] hover:bg-[hsl(var(--launcher-accent-primary)/0.9)] text-white transition-colors flex items-center gap-1.5"
                title="Add models from folder"
              >
                <FolderPlus className="w-3.5 h-3.5" />
                Add Models
              </button>
            </div>
          </div>

          {/* Search and Filters */}
          <div className="space-y-2">
            {/* Search Bar */}
            <div className="relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[hsl(var(--launcher-text-muted))]" />
              <input
                type="text"
                placeholder="Search models..."
                value={searchQuery}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => setSearchQuery(e.target.value)}
                className="w-full pl-9 pr-3 py-2 text-sm bg-[hsl(var(--launcher-bg-primary))] border border-[hsl(var(--launcher-border))] rounded text-[hsl(var(--launcher-text-primary))] placeholder:text-[hsl(var(--launcher-text-muted))] focus:outline-none focus:border-[hsl(var(--launcher-accent-primary))] transition-colors"
              />
            </div>

            {/* Filter Bar */}
            <div className="flex items-center gap-2">
              <Filter className="w-4 h-4 text-[hsl(var(--launcher-text-muted))]" />
              <select
                value={selectedCategory}
                onChange={(e: React.ChangeEvent<HTMLSelectElement>) => setSelectedCategory(e.target.value)}
                className="px-2 py-1 text-xs bg-[hsl(var(--launcher-bg-primary))] border border-[hsl(var(--launcher-border))] rounded text-[hsl(var(--launcher-text-secondary))] focus:outline-none focus:border-[hsl(var(--launcher-accent-primary))] transition-colors"
              >
                {categories.map((cat: string) => (
                  <option key={cat} value={cat}>
                    {cat === 'all' ? 'All Categories' : cat}
                  </option>
                ))}
              </select>
              <button
                onClick={() => setShowStarredOnly(!showStarredOnly)}
                className={`px-2 py-1 text-xs rounded border transition-colors flex items-center gap-1 ${
                  showStarredOnly
                    ? 'bg-[hsl(var(--launcher-accent-primary))] border-[hsl(var(--launcher-accent-primary))] text-white'
                    : 'bg-[hsl(var(--launcher-bg-primary))] border-[hsl(var(--launcher-border))] text-[hsl(var(--launcher-text-secondary))]'
                }`}
              >
                <Star className="w-3 h-3" fill={showStarredOnly ? 'currentColor' : 'none'} />
                Starred Only
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Model List */}
      <div className="flex-1 overflow-y-auto">
        <div className="p-4 space-y-4">
          {filteredGroups.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-64 text-[hsl(var(--launcher-text-muted))]">
              <HardDrive className="w-12 h-12 mb-3 opacity-50" />
              <p className="text-sm text-center">
                {totalModels === 0
                  ? 'No models found. Add models to your library to get started.'
                  : 'No models match your filters.'}
              </p>
              {totalModels > 0 && (
                <button
                  onClick={() => {
                    setSearchQuery('');
                    setSelectedCategory('all');
                    setShowStarredOnly(false);
                  }}
                  className="mt-2 text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline"
                >
                  Clear filters
                </button>
              )}
            </div>
          ) : (
            filteredGroups.map((group: ModelCategory) => (
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
                            <ExternalLink className="w-4 h-4" />
                          </button>
                        </div>
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
