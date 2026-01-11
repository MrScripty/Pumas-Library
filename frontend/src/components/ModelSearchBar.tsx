/**
 * Model Search Bar Component
 *
 * Search input and filter controls for local/remote models.
 * Extracted from ModelManager.tsx
 */

import { Search, Filter, Globe, Folder, Import } from 'lucide-react';

interface ModelSearchBarProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  isDownloadMode: boolean;
  onToggleMode: () => void;
  isCategoryFiltered: boolean;
  onFilterClick: () => void;
  totalModels: number;
  showCategoryMenu: boolean;
  filterList: string[];
  selectedFilter: string;
  onSelectFilter: (filter: string) => void;
  onOpenModelsRoot?: () => void;
  onImportModels?: () => void;
  showModeToggle?: boolean;
}

export function ModelSearchBar({
  searchQuery,
  onSearchChange,
  isDownloadMode,
  onToggleMode,
  isCategoryFiltered,
  onFilterClick,
  totalModels,
  showCategoryMenu,
  filterList,
  selectedFilter,
  onSelectFilter,
  onOpenModelsRoot,
  onImportModels,
  showModeToggle = true,
}: ModelSearchBarProps) {
  return (
    <div className="border-b border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary))]">
      <div className="p-4 pb-3">
        {/* Search and Filters */}
        <div className="relative flex items-center gap-2">
          <button
            type="button"
            onClick={onFilterClick}
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
              onChange={(e) => onSearchChange(e.target.value)}
              className="w-full pl-9 pr-16 py-2 text-sm bg-[hsl(var(--launcher-bg-primary))] border border-[hsl(var(--launcher-border))] rounded text-[hsl(var(--launcher-text-primary))] placeholder:text-[hsl(var(--launcher-text-muted))] focus:outline-none focus:border-[hsl(var(--launcher-accent-primary))] transition-colors"
            />
            <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
              {onImportModels && !isDownloadMode && (
                <button
                  type="button"
                  onClick={onImportModels}
                  className="p-1 rounded text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))] transition-colors"
                  title="Import models"
                  aria-label="Import models"
                >
                  <Import className="w-4 h-4" />
                </button>
              )}
              {onOpenModelsRoot && (
                <button
                  type="button"
                  onClick={onOpenModelsRoot}
                  className="p-1 rounded text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))] transition-colors"
                  title="Open models folder"
                  aria-label="Open models folder"
                >
                  <Folder className="w-4 h-4" />
                </button>
              )}
              {showModeToggle && (
                <button
                  type="button"
                  onClick={onToggleMode}
                  className={`p-1 rounded transition-colors ${
                    isDownloadMode
                      ? 'text-[hsl(var(--launcher-accent-primary))]'
                      : 'text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))]'
                  }`}
                  title={isDownloadMode ? 'Exit download mode' : 'Search Hugging Face models'}
                  aria-label={isDownloadMode ? 'Exit download mode' : 'Search Hugging Face models'}
                  aria-pressed={isDownloadMode}
                >
                  <Globe className="w-4 h-4" />
                </button>
              )}
            </div>
          </div>
          {showCategoryMenu && (
            <div className="absolute left-0 top-full mt-2 w-48 rounded border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-overlay))] shadow-[0_12px_24px_hsl(var(--launcher-bg-primary)/0.6)] z-10">
              {filterList.map((item) => {
                const isSelected = selectedFilter === item;
                return (
                  <button
                    key={item}
                    type="button"
                    onClick={() => onSelectFilter(item)}
                    className={`w-full px-3 py-2 text-left text-xs transition-colors ${
                      isSelected
                        ? 'text-[hsl(var(--launcher-accent-primary))] bg-[hsl(var(--launcher-bg-tertiary)/0.6)]'
                        : 'text-[hsl(var(--launcher-text-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)]'
                    }`}
                  >
                    {item === 'all'
                      ? isDownloadMode
                        ? 'All Kinds'
                        : 'All Categories'
                      : item}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
