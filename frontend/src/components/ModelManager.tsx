/**
 * Model Manager Component (Refactored)
 *
 * Main component for managing local and remote models.
 * Includes drag-and-drop import support.
 */

import React, { useState, useMemo, useCallback, useEffect, useRef } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { modelsAPI } from '../api/models';
import type { ModelCategory, ModelInfo, RelatedModelsState, RemoteModelInfo } from '../types/apps';
import { useRemoteModelSearch } from '../hooks/useRemoteModelSearch';
import { useModelDownloads } from '../hooks/useModelDownloads';
import { useNetworkStatus } from '../hooks/useNetworkStatus';
import { ModelSearchBar } from './ModelSearchBar';
import { LocalModelsList } from './LocalModelsList';
import { RemoteModelsList } from './RemoteModelsList';
import { ModelImportDialog } from './ModelImportDialog';
import { LinkHealthStatus } from './LinkHealthStatus';
import { MigrationReportsPanel } from './MigrationReportsPanel';
import { HuggingFaceAuthDialog } from './HuggingFaceAuthDialog';
import { NetworkStatusBanner } from './NetworkStatusBanner';
import { getReleaseTimestamp } from '../utils/modelFormatters';
import { getLogger } from '../utils/logger';
import { APIError, NetworkError } from '../errors';

const logger = getLogger('ModelManager');

