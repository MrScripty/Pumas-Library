import React from 'react';
import {
  Download,
  ExternalLink,
  X,
} from 'lucide-react';
import type { RemoteModelInfo } from '../types/apps';
import type { DownloadStatus } from '../hooks/modelDownloadState';
import { RemoteModelDownloadMenu } from './RemoteModelDownloadMenu';
import { RemoteModelSummary } from './RemoteModelSummary';
import { IconButton, ListItem } from './ui';

interface RemoteModelListItemProps {
  model: RemoteModelInfo;
  downloadStatus?: DownloadStatus;
  modelError?: string;
  isHydratingDetails: boolean;
  isMenuOpen: boolean;
  selectedGroups: Set<string>;
  onToggleMenu: () => void;
  onCloseMenu: () => void;
  onToggleGroup: (label: string) => void;
  onClearSelection: () => void;
  onHydrateModelDetails?: (model: RemoteModelInfo) => Promise<void>;
  onStartDownload: (model: RemoteModelInfo, quant?: string | null, filenames?: string[] | null) => Promise<void>;
  onCancelDownload: (repoId: string) => Promise<void>;
  onPauseDownload: (repoId: string) => Promise<void>;
  onResumeDownload: (repoId: string) => Promise<void>;
  onOpenUrl: (url: string) => void;
  onSearchDeveloper?: (developer: string) => void;
  onHfAuthClick?: () => void;
}

