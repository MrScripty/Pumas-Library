import React, { useEffect, useMemo, useState } from 'react';
import { Star, ExternalLink, Search, Folder, Download, Filter, HardDrive, Calendar, Tag } from 'lucide-react';
import type { ModelCategory, RemoteModelInfo } from '../types/apps';

interface ModelManagerProps {
  modelGroups: ModelCategory[];
  starredModels: Set<string>;
  linkedModels: Set<string>;
  onToggleStar: (modelId: string) => void;
  onToggleLink: (modelId: string) => void;
  selectedAppId: string | null;
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
  onAddModels,
  onOpenModelsRoot,
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('all');
  const [showCategoryMenu, setShowCategoryMenu] = useState(false);
  const [isDownloadMode, setIsDownloadMode] = useState(false);
  const [selectedKind, setSelectedKind] = useState<string>('all');
  const [remoteResults, setRemoteResults] = useState<RemoteModelInfo[]>([]);
  const [remoteError, setRemoteError] = useState<string | null>(null);
  const [isRemoteLoading, setIsRemoteLoading] = useState(false);

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

    return groups;
  }, [modelGroups, searchQuery, selectedCategory, starredModels]);

  // Count total models
  const totalModels = useMemo(() => {
    return modelGroups.reduce((sum: number, group: ModelCategory) => sum + group.models.length, 0);
  }, [modelGroups]);
  const isCategoryFiltered = isDownloadMode ? selectedKind !== 'all' : selectedCategory !== 'all';

  const remoteKinds = useMemo(() => {
    const kinds = new Set<string>();
    remoteResults.forEach((model) => {
      if (model.kind && model.kind !== 'unknown') {
        kinds.add(model.kind);
      }
    });
    return ['all', ...Array.from(kinds).sort()];
  }, [remoteResults]);

  const filteredRemoteResults = useMemo(() => {
    if (selectedKind === 'all') {
      return remoteResults;
    }
    return remoteResults.filter((model) => model.kind === selectedKind);
  }, [remoteResults, selectedKind]);

  useEffect(() => {
    if (!isDownloadMode) {
      return;
    }

    const trimmedQuery = searchQuery.trim();
    if (!trimmedQuery) {
      setRemoteResults([]);
      setRemoteError(null);
      setIsRemoteLoading(false);
      return;
    }

    let isActive = true;
    const handle = setTimeout(async () => {
      if (!window.pywebview?.api?.search_hf_models) {
        if (isActive) {
          setRemoteError('Hugging Face search is unavailable.');
          setRemoteResults([]);
          setIsRemoteLoading(false);
        }
        return;
      }

      setIsRemoteLoading(true);
      setRemoteError(null);
      try {
        const result = await window.pywebview.api.search_hf_models(trimmedQuery, null, 25);
        if (!isActive) {
          return;
        }
        if (result.success) {
          setRemoteResults(result.models as RemoteModelInfo[]);
        } else {
          setRemoteError(result.error || 'Search failed.');
          setRemoteResults([]);
        }
      } catch (err) {
        if (!isActive) {
          return;
        }
        const message = err instanceof Error ? err.message : 'Search failed.';
        setRemoteError(message);
        setRemoteResults([]);
      } finally {
        if (isActive) {
          setIsRemoteLoading(false);
        }
      }
    }, 300);

    return () => {
      isActive = false;
      clearTimeout(handle);
    };
  }, [isDownloadMode, searchQuery]);

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

  const openRemoteUrl = (url: string) => {
    if (window.pywebview?.api?.open_url) {
      void window.pywebview.api.open_url(url);
      return;
    }
    window.open(url, '_blank', 'noopener');
  };

  return (
    <div className="flex-1 bg-[hsl(var(--launcher-bg-tertiary)/0.2)] overflow-hidden flex flex-col">
      {/* Header */}
      <div className="border-b border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary))]">
        <div className="p-4 pb-3">
          {/* Search and Filters */}
          <div className="relative flex items-center gap-2">
            <button
              type="button"
              onClick={() => setShowCategoryMenu((prev) => !prev)}
              className={`p-1.5 rounded transition-colors ${
                isCategoryFiltered
                  ? 'text-[hsl(var(--launcher-accent-primary))]'
                  : 'text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))]'
              }`}
              title={isDownloadMode ? 'Filter by model kind' : 'Filter by category'}
              aria-label={isDownloadMode ? 'Filter by model kind' : 'Filter by category'}
              aria-expanded={showCategoryMenu}
            >
              <Filter className="w-4 h-4" />
            </button>
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[hsl(var(--launcher-text-muted))]" />
              <input
                type="text"
                placeholder={isDownloadMode ? 'Search Hugging Face models' : `Search ${totalModels} models`}
                value={searchQuery}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => setSearchQuery(e.target.value)}
                className="w-full pl-9 pr-16 py-2 text-sm bg-[hsl(var(--launcher-bg-primary))] border border-[hsl(var(--launcher-border))] rounded text-[hsl(var(--launcher-text-primary))] placeholder:text-[hsl(var(--launcher-text-muted))] focus:outline-none focus:border-[hsl(var(--launcher-accent-primary))] transition-colors"
              />
              <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
                {onOpenModelsRoot && (
                  <button
                    onClick={onOpenModelsRoot}
                    className="p-1 rounded text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))] transition-colors"
                    title="Open models folder"
                    aria-label="Open models folder"
                  >
                    <Folder className="w-4 h-4" />
                  </button>
                )}
                {onAddModels && (
                  <button
                    onClick={() => {
                      setIsDownloadMode((prev) => !prev);
                      setShowCategoryMenu(false);
                    }}
                    className={`p-1 rounded transition-colors ${
                      isDownloadMode
                        ? 'text-[hsl(var(--launcher-accent-primary))]'
                        : 'text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))]'
                    }`}
                    title={isDownloadMode ? 'Exit download mode' : 'Search Hugging Face models'}
                    aria-label={isDownloadMode ? 'Exit download mode' : 'Search Hugging Face models'}
                    aria-pressed={isDownloadMode}
                  >
                    <Download className="w-4 h-4" />
                  </button>
                )}
              </div>
            </div>
            {showCategoryMenu && (
              <div className="absolute left-0 top-full mt-2 w-48 rounded border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-overlay))] shadow-[0_12px_24px_hsl(var(--launcher-bg-primary)/0.6)] z-10">
                {(isDownloadMode ? remoteKinds : categories).map((cat: string) => {
                  const isSelected = isDownloadMode ? selectedKind === cat : selectedCategory === cat;
                  return (
                    <button
                      key={cat}
                      type="button"
                      onClick={() => {
                        if (isDownloadMode) {
                          setSelectedKind(cat);
                        } else {
                          setSelectedCategory(cat);
                        }
                        setShowCategoryMenu(false);
                      }}
                      className={`w-full px-3 py-2 text-left text-xs transition-colors ${
                        isSelected
                          ? 'text-[hsl(var(--launcher-accent-primary))] bg-[hsl(var(--launcher-bg-tertiary)/0.6)]'
                          : 'text-[hsl(var(--launcher-text-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)]'
                      }`}
                    >
                      {cat === 'all'
                        ? isDownloadMode
                          ? 'All Kinds'
                          : 'All Categories'
                        : cat}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Model List */}
      <div className="flex-1 overflow-y-auto">
        {isDownloadMode ? (
          <div className="p-4 space-y-3">
            {isRemoteLoading && (
              <div className="flex items-center gap-2 text-xs text-[hsl(var(--launcher-text-muted))]">
                <Search className="w-3.5 h-3.5" />
                <span>Searching Hugging Face...</span>
              </div>
            )}
            {remoteError && (
              <div className="text-xs text-[hsl(var(--launcher-accent-error))]">
                {remoteError}
              </div>
            )}
            {!isRemoteLoading && !remoteError && filteredRemoteResults.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-64 text-[hsl(var(--launcher-text-muted))]">
                <Search className="w-10 h-10 mb-3 opacity-50" />
                <p className="text-sm text-center">
                  {searchQuery.trim()
                    ? 'No Hugging Face models match your search.'
                    : 'Type to search Hugging Face models.'}
                </p>
                {(searchQuery.trim() || selectedKind !== 'all') && (
                  <button
                    onClick={() => {
                      setSearchQuery('');
                      setSelectedKind('all');
                    }}
                    className="mt-2 text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline"
                  >
                    Clear filters
                  </button>
                )}
              </div>
            ) : (
              !isRemoteLoading &&
              !remoteError &&
              filteredRemoteResults.map((model) => (
                <div
                  key={model.repoId}
                  className="rounded transition-colors bg-[hsl(var(--launcher-bg-tertiary)/0.2)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.35)]"
                >
                  <div className="flex items-start justify-between gap-3 p-3">
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-semibold text-[hsl(var(--launcher-text-primary))] truncate">
                          {model.name}
                        </span>
                        <span className="text-xs text-[hsl(var(--launcher-text-muted))]">
                          {model.kind}
                        </span>
                      </div>
                      <p className="text-xs text-[hsl(var(--launcher-text-muted))] mt-0.5 truncate">
                        {model.repoId}
                      </p>
                      <div className="flex flex-wrap gap-3 mt-2 text-xs text-[hsl(var(--launcher-text-muted))]">
                        <span>Developer: {model.developer || 'Unknown'}</span>
                        <span>
                          Format: {model.formats.length ? model.formats.join(', ') : 'Unknown'}
                        </span>
                        <span>Quant: {model.quants.length ? model.quants.join(', ') : 'Unknown'}</span>
                      </div>
                    </div>
                    <button
                      onClick={() => openRemoteUrl(model.url)}
                      className="flex-shrink-0 text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-accent-primary))] transition-colors"
                      title={`Open ${model.url}`}
                      aria-label={`Open ${model.url}`}
                    >
                      <ExternalLink className="w-4 h-4" />
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        ) : (
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
        )}
      </div>
    </div>
  );
};
