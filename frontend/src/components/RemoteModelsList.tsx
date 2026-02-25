/**
 * Remote Models List Component
 *
 * Displays HuggingFace search results with download functionality.
 * Extracted from ModelManager.tsx
 */

import React, { useState } from 'react';
import {
  Search,
  Download,
  X,
  ExternalLink,
  UserRound,
  UserRoundSearch,
  ChartSpline,
  Blocks,
  ChartPie,
  Cpu,
  Key,
} from 'lucide-react';
import type { RemoteModelInfo } from '../types/apps';
import type { DownloadStatus } from '../hooks/useModelDownloads';
import { ModelKindIcon } from './ModelKindIcon';
import {
  formatDownloadSize,
  formatReleaseDate,
  formatDownloads,
  resolveReleaseIcon,
} from '../utils/modelFormatters';
import { EmptyState, IconButton, ListItem, MetadataItem } from './ui';

interface RemoteModelsListProps {
  models: RemoteModelInfo[];
  isLoading: boolean;
  error: string | null;
  searchQuery: string;
  downloadStatusByRepo: Record<string, DownloadStatus>;
  downloadErrors: Record<string, string>;
  onStartDownload: (model: RemoteModelInfo, quant?: string | null) => Promise<void>;
  onCancelDownload: (repoId: string) => Promise<void>;
  onPauseDownload: (repoId: string) => Promise<void>;
  onResumeDownload: (repoId: string) => Promise<void>;
  onOpenUrl: (url: string) => void;
  onSearchDeveloper?: (developer: string) => void;
  onClearFilters?: () => void;
  selectedKind: string;
  onHfAuthClick?: () => void;
}