function hasExactDownloadDetails(model: RemoteModelInfo): boolean {
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

export function RemoteModelListItem({
  model,
  downloadStatus,
  modelError,
  isHydratingDetails,
  isMenuOpen,
  selectedGroups,
  onToggleMenu,
  onCloseMenu,
  onToggleGroup,
  onClearSelection,
  onHydrateModelDetails,
  onStartDownload,
  onCancelDownload,
  onPauseDownload,
  onResumeDownload,
  onOpenUrl,
  onSearchDeveloper,
  onHfAuthClick,
}: RemoteModelListItemProps) {
  const isDownloading = downloadStatus
    ? ['queued', 'downloading', 'cancelling', 'pausing'].includes(downloadStatus.status)
    : false;
  const isPaused = downloadStatus?.status === 'paused';
  const isErrored = downloadStatus?.status === 'error';
  const isQueued = downloadStatus?.status === 'queued';
  const isPausing = downloadStatus?.status === 'pausing';
  const retryHint = downloadStatus?.retrying
    ? `Retrying ${
        downloadStatus.retryLimit
          ? `${downloadStatus.retryAttempt ?? 0}/${downloadStatus.retryLimit}`
          : `attempt ${downloadStatus.retryAttempt ?? 0}/unlimited`
      }${
        downloadStatus.nextRetryDelaySeconds
          ? ` in ${downloadStatus.nextRetryDelaySeconds.toFixed(1)}s`
          : ''
      }`
    : null;
  const progressValue = downloadStatus?.progress ?? 0;
  const progressDegrees = Math.min(360, Math.max(0, Math.round(progressValue * 360)));
  const ringDegrees = isQueued ? 60 : progressDegrees;
  const hasExactDetails = hasExactDownloadDetails(model);
  const downloadOptions = model.downloadOptions?.length
    ? model.downloadOptions
    : model.quants.map((quant) => ({
        quant,
        sizeBytes: model.quantSizes?.[quant] ?? null,
        fileGroup: null as { filenames: string[]; shardCount: number; label: string } | null | undefined,
      }));
  const hasFileGroups = downloadOptions.some((option) => option.fileGroup);
  const quantLabels = hasFileGroups
    ? downloadOptions.map((option) => option.fileGroup?.label ?? option.quant)
    : downloadOptions.map((option) => option.quant);
  const collectSelectedFilenames = (): string[] => {
    const filenames: string[] = [];
    for (const option of downloadOptions) {
      const label = option.fileGroup?.label ?? option.quant;
      if (selectedGroups.has(label) && option.fileGroup) {
        filenames.push(...option.fileGroup.filenames);
      }
    }
    return filenames;
  };
  const selectedTotalBytes = downloadOptions
    .filter((option) => selectedGroups.has(option.fileGroup?.label ?? option.quant))
    .reduce((sum, option) => sum + (option.sizeBytes ?? 0), 0);

  return (
    <ListItem>
      <div className="flex items-start justify-between gap-2 p-2">
        <RemoteModelSummary
          isHydratingDetails={isHydratingDetails}
          model={model}
          modelError={modelError}
          quantLabels={quantLabels}
          retryHint={retryHint}
          onHfAuthClick={onHfAuthClick}
          onSearchDeveloper={onSearchDeveloper}
        />

        <div className="relative flex flex-col items-center gap-1">
          <IconButton
            icon={<ExternalLink />}
            tooltip="Open"
            onClick={() => onOpenUrl(model.url)}
            size="sm"
          />
          {isDownloading && !isQueued && !isPausing && (
            <IconButton
              icon={<span className="text-[10px] font-bold">| |</span>}
              tooltip="Pause download"
              onClick={() => void onPauseDownload(model.repoId)}
              size="sm"
            />
          )}
          {(isPaused || isErrored) && (
            <IconButton
              icon={<Download />}
              tooltip={isPaused ? 'Resume download' : 'Retry download'}
              onClick={() => void onResumeDownload(model.repoId)}
              size="sm"
            />
          )}
          {isDownloading && downloadOptions.length > 0 && (
            <IconButton
              icon={<Download />}
              tooltip="Queue another download"
              onClick={onToggleMenu}
              size="sm"
            />
          )}
          <button
            onClick={() => {
              if (isDownloading) {
                onCloseMenu();
                void onCancelDownload(model.repoId);
                return;
              }
              if (isPaused || isErrored) {
                void onCancelDownload(model.repoId);
                return;
              }
              if (!hasExactDetails && onHydrateModelDetails) {
                onToggleMenu();
                if (!isMenuOpen) {
                  void onHydrateModelDetails(model);
                }
                return;
              }
              if (downloadOptions.length > 0) {
                onToggleMenu();
              } else {
                void onStartDownload(model, null);
              }
            }}
            className={`group flex-shrink-0 transition-colors ${
              isMenuOpen
                ? 'text-[hsl(var(--launcher-accent-primary))]'
                : 'text-[hsl(var(--text-muted))] hover:text-[hsl(var(--launcher-accent-primary))]'
            }`}
            title={isDownloading ? 'Cancel download' : isPaused ? 'Cancel (delete partial)' : 'Download options'}
            aria-label={isDownloading ? 'Cancel download' : isPaused ? 'Cancel' : 'Download options'}
            aria-pressed={isMenuOpen}
          >
            <span className="relative flex h-4 w-4 items-center justify-center">
              {isDownloading && (
                <>
                  <span
                    className={`download-progress-ring ${isQueued ? 'is-waiting' : ''} ${isPaused ? 'is-paused' : ''}`}
                    style={{ '--progress': `${ringDegrees}deg` } as React.CSSProperties}
                  />
                  {!isQueued && !isPaused && <span className="download-scan-ring" />}
                </>
              )}
              {isDownloading ? (
                <>
                  <Download className="h-4 w-4 transition-opacity group-hover:opacity-30" />
                  <X className="absolute h-4 w-4 opacity-0 transition-opacity group-hover:opacity-100" />
                </>
              ) : (
                <Download className="h-4 w-4" />
              )}
            </span>
          </button>

          {isMenuOpen && (
            <RemoteModelDownloadMenu
              downloadOptions={downloadOptions}
              hasExactDetails={hasExactDetails}
              hasFileGroups={hasFileGroups}
              isHydratingDetails={isHydratingDetails}
              model={model}
              selectedGroups={selectedGroups}
              selectedTotalBytes={selectedTotalBytes}
              onClearSelection={onClearSelection}
              onCloseMenu={onCloseMenu}
              onStartDownload={onStartDownload}
              onToggleGroup={onToggleGroup}
              collectSelectedFilenames={collectSelectedFilenames}
            />
          )}
        </div>
      </div>
    </ListItem>
  );
}
