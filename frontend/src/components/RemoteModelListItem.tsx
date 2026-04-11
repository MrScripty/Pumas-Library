import React from 'react';
import {
  Blocks,
  ChartPie,
  ChartSpline,
  Cpu,
  Download,
  ExternalLink,
  Key,
  Loader2,
  UserRound,
  UserRoundSearch,
  X,
} from 'lucide-react';
import type { RemoteModelInfo } from '../types/apps';
import type { DownloadStatus } from '../hooks/useModelDownloads';
import { ModelKindIcon } from './ModelKindIcon';
import {
  formatDownloadSize,
  formatDownloads,
  formatReleaseDate,
  resolveReleaseIcon,
} from '../utils/modelFormatters';
import { IconButton, ListItem, MetadataItem } from './ui';

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

function formatDownloadSizeRange(model: RemoteModelInfo, isHydrating: boolean): string {
  const optionSizes = model.downloadOptions?.map((option) => option.sizeBytes) ?? [];
  const validSizes = optionSizes.filter((size): size is number => typeof size === 'number' && size > 0);
  if (validSizes.length > 1) {
    const min = Math.min(...validSizes);
    const max = Math.max(...validSizes);
    const formatValue = (bytes: number) => {
      const gb = bytes / (1024 ** 3);
      return gb >= 10 ? gb.toFixed(1) : gb.toFixed(2);
    };
    return `${formatValue(min)}-${formatValue(max)} GB`;
  }
  if (validSizes.length === 1) {
    return formatDownloadSize(validSizes[0]);
  }
  if (typeof model.totalSizeBytes === 'number' && model.totalSizeBytes > 0) {
    return formatDownloadSize(model.totalSizeBytes);
  }
  return isHydrating ? 'Loading details...' : 'Load details';
}