export function RemoteModelsList({
  models,
  isLoading,
  error,
  searchQuery,
  downloadStatusByRepo,
  downloadErrors,
  onStartDownload,
  onCancelDownload,
  onPauseDownload,
  onResumeDownload,
  onOpenUrl,
  onSearchDeveloper,
  onClearFilters,
  selectedKind,
  onHfAuthClick,
}: RemoteModelsListProps) {
  const [openQuantMenuRepoId, setOpenQuantMenuRepoId] = useState<string | null>(null);

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 text-xs text-[hsl(var(--text-muted))]">
        <Search className="w-3.5 h-3.5 animate-pulse" />
        <span>Searching Hugging Face...</span>
      </div>
    );
  }

  if (error) {
    return <div className="text-xs text-[hsl(var(--accent-error))]">{error}</div>;
  }

  if (models.length === 0) {
    return (
      <EmptyState
        icon={<Search />}
        message={searchQuery.trim()
          ? 'No Hugging Face models match your search.'
          : 'Type to search Hugging Face models.'}
        action={(searchQuery.trim() || selectedKind !== 'all') && onClearFilters ? {
          label: 'Clear filters',
          onClick: onClearFilters,
        } : undefined}
      />
    );
  }

  const formatDownloadSizeRange = (model: RemoteModelInfo): string => {
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
    return formatDownloadSize(model.totalSizeBytes ?? null);
  };

  /**
   * Get styling classes for inference engine badge.
   * Uses theme variables with different accent colors for recognition.
   */
  const getEngineStyle = (engine: string): string => {
    // Use theme accent colors based on engine type
    switch (engine.toLowerCase()) {
      case 'ollama':
        // Primary accent (green) for main inference engine
        return 'bg-[hsl(var(--launcher-accent-primary)/0.15)] text-[hsl(var(--launcher-accent-primary))]';
      case 'llama.cpp':
        // Info accent (cyan) for low-level inference
        return 'bg-[hsl(var(--launcher-accent-info)/0.15)] text-[hsl(var(--launcher-accent-info))]';
      case 'candle':
      case 'transformers':
        // Warning accent (orange) for ML frameworks
        return 'bg-[hsl(var(--launcher-accent-warning)/0.15)] text-[hsl(var(--launcher-accent-warning))]';
      case 'diffusers':
        // GPU accent (orange) for diffusion models
        return 'bg-[hsl(var(--launcher-accent-gpu)/0.15)] text-[hsl(var(--launcher-accent-gpu))]';
      case 'onnx-runtime':
      case 'tensorrt':
        // RAM accent (cyan) for optimized runtimes
        return 'bg-[hsl(var(--launcher-accent-ram)/0.15)] text-[hsl(var(--launcher-accent-ram))]';
      default:
        // Default to muted secondary
        return 'bg-[hsl(var(--launcher-bg-secondary)/0.5)] text-[hsl(var(--text-secondary))]';
    }
  };

  return (
    <>
      {models.map((model) => {
        const downloadStatus = downloadStatusByRepo[model.repoId];
        const isDownloading = downloadStatus
          ? ['queued', 'downloading', 'cancelling', 'pausing'].includes(downloadStatus.status)
          : false;
        const isPaused = downloadStatus?.status === 'paused';
        const isErrored = downloadStatus?.status === 'error';
        const isQueued = downloadStatus?.status === 'queued';
        const isPausing = downloadStatus?.status === 'pausing';
        const modelError = downloadErrors[model.repoId];
        const progressValue = downloadStatus?.progress ?? 0;
        const progressDegrees = Math.min(360, Math.max(0, Math.round(progressValue * 360)));
        const ringDegrees = isQueued ? 60 : progressDegrees;
        const downloadOptions = model.downloadOptions?.length
          ? model.downloadOptions
          : model.quants.map((quant) => ({
              quant,
              sizeBytes: model.quantSizes?.[quant] ?? null,
            }));
        const quantLabels = downloadOptions.map((option) => option.quant);

        return (
          <ListItem key={model.repoId}>
            <div className="flex items-start justify-between gap-2 p-2">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-semibold text-[hsl(var(--text-primary))] truncate">
                    {model.name}
                  </span>
                </div>
                <div className="mt-1 flex items-start justify-between gap-4 text-xs text-[hsl(var(--text-muted))]">
                  <div className="flex flex-col gap-1 min-w-0">
                    {model.developer && onSearchDeveloper && (
                      <button
                        type="button"
                        onClick={() => onSearchDeveloper(model.developer)}
                        className="group inline-flex items-center gap-1 text-left"
                        title="Search by developer"
                      >
                        <span className="inline-flex">
                          <UserRound className="w-3.5 h-3.5 group-hover:hidden" />
                          <UserRoundSearch className="w-3.5 h-3.5 hidden group-hover:inline-flex" />
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
                  <div className="flex flex-col gap-1 items-end text-[hsl(var(--text-muted))]">
                    <span className="inline-flex items-center gap-1">
                      <span title="Release date" aria-label="Release date" className="inline-flex">
                        {(() => {
                          const ReleaseIcon = resolveReleaseIcon(model.releaseDate);
                          return <ReleaseIcon className="w-3.5 h-3.5" />;
                        })()}
                      </span>
                      {formatReleaseDate(model.releaseDate)}
                    </span>
                    <span className="inline-flex items-center gap-1">
                      <span title="Downloads" aria-label="Downloads" className="inline-flex">
                        <ChartSpline className="w-3.5 h-3.5" />
                      </span>
                      {formatDownloads(model.downloads)}
                    </span>
                  </div>
                </div>
                <div className="flex flex-wrap gap-2 mt-1.5 text-xs text-[hsl(var(--text-muted))]">
                  <MetadataItem icon={<Blocks />}>
                    {model.formats.length ? model.formats.join(', ') : 'Unknown'}
                  </MetadataItem>
                  <MetadataItem icon={<ChartPie />}>
                    {quantLabels.length ? quantLabels.join(', ') : 'Unknown'}
                  </MetadataItem>
                  <MetadataItem icon={<Download />}>
                    {formatDownloadSizeRange(model)}
                  </MetadataItem>
                </div>
                {model.compatibleEngines && model.compatibleEngines.length > 0 && (
                  <div className="flex flex-wrap gap-1 mt-1.5">
                    <Cpu className="w-3.5 h-3.5 text-[hsl(var(--text-muted))] mr-0.5" />
                    {model.compatibleEngines.map((engine) => (
                      <span
                        key={engine}
                        className={`px-1.5 py-0.5 text-[10px] font-medium rounded ${getEngineStyle(engine)}`}
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
                        <Key className="w-3 h-3" />
                        Sign in to HuggingFace
                      </button>
                    )}
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
                {/* Pause button (when actively downloading) */}
                {isDownloading && !isQueued && !isPausing && (
                  <IconButton
                    icon={<span className="text-[10px] font-bold">| |</span>}
                    tooltip="Pause download"
                    onClick={() => void onPauseDownload(model.repoId)}
                    size="sm"
                  />
                )}
                {/* Resume button (when paused or errored) */}
                {(isPaused || isErrored) && (
                  <IconButton
                    icon={<Download />}
                    tooltip={isPaused ? 'Resume download' : 'Retry download'}
                    onClick={() => void onResumeDownload(model.repoId)}
                    size="sm"
                  />
                )}
                <button
                  onClick={() => {
                    if (isDownloading) {
                      setOpenQuantMenuRepoId(null);
                      void onCancelDownload(model.repoId);
                      return;
                    }
                    if (isPaused || isErrored) {
                      // Cancel removes the .part file
                      void onCancelDownload(model.repoId);
                      return;
                    }
                    if (downloadOptions.length > 0) {
                      setOpenQuantMenuRepoId((prev) =>
                        prev === model.repoId ? null : model.repoId
                      );
                    } else {
                      void onStartDownload(model, null);
                    }
                  }}
                  className={`group flex-shrink-0 transition-colors ${
                    openQuantMenuRepoId === model.repoId
                      ? 'text-[hsl(var(--launcher-accent-primary))]'
                      : 'text-[hsl(var(--text-muted))] hover:text-[hsl(var(--launcher-accent-primary))]'
                  }`}
                  title={isDownloading ? 'Cancel download' : isPaused ? 'Cancel (delete partial)' : 'Download options'}
                  aria-label={isDownloading ? 'Cancel download' : isPaused ? 'Cancel' : 'Download options'}
                  aria-pressed={openQuantMenuRepoId === model.repoId}
                >
                  <span className="relative flex h-4 w-4 items-center justify-center">
                    {(isDownloading || isPaused) && (
                      <>
                        <span
                          className={`download-progress-ring ${isQueued ? 'is-waiting' : ''} ${isPaused ? 'is-paused' : ''}`}
                          style={
                            {
                              '--progress': `${ringDegrees}deg`,
                            } as React.CSSProperties
                          }
                        />
                        {!isQueued && !isPaused && <span className="download-scan-ring" />}
                      </>
                    )}
                    {isDownloading || isPaused ? (
                      <>
                        <Download
                          className={`h-4 w-4 transition-opacity ${
                            isDownloading ? 'group-hover:opacity-30' : ''
                          }`}
                        />
                        <X className="absolute h-4 w-4 opacity-0 transition-opacity group-hover:opacity-100" />
                      </>
                    ) : (
                      <Download className="h-4 w-4" />
                    )}
                  </span>
                </button>
                {downloadOptions.length > 0 && openQuantMenuRepoId === model.repoId && !isDownloading && (
                  <div className="absolute right-0 top-full mt-2 min-w-[160px] rounded border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-overlay))] shadow-[0_12px_24px_hsl(var(--launcher-bg-primary)/0.6)] z-10">
                    {downloadOptions.map((option) => (
                      <button
                        key={option.quant}
                        type="button"
                        onClick={() => {
                          setOpenQuantMenuRepoId(null);
                          void onStartDownload(model, option.quant);
                        }}
                        className="w-full px-3 py-2 text-left text-xs text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors"
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
                        setOpenQuantMenuRepoId(null);
                        void onStartDownload(model, null);
                      }}
                      className="w-full px-3 py-2 text-left text-xs text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors"
                    >
                      All files
                      {model.totalSizeBytes ? ` (${formatDownloadSize(model.totalSizeBytes)})` : ''}
                    </button>
                  </div>
                )}
              </div>
            </div>
          </ListItem>
        );
      })}
    </>
  );
}
