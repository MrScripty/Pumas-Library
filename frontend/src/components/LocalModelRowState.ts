import type { ModelInfo, RelatedModelsState } from '../types/apps';

export interface LocalModelRowState {
  canPause: boolean;
  canRecoverPartial: boolean;
  canResume: boolean;
  canShowRelated: boolean;
  isActiveDownload: boolean;
  isConvertible: boolean;
  isDownloading: boolean;
  isExpanded: boolean;
  isLinked: boolean;
  isPartialDownload: boolean;
  isPaused: boolean;
  isQueued: boolean;
  isRecoveringPartial: boolean;
  isStarred: boolean;
  partialError?: string;
  partialRepoId?: string;
  relatedModels: RelatedModelsState['models'];
  relatedStatus: RelatedModelsState['status'];
  relatedState?: RelatedModelsState;
  ringDegrees: number;
}

interface DownloadCapabilities {
  canPause: boolean;
  canResume: boolean;
  isActiveDownload: boolean;
  isPaused: boolean;
  isQueued: boolean;
  ringDegrees: number;
}

interface PartialDownloadState {
  canRecoverPartial: boolean;
  isRecoveringPartial: boolean;
  partialError?: string;
  partialRepoId?: string;
}

function getRingDegrees(model: ModelInfo, isQueued: boolean): number {
  const progressValue = Math.min(1, Math.max(0, model.downloadProgress ?? 0));
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
  const hasDownloadRepo = Boolean(model.downloadRepoId);

  return {
    canPause:
      isDownloading &&
      (model.downloadStatus === 'downloading' || model.downloadStatus === 'queued') &&
      canPauseDownload &&
      hasDownloadRepo,
    canResume:
      isDownloading &&
      (isPaused || model.downloadStatus === 'error') &&
      canResumeDownload &&
      hasDownloadRepo,
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
  recoveringPartialRepoIds,
}: {
  canRecoverDownload: boolean;
  downloadErrors?: Record<string, string>;
  isDownloading: boolean;
  isPartialDownload: boolean;
  model: ModelInfo;
  recoveringPartialRepoIds?: Set<string>;
}): PartialDownloadState {
  const partialRepoId = model.repoId ?? model.downloadRepoId;

  return {
    canRecoverPartial:
      !isDownloading &&
      isPartialDownload &&
      canRecoverDownload &&
      Boolean(model.repoId) &&
      Boolean(model.modelDir),
    isRecoveringPartial: Boolean(partialRepoId && recoveringPartialRepoIds?.has(partialRepoId)),
    partialError: partialRepoId ? downloadErrors?.[partialRepoId] : undefined,
    partialRepoId,
  };
}

export function getLocalModelRowState({
  downloadErrors,
  excludedModels,
  expandedRelated,
  model,
  recoveringPartialRepoIds,
  relatedModelsById,
  starredModels,
  canConvertModel,
  canPauseDownload,
  canRecoverDownload,
  canResumeDownload,
}: {
  downloadErrors?: Record<string, string>;
  excludedModels: Set<string>;
  expandedRelated: Set<string>;
  model: ModelInfo;
  recoveringPartialRepoIds?: Set<string>;
  relatedModelsById: Record<string, RelatedModelsState>;
  starredModels: Set<string>;
  canConvertModel: boolean;
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
    recoveringPartialRepoIds,
  });

  return {
    canPause: download.canPause,
    canRecoverPartial: partial.canRecoverPartial,
    canResume: download.canResume,
    canShowRelated: Boolean(model.relatedAvailable) && !isDownloading && !isPartialDownload,
    isActiveDownload: download.isActiveDownload,
    isConvertible: !isDownloading && !isPartialDownload && Boolean(model.primaryFormat) && canConvertModel,
    isDownloading,
    isExpanded: expandedRelated.has(model.id),
    isLinked: !excludedModels.has(model.id),
    isPartialDownload,
    isPaused: download.isPaused,
    isQueued: download.isQueued,
    isRecoveringPartial: partial.isRecoveringPartial,
    isStarred: starredModels.has(model.id),
    partialError: partial.partialError,
    partialRepoId: partial.partialRepoId,
    relatedModels: relatedState?.models ?? [],
    relatedStatus: relatedState?.status ?? 'idle',
    relatedState,
    ringDegrees: download.ringDegrees,
  };
}
