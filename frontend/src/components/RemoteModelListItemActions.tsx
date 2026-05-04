import React from 'react';
import { Download, ExternalLink, X } from 'lucide-react';
import type { RemoteModelInfo } from '../types/apps';
import { IconButton } from './ui';
import { RemoteModelDownloadMenu } from './RemoteModelDownloadMenu';
import {
  collectSelectedRemoteFilenames,
  type RemoteDownloadFlags,
  type RemoteDownloadOption,
} from './RemoteModelListItemState';

interface RemoteModelActionProps {
  downloadOptions: RemoteDownloadOption[];
  flags: RemoteDownloadFlags;
  hasExactDetails: boolean;
  hasFileGroups: boolean;
  isHydratingDetails: boolean;
  isMenuOpen: boolean;
  model: RemoteModelInfo;
  downloadKey: string;
  progressDegrees: number;
  selectedGroups: Set<string>;
  selectedTotalBytes: number;
  onCancelDownload: (downloadKey: string) => Promise<void>;
  onClearSelection: () => void;
  onCloseMenu: () => void;
  onHydrateModelDetails?: (model: RemoteModelInfo) => Promise<void>;
  onOpenUrl: (url: string) => void;
  onPauseDownload: (downloadKey: string) => Promise<void>;
  onResumeDownload: (downloadKey: string) => Promise<void>;
  onStartDownload: (model: RemoteModelInfo, quant?: string | null, filenames?: string[] | null) => Promise<void>;
  onToggleGroup: (label: string) => void;
  onToggleMenu: () => void;
}

function getPrimaryDownloadTitle(flags: RemoteDownloadFlags): string {
  if (flags.isDownloading) {
    return 'Cancel download';
  }
  if (flags.isPaused) {
    return 'Cancel (delete partial)';
  }
  return 'Download options';
}

function getPrimaryDownloadLabel(flags: RemoteDownloadFlags): string {
  if (flags.isDownloading) {
    return 'Cancel download';
  }
  if (flags.isPaused) {
    return 'Cancel';
  }
  return 'Download options';
}

function handlePrimaryDownloadClick({
  downloadOptions,
  flags,
  hasExactDetails,
  isMenuOpen,
  model,
  downloadKey,
  onCancelDownload,
  onCloseMenu,
  onHydrateModelDetails,
  onStartDownload,
  onToggleMenu,
}: Pick<
  RemoteModelActionProps,
  | 'downloadOptions'
  | 'flags'
  | 'hasExactDetails'
  | 'isMenuOpen'
  | 'model'
  | 'downloadKey'
  | 'onCancelDownload'
  | 'onCloseMenu'
  | 'onHydrateModelDetails'
  | 'onStartDownload'
  | 'onToggleMenu'
>): void {
  if (flags.isDownloading) {
    onCloseMenu();
    void onCancelDownload(downloadKey);
    return;
  }
  if (flags.isPaused || flags.isErrored) {
    void onCancelDownload(downloadKey);
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
}

function DownloadProgressRings({
  flags,
  ringDegrees,
}: {
  flags: RemoteDownloadFlags;
  ringDegrees: number;
}) {
  if (!flags.isDownloading) {
    return null;
  }

  return (
    <>
      <span
        className={`download-progress-ring ${flags.isQueued ? 'is-waiting' : ''} ${flags.isPaused ? 'is-paused' : ''}`}
        style={{ '--progress': `${ringDegrees}deg` } as React.CSSProperties}
      />
      {!flags.isQueued && !flags.isPaused && <span className="download-scan-ring" />}
    </>
  );
}

function PrimaryDownloadIcon({ isDownloading }: { isDownloading: boolean }) {
  if (isDownloading) {
    return (
      <>
        <Download className="h-4 w-4 transition-opacity group-hover:opacity-30" />
        <X className="absolute h-4 w-4 opacity-0 transition-opacity group-hover:opacity-100" />
      </>
    );
  }

  return <Download className="h-4 w-4" />;
}

export function RemoteModelListItemActions({
  downloadOptions,
  flags,
  hasExactDetails,
  hasFileGroups,
  isHydratingDetails,
  isMenuOpen,
  model,
  downloadKey,
  progressDegrees,
  selectedGroups,
  selectedTotalBytes,
  onCancelDownload,
  onClearSelection,
  onCloseMenu,
  onHydrateModelDetails,
  onOpenUrl,
  onPauseDownload,
  onResumeDownload,
  onStartDownload,
  onToggleGroup,
  onToggleMenu,
}: RemoteModelActionProps) {
  const ringDegrees = flags.isQueued ? 60 : progressDegrees;

  return (
    <div className="relative flex flex-col items-center gap-1">
      <IconButton icon={<ExternalLink />} tooltip="Open" onClick={() => onOpenUrl(model.url)} size="sm" />
      {flags.isDownloading && !flags.isQueued && !flags.isPausing && (
        <IconButton
          icon={<span className="text-[10px] font-bold">| |</span>}
          tooltip="Pause download"
          onClick={() => void onPauseDownload(downloadKey)}
          size="sm"
        />
      )}
      {(flags.isPaused || flags.isErrored) && (
        <IconButton
          icon={<Download />}
          tooltip={flags.isPaused ? 'Resume download' : 'Retry download'}
          onClick={() => void onResumeDownload(downloadKey)}
          size="sm"
        />
      )}
      {flags.isDownloading && downloadOptions.length > 0 && (
        <IconButton icon={<Download />} tooltip="Queue another download" onClick={onToggleMenu} size="sm" />
      )}
      <button
        onClick={() =>
          handlePrimaryDownloadClick({
            downloadOptions,
            flags,
            hasExactDetails,
            isMenuOpen,
            model,
            downloadKey,
            onCancelDownload,
            onCloseMenu,
            onHydrateModelDetails,
            onStartDownload,
            onToggleMenu,
          })
        }
        className={`group flex-shrink-0 transition-colors ${
          isMenuOpen
            ? 'text-[hsl(var(--launcher-accent-primary))]'
            : 'text-[hsl(var(--text-muted))] hover:text-[hsl(var(--launcher-accent-primary))]'
        }`}
        title={getPrimaryDownloadTitle(flags)}
        aria-label={getPrimaryDownloadLabel(flags)}
        aria-pressed={isMenuOpen}
      >
        <span className="relative flex h-4 w-4 items-center justify-center">
          <DownloadProgressRings flags={flags} ringDegrees={ringDegrees} />
          <PrimaryDownloadIcon isDownloading={flags.isDownloading} />
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
          collectSelectedFilenames={() => collectSelectedRemoteFilenames(downloadOptions, selectedGroups)}
        />
      )}
    </div>
  );
}
