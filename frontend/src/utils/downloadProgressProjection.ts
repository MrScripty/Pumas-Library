import type { DownloadProgressOutcome } from '@pumas/desktop-contract';
import type { ModelDownloadSnapshotEntry } from '../types/api-models';

/** Canonical list and push progress share one nullable-to-presentation projection. */
export function projectDownloadProgress(progress: DownloadProgressOutcome): ModelDownloadSnapshotEntry {
  return {
    downloadId: progress.downloadId,
    libraryModelId: progress.libraryModelId,
    status: progress.status,
    repoId: progress.repoId ?? undefined,
    selectedArtifactId: progress.selectedArtifactId,
    progress: progress.progress ?? undefined,
    downloadedBytes: progress.downloadedBytes ?? undefined,
    totalBytes: progress.totalBytes ?? undefined,
    speed: progress.speed ?? undefined,
    etaSeconds: progress.etaSeconds ?? undefined,
    modelName: progress.modelName ?? undefined,
    modelType: progress.modelType ?? undefined,
    retryAttempt: progress.retryAttempt ?? undefined,
    retryLimit: progress.retryLimit ?? undefined,
    retrying: progress.retrying ?? undefined,
    nextRetryDelaySeconds: progress.nextRetryDelaySeconds ?? undefined,
    error: progress.error ?? undefined,
  };
}
