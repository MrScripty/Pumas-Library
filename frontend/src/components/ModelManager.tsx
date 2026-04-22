/**
 * Model Manager Component (Refactored)
 *
 * Main component for managing local and remote models.
 * Includes drag-and-drop import support.
 */

import React, { useMemo, useEffect, useRef } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import type { ModelCategory, RemoteModelInfo } from '../types/apps';
import { useExistingLibraryChooser } from '../hooks/useExistingLibraryChooser';
import { useHfAuthPrompt } from '../hooks/useHfAuthPrompt';
import { useRemoteModelSearch } from '../hooks/useRemoteModelSearch';
import { useModelDownloads } from '../hooks/useModelDownloads';
import { useModelImportPicker } from '../hooks/useModelImportPicker';
import { useModelLibraryActions } from '../hooks/useModelLibraryActions';
import { useModelManagerFilters } from '../hooks/useModelManagerFilters';
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
  const {
    chooseExistingLibrary,
    isChoosingExistingLibrary,
  } = useExistingLibraryChooser({ onChooseExistingLibrary });

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
  const {
    clearLocalFilters: handleClearLocalFilters,
    clearRemoteFilters: handleClearRemoteFilters,
    hasLocalFilters,
    isCategoryFiltered,
    isDownloadMode,
    searchDeveloper: handleSearchDeveloper,
    searchQuery,
    selectedCategory,
    selectedFilter,
    selectedKind,
    selectFilter: handleFilterSelect,
    setSearchQuery,
    showCategoryMenu,
    toggleFilterMenu,
    toggleMode: handleToggleMode,
  } = useModelManagerFilters();

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

  const filterList = isDownloadMode ? remoteKinds : categories;
  const {
    closeImportDialog,
    completeImport,
    importPaths,
    openImportPicker,
    showImportDialog,
  } = useModelImportPicker({ onModelsImported });

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
  const {
    closeHfAuth,
    isHfAuthOpen,
    openHfAuth,
  } = useHfAuthPrompt({ downloadErrors, isAuthRequiredError });

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
  const totalModels = useMemo(() => {
    return localModelGroups.reduce((sum: number, group: ModelCategory) => sum + group.models.length, 0);
  }, [localModelGroups]);

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
        openHfAuth();
      }
    }
  };

  return (
    <>
      {/* Import dialog (for file picker button) */}
      {showImportDialog && importPaths.length > 0 && (
        <ModelImportDialog
          importPaths={importPaths}
          onClose={closeImportDialog}
          onImportComplete={completeImport}
        />
      )}

      {/* HuggingFace Auth Dialog */}
      <HuggingFaceAuthDialog
        isOpen={isHfAuthOpen}
        onClose={closeHfAuth}
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
        onFilterClick={toggleFilterMenu}
        totalModels={totalModels}
        hasActiveDownloads={hasActiveDownloads}
        showCategoryMenu={showCategoryMenu}
        filterList={filterList}
        selectedFilter={selectedFilter}
        onSelectFilter={handleFilterSelect}
        onOpenModelsRoot={onOpenModelsRoot}
        onImportModels={openImportPicker}
        onHfAuthClick={openHfAuth}
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
              onHfAuthClick={openHfAuth}
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
                onChooseExistingLibrary={onChooseExistingLibrary ? chooseExistingLibrary : undefined}
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
