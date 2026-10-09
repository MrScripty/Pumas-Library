import React from 'react';
import { Download, ExternalLink, X } from 'lucide-react';
import type { RemoteModelInfo } from '../types/apps';
import { IconButton, Popover, type PopoverTriggerProps } from './ui';
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
  hydrationError?: string;
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
  if (flags.isPaused || flags.isErrored) {
    return 'Cancel (delete partial)';
  }
  return 'Download options';
}

function getPrimaryDownloadLabel(flags: RemoteDownloadFlags): string {
  if (flags.isDownloading) {
    return 'Cancel download';
  }
  if (flags.isPaused || flags.isErrored) {
    return 'Cancel';
  }
  return 'Download options';
}

function handleDirectPrimaryDownload({
  flags,
  model,
  downloadKey,
  onCancelDownload,
  onCloseMenu,
  onStartDownload,
}: Pick<
  RemoteModelActionProps,
  | 'flags'
  | 'model'
  | 'downloadKey'
  | 'onCancelDownload'
  | 'onCloseMenu'
  | 'onStartDownload'
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
  void onStartDownload(model, null);
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
  hydrationError,
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
  const hasQueueAnotherTrigger = flags.isDownloading && downloadOptions.length > 0;
  const hasPrimaryMenuTrigger =
    !flags.isDownloading &&
    !flags.isPaused &&
    !flags.isErrored &&
    ((!hasExactDetails && Boolean(onHydrateModelDetails)) || hasExactDetails || downloadOptions.length > 0);
  const hasMenuTrigger = hasQueueAnotherTrigger || hasPrimaryMenuTrigger;

  const handleMenuOpenChange = (nextIsOpen: boolean) => {
    if (nextIsOpen === isMenuOpen) {
      return;
    }
    if (nextIsOpen) {
      onToggleMenu();
    } else {
      onCloseMenu();
    }
  };

  const renderActions = (menuTriggerProps?: PopoverTriggerProps) => (
    <>
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
      {hasQueueAnotherTrigger && menuTriggerProps && (
        <button
          type="button"
          {...menuTriggerProps}
          className="relative inline-flex cursor-pointer items-center justify-center rounded p-1 text-[hsl(var(--text-secondary))] transition-colors hover:bg-[hsl(var(--surface-interactive-hover))] hover:text-[hsl(var(--text-primary))]"
          title="Queue another download"
          aria-label="Queue another download"
        >
          <Download className="h-3.5 w-3.5" />
        </button>
      )}
      <button
        type="button"
        {...(hasPrimaryMenuTrigger ? menuTriggerProps : undefined)}
        onClick={
          hasPrimaryMenuTrigger && menuTriggerProps
            ? (event) => {
                menuTriggerProps.onClick(event);
                if (!isMenuOpen && !hasExactDetails && !hydrationError && onHydrateModelDetails) {
                  void onHydrateModelDetails(model);
                }
              }
            : () =>
                handleDirectPrimaryDownload({
                  flags,
                  model,
                  downloadKey,
                  onCancelDownload,
                  onCloseMenu,
                  onStartDownload,
                })
        }
        className={`group flex-shrink-0 transition-colors ${
          isMenuOpen && hasPrimaryMenuTrigger
            ? 'text-[hsl(var(--launcher-accent-primary))]'
            : 'text-[hsl(var(--text-muted))] hover:text-[hsl(var(--launcher-accent-primary))]'
        }`}
        title={getPrimaryDownloadTitle(flags)}
        aria-label={getPrimaryDownloadLabel(flags)}
      >
        <span className="relative flex h-4 w-4 items-center justify-center">
          <DownloadProgressRings flags={flags} ringDegrees={ringDegrees} />
          <PrimaryDownloadIcon isDownloading={flags.isDownloading} />
        </span>
      </button>
    </>
  );

  if (!hasMenuTrigger) {
    return <div className="relative flex flex-col items-center gap-1">{renderActions()}</div>;
  }

  return (
    <Popover
      isOpen={isMenuOpen}
      label={`Download options for ${model.name}`}
      onOpenChange={handleMenuOpenChange}
      rootClassName="relative flex flex-col items-center gap-1"
      contentClassName="absolute right-0 top-full z-10 mt-2 min-w-[200px] rounded border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-overlay))] shadow-[0_12px_24px_hsl(var(--launcher-bg-primary)/0.6)]"
      trigger={(triggerProps) => renderActions(triggerProps)}
    >
      <RemoteModelDownloadMenu
        downloadOptions={downloadOptions}
        hasExactDetails={hasExactDetails}
        hasFileGroups={hasFileGroups}
        isHydratingDetails={isHydratingDetails}
        hydrationError={hydrationError}
        onLoadDetails={onHydrateModelDetails ? () => { void onHydrateModelDetails(model); } : undefined}
        model={model}
        selectedGroups={selectedGroups}
        selectedTotalBytes={selectedTotalBytes}
        onClearSelection={onClearSelection}
        onCloseMenu={onCloseMenu}
        onStartDownload={onStartDownload}
        onToggleGroup={onToggleGroup}
        collectSelectedFilenames={() => collectSelectedRemoteFilenames(downloadOptions, selectedGroups)}
      />
    </Popover>
  );
}