function getEngineStyle(engine: string): string {
  switch (engine.toLowerCase()) {
    case 'ollama':
      return 'bg-[hsl(var(--launcher-accent-primary)/0.15)] text-[hsl(var(--launcher-accent-primary))]';
    case 'llama.cpp':
      return 'bg-[hsl(var(--launcher-accent-info)/0.15)] text-[hsl(var(--launcher-accent-info))]';
    case 'candle':
    case 'transformers':
      return 'bg-[hsl(var(--launcher-accent-warning)/0.15)] text-[hsl(var(--launcher-accent-warning))]';
    case 'diffusers':
      return 'bg-[hsl(var(--launcher-accent-gpu)/0.15)] text-[hsl(var(--launcher-accent-gpu))]';
    case 'onnx-runtime':
    case 'tensorrt':
      return 'bg-[hsl(var(--launcher-accent-ram)/0.15)] text-[hsl(var(--launcher-accent-ram))]';
    default:
      return 'bg-[hsl(var(--launcher-bg-secondary)/0.5)] text-[hsl(var(--text-secondary))]';
  }
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
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className="truncate text-sm font-semibold text-[hsl(var(--text-primary))]">
              {model.name}
            </span>
          </div>
          <div className="mt-1 flex items-start justify-between gap-4 text-xs text-[hsl(var(--text-muted))]">
            <div className="flex min-w-0 flex-col gap-1">
              {model.developer && onSearchDeveloper && (
                <button
                  type="button"
                  onClick={() => onSearchDeveloper(model.developer)}
                  className="group inline-flex items-center gap-1 text-left"
                  title="Search by developer"
                >
                  <span className="inline-flex">
                    <UserRound className="h-3.5 w-3.5 group-hover:hidden" />
                    <UserRoundSearch className="hidden h-3.5 w-3.5 group-hover:inline-flex" />
                  </span>
                  {model.developer}
                </button>
              )}
              <span
                className="inline-flex items-center gap-1"
                title={model.kind}
                aria-label={model.kind}
              >
                <ModelKindIcon kind={model.kind} />
              </span>
            </div>
            <div className="flex flex-col items-end gap-1 text-[hsl(var(--text-muted))]">
              <span className="inline-flex items-center gap-1">
                <span title="Release date" aria-label="Release date" className="inline-flex">
                  {(() => {
                    const ReleaseIcon = resolveReleaseIcon(model.releaseDate);
                    return <ReleaseIcon className="h-3.5 w-3.5" />;
                  })()}
                </span>
                {formatReleaseDate(model.releaseDate)}
              </span>
              <span className="inline-flex items-center gap-1">
                <span title="Downloads" aria-label="Downloads" className="inline-flex">
                  <ChartSpline className="h-3.5 w-3.5" />
                </span>
                {formatDownloads(model.downloads)}
              </span>
            </div>
          </div>
          <div className="mt-1.5 flex flex-wrap gap-2 text-xs text-[hsl(var(--text-muted))]">
            <MetadataItem icon={<Blocks />}>
              {model.formats.length ? model.formats.join(', ') : 'Unknown'}
            </MetadataItem>
            <MetadataItem icon={<ChartPie />}>
              {quantLabels.length ? quantLabels.join(', ') : 'Unknown'}
            </MetadataItem>
            <MetadataItem icon={<Download />}>
              {formatDownloadSizeRange(model, isHydratingDetails)}
            </MetadataItem>
          </div>
          {model.compatibleEngines && model.compatibleEngines.length > 0 && (
            <div className="mt-1.5 flex flex-wrap gap-1">
              <Cpu className="mr-0.5 h-3.5 w-3.5 text-[hsl(var(--text-muted))]" />
              {model.compatibleEngines.map((engine) => (
                <span
                  key={engine}
                  className={`rounded px-1.5 py-0.5 text-[10px] font-medium ${getEngineStyle(engine)}`}
                  title={`Compatible with ${engine}`}
                >
                  {engine}
                </span>
              ))}
            </div>
          )}
          {modelError && (
            <div className="mt-1.5 text-xs text-[hsl(var(--accent-error))]">
              {modelError}
              {/\b401\b/.test(modelError) && onHfAuthClick && (
                <button
                  type="button"
                  onClick={onHfAuthClick}
                  className="ml-2 inline-flex items-center gap-1 text-[hsl(var(--accent-primary))] hover:underline"
                >
                  <Key className="h-3 w-3" />
                  Sign in to HuggingFace
                </button>
              )}
            </div>
          )}
          {retryHint && (
            <div className="mt-1 text-xs text-[hsl(var(--launcher-accent-warning))]">
              {retryHint}
            </div>
          )}
        </div>

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
            <div className="absolute right-0 top-full z-10 mt-2 min-w-[200px] rounded border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-overlay))] shadow-[0_12px_24px_hsl(var(--launcher-bg-primary)/0.6)]">
              {isHydratingDetails && !hasExactDetails ? (
                <div className="flex items-center gap-2 px-3 py-3 text-xs text-[hsl(var(--text-muted))]">
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  Loading exact download details...
                </div>
              ) : hasFileGroups ? (
                <>
                  {downloadOptions.map((option) => {
                    const label = option.fileGroup?.label ?? option.quant;
                    const shardCount = option.fileGroup?.shardCount ?? 1;
                    const checked = selectedGroups.has(label);
                    return (
                      <label
                        key={label}
                        className="flex w-full cursor-pointer items-center gap-2 px-3 py-1.5 text-xs text-[hsl(var(--text-secondary))] transition-colors hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
                      >
                        <input
                          type="checkbox"
                          checked={checked}
                          onChange={() => onToggleGroup(label)}
                          className="accent-[hsl(var(--launcher-accent-primary))]"
                        />
                        <span className="min-w-0 flex-1 truncate" title={label}>
                          {label}
                          {shardCount > 1 ? ` (${shardCount} shards)` : ''}
                        </span>
                        <span className="flex-shrink-0 text-[hsl(var(--text-muted))]">
                          {typeof option.sizeBytes === 'number' && option.sizeBytes > 0
                            ? formatDownloadSize(option.sizeBytes)
                            : ''}
                        </span>
                      </label>
                    );
                  })}
                  <div className="mt-1 flex flex-col gap-1.5 border-t border-[hsl(var(--launcher-border))] px-3 pb-2 pt-1">
                    <button
                      type="button"
                      disabled={selectedGroups.size === 0}
                      onClick={() => {
                        onCloseMenu();
                        const filenames = collectSelectedFilenames();
                        if (filenames.length > 0) {
                          void onStartDownload(model, null, filenames);
                        }
                        onClearSelection();
                      }}
                      className="w-full rounded bg-[hsl(var(--launcher-accent-primary)/0.15)] py-1.5 text-xs font-medium text-[hsl(var(--launcher-accent-primary))] transition-colors hover:bg-[hsl(var(--launcher-accent-primary)/0.25)] disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      Download selected
                      {selectedTotalBytes > 0 ? ` (${formatDownloadSize(selectedTotalBytes)})` : ''}
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        onCloseMenu();
                        void onStartDownload(model, null, null);
                        onClearSelection();
                      }}
                      className="w-full py-1 text-[10px] text-[hsl(var(--text-muted))] transition-colors hover:text-[hsl(var(--text-secondary))]"
                    >
                      All files
                      {model.totalSizeBytes ? ` (${formatDownloadSize(model.totalSizeBytes)})` : ''}
                    </button>
                  </div>
                </>
              ) : (
                <>
                  {downloadOptions.map((option) => (
                    <button
                      key={option.quant}
                      type="button"
                      onClick={() => {
                        onCloseMenu();
                        void onStartDownload(model, option.quant);
                      }}
                      className="w-full px-3 py-2 text-left text-xs text-[hsl(var(--text-secondary))] transition-colors hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
                    >
                      {option.quant}
                      {typeof option.sizeBytes === 'number' && option.sizeBytes > 0
                        ? ` (${formatDownloadSize(option.sizeBytes)})`
                        : ' (Unknown)'}
                    </button>
                  ))}
                  <button
                    type="button"
                    onClick={() => {
                      onCloseMenu();
                      void onStartDownload(model, null);
                    }}
                    className="w-full px-3 py-2 text-left text-xs text-[hsl(var(--text-secondary))] transition-colors hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)]"
                  >
                    All files
                    {model.totalSizeBytes ? ` (${formatDownloadSize(model.totalSizeBytes)})` : ''}
                  </button>
                </>
              )}
            </div>
          )}
        </div>
      </div>
    </ListItem>
  );
}
