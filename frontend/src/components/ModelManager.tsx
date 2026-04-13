/**
 * Model Manager Component (Refactored)
 *
 * Main component for managing local and remote models.
 * Includes drag-and-drop import support.
 */

import React, { useState, useMemo, useCallback, useEffect, useRef } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { ModelCategory, RemoteModelInfo } from '../types/apps';
import { useRemoteModelSearch } from '../hooks/useRemoteModelSearch';
import { useModelDownloads } from '../hooks/useModelDownloads';
import { useModelLibraryActions } from '../hooks/useModelLibraryActions';
import { useNetworkStatus } from '../hooks/useNetworkStatus';
import { ModelSearchBar } from './ModelSearchBar';
import { LocalModelsList } from './LocalModelsList';
import { RemoteModelsList } from './RemoteModelsList';
import { ModelImportDialog } from './ModelImportDialog';
import { LinkHealthStatus } from './LinkHealthStatus';
import { MigrationReportsPanel } from './MigrationReportsPanel';
import { HuggingFaceAuthDialog } from './HuggingFaceAuthDialog';
import { NetworkStatusBanner } from './NetworkStatusBanner';
import { getLogger } from '../utils/logger';
import { APIError, NetworkError } from '../errors';
import {
  buildDownloadingModels,
  filterLocalModelGroups,
  isAuthRequiredError,
  mergeLocalModelGroups,
  resolveDownloadModelType,
  sortAndFilterRemoteResults,
} from './ModelManagerUtils';

const logger = getLogger('ModelManager');

export interface ModelManagerProps {
  modelGroups: ModelCategory[];
  starredModels: Set<string>;
  excludedModels: Set<string>;
  onToggleStar: (modelId: string) => void;
  onToggleLink: (modelId: string) => void;
  selectedAppId: string | null;
  onAddModels?: () => void;
  onOpenModelsRoot?: () => void;
  /** Callback when models are imported to refresh the list */
  onModelsImported?: () => void;
  /** Active version tag for link health check */
  activeVersion?: string | null;
  onChooseExistingLibrary?: () => Promise<void> | void;
}

