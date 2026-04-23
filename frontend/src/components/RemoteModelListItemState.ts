import type { DownloadStatus } from '../hooks/modelDownloadState';
import type { RemoteModelInfo } from '../types/apps';

export type RemoteDownloadOption = NonNullable<RemoteModelInfo['downloadOptions']>[number];

export interface RemoteDownloadFlags {
  isDownloading: boolean;
  isErrored: boolean;
  isPaused: boolean;
  isPausing: boolean;
  isQueued: boolean;
}

export function getRemoteDownloadFlags(downloadStatus: DownloadStatus | undefined): RemoteDownloadFlags {
  const status = downloadStatus?.status;
  return {
    isDownloading: status ? ['queued', 'downloading', 'cancelling', 'pausing'].includes(status) : false,
    isErrored: status === 'error',
    isPaused: status === 'paused',
    isPausing: status === 'pausing',
    isQueued: status === 'queued',
  };
}

export function formatDownloadRetryHint(downloadStatus: DownloadStatus | undefined): string | null {
  if (!downloadStatus?.retrying) {
    return null;
  }

  const attempt = downloadStatus.retryAttempt ?? 0;
  const retryProgress = downloadStatus.retryLimit
    ? `${attempt}/${downloadStatus.retryLimit}`
    : `attempt ${attempt}/unlimited`;
  const retryDelay = downloadStatus.nextRetryDelaySeconds
    ? ` in ${downloadStatus.nextRetryDelaySeconds.toFixed(1)}s`
    : '';
  return `Retrying ${retryProgress}${retryDelay}`;
}

export function hasExactDownloadDetails(model: RemoteModelInfo): boolean {
  if (typeof model.totalSizeBytes === 'number' && model.totalSizeBytes > 0) {
    return true;
  }

  return (
    model.downloadOptions?.some(
      (option) =>
        (typeof option.sizeBytes === 'number' && option.sizeBytes > 0) || Boolean(option.fileGroup)
    ) ?? false
  );
}

export function getRemoteDownloadOptions(model: RemoteModelInfo): RemoteDownloadOption[] {
  if (model.downloadOptions?.length) {
    return model.downloadOptions;
  }

  return model.quants.map((quant) => ({
    fileGroup: null,
    quant,
    sizeBytes: model.quantSizes?.[quant] ?? null,
  }));
}

export function hasRemoteFileGroups(downloadOptions: RemoteDownloadOption[]): boolean {
  return downloadOptions.some((option) => option.fileGroup);
}

export function getRemoteQuantLabels(
  downloadOptions: RemoteDownloadOption[],
  hasFileGroups: boolean
): string[] {
  if (hasFileGroups) {
    return downloadOptions.map((option) => option.fileGroup?.label ?? option.quant);
  }
  return downloadOptions.map((option) => option.quant);
}

export function collectSelectedRemoteFilenames(
  downloadOptions: RemoteDownloadOption[],
  selectedGroups: Set<string>
): string[] {
  const filenames: string[] = [];
  for (const option of downloadOptions) {
    const label = option.fileGroup?.label ?? option.quant;
    if (selectedGroups.has(label) && option.fileGroup) {
      filenames.push(...option.fileGroup.filenames);
    }
  }
  return filenames;
}

export function getSelectedRemoteTotalBytes(
  downloadOptions: RemoteDownloadOption[],
  selectedGroups: Set<string>
): number {
  return downloadOptions
    .filter((option) => selectedGroups.has(option.fileGroup?.label ?? option.quant))
    .reduce((sum, option) => sum + (option.sizeBytes ?? 0), 0);
}