/** Detect HTTP 401 Unauthorized errors that indicate missing HF authentication. */
function isAuthRequiredError(errorMessage: string): boolean {
  return /\b401\b/.test(errorMessage);
}

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
}) => {
  // UI State
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('all');
  const [selectedKind, setSelectedKind] = useState<string>('all');
  const [showCategoryMenu, setShowCategoryMenu] = useState(false);
  const [isDownloadMode, setIsDownloadMode] = useState(false);
  const [expandedRelated, setExpandedRelated] = useState<Set<string>>(new Set());
  const [relatedModelsById, setRelatedModelsById] = useState<
    Record<string, RelatedModelsState>
  >({});

  // Import State
  const [droppedFiles, setDroppedFiles] = useState<string[]>([]);
  const [showImportDialog, setShowImportDialog] = useState(false);

  // HuggingFace Auth State
  const [showHfAuth, setShowHfAuth] = useState(false);

  // Custom Hooks
  const { results: remoteResults, kinds: remoteKinds, error: remoteError, isLoading: isRemoteLoading } =
    useRemoteModelSearch({
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
    for (const [repoId, status] of Object.entries(downloadStatusByRepo)) {
      if (status.status === 'completed' && prev[repoId] && prev[repoId] !== 'completed') {
        anyNewlyCompleted = true;
        logger.info('Download completed, will refresh model list', { repoId });
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
    return Object.entries(downloadStatusByRepo)
      .filter(([, status]) => ['queued', 'downloading', 'cancelling', 'pausing', 'paused', 'error'].includes(status.status))
      .map(([repoId, status]) => {
        const name = status.modelName || repoId.split('/').pop() || repoId;
        return {
          id: `download:${repoId}`,
          name,
          category: status.modelType || 'llm',
          path: repoId,
          size: status.totalBytes,
          isDownloading: true,
          downloadProgress: status.progress,
          downloadStatus: status.status as ModelInfo['downloadStatus'],
          downloadRepoId: repoId,
          downloadTotalBytes: status.totalBytes,
        } as ModelInfo;
      });
  }, [downloadStatusByRepo]);

  const localModelGroups = useMemo(() => {
    if (downloadingModels.length === 0) {
      return modelGroups;
    }

    // Build lookup of active downloads by lowercase repoId for case-insensitive merging
    const downloadByRepoId = new Map<string, ModelInfo>();
    for (const dl of downloadingModels) {
      if (dl.downloadRepoId) {
        const key = dl.downloadRepoId.toLowerCase();
        // Keep the first entry when duplicates exist (same repo, different casing)
        if (!downloadByRepoId.has(key)) {
          downloadByRepoId.set(key, dl);
        }
      }
    }

    const mergedRepoKeys = new Set<string>();
    const groupMap = new Map<string, ModelInfo[]>();

    // Merge download state onto library models that match by repoId (case-insensitive)
    modelGroups.forEach((group) => {
      const models = group.models.map((model) => {
        const key = model.repoId?.toLowerCase();
        if (key && downloadByRepoId.has(key)) {
          const dl = downloadByRepoId.get(key)!;
          mergedRepoKeys.add(key);
          return {
            ...model,
            isDownloading: true,
            downloadProgress: dl.downloadProgress,
            downloadStatus: dl.downloadStatus,
            downloadRepoId: dl.downloadRepoId,
            downloadTotalBytes: dl.downloadTotalBytes,
          };
        }
        return model;
      });
      groupMap.set(group.category, models);
    });

    // Add download-only entries (no matching library model, deduplicated)
    const orphanDownloads = downloadingModels.filter(
      (dl) => !dl.downloadRepoId || !mergedRepoKeys.has(dl.downloadRepoId.toLowerCase())
    );
    orphanDownloads.forEach((model) => {
      const existing = groupMap.get(model.category);
      if (existing) {
        groupMap.set(model.category, [model, ...existing]);
      } else {
        groupMap.set(model.category, [model]);
      }
    });

    const orderedCategories = Array.from(
      new Set([
        ...modelGroups.map((group) => group.category),
        ...orphanDownloads.map((model) => model.category),
      ])
    );

    return orderedCategories.map((category) => ({
      category,
      models: groupMap.get(category) || [],
    }));
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

  // Filter local models
  const filteredGroups = useMemo(() => {
    let groups = localModelGroups;

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
  }, [localModelGroups, searchQuery, selectedCategory]);

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

  const resolveDownloadModelType = (kind: string): string => {
    const PIPELINE_TAG_TO_MODEL_TYPE: Record<string, string> = {
      // Text generation (LLMs)
      'text-generation': 'llm',
      'text2text-generation': 'llm',
      'question-answering': 'llm',
      'token-classification': 'llm',
      'text-classification': 'llm',
      'fill-mask': 'llm',
      'translation': 'llm',
      'summarization': 'llm',
      'conversational': 'llm',
      // Diffusion / image & video generation
      'text-to-image': 'diffusion',
      'image-to-image': 'diffusion',
      'unconditional-image-generation': 'diffusion',
      'image-inpainting': 'diffusion',
      'text-to-video': 'diffusion',
      'video-classification': 'diffusion',
      'text-to-3d': 'diffusion',
      'image-to-3d': 'diffusion',
      // Audio
      'text-to-audio': 'audio',
      'text-to-speech': 'audio',
      'automatic-speech-recognition': 'audio',
      'audio-classification': 'audio',
      'audio-to-audio': 'audio',
      'voice-activity-detection': 'audio',
      // Vision
      'image-classification': 'vision',
      'image-segmentation': 'vision',
      'object-detection': 'vision',
      'zero-shot-image-classification': 'vision',
      'depth-estimation': 'vision',
      'image-feature-extraction': 'vision',
      'zero-shot-object-detection': 'vision',
      'image-to-text': 'vision',
      'visual-question-answering': 'vision',
      'document-question-answering': 'vision',
      'video-text-to-text': 'vision',
      // Embedding
      'feature-extraction': 'embedding',
      'sentence-similarity': 'embedding',
    };
    return PIPELINE_TAG_TO_MODEL_TYPE[kind.toLowerCase()] ?? 'unknown';
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
        model.kind || '',
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

  const openRemoteUrl = (url: string) => {
    if (isAPIAvailable()) {
      void api.open_url(url);
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

  const fetchRelatedModels = useCallback(async (modelId: string) => {
    let shouldFetch = false;
    setRelatedModelsById((prev) => {
      const current = prev[modelId];
      if (current && (current.status === 'loading' || current.status === 'loaded')) {
        return prev;
      }
      shouldFetch = true;
      return {
        ...prev,
        [modelId]: {
          status: 'loading',
          models: [],
        },
      };
    });

    if (!shouldFetch) {
      return;
    }

    if (!isAPIAvailable()) {
      setRelatedModelsById((prev) => ({
        ...prev,
        [modelId]: {
          status: 'error',
          models: [],
          error: 'Related models unavailable.',
        },
      }));
      return;
    }

    try {
      const result = await modelsAPI.getRelatedModels(modelId, 25);
      if (result.success) {
        setRelatedModelsById((prev) => ({
          ...prev,
          [modelId]: {
            status: 'loaded',
            models: result.models ?? [],
          },
        }));
      } else {
        setRelatedModelsById((prev) => ({
          ...prev,
          [modelId]: {
            status: 'error',
            models: [],
            error: result.error || 'Related models unavailable.',
          },
        }));
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Related models unavailable.';
      if (error instanceof APIError) {
        logger.error('API error fetching related models', {
          error: error.message,
          endpoint: error.endpoint,
          modelId,
        });
      } else if (error instanceof Error) {
        logger.error('Failed to fetch related models', { error: error.message, modelId });
      } else {
        logger.error('Unknown error fetching related models', { error, modelId });
      }
      setRelatedModelsById((prev) => ({
        ...prev,
        [modelId]: {
          status: 'error',
          models: [],
          error: message,
        },
      }));
    }
  }, []);

  const handleToggleRelated = useCallback(
    (modelId: string) => {
      const isExpanded = expandedRelated.has(modelId);
      setExpandedRelated((prev) => {
        const next = new Set(prev);
        if (isExpanded) {
          next.delete(modelId);
        } else {
          next.add(modelId);
        }
        return next;
      });
      if (!isExpanded) {
        void fetchRelatedModels(modelId);
      }
    },
    [expandedRelated, fetchRelatedModels]
  );

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
    setDroppedFiles([]);
  }, []);

  const handleImportComplete = useCallback(() => {
    logger.info('Import complete, refreshing model list');
    if (onModelsImported) {
      onModelsImported();
    }
  }, [onModelsImported]);

  // Handler for file picker import button
  const handleDeleteModel = useCallback(async (modelId: string) => {
    try {
      // Cancel any active download for this model first
      for (const [repoId, status] of Object.entries(downloadStatusByRepo)) {
        if (['queued', 'downloading', 'pausing', 'paused', 'error'].includes(status.status)) {
          // Match by repoId: model IDs are like "llm/family/name", repoIds are "family/name"
          const modelSuffix = modelId.split('/').slice(1).join('/');
          if (repoId === modelSuffix || repoId.toLowerCase() === modelSuffix.toLowerCase()) {
            logger.info('Cancelling active download before delete', { modelId, repoId });
            await cancelDownload(repoId);
          }
        }
      }

      const result = await modelsAPI.deleteModel(modelId);
      if (result.success) {
        logger.info('Model deleted', { modelId });
        onModelsImported?.();
      } else {
        logger.error('Failed to delete model', { modelId, error: result.error });
      }
    } catch (error) {
      if (error instanceof Error) {
        logger.error('Error deleting model', { modelId, error: error.message });
      }
    }
  }, [onModelsImported, downloadStatusByRepo, cancelDownload]);

  const handleConvertModel = useCallback((modelId: string) => {
    logger.info('Convert model requested', { modelId });
    // TODO: Open conversion dialog with format/quant options
  }, []);

  const handleImportClick = useCallback(async () => {
    if (!isAPIAvailable()) {
      logger.warn('open_model_import_dialog API not available');
      return;
    }

    try {
      const result = await api.open_model_import_dialog();
      if (result.success && result.paths.length > 0) {
        logger.info('Files selected for import', { count: result.paths.length });
        setDroppedFiles(result.paths);
        setShowImportDialog(true);
      }
    } catch (error) {
      logger.error('Failed to open model import dialog', { error });
    }
  }, []);

  return (
    <>
      {/* Import dialog (for file picker button) */}
      {showImportDialog && droppedFiles.length > 0 && (
        <ModelImportDialog
          filePaths={droppedFiles}
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
                onDeleteModel={handleDeleteModel}
                onConvertModel={handleConvertModel}
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
