/**
 * Model Manager Component (Refactored)
 *
 * Main component for managing local and remote models.
 * Reduced from 986 lines to ~280 lines by extracting hooks and components.
 */

import React, { useState, useMemo } from 'react';
import type { ModelCategory } from '../types/apps';
import { useRemoteModelSearch } from '../hooks/useRemoteModelSearch';
import { useModelDownloads } from '../hooks/useModelDownloads';
import { ModelSearchBar } from './ModelSearchBar';
import { LocalModelsList } from './LocalModelsList';
import { RemoteModelsList } from './RemoteModelsList';
import { getReleaseTimestamp } from '../utils/modelFormatters';
import { getLogger } from '../utils/logger';
import { APIError, NetworkError } from '../errors';

const logger = getLogger('ModelManager');

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
  // UI State
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('all');
  const [selectedKind, setSelectedKind] = useState<string>('all');
  const [showCategoryMenu, setShowCategoryMenu] = useState(false);
  const [isDownloadMode, setIsDownloadMode] = useState(false);

  // Custom Hooks
  const { results: remoteResults, kinds: remoteKinds, error: remoteError, isLoading: isRemoteLoading } =
    useRemoteModelSearch({
      enabled: isDownloadMode,
      searchQuery,
    });

  const {
    downloadStatusByRepo,
    downloadError,
    downloadRepoId,
    startDownload,
    cancelDownload,
    setDownloadError,
    setDownloadRepoId,
  } = useModelDownloads();

  // Computed Values
  const categories = useMemo(() => {
    const cats = modelGroups.map((g: ModelCategory) => g.category);
    return ['all', ...cats];
  }, [modelGroups]);

  const totalModels = useMemo(() => {
    return modelGroups.reduce((sum: number, group: ModelCategory) => sum + group.models.length, 0);
  }, [modelGroups]);

  const isCategoryFiltered = isDownloadMode ? selectedKind !== 'all' : selectedCategory !== 'all';
  const hasLocalFilters = Boolean(searchQuery.trim()) || selectedCategory !== 'all';

  // Filter local models
  const filteredGroups = useMemo(() => {
    let groups = modelGroups;

    // Filter by category
    if (selectedCategory !== 'all') {
      groups = groups.filter((g: ModelCategory) => g.category === selectedCategory);
    }

    // Filter by search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      groups = groups
        .map((group: ModelCategory) => ({
          ...group,
          models: group.models.filter(
            (model) =>
              model.name.toLowerCase().includes(query) || model.path?.toLowerCase().includes(query)
          ),
        }))
        .filter((group: ModelCategory) => group.models.length > 0);
    }

    return groups;
  }, [modelGroups, searchQuery, selectedCategory]);

  // Filter remote results
  const filteredRemoteResults = useMemo(() => {
    const filtered =
      selectedKind === 'all'
        ? remoteResults
        : remoteResults.filter((model) => model.kind === selectedKind);

    return [...filtered].sort(
      (a, b) => getReleaseTimestamp(b.releaseDate) - getReleaseTimestamp(a.releaseDate)
    );
  }, [remoteResults, selectedKind]);

  // Handlers
  const handleClearLocalFilters = () => {
    setSearchQuery('');
    setSelectedCategory('all');
  };

  const handleClearRemoteFilters = () => {
    setSearchQuery('');
    setSelectedKind('all');
  };

  const resolveDownloadModelType = (kind: string) => {
    const normalized = kind.toLowerCase();
    if (
      normalized.includes('image') ||
      normalized.includes('video') ||
      normalized.includes('3d')
    ) {
      return 'diffusion';
    }
    return 'llm';
  };

  const handleStartRemoteDownload = async (model: any, quant?: string | null) => {
    if (!window.pywebview?.api?.start_model_download_from_hf) {
      const errorMsg = 'Download is unavailable.';
      logger.error('Download API not available');
      setDownloadError(errorMsg);
      return;
    }

    const repoId = model.repoId;
    const developer = model.developer || repoId.split('/')[0] || 'huggingface';
    const officialName = model.name || repoId;
    const modelType = resolveDownloadModelType(model.kind || '');

    logger.info('Starting remote model download', { repoId, developer, officialName, modelType, quant });
    setDownloadError(null);
    setDownloadRepoId(repoId);
    try {
      if (!window.pywebview?.api) return;
      const result = await window.pywebview.api.start_model_download_from_hf(
        repoId,
        developer,
        officialName,
        modelType,
        model.kind || '',
        quant || null
      );
      if (!result.success || !result.download_id) {
        const errorMsg = result.error || 'Download failed.';
        logger.error('Remote download failed', { error: errorMsg, repoId });
        setDownloadError(errorMsg);
        return;
      }
      logger.info('Remote download started successfully', { repoId, downloadId: result.download_id });
      startDownload(repoId, result.download_id);
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Download failed.';
      if (error instanceof APIError) {
        logger.error('API error starting remote download', { error: error.message, endpoint: error.endpoint, repoId });
      } else if (error instanceof NetworkError) {
        logger.error('Network error starting remote download', { error: error.message, url: error.url, status: error.status, repoId });
      } else if (error instanceof Error) {
        logger.error('Failed to start remote download', { error: error.message, repoId });
      } else {
        logger.error('Unknown error starting remote download', { error, repoId });
      }
      setDownloadError(message);
    }
  };

  const openRemoteUrl = (url: string) => {
    if (window.pywebview?.api?.open_url) {
      void window.pywebview.api.open_url(url);
      return;
    }
    window.open(url, '_blank', 'noopener');
  };

  const handleSearchDeveloper = (developer: string) => {
    setIsDownloadMode(true);
    setSearchQuery(developer);
    setSelectedKind('all');
    setShowCategoryMenu(false);
  };

  // Get current filter list
  const filterList = isDownloadMode ? remoteKinds : categories;
  const selectedFilter = isDownloadMode ? selectedKind : selectedCategory;
  const setSelectedFilter = isDownloadMode ? setSelectedKind : setSelectedCategory;

  const handleToggleMode = () => {
    setIsDownloadMode((prev) => !prev);
    setShowCategoryMenu(false);
  };

  const handleFilterSelect = (item: string) => {
    setSelectedFilter(item);
    setShowCategoryMenu(false);
  };

  return (
    <div className="flex-1 bg-[hsl(var(--launcher-bg-tertiary)/0.2)] overflow-hidden flex flex-col">
      {/* Header */}
      <ModelSearchBar
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        isDownloadMode={isDownloadMode}
        onToggleMode={handleToggleMode}
        isCategoryFiltered={isCategoryFiltered}
        onFilterClick={() => setShowCategoryMenu((prev) => !prev)}
        totalModels={totalModels}
        showCategoryMenu={showCategoryMenu}
        filterList={filterList}
        selectedFilter={selectedFilter}
        onSelectFilter={handleFilterSelect}
        onOpenModelsRoot={onOpenModelsRoot}
        showModeToggle={Boolean(onAddModels)}
      />

      {/* Model List */}
      <div className="flex-1 overflow-y-auto">
        <div className={isDownloadMode ? 'p-4 space-y-3' : 'p-4 space-y-4'}>
          {isDownloadMode ? (
            <RemoteModelsList
              models={filteredRemoteResults}
              isLoading={isRemoteLoading}
              error={remoteError}
              searchQuery={searchQuery}
              downloadStatusByRepo={downloadStatusByRepo}
              downloadError={downloadError}
              downloadRepoId={downloadRepoId}
              onStartDownload={handleStartRemoteDownload}
              onCancelDownload={cancelDownload}
              onOpenUrl={openRemoteUrl}
              onSearchDeveloper={handleSearchDeveloper}
              onClearFilters={handleClearRemoteFilters}
              selectedKind={selectedKind}
            />
          ) : (
            <LocalModelsList
              modelGroups={filteredGroups}
              starredModels={starredModels}
              linkedModels={linkedModels}
              onToggleStar={onToggleStar}
              onToggleLink={onToggleLink}
              selectedAppId={selectedAppId}
              totalModels={totalModels}
              hasFilters={hasLocalFilters}
              onClearFilters={handleClearLocalFilters}
            />
          )}
        </div>
      </div>
    </div>
  );
};