export const ModelManager: React.FC<ModelManagerProps> = ({
  modelGroups,
  starredModels,
  excludedModels,
  onToggleStar,
  onToggleLink,
  selectedAppId,
  onAddModels,
  onOpenModelsRoot,
  onModelsImported,
  activeVersion,
  onChooseExistingLibrary,
}) => {
  // UI State
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('all');
  const [selectedKind, setSelectedKind] = useState<string>('all');
  const [showCategoryMenu, setShowCategoryMenu] = useState(false);
  const [isDownloadMode, setIsDownloadMode] = useState(false);

  // Import State
  const [importPaths, setImportPaths] = useState<string[]>([]);
  const [showImportDialog, setShowImportDialog] = useState(false);

  // HuggingFace Auth State
  const [showHfAuth, setShowHfAuth] = useState(false);
  const [isChoosingExistingLibrary, setIsChoosingExistingLibrary] = useState(false);

  // Custom Hooks
  const {
    results: remoteResults,
    kinds: remoteKinds,
    error: remoteError,
    isLoading: isRemoteLoading,
    hydratingRepoIds,
    hydrateModelDetails,
  } = useRemoteModelSearch({
    enabled: isDownloadMode,
    searchQuery,
  });

  const {
    downloadStatusByRepo,
    downloadErrors,
    hasActiveDownloads,
    startDownload,
    cancelDownload,
    pauseDownload,
    resumeDownload,
    setDownloadErrors,
  } = useModelDownloads();

  // Network status for offline/rate limit indicators
  const { isOffline, isRateLimited, successRate, circuitBreakerRejections } = useNetworkStatus();
  const {
    expandedRelated,
    handleConvertModel,
    handleDeleteModel,
    handleRecoverPartialDownload,
    handleToggleRelated,
    openRemoteUrl,
    recoveringPartialRepoIds,
    relatedModelsById,
  } = useModelLibraryActions({
    cancelDownload,
    downloadStatusByRepo,
    onModelsImported,
    setDownloadErrors,
    startDownload,
  });

  // Auto-open HF auth dialog when a download fails with 401
  const prevDownloadErrorsRef = useRef<Record<string, string>>({});
  useEffect(() => {
    const prev = prevDownloadErrorsRef.current;
    for (const [repoId, errorMsg] of Object.entries(downloadErrors)) {
      if (!prev[repoId] && isAuthRequiredError(errorMsg)) {
        setShowHfAuth(true);
        break;
      }
    }
    prevDownloadErrorsRef.current = downloadErrors;
  }, [downloadErrors]);

  // Auto-refresh model list when downloads complete
  const prevDownloadStatusRef = useRef<Record<string, string>>({});
  useEffect(() => {
    const prev = prevDownloadStatusRef.current;
    let anyNewlyCompleted = false;
    const refreshOnDisappearStatuses = new Set(['queued', 'downloading', 'pausing']);
    for (const [repoId, status] of Object.entries(downloadStatusByRepo)) {
      if (status.status === 'completed' && prev[repoId] && prev[repoId] !== 'completed') {
        anyNewlyCompleted = true;
        logger.info('Download completed, will refresh model list', { repoId });
      }
    }
    for (const [repoId, prevStatus] of Object.entries(prev)) {
      if (!downloadStatusByRepo[repoId] && refreshOnDisappearStatuses.has(prevStatus)) {
        anyNewlyCompleted = true;
        logger.info('Download left tracked state, will refresh model list', { repoId, prevStatus });
      }
    }
    prevDownloadStatusRef.current = Object.fromEntries(
      Object.entries(downloadStatusByRepo).map(([k, v]) => [k, v.status])
    );
    if (anyNewlyCompleted) {
      // Delay to allow backend import_in_place indexing to finish
      setTimeout(() => onModelsImported?.(), 1000);
    }
  }, [downloadStatusByRepo, onModelsImported]);

  // Computed Values
  const downloadingModels = useMemo(() => {
    return buildDownloadingModels(downloadStatusByRepo);
  }, [downloadStatusByRepo]);

  const localModelGroups = useMemo(() => {
    return mergeLocalModelGroups(modelGroups, downloadingModels);
  }, [modelGroups, downloadingModels]);

  const categories = useMemo(() => {
    const cats = localModelGroups.map((g: ModelCategory) => g.category);
    return ['all', ...cats];
  }, [localModelGroups]);

  const totalModels = useMemo(() => {
    return localModelGroups.reduce((sum: number, group: ModelCategory) => sum + group.models.length, 0);
  }, [localModelGroups]);

  const isCategoryFiltered = isDownloadMode ? selectedKind !== 'all' : selectedCategory !== 'all';
  const hasLocalFilters = Boolean(searchQuery.trim()) || selectedCategory !== 'all';
  const integrityIssueCount = useMemo(() => {
    return localModelGroups.reduce(
      (count, group) => count + group.models.filter((model) => model.hasIntegrityIssue).length,
      0
    );
  }, [localModelGroups]);

  // Filter local models
  const filteredGroups = useMemo(() => {
    return filterLocalModelGroups(localModelGroups, searchQuery, selectedCategory);
  }, [localModelGroups, searchQuery, selectedCategory]);

  // Filter remote results
  const filteredRemoteResults = useMemo(() => {
    return sortAndFilterRemoteResults(remoteResults, selectedKind);
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

  const handleStartRemoteDownload = async (model: RemoteModelInfo, quant?: string | null, filenames?: string[] | null) => {
    if (!isAPIAvailable()) {
      logger.error('Download API not available');
      return;
    }

    const repoId = model.repoId;
    const developer = model.developer || repoId.split('/')[0] || 'huggingface';
    const officialName = model.name || repoId;
    const modelType = resolveDownloadModelType(model.kind || '');
    const pipelineTag = model.kind || '';
    const releaseDate = model.releaseDate || null;
    const downloadUrl = model.url || null;

    logger.info('Starting remote model download', { repoId, developer, officialName, modelType, quant, filenames: filenames?.length });
    // Clear any previous error for this download
    setDownloadErrors((prev) => {
      if (!prev[repoId]) return prev;
      const next = { ...prev };
      delete next[repoId];
      return next;
    });
    try {
      if (!isAPIAvailable()) return;
      const result = await api.start_model_download_from_hf(
        repoId,
        developer,
        officialName,
        modelType,
        pipelineTag,
        releaseDate,
        downloadUrl,
        quant || null,
        filenames || null
      );
      if (!result.success || !result.download_id) {
        const errorMsg = result.error || 'Download failed.';
        logger.error('Remote download failed', { error: errorMsg, repoId });
        setDownloadErrors((prev) => ({ ...prev, [repoId]: errorMsg }));
        return;
      }
      logger.info('Remote download started successfully', { repoId, downloadId: result.download_id });
      startDownload(repoId, result.download_id, { modelName: officialName, modelType });
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
      setDownloadErrors((prev) => ({ ...prev, [repoId]: message }));
      if (isAuthRequiredError(message)) {
        setShowHfAuth(true);
      }
    }
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

  // Import handlers (for file picker button)
  const handleImportDialogClose = useCallback(() => {
    setShowImportDialog(false);
    setImportPaths([]);
  }, []);

  const handleImportComplete = useCallback(() => {
    logger.info('Import complete, refreshing model list');
    if (onModelsImported) {
      onModelsImported();
    }
  }, [onModelsImported]);

  const handleImportClick = useCallback(async () => {
    if (!isAPIAvailable()) {
      logger.warn('open_model_import_dialog API not available');
      return;
    }

    try {
      const result = await api.open_model_import_dialog();
      if (result.success && result.paths.length > 0) {
        logger.info('Import paths selected', { count: result.paths.length });
        setImportPaths(result.paths);
        setShowImportDialog(true);
      }
    } catch (error) {
      logger.error('Failed to open model import dialog', { error });
    }
  }, []);

  const handleChooseExistingLibrary = useCallback(async () => {
    if (!onChooseExistingLibrary || isChoosingExistingLibrary) {
      return;
    }

    setIsChoosingExistingLibrary(true);
    try {
      await onChooseExistingLibrary();
    } finally {
      setIsChoosingExistingLibrary(false);
    }
  }, [isChoosingExistingLibrary, onChooseExistingLibrary]);

  return (
    <>
      {/* Import dialog (for file picker button) */}
      {showImportDialog && importPaths.length > 0 && (
        <ModelImportDialog
          importPaths={importPaths}
          onClose={handleImportDialogClose}
          onImportComplete={handleImportComplete}
        />
      )}

      {/* HuggingFace Auth Dialog */}
      <HuggingFaceAuthDialog
        isOpen={showHfAuth}
        onClose={() => setShowHfAuth(false)}
      />

    <div className="flex-1 bg-[hsl(var(--launcher-bg-tertiary)/0.2)] overflow-hidden flex flex-col">
      {/* Network status banner */}
      <NetworkStatusBanner
        isOffline={isOffline}
        isRateLimited={isRateLimited}
        successRate={successRate}
        circuitBreakerRejections={circuitBreakerRejections}
      />
      {/* Header */}
      <ModelSearchBar
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        isDownloadMode={isDownloadMode}
        onToggleMode={handleToggleMode}
        isCategoryFiltered={isCategoryFiltered}
        onFilterClick={() => setShowCategoryMenu((prev) => !prev)}
        totalModels={totalModels}
        hasActiveDownloads={hasActiveDownloads}
        showCategoryMenu={showCategoryMenu}
        filterList={filterList}
        selectedFilter={selectedFilter}
        onSelectFilter={handleFilterSelect}
        onOpenModelsRoot={onOpenModelsRoot}
        onImportModels={handleImportClick}
        onHfAuthClick={() => setShowHfAuth(true)}
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
              downloadErrors={downloadErrors}
              hydratingRepoIds={hydratingRepoIds}
              onHydrateModelDetails={hydrateModelDetails}
              onStartDownload={handleStartRemoteDownload}
              onCancelDownload={cancelDownload}
              onPauseDownload={pauseDownload}
              onResumeDownload={resumeDownload}
              onOpenUrl={openRemoteUrl}
              onSearchDeveloper={handleSearchDeveloper}
              onClearFilters={handleClearRemoteFilters}
              selectedKind={selectedKind}
              onHfAuthClick={() => setShowHfAuth(true)}
            />
          ) : (
            <>
              {integrityIssueCount > 0 && (
                <div className="rounded border border-[hsl(var(--accent-warning)/0.35)] bg-[hsl(var(--accent-warning)/0.12)] px-3 py-2 text-xs text-[hsl(var(--accent-warning))]">
                  Library integrity warning: {integrityIssueCount} model entr{integrityIssueCount === 1 ? 'y' : 'ies'} have duplicate repo records. Reconciliation will keep one visible entry and mark the issue.
                </div>
              )}
              <LocalModelsList
                modelGroups={filteredGroups}
                starredModels={starredModels}
                excludedModels={excludedModels}
                onToggleStar={onToggleStar}
                onToggleLink={onToggleLink}
                selectedAppId={selectedAppId}
                totalModels={totalModels}
                hasFilters={hasLocalFilters}
                onClearFilters={handleClearLocalFilters}
                relatedModelsById={relatedModelsById}
                expandedRelated={expandedRelated}
                onToggleRelated={handleToggleRelated}
                onOpenRelatedUrl={openRemoteUrl}
                onPauseDownload={pauseDownload}
                onResumeDownload={resumeDownload}
                onCancelDownload={cancelDownload}
                onRecoverPartialDownload={handleRecoverPartialDownload}
                recoveringPartialRepoIds={recoveringPartialRepoIds}
                downloadErrors={downloadErrors}
                onDeleteModel={handleDeleteModel}
                onConvertModel={handleConvertModel}
                onChooseExistingLibrary={onChooseExistingLibrary ? handleChooseExistingLibrary : undefined}
                isChoosingExistingLibrary={isChoosingExistingLibrary}
              />
              {/* Link Health Status */}
              <div className="mt-4">
                <LinkHealthStatus activeVersion={activeVersion} />
              </div>
              {/* Migration Reports */}
              <div className="mt-4">
                <MigrationReportsPanel />
              </div>
            </>
          )}
        </div>
      </div>
    </div>
    </>
  );
};
