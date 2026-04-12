import { useCallback, useState, type Dispatch, type SetStateAction } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { modelsAPI } from '../api/models';
import { APIError } from '../errors';
import type { ModelInfo, RelatedModelsState } from '../types/apps';
import { getLogger } from '../utils/logger';
import type { DownloadStatus } from './modelDownloadState';

const logger = getLogger('useModelLibraryActions');

function formatPartialResumeError(reasonCode?: string, fallback?: string): string {
  switch (reasonCode) {
    case 'dest_dir_missing':
      return 'Partial files directory is missing.';
    case 'invalid_repo_id':
      return 'Cannot recover: invalid repository ID.';
    case 'repo_not_found':
      return 'Cannot recover: repository was not found on HuggingFace.';
    case 'rate_limited':
      return 'HuggingFace rate-limited the request. Try again shortly.';
    case 'network_error':
      return 'Network error while resuming partial download.';
    case 'permission_denied':
      return 'Permission denied for partial files directory.';
    case 'hf_client_unavailable':
      return 'HuggingFace client is not available.';
    case 'resume_rejected':
      return 'Tracked partial download is not resumable from its current state.';
    case 'already_completed':
      return 'Download is already completed.';
    case 'already_cancelled':
      return 'Download was cancelled; start a new download.';
    default:
      return fallback || 'Failed to resume partial download.';
  }
}

interface UseModelLibraryActionsOptions {
  downloadStatusByRepo: Record<string, DownloadStatus>;
  cancelDownload: (repoId: string) => Promise<void> | void;
  onModelsImported?: () => void;
  setDownloadErrors: Dispatch<SetStateAction<Record<string, string>>>;
  startDownload: (
    repoId: string,
    downloadId: string,
    details?: { modelName?: string; modelType?: string }
  ) => void;
}

export function useModelLibraryActions({
  downloadStatusByRepo,
  cancelDownload,
  onModelsImported,
  setDownloadErrors,
  startDownload,
}: UseModelLibraryActionsOptions) {
  const [expandedRelated, setExpandedRelated] = useState<Set<string>>(new Set());
  const [recoveringPartialRepoIds, setRecoveringPartialRepoIds] = useState<Set<string>>(new Set());
  const [relatedModelsById, setRelatedModelsById] = useState<
    Record<string, RelatedModelsState>
  >({});

  const openRemoteUrl = useCallback((url: string) => {
    if (isAPIAvailable()) {
      void api.open_url(url);
      return;
    }
    window.open(url, '_blank', 'noopener');
  }, []);

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

  const handleRecoverPartialDownload = useCallback(async (model: ModelInfo) => {
    if (!isAPIAvailable()) {
      logger.error('Recover download API not available');
      return;
    }

    const repoId = model.repoId;
    const destDir = model.modelDir;
    if (!repoId || !destDir) {
      logger.warn('Cannot recover partial download without repoId + modelDir', {
        modelId: model.id,
        repoId,
        destDir,
      });
      return;
    }

    setDownloadErrors((prev) => {
      if (!prev[repoId]) return prev;
      const next = { ...prev };
      delete next[repoId];
      return next;
    });
    setRecoveringPartialRepoIds((prev) => {
      const next = new Set(prev);
      next.add(repoId);
      return next;
    });

    try {
      const result = await modelsAPI.resumePartialDownload(repoId, destDir);
      const action = result.action ?? 'none';
      if (!result.success || action === 'none' || !result.download_id) {
        const errorMsg = formatPartialResumeError(result.reason_code, result.error);
        logger.error('Resume partial download failed', {
          repoId,
          destDir,
          action,
          reasonCode: result.reason_code,
          error: errorMsg,
        });
        setDownloadErrors((prev) => ({ ...prev, [repoId]: errorMsg }));
        return;
      }

      logger.info('Partial download action completed', {
        repoId,
        action,
        downloadId: result.download_id,
      });
      startDownload(repoId, result.download_id, {
        modelName: model.name,
        modelType: model.category,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to resume partial download.';
      if (error instanceof APIError) {
        logger.error('API error recovering partial download', {
          error: error.message,
          endpoint: error.endpoint,
          repoId,
          destDir,
        });
      } else if (error instanceof Error) {
        logger.error('Failed to recover partial download', { error: error.message, repoId, destDir });
      } else {
        logger.error('Unknown error recovering partial download', { error, repoId, destDir });
      }
      setDownloadErrors((prev) => ({ ...prev, [repoId]: message }));
    } finally {
      setRecoveringPartialRepoIds((prev) => {
        if (!prev.has(repoId)) return prev;
        const next = new Set(prev);
        next.delete(repoId);
        return next;
      });
    }
  }, [setDownloadErrors, startDownload]);

  const handleDeleteModel = useCallback(async (modelId: string) => {
    try {
      for (const [repoId, status] of Object.entries(downloadStatusByRepo)) {
        if (['queued', 'downloading', 'pausing', 'paused', 'error'].includes(status.status)) {
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
    // TODO(@jeremy): Open conversion dialog with format/quant options
  }, []);

  return {
    expandedRelated,
    handleConvertModel,
    handleDeleteModel,
    handleRecoverPartialDownload,
    handleToggleRelated,
    openRemoteUrl,
    recoveringPartialRepoIds,
    relatedModelsById,
  };
}
