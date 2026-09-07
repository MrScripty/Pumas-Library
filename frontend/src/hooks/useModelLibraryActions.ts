import { useCallback, useRef, useState, type Dispatch, type SetStateAction } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { modelsAPI } from '../api/models';
import { APIError } from '../errors';
import type { ModelInfo, RelatedModelsState } from '../types/apps';
import { getLogger } from '../utils/logger';

const logger = getLogger('useModelLibraryActions');

function formatPartialResumeError(reasonCode?: string | null, fallback?: string | null): string {
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
  onModelsImported?: () => void;
  setDownloadErrors: Dispatch<SetStateAction<Record<string, string>>>;
  startDownload: (
    downloadKey: string,
    downloadId: string,
    details?: {
      libraryModelId?: string | null;
      modelName?: string;
      modelType?: string;
      repoId?: string;
      selectedArtifactId?: string | null;
      artifactId?: string | null;
    }
  ) => void;
}

export function useModelLibraryActions({
  onModelsImported,
  setDownloadErrors,
  startDownload,
}: UseModelLibraryActionsOptions) {
  const [expandedRelated, setExpandedRelated] = useState<Set<string>>(new Set());
  const [recoveringPartialModelIds, setRecoveringPartialModelIds] = useState<Set<string>>(new Set());
  const pendingRecoveryModelIds = useRef(new Set<string>());
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
    const current = relatedModelsById[modelId];
    if (current && (current.status === 'loading' || current.status === 'loaded')) {
      return;
    }

    setRelatedModelsById((prev) => ({
      ...prev,
      [modelId]: {
        status: 'loading',
        models: [],
      },
    }));

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
            models: result.models,
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
  }, [relatedModelsById]);

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

    const recovery = model.recovery;
    if (model.provenance !== 'catalog' || !model.isPartialDownload || !recovery) {
      logger.warn('Cannot resume without a current catalog recovery ticket', {
        modelId: model.id,
      });
      return;
    }
    const modelId = model.id;
    const repoId = recovery.repoId;
    if (pendingRecoveryModelIds.current.has(modelId)) return;
    pendingRecoveryModelIds.current.add(modelId);

    setDownloadErrors((prev) => {
      if (!prev[modelId]) return prev;
      const next = { ...prev };
      delete next[modelId];
      return next;
    });
    setRecoveringPartialModelIds((prev) => {
      const next = new Set(prev);
      next.add(modelId);
      return next;
    });

    try {
      const result = await modelsAPI.resumePartialDownload(modelId, recovery.recoveryToken);
      const action = result.action;
      if (!result.success || action === 'none' || !result.download_id) {
        const errorMsg = formatPartialResumeError(result.reason_code, result.error);
        logger.error('Resume partial download failed', {
          repoId,
          modelId,
          action,
          reasonCode: result.reason_code,
          error: errorMsg,
        });
        setDownloadErrors((prev) => ({ ...prev, [modelId]: errorMsg }));
        return;
      }

      logger.info('Partial download action completed', {
        repoId,
        action,
        downloadId: result.download_id,
      });
      startDownload(result.download_id, result.download_id, {
        libraryModelId: modelId,
        repoId,
        selectedArtifactId: recovery.selectedArtifactId,
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
          modelId,
        });
      } else if (error instanceof Error) {
        logger.error('Failed to recover partial download', { error: error.message, repoId, modelId });
      } else {
        logger.error('Unknown error recovering partial download', { error, repoId, modelId });
      }
      setDownloadErrors((prev) => ({ ...prev, [modelId]: message }));
    } finally {
      pendingRecoveryModelIds.current.delete(modelId);
      setRecoveringPartialModelIds((prev) => {
        if (!prev.has(modelId)) return prev;
        const next = new Set(prev);
        next.delete(modelId);
        return next;
      });
    }
  }, [setDownloadErrors, startDownload]);

  const handleDeleteModel = useCallback(async (modelId: string) => {
    try {
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
  }, [onModelsImported]);

  return {
    expandedRelated,
    handleDeleteModel,
    handleRecoverPartialDownload,
    handleToggleRelated,
    openRemoteUrl,
    recoveringPartialModelIds,
    relatedModelsById,
  };
}
