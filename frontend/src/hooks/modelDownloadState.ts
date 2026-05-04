export interface DownloadStatus {
  downloadId: string;
  status: 'queued' | 'downloading' | 'pausing' | 'paused' | 'cancelling' | 'completed' | 'cancelled' | 'error';
  progress: number;
  repoId?: string;
  selectedArtifactId?: string | null;
  artifactId?: string | null;
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

interface ModelDownloadStatusPayload {
  repoId?: string;
  selectedArtifactId?: string | null;
  artifactId?: string | null;
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
}

interface RepoDownloadCandidate extends RepoDownloadSelection {
  key: string;
}

export interface DownloadArtifactIdentity {
  repoId?: string | null;
  selectedArtifactId?: string | null;
  artifactId?: string | null;
}

export function getDownloadArtifactKey(identity: DownloadArtifactIdentity): string | null {
  return identity.selectedArtifactId ?? identity.artifactId ?? identity.repoId ?? null;
}

function isTrackedStatus(status: string): status is DownloadStatus['status'] {
  return (TRACKED_STATUSES as readonly string[]).includes(status);
}

function optionalNumber(value: number | undefined): number | undefined {
  return typeof value === 'number' ? value : undefined;
}

function shouldReplaceSelection(current: DownloadStatus, candidate: DownloadStatus): boolean {
  const currentPriority = STATUS_PRIORITY[current.status];
  const candidatePriority = STATUS_PRIORITY[candidate.status];
  if (candidatePriority !== currentPriority) {
    return candidatePriority < currentPriority;
  }

  const currentBytes = current.downloadedBytes ?? 0;
  const candidateBytes = candidate.downloadedBytes ?? 0;
  if (candidateBytes !== currentBytes) {
    return candidateBytes > currentBytes;
  }

  const currentProgress = current.progress;
  const candidateProgress = candidate.progress;
  return candidateProgress > currentProgress;
}

function toRepoDownloadCandidate(
  download: ModelDownloadStatusPayload
): RepoDownloadCandidate | null {
  const key = getDownloadArtifactKey(download);
  const status = download.status;
  if (!key || !status || !isTrackedStatus(status) || !download.downloadId) {
    return null;
  }

  return {
    key,
    status: {
      downloadId: download.downloadId,
      status,
      progress: typeof download.progress === 'number' ? download.progress : 0,
      repoId: download.repoId,
      selectedArtifactId: download.selectedArtifactId,
      artifactId: download.artifactId,
      downloadedBytes: optionalNumber(download.downloadedBytes),
      totalBytes: optionalNumber(download.totalBytes),
      speed: optionalNumber(download.speed),
      etaSeconds: optionalNumber(download.etaSeconds),
      modelName: download.modelName,
      modelType: download.modelType,
      retryAttempt: optionalNumber(download.retryAttempt),
      retryLimit: optionalNumber(download.retryLimit),
      retrying: typeof download.retrying === 'boolean' ? download.retrying : undefined,
      nextRetryDelaySeconds: optionalNumber(download.nextRetryDelaySeconds),
    },
    error: download.error,
  };
}

export function selectDownloadsByRepo(downloads: ModelDownloadStatusPayload[]): {
  statuses: Record<string, DownloadStatus>;
  errors: Record<string, string>;
} {
  const selected: Record<string, RepoDownloadSelection> = {};

  for (const download of downloads) {
    const candidate = toRepoDownloadCandidate(download);
    if (!candidate) {
      continue;
    }

    const current = selected[candidate.key]?.status;
    if (!current || shouldReplaceSelection(current, candidate.status)) {
      selected[candidate.key] = {
        status: candidate.status,
        error: candidate.error,
      };
    }
  }

  const statuses: Record<string, DownloadStatus> = {};
  const errors: Record<string, string> = {};
  for (const [key, selectedDownload] of Object.entries(selected)) {
    statuses[key] = selectedDownload.status;
    if (selectedDownload.status.status === 'error' && selectedDownload.error) {
      errors[key] = selectedDownload.error;
    }
  }

  return { statuses, errors };
}
