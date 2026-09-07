import type { ModelInfo, RelatedModelsState } from '../types/apps';

export interface LocalModelRowState {
  canPause: boolean;
  canRecoverPartial: boolean;
  canResume: boolean;
  canShowRelated: boolean;
  hasRetainedProgressRing: boolean;
  isActiveDownload: boolean;
  isDownloading: boolean;
  isExpanded: boolean;
  isLinked: boolean;
  isPartialDownload: boolean;
  isPaused: boolean;
  isQueued: boolean;
  isRecoveringPartial: boolean;
  isStarred: boolean;
  partialError?: string;
  relatedModels: RelatedModelsState['models'];
  relatedStatus: RelatedModelsState['status'];
  relatedState?: RelatedModelsState;
  ringDegrees: number;
}

interface DownloadCapabilities {
  canPause: boolean;
  canResume: boolean;
  hasRetainedProgressRing: boolean;
  isActiveDownload: boolean;
  isPaused: boolean;
  isQueued: boolean;
  ringDegrees: number;
}

interface PartialDownloadState {
  canRecoverPartial: boolean;
  isRecoveringPartial: boolean;
  partialError?: string;
}

function getRingDegrees(model: ModelInfo, isQueued: boolean): number {
  const maximumProgress = model.isPartialDownload ? 0.99 : 1;
  const progressValue = Math.min(maximumProgress, Math.max(0, model.downloadProgress ?? 0));
  const progressDegrees = Math.round(progressValue * 360);
  return isQueued ? 60 : Math.min(360, Math.max(0, progressDegrees));
}

function getDownloadCapabilities({
  canPauseDownload,
  canResumeDownload,
  isDownloading,
  model,
}: {
  canPauseDownload: boolean;
  canResumeDownload: boolean;
  isDownloading: boolean;
  model: ModelInfo;
}): DownloadCapabilities {
  const isQueued = model.downloadStatus === 'queued';
  const isPaused = model.downloadStatus === 'paused';
  const hasDownloadKey = Boolean(model.downloadKey ?? model.downloadRepoId);
  const hasRetainedProgressRing = !isDownloading && Boolean(model.isPartialDownload);

  return {
    canPause:
      isDownloading &&
      (model.downloadStatus === 'downloading' || model.downloadStatus === 'queued') &&
      canPauseDownload &&
      hasDownloadKey,
    canResume:
      isDownloading &&
      (isPaused || model.downloadStatus === 'error') &&
      canResumeDownload &&
      hasDownloadKey,
    hasRetainedProgressRing,
    isActiveDownload: ['queued', 'downloading', 'pausing', 'cancelling'].includes(model.downloadStatus ?? ''),
    isPaused,
    isQueued,
    ringDegrees: getRingDegrees(model, isQueued),
  };
}

function getPartialDownloadState({
  canRecoverDownload,
  downloadErrors,
  isDownloading,
  isPartialDownload,
  model,
  recoveringPartialModelIds,
}: {
  canRecoverDownload: boolean;
  downloadErrors?: Record<string, string>;
  isDownloading: boolean;
  isPartialDownload: boolean;
  model: ModelInfo;
  recoveringPartialModelIds?: Set<string>;
}): PartialDownloadState {
  const hasRecovery = model.provenance === 'catalog' && Boolean(model.recovery);
  const missingRecoveryMetadata = isPartialDownload && !hasRecovery;

  return {
    canRecoverPartial:
      !isDownloading &&
      isPartialDownload &&
      canRecoverDownload &&
      hasRecovery,
    isRecoveringPartial: Boolean(recoveringPartialModelIds?.has(model.id)),
    partialError: missingRecoveryMetadata
      ? 'Resume is unavailable until current recovery information is available.'
      : downloadErrors?.[model.id],
  };
}

export function getLocalModelRowState({
  downloadErrors,
  excludedModels,
  expandedRelated,
  model,
  recoveringPartialModelIds,
  relatedModelsById,
  starredModels,
  canPauseDownload,
  canRecoverDownload,
  canResumeDownload,
}: {
  downloadErrors?: Record<string, string>;
  excludedModels: Set<string>;
  expandedRelated: Set<string>;
  model: ModelInfo;
  recoveringPartialModelIds?: Set<string>;
  relatedModelsById: Record<string, RelatedModelsState>;
  starredModels: Set<string>;
  canPauseDownload: boolean;
  canRecoverDownload: boolean;
  canResumeDownload: boolean;
}): LocalModelRowState {
  const isPartialDownload = Boolean(model.isPartialDownload);
  const isDownloading = Boolean(model.isDownloading);
  const relatedState = relatedModelsById[model.id];
  const download = getDownloadCapabilities({
    canPauseDownload,
    canResumeDownload,
    isDownloading,
    model,
  });
  const partial = getPartialDownloadState({
    canRecoverDownload,
    downloadErrors,
    isDownloading,
    isPartialDownload,
    model,
    recoveringPartialModelIds,
  });

  return {
    canPause: download.canPause,
    canRecoverPartial: partial.canRecoverPartial,
    canResume: download.canResume,
    canShowRelated: Boolean(model.relatedAvailable) && !isDownloading && !isPartialDownload,
    hasRetainedProgressRing: download.hasRetainedProgressRing,
    isActiveDownload: download.isActiveDownload,
    isDownloading,
    isExpanded: expandedRelated.has(model.id),
    isLinked: model.provenance !== 'cached' && !excludedModels.has(model.id),
    isPartialDownload,
    isPaused: download.isPaused,
    isQueued: download.isQueued,
    isRecoveringPartial: partial.isRecoveringPartial,
    isStarred: model.provenance !== 'cached' && starredModels.has(model.id),
    partialError: partial.partialError,
    relatedModels: relatedState?.models ?? [],
    relatedStatus: relatedState?.status ?? 'idle',
    relatedState,
    ringDegrees: download.ringDegrees,
  };
}
