export interface DownloadStatus {
  downloadId: string;
  status: 'queued' | 'downloading' | 'pausing' | 'paused' | 'cancelling' | 'completed' | 'cancelled' | 'error';
  progress: number;
  downloadedBytes?: number;
  totalBytes?: number;
  speed?: number;
  etaSeconds?: number;
  modelName?: string;
  modelType?: string;
  retryAttempt?: number;
  retryLimit?: number;
  retrying?: boolean;
  nextRetryDelaySeconds?: number;
}

const TRACKED_STATUSES = ['queued', 'downloading', 'pausing', 'paused', 'cancelling', 'error'] as const;
const STATUS_PRIORITY: Record<DownloadStatus['status'], number> = {
  downloading: 0,
  pausing: 1,
  cancelling: 2,
  queued: 3,
  paused: 4,
  error: 5,
  completed: 99,
  cancelled: 99,
};

interface RepoDownloadSelection {
  status: DownloadStatus;
  error?: string;
}

function isTrackedStatus(status: string): status is DownloadStatus['status'] {
  return (TRACKED_STATUSES as readonly string[]).includes(status);
}

function shouldReplaceSelection(current: DownloadStatus, candidate: DownloadStatus): boolean {
  const currentPriority = STATUS_PRIORITY[current.status] ?? 999;
  const candidatePriority = STATUS_PRIORITY[candidate.status] ?? 999;
  if (candidatePriority !== currentPriority) {
    return candidatePriority < currentPriority;
  }

  const currentBytes = current.downloadedBytes ?? 0;
  const candidateBytes = candidate.downloadedBytes ?? 0;
  if (candidateBytes !== currentBytes) {
    return candidateBytes > currentBytes;
  }

  const currentProgress = current.progress ?? 0;
  const candidateProgress = candidate.progress ?? 0;
  return candidateProgress > currentProgress;
}

export function selectDownloadsByRepo(downloads: Array<{
  repoId?: string;
  downloadId?: string;
  status?: string;
  progress?: number;
  downloadedBytes?: number;
  totalBytes?: number;
  speed?: number;
  etaSeconds?: number;
  modelName?: string;
  modelType?: string;
  retryAttempt?: number;
  retryLimit?: number;
  retrying?: boolean;
  nextRetryDelaySeconds?: number;
  error?: string;
}>): {
  statuses: Record<string, DownloadStatus>;
  errors: Record<string, string>;
} {
  const selected: Record<string, RepoDownloadSelection> = {};

  for (const download of downloads) {
    const repoId = download.repoId;
    const status = download.status;
    if (!repoId || !status || !isTrackedStatus(status) || !download.downloadId) {
      continue;
    }

    const candidate: DownloadStatus = {
      downloadId: download.downloadId,
      status,
      progress: typeof download.progress === 'number' ? download.progress : 0,
      downloadedBytes:
        typeof download.downloadedBytes === 'number' ? download.downloadedBytes : undefined,
      totalBytes: typeof download.totalBytes === 'number' ? download.totalBytes : undefined,
      speed: typeof download.speed === 'number' ? download.speed : undefined,
      etaSeconds: typeof download.etaSeconds === 'number' ? download.etaSeconds : undefined,
      modelName: download.modelName,
      modelType: download.modelType,
      retryAttempt: typeof download.retryAttempt === 'number' ? download.retryAttempt : undefined,
      retryLimit: typeof download.retryLimit === 'number' ? download.retryLimit : undefined,
      retrying: typeof download.retrying === 'boolean' ? download.retrying : undefined,
      nextRetryDelaySeconds:
        typeof download.nextRetryDelaySeconds === 'number'
          ? download.nextRetryDelaySeconds
          : undefined,
    };

    const current = selected[repoId]?.status;
    if (!current || shouldReplaceSelection(current, candidate)) {
      selected[repoId] = {
        status: candidate,
        error: download.error,
      };
    }
  }

  const statuses: Record<string, DownloadStatus> = {};
  const errors: Record<string, string> = {};
  for (const [repoId, selectedDownload] of Object.entries(selected)) {
    statuses[repoId] = selectedDownload.status;
    if (selectedDownload.status.status === 'error' && selectedDownload.error) {
      errors[repoId] = selectedDownload.error;
    }
  }

  return { statuses, errors };
}
