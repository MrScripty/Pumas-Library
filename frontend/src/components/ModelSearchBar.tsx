/**
 * Model Search Bar Component
 *
 * Search input and filter controls for local/remote models.
 * Extracted from ModelManager.tsx
 */

import { useRef } from 'react';
import { Search, Filter, Globe, Folder, Import, Key } from 'lucide-react';
import { Popover } from './ui';

interface ModelSearchBarProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  isDownloadMode: boolean;
  onToggleMode: () => void;
  isCategoryFiltered: boolean;
  onFilterClick: () => void;
  totalModels: number | null;
  hasActiveDownloads?: boolean;
  showCategoryMenu: boolean;
  filterList: string[];
  selectedFilter: string;
  onSelectFilter: (filter: string) => void;
  onOpenModelsRoot?: () => void;
  onImportModels?: () => void;
  isPickingModels?: boolean;
  onHfAuthClick?: () => void;
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
  hasActiveDownloads = false,
  showCategoryMenu,
  filterList,
  selectedFilter,
  onSelectFilter,
  onOpenModelsRoot,
  onImportModels,
  isPickingModels = false,
  onHfAuthClick,
  showModeToggle = true,
}: ModelSearchBarProps) {
  const initialFilterFocusRef = useRef<HTMLButtonElement>(null);
  const filterLabel = isDownloadMode ? 'Filter by model kind' : 'Filter by category';

  const handleFilterOpenChange = (nextIsOpen: boolean) => {
    if (nextIsOpen !== showCategoryMenu) {
      onFilterClick();
    }
  };

  return (
    <div className="border-b border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary))]">
      <div className="p-4 pb-3">
        {/* Search and Filters */}
        <div className="relative flex items-center gap-2">
          <Popover
            isOpen={showCategoryMenu}
            label={filterLabel}
            onOpenChange={handleFilterOpenChange}
            initialFocusRef={initialFilterFocusRef}
            contentClassName="absolute left-0 top-full mt-2 w-48 rounded border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-overlay))] shadow-[0_12px_24px_hsl(var(--launcher-bg-primary)/0.6)] z-10"
            trigger={(triggerProps) => (
              <button
                type="button"
                {...triggerProps}
                className={`p-1.5 rounded transition-colors ${
                  isCategoryFiltered
                    ? 'text-[hsl(var(--launcher-accent-primary))]'
                    : 'text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))]'
                }`}
                title={filterLabel}
                aria-label={filterLabel}
              >
                <Filter className="w-4 h-4" />
              </button>
            )}
          >
            {filterList.map((item) => {
              const isSelected = selectedFilter === item;
              return (
                <button
                  key={item}
                  ref={isSelected ? initialFilterFocusRef : undefined}
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
          </Popover>
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[hsl(var(--launcher-text-muted))]" />
            <input
              type="text"
              placeholder={isDownloadMode ? 'Search Hugging Face models' : totalModels === null ? 'Search library models' : `Search ${totalModels} models`}
              value={searchQuery}
              onChange={(e) => onSearchChange(e.target.value)}
              className="w-full pl-9 pr-16 py-2 text-sm bg-[hsl(var(--launcher-bg-primary))] border border-[hsl(var(--launcher-border))] rounded text-[hsl(var(--launcher-text-primary))] placeholder:text-[hsl(var(--launcher-text-muted))] focus:outline-none focus:border-[hsl(var(--launcher-accent-primary))] transition-colors"
            />
            <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
              {onImportModels && !isDownloadMode && (
                <button
                  type="button"
                  onClick={onImportModels}
                  disabled={isPickingModels}
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
              {onHfAuthClick && isDownloadMode && (
                <button
                  type="button"
                  onClick={onHfAuthClick}
                  className="p-1 rounded text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))] transition-colors"
                  title="HuggingFace authentication"
                  aria-label="HuggingFace authentication"
                >
                  <Key className="w-4 h-4" />
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
                  <span className="relative flex h-4 w-4 items-center justify-center">
                    {hasActiveDownloads && <span className="download-scan-ring" />}
                    <Globe className="w-4 h-4" />
                  </span>
                </button>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
