import React, { useEffect, useMemo, useState } from 'react';
import {
  ArrowLeftRight,
  ArrowRight,
  AudioWaveform,
  Box,
  Blocks,
  Calendar,
  CalendarCheck,
  CalendarFold,
  Calendars,
  ChartSpline,
  ChartPie,
  Download,
  ExternalLink,
  Filter,
  Folder,
  HardDrive,
  Image,
  Languages,
  Search,
  Shapes,
  Star,
  Tag,
  TvMinimalPlay,
  UserRound,
  UserRoundSearch,
  X,
} from 'lucide-react';
import type { ModelCategory, RemoteModelInfo } from '../types/apps';

interface ModelManagerProps {
  modelGroups: ModelCategory[];
  starredModels: Set<string>;
  linkedModels: Set<string>;
  onToggleStar: (modelId: string) => void;
  onToggleLink: (modelId: string) => void;
  selectedAppId: string | null;
  onAddModels?: () => void;
  onOpenModelsRoot?: () => void;
}

export const ModelManager: React.FC<ModelManagerProps> = ({
  modelGroups,
  starredModels,
  linkedModels,
  onToggleStar,
  onToggleLink,
  selectedAppId,
  onAddModels,
  onOpenModelsRoot,
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('all');
  const [showCategoryMenu, setShowCategoryMenu] = useState(false);
  const [isDownloadMode, setIsDownloadMode] = useState(false);
  const [selectedKind, setSelectedKind] = useState<string>('all');
  const [remoteResults, setRemoteResults] = useState<RemoteModelInfo[]>([]);
  const [remoteError, setRemoteError] = useState<string | null>(null);
  const [isRemoteLoading, setIsRemoteLoading] = useState(false);
  const [remoteDownloadError, setRemoteDownloadError] = useState<string | null>(null);
  const [remoteDownloadRepoId, setRemoteDownloadRepoId] = useState<string | null>(null);
  const [openQuantMenuRepoId, setOpenQuantMenuRepoId] = useState<string | null>(null);
  const [downloadStatusByRepo, setDownloadStatusByRepo] = useState<Record<string, {
    downloadId: string;
    status: 'queued' | 'downloading' | 'cancelling' | 'completed' | 'cancelled' | 'error';
    progress: number;
    downloadedBytes?: number;
    totalBytes?: number;
  }>>({});
  const downloadStatusRef = React.useRef(downloadStatusByRepo);
  const downloadPollingRef = React.useRef<number | null>(null);

  // Get all unique categories
  const categories = useMemo(() => {
    const cats = modelGroups.map((g: ModelCategory) => g.category);
    return ['all', ...cats];
  }, [modelGroups]);

  // Filter models based on search and filters
  const filteredGroups = useMemo(() => {
    let groups = modelGroups;

    // Filter by category
    if (selectedCategory !== 'all') {
      groups = groups.filter((g: ModelCategory) => g.category === selectedCategory);
    }

    // Filter by search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      groups = groups.map((group: ModelCategory) => ({
        ...group,
        models: group.models.filter(model =>
          model.name.toLowerCase().includes(query) ||
          model.path?.toLowerCase().includes(query)
        ),
      })).filter((group: ModelCategory) => group.models.length > 0);
    }

    return groups;
  }, [modelGroups, searchQuery, selectedCategory, starredModels]);

  // Count total models
  const totalModels = useMemo(() => {
    return modelGroups.reduce((sum: number, group: ModelCategory) => sum + group.models.length, 0);
  }, [modelGroups]);
  const isCategoryFiltered = isDownloadMode ? selectedKind !== 'all' : selectedCategory !== 'all';

  const remoteKinds = useMemo(() => {
    const kinds = new Set<string>();
    remoteResults.forEach((model) => {
      if (model.kind && model.kind !== 'unknown') {
        kinds.add(model.kind);
      }
    });
    return ['all', ...Array.from(kinds).sort()];
  }, [remoteResults]);

  const filteredRemoteResults = useMemo(() => {
    const getReleaseTimestamp = (date?: string) => {
      if (!date) return 0;
      const parsed = new Date(date);
      const time = parsed.getTime();
      return Number.isNaN(time) ? 0 : time;
    };

    const filtered = selectedKind === 'all'
      ? remoteResults
      : remoteResults.filter((model) => model.kind === selectedKind);

    return [...filtered].sort(
      (a, b) => getReleaseTimestamp(b.releaseDate) - getReleaseTimestamp(a.releaseDate)
    );
  }, [remoteResults, selectedKind]);

  useEffect(() => {
    if (!isDownloadMode) {
      return;
    }

    const trimmedQuery = searchQuery.trim();
    if (!trimmedQuery) {
      setRemoteResults([]);
      setRemoteError(null);
      setIsRemoteLoading(false);
      return;
    }

    let isActive = true;
    const handle = setTimeout(async () => {
      if (!window.pywebview?.api?.search_hf_models) {
        if (isActive) {
          setRemoteError('Hugging Face search is unavailable.');
          setRemoteResults([]);
          setIsRemoteLoading(false);
        }
        return;
      }

      setIsRemoteLoading(true);
      setRemoteError(null);
      try {
        const result = await window.pywebview.api.search_hf_models(trimmedQuery, null, 25);
        if (!isActive) {
          return;
        }
        if (result.success) {
          setRemoteResults(result.models as RemoteModelInfo[]);
        } else {
          setRemoteError(result.error || 'Search failed.');
          setRemoteResults([]);
        }
      } catch (err) {
        if (!isActive) {
          return;
        }
        const message = err instanceof Error ? err.message : 'Search failed.';
        setRemoteError(message);
        setRemoteResults([]);
      } finally {
        if (isActive) {
          setIsRemoteLoading(false);
        }
      }
    }, 300);

    return () => {
      isActive = false;
      clearTimeout(handle);
    };
  }, [isDownloadMode, searchQuery]);

  useEffect(() => {
    downloadStatusRef.current = downloadStatusByRepo;
  }, [downloadStatusByRepo, remoteDownloadRepoId]);

  useEffect(() => {
    const hasActiveDownloads = Object.values(downloadStatusByRepo).some((status) =>
      ['queued', 'downloading', 'cancelling'].includes(status.status)
    );

    if (!hasActiveDownloads) {
      if (downloadPollingRef.current) {
        window.clearInterval(downloadPollingRef.current);
        downloadPollingRef.current = null;
      }
      return;
    }

    if (downloadPollingRef.current) {
      return;
    }

    downloadPollingRef.current = window.setInterval(async () => {
      const entries = Object.entries(downloadStatusRef.current).filter(([, status]) =>
        ['queued', 'downloading', 'cancelling'].includes(status.status)
      );
      if (!window.pywebview?.api?.get_model_download_status || entries.length === 0) {
        return;
      }

      const updates = await Promise.all(
        entries.map(async ([repoId, status]) => {
          const result = await window.pywebview.api.get_model_download_status(status.downloadId);
          if (!result.success) {
            return { repoId, status: 'error' as const, error: result.error || 'Download failed.' };
          }
          return {
            repoId,
            status: (result.status || 'downloading') as
              | 'queued'
              | 'downloading'
              | 'cancelling'
              | 'completed'
              | 'cancelled'
              | 'error',
            progress: typeof result.progress === 'number' ? result.progress : 0,
            downloadedBytes: typeof result.downloaded_bytes === 'number' ? result.downloaded_bytes : undefined,
            totalBytes: typeof result.total_bytes === 'number' ? result.total_bytes : undefined,
            error: result.error,
          };
        })
      );

      setDownloadStatusByRepo((prev) => {
        const next = { ...prev };
        updates.forEach((update) => {
          if (!update) {
            return;
          }
          const previous = prev[update.repoId];
          if (!previous) {
            return;
          }
          next[update.repoId] = {
            ...previous,
            status: update.status,
            progress: update.progress ?? previous.progress,
            downloadedBytes: update.downloadedBytes ?? previous.downloadedBytes,
            totalBytes: update.totalBytes ?? previous.totalBytes,
          };
          if (update.status === 'error') {
            setRemoteDownloadRepoId(update.repoId);
            setRemoteDownloadError(update.error || 'Download failed.');
          } else if (update.status === 'completed' || update.status === 'cancelled') {
            if (remoteDownloadRepoId === update.repoId) {
              setRemoteDownloadError(null);
            }
          }
        });
        return next;
      });
    }, 800);

    return () => {
      if (downloadPollingRef.current) {
        window.clearInterval(downloadPollingRef.current);
        downloadPollingRef.current = null;
      }
    };
  }, [downloadStatusByRepo, remoteDownloadRepoId]);

  // Format file size
  const formatSize = (bytes?: number): string => {
    if (!bytes) return 'Unknown';
    const gb = bytes / (1024 ** 3);
    if (gb >= 1) return `${gb.toFixed(2)} GB`;
    const mb = bytes / (1024 ** 2);
    return `${mb.toFixed(2)} MB`;
  };

  // Format date
  const formatDate = (dateStr?: string): string => {
    if (!dateStr) return 'Unknown';
    try {
      return new Date(dateStr).toLocaleDateString();
    } catch {
      return 'Unknown';
    }
  };

  const resolveReleaseIcon = (dateStr?: string) => {
    if (!dateStr) return Calendar;
    const parsed = new Date(dateStr);
    if (Number.isNaN(parsed.getTime())) {
      return Calendar;
    }
    const now = new Date();
    const diffMs = now.getTime() - parsed.getTime();
    const diffDays = diffMs / (1000 * 60 * 60 * 24);
    if (diffDays <= 60) {
      return CalendarCheck;
    }
    if (diffDays <= 240) {
      return CalendarFold;
    }
    return Calendars;
  };

  const formatReleaseDate = (dateStr?: string): string => {
    if (!dateStr) return 'Unknown';
    const parsed = new Date(dateStr);
    if (Number.isNaN(parsed.getTime())) {
      return 'Unknown';
    }
    return parsed.toLocaleDateString();
  };

  const formatDownloads = (downloads?: number | null): string => {
    if (typeof downloads !== 'number') {
      return 'Unknown';
    }
    return downloads.toLocaleString();
  };

  const formatDownloadSize = (bytes?: number | null): string => {
    if (typeof bytes !== 'number' || bytes <= 0) {
      return 'Unknown';
    }
    const gb = bytes / (1024 ** 3);
    const rounded = gb >= 10 ? gb.toFixed(1) : gb.toFixed(2);
    return `${rounded} GB`;
  };

  const formatDownloadSizeValue = (bytes?: number | null): string => {
    if (typeof bytes !== 'number' || bytes <= 0) {
      return 'Unknown';
    }
    const gb = bytes / (1024 ** 3);
    return gb >= 10 ? gb.toFixed(1) : gb.toFixed(2);
  };

  const formatDownloadSizeRange = (model: RemoteModelInfo): string => {
    const optionSizes = model.downloadOptions?.map((option) => option.sizeBytes) ?? [];
    const validSizes = optionSizes.filter((size) => typeof size === 'number' && size > 0);
    if (validSizes.length > 1) {
      const min = Math.min(...validSizes);
      const max = Math.max(...validSizes);
      return `${formatDownloadSizeValue(min)}-${formatDownloadSizeValue(max)} GB`;
    }
    if (validSizes.length === 1) {
      return formatDownloadSize(validSizes[0]);
    }
    const fallbackSizes = model.quantSizes ? Object.values(model.quantSizes) : [];
    const fallbackValidSizes = fallbackSizes.filter((size) => typeof size === 'number' && size > 0);
    if (fallbackValidSizes.length > 1) {
      const min = Math.min(...fallbackValidSizes);
      const max = Math.max(...fallbackValidSizes);
      return `${formatDownloadSizeValue(min)}-${formatDownloadSizeValue(max)} GB`;
    }
    if (fallbackValidSizes.length === 1) {
      return formatDownloadSize(fallbackValidSizes[0]);
    }
    return formatDownloadSize(model.totalSizeBytes ?? null);
  };

  const resolveDownloadModelType = (kind: string) => {
    const normalized = kind.toLowerCase();
    if (normalized.includes('image') || normalized.includes('video') || normalized.includes('3d')) {
      return 'diffusion';
    }
    return 'llm';
  };

  const startRemoteDownload = async (model: RemoteModelInfo, quant?: string | null) => {
    if (!window.pywebview?.api?.start_model_download_from_hf) {
      setRemoteDownloadError('Download is unavailable.');
      return;
    }

    const repoId = model.repoId;
    const developer = model.developer || repoId.split('/')[0] || 'huggingface';
    const officialName = model.name || repoId;
    const modelType = resolveDownloadModelType(model.kind || '');

    setRemoteDownloadError(null);
    setRemoteDownloadRepoId(repoId);
    try {
      const result = await window.pywebview.api.start_model_download_from_hf(
        repoId,
        developer,
        officialName,
        modelType,
        model.kind || '',
        quant || null
      );
      if (!result.success || !result.download_id) {
        setRemoteDownloadError(result.error || 'Download failed.');
        return;
      }
      setDownloadStatusByRepo((prev) => ({
        ...prev,
        [repoId]: {
          downloadId: result.download_id,
          status: 'queued',
          progress: 0,
          totalBytes: result.total_bytes,
        },
      }));
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Download failed.';
      setRemoteDownloadError(message);
    }
  };

  const cancelRemoteDownload = async (repoId: string) => {
    const status = downloadStatusByRepo[repoId];
    if (!status?.downloadId || !window.pywebview?.api?.cancel_model_download) {
      return;
    }

    try {
      await window.pywebview.api.cancel_model_download(status.downloadId);
      setRemoteDownloadError(null);
      setDownloadStatusByRepo((prev) => ({
        ...prev,
        [repoId]: {
          ...status,
          status: 'cancelling',
        },
      }));
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Cancel failed.';
      setRemoteDownloadError(message);
      setRemoteDownloadRepoId(repoId);
    }
  };

  const resolveKindIcon = (token: string) => {
    const normalized = token.toLowerCase();
    if (normalized.includes('classification')) return Shapes;
    if (normalized.includes('text')) return Languages;
    if (normalized.includes('image')) return Image;
    if (normalized.includes('audio')) return AudioWaveform;
    if (normalized.includes('video')) return TvMinimalPlay;
    if (normalized.includes('3d')) return Box;
    return Tag;
  };

  const resolveKindLabel = (token: string) => {
    const normalized = token.toLowerCase();
    if (normalized.includes('classification')) return 'Classification';
    if (normalized.includes('text')) return 'Text';
    if (normalized.includes('image')) return 'Image';
    if (normalized.includes('audio')) return 'Audio';
    if (normalized.includes('video')) return 'Video';
    if (normalized.includes('3d')) return '3D';
    return 'Unknown';
  };

  const renderKindToken = (token: string) => {
    const Icon = resolveKindIcon(token);
    const label = resolveKindLabel(token);
    return (
      <span title={label} aria-label={label} className="inline-flex">
        <Icon className="w-3.5 h-3.5" />
      </span>
    );
  };

  const renderKindIcons = (kind: string) => {
    if (!kind || kind === 'unknown') {
      return (
        <span title="Unknown" aria-label="Unknown" className="inline-flex">
          <Tag className="w-3.5 h-3.5" />
        </span>
      );
    }

    const normalized = kind.toLowerCase();
    if (!normalized.includes('-to-')) {
      const tokens = normalized.split('-').filter(Boolean);
      if (tokens.length <= 1) {
        return renderKindToken(normalized);
      }
      return (
        <>
          {tokens.map((token) => (
            <React.Fragment key={token}>{renderKindToken(token)}</React.Fragment>
          ))}
        </>
      );
    }

    const [fromRaw, toRaw] = normalized.split('-to-');
    const fromTokens = fromRaw.split('-').filter(Boolean);
    const toTokens = toRaw.split('-').filter(Boolean);
    const isBidirectional =
      fromTokens.length === toTokens.length &&
      fromTokens.every((token) => toTokens.includes(token));
    const ArrowIcon = isBidirectional ? ArrowLeftRight : ArrowRight;
    const arrowLabel = isBidirectional ? 'Bidirectional' : 'To';

    return (
      <>
        {fromTokens.length > 0
          ? fromTokens.map((token, index) => (
              <React.Fragment key={`from-${token}-${index}`}>
                {renderKindToken(token)}
              </React.Fragment>
            ))
          : renderKindToken(fromRaw)}
        <span title={arrowLabel} aria-label={arrowLabel} className="inline-flex">
          <ArrowIcon className="w-3.5 h-3.5 opacity-70" />
        </span>
        {toTokens.length > 0
          ? toTokens.map((token, index) => (
              <React.Fragment key={`to-${token}-${index}`}>
                {renderKindToken(token)}
              </React.Fragment>
            ))
          : renderKindToken(toRaw)}
      </>
    );
  };

  const openRemoteUrl = (url: string) => {
    if (window.pywebview?.api?.open_url) {
      void window.pywebview.api.open_url(url);
      return;
    }
    window.open(url, '_blank', 'noopener');
  };

  return (
    <div className="flex-1 bg-[hsl(var(--launcher-bg-tertiary)/0.2)] overflow-hidden flex flex-col">
      {/* Header */}
      <div className="border-b border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary))]">
        <div className="p-4 pb-3">
          {/* Search and Filters */}
          <div className="relative flex items-center gap-2">
            <button
              type="button"
              onClick={() => setShowCategoryMenu((prev) => !prev)}
              className={`p-1.5 rounded transition-colors ${
                isCategoryFiltered
                  ? 'text-[hsl(var(--launcher-accent-primary))]'
                  : 'text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))]'
              }`}
              title={isDownloadMode ? 'Filter by model kind' : 'Filter by category'}
              aria-label={isDownloadMode ? 'Filter by model kind' : 'Filter by category'}
              aria-expanded={showCategoryMenu}
            >
              <Filter className="w-4 h-4" />
            </button>
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-[hsl(var(--launcher-text-muted))]" />
              <input
                type="text"
                placeholder={isDownloadMode ? 'Search Hugging Face models' : `Search ${totalModels} models`}
                value={searchQuery}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => setSearchQuery(e.target.value)}
                className="w-full pl-9 pr-16 py-2 text-sm bg-[hsl(var(--launcher-bg-primary))] border border-[hsl(var(--launcher-border))] rounded text-[hsl(var(--launcher-text-primary))] placeholder:text-[hsl(var(--launcher-text-muted))] focus:outline-none focus:border-[hsl(var(--launcher-accent-primary))] transition-colors"
              />
              <div className="absolute right-2 top-1/2 -translate-y-1/2 flex items-center gap-1">
                {onOpenModelsRoot && (
                  <button
                    onClick={onOpenModelsRoot}
                    className="p-1 rounded text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))] transition-colors"
                    title="Open models folder"
                    aria-label="Open models folder"
                  >
                    <Folder className="w-4 h-4" />
                  </button>
                )}
                {onAddModels && (
                  <button
                    onClick={() => {
                      setIsDownloadMode((prev) => !prev);
                      setShowCategoryMenu(false);
                    }}
                    className={`p-1 rounded transition-colors ${
                      isDownloadMode
                        ? 'text-[hsl(var(--launcher-accent-primary))]'
                        : 'text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-text-secondary))]'
                    }`}
                    title={isDownloadMode ? 'Exit download mode' : 'Search Hugging Face models'}
                    aria-label={isDownloadMode ? 'Exit download mode' : 'Search Hugging Face models'}
                    aria-pressed={isDownloadMode}
                  >
                    <Download className="w-4 h-4" />
                  </button>
                )}
              </div>
            </div>
            {showCategoryMenu && (
              <div className="absolute left-0 top-full mt-2 w-48 rounded border border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-overlay))] shadow-[0_12px_24px_hsl(var(--launcher-bg-primary)/0.6)] z-10">
                {(isDownloadMode ? remoteKinds : categories).map((cat: string) => {
                  const isSelected = isDownloadMode ? selectedKind === cat : selectedCategory === cat;
                  return (
                    <button
                      key={cat}
                      type="button"
                      onClick={() => {
                        if (isDownloadMode) {
                          setSelectedKind(cat);
                        } else {
                          setSelectedCategory(cat);
                        }
                        setShowCategoryMenu(false);
                      }}
                      className={`w-full px-3 py-2 text-left text-xs transition-colors ${
                        isSelected
                          ? 'text-[hsl(var(--launcher-accent-primary))] bg-[hsl(var(--launcher-bg-tertiary)/0.6)]'
                          : 'text-[hsl(var(--launcher-text-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)]'
                      }`}
                    >
                      {cat === 'all'
                        ? isDownloadMode
                          ? 'All Kinds'
                          : 'All Categories'
                        : cat}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Model List */}
      <div className="flex-1 overflow-y-auto">
        {isDownloadMode ? (
          <div className="p-4 space-y-3">
            {isRemoteLoading && (
              <div className="flex items-center gap-2 text-xs text-[hsl(var(--launcher-text-muted))]">
                <Search className="w-3.5 h-3.5" />
                <span>Searching Hugging Face...</span>
              </div>
            )}
            {remoteError && (
              <div className="text-xs text-[hsl(var(--launcher-accent-error))]">
                {remoteError}
              </div>
            )}
            {!isRemoteLoading && !remoteError && filteredRemoteResults.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-64 text-[hsl(var(--launcher-text-muted))]">
                <Search className="w-10 h-10 mb-3 opacity-50" />
                <p className="text-sm text-center">
                  {searchQuery.trim()
                    ? 'No Hugging Face models match your search.'
                    : 'Type to search Hugging Face models.'}
                </p>
                {(searchQuery.trim() || selectedKind !== 'all') && (
                  <button
                    onClick={() => {
                      setSearchQuery('');
                      setSelectedKind('all');
                    }}
                    className="mt-2 text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline"
                  >
                    Clear filters
                  </button>
                )}
              </div>
            ) : (
              !isRemoteLoading &&
              !remoteError &&
              filteredRemoteResults.map((model) => {
                const downloadStatus = downloadStatusByRepo[model.repoId];
                const isDownloading = downloadStatus
                  ? ['queued', 'downloading', 'cancelling'].includes(downloadStatus.status)
                  : false;
                const progressValue = downloadStatus?.progress ?? 0;
                const downloadOptions = model.downloadOptions?.length
                  ? model.downloadOptions
                  : model.quants.map((quant) => ({
                      quant,
                      sizeBytes: model.quantSizes?.[quant] ?? null,
                    }));
                const quantLabels = downloadOptions.map((option) => option.quant);

                return (
                  <div
                    key={model.repoId}
                    className="rounded transition-colors bg-[hsl(var(--launcher-bg-tertiary)/0.2)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.35)]"
                  >
                    <div className="flex items-start justify-between gap-3 p-3">
                      <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-semibold text-[hsl(var(--launcher-text-primary))] truncate">
                          {model.name}
                        </span>
                      </div>
                      <div className="mt-1 flex items-start justify-between gap-4 text-xs text-[hsl(var(--launcher-text-muted))]">
                        <div className="flex flex-col gap-1 min-w-0">
                          <button
                            type="button"
                            onClick={() => {
                              if (!model.developer) {
                                return;
                              }
                              setIsDownloadMode(true);
                              setSearchQuery(model.developer);
                              setSelectedKind('all');
                              setShowCategoryMenu(false);
                            }}
                            className="group inline-flex items-center gap-1 text-left"
                            title={model.developer ? 'Search by developer' : 'Developer unknown'}
                          >
                            <span className="inline-flex">
                              <UserRound className="w-3.5 h-3.5 group-hover:hidden" />
                              <UserRoundSearch className="w-3.5 h-3.5 hidden group-hover:inline-flex" />
                            </span>
                            {model.developer || 'Unknown'}
                          </button>
                          <span
                            className="inline-flex items-center gap-1"
                            title={model.kind}
                            aria-label={model.kind}
                          >
                            {renderKindIcons(model.kind)}
                          </span>
                        </div>
                        <div className="flex flex-col gap-1 items-end text-[hsl(var(--launcher-text-muted))]">
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
                      <div className="flex flex-wrap gap-3 mt-2 text-xs text-[hsl(var(--launcher-text-muted))]">
                        <span className="inline-flex items-center gap-1">
                          <span title="Format" aria-label="Format" className="inline-flex">
                            <Blocks className="w-3.5 h-3.5" />
                          </span>
                          {model.formats.length ? model.formats.join(', ') : 'Unknown'}
                        </span>
                        <span className="inline-flex items-center gap-1">
                          <span title="Quantization" aria-label="Quantization" className="inline-flex">
                            <ChartPie className="w-3.5 h-3.5" />
                          </span>
                          {quantLabels.length ? quantLabels.join(', ') : 'Unknown'}
                        </span>
                        <span className="inline-flex items-center gap-1">
                          <span title="Download size" aria-label="Download size" className="inline-flex">
                            <Download className="w-3.5 h-3.5" />
                          </span>
                          {formatDownloadSizeRange(model)}
                        </span>
                      </div>
                      {remoteDownloadError && remoteDownloadRepoId === model.repoId && (
                        <div className="mt-2 text-xs text-[hsl(var(--launcher-accent-error))]">
                          {remoteDownloadError}
                        </div>
                      )}
                    </div>
                    <div className="relative flex flex-col items-center gap-2">
                      <button
                        onClick={() => openRemoteUrl(model.url)}
                        className="flex-shrink-0 text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-accent-primary))] transition-colors"
                        title={`Open ${model.url}`}
                        aria-label={`Open ${model.url}`}
                      >
                        <ExternalLink className="w-4 h-4" />
                      </button>
                      <button
                        onClick={() => {
                          if (isDownloading) {
                            setOpenQuantMenuRepoId(null);
                            void cancelRemoteDownload(model.repoId);
                            return;
                          }
                          if (downloadOptions.length > 0) {
                            setOpenQuantMenuRepoId((prev) =>
                              prev === model.repoId ? null : model.repoId
                            );
                          } else {
                            void startRemoteDownload(model, null);
                          }
                          setRemoteDownloadError(null);
                          setRemoteDownloadRepoId(model.repoId);
                        }}
                        className={`group flex-shrink-0 transition-colors ${
                          openQuantMenuRepoId === model.repoId
                            ? 'text-[hsl(var(--launcher-accent-primary))]'
                            : 'text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-accent-primary))]'
                        }`}
                        title={isDownloading ? 'Cancel download' : 'Download options'}
                        aria-label={isDownloading ? 'Cancel download' : 'Download options'}
                        aria-pressed={openQuantMenuRepoId === model.repoId}
                      >
                        <span className="relative flex h-4 w-4 items-center justify-center">
                          {isDownloading && (
                            <>
                              <span
                                className="download-progress-ring"
                                style={
                                  {
                                    '--progress': `${Math.min(360, Math.max(0, Math.round(progressValue * 360)))}deg`,
                                  } as React.CSSProperties
                                }
                              />
                              <span className="download-scan-ring" />
                            </>
                          )}
                          <Download
                            className={`h-4 w-4 transition-opacity ${
                              isDownloading ? 'group-hover:opacity-30' : ''
                            }`}
                          />
                          {isDownloading && (
                            <X className="absolute h-4 w-4 opacity-0 transition-opacity group-hover:opacity-100" />
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
                                void startRemoteDownload(model, option.quant);
                              }}
                              className="w-full px-3 py-2 text-left text-xs text-[hsl(var(--launcher-text-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors"
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
                              void startRemoteDownload(model, null);
                            }}
                            className="w-full px-3 py-2 text-left text-xs text-[hsl(var(--launcher-text-secondary))] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.5)] transition-colors"
                          >
                            All files
                            {model.totalSizeBytes ? ` (${formatDownloadSize(model.totalSizeBytes)})` : ''}
                          </button>
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              );
            })
            )}
          </div>
        ) : (
          <div className="p-4 space-y-4">
            {filteredGroups.length === 0 ? (
              <div className="flex flex-col items-center justify-center h-64 text-[hsl(var(--launcher-text-muted))]">
                <HardDrive className="w-12 h-12 mb-3 opacity-50" />
                <p className="text-sm text-center">
                  {totalModels === 0
                    ? 'No models found. Add models to your library to get started.'
                    : 'No models match your filters.'}
                </p>
                {totalModels > 0 && (
                  <button
                    onClick={() => {
                      setSearchQuery('');
                      setSelectedCategory('all');
                    }}
                    className="mt-2 text-xs text-[hsl(var(--launcher-accent-primary))] hover:underline"
                  >
                    Clear filters
                  </button>
                )}
              </div>
            ) : (
              filteredGroups.map((group: ModelCategory) => (
                <div key={group.category} className="space-y-2">
                  <div className="flex items-center gap-2 px-1">
                    <Tag className="w-3.5 h-3.5 text-[hsl(var(--launcher-text-muted))]" />
                    <p className="text-xs font-semibold text-[hsl(var(--launcher-text-muted))] uppercase tracking-wider">
                      {group.category}
                    </p>
                    <span className="text-xs text-[hsl(var(--launcher-text-muted))]">
                      ({group.models.length})
                    </span>
                  </div>
                  <div className="space-y-1.5">
                    {group.models.map((model) => {
                      const isStarred = starredModels.has(model.id);
                      const isLinked = linkedModels.has(model.id);
                      return (
                        <div
                          key={model.id}
                          className={`rounded transition-colors group ${
                            isLinked
                              ? 'bg-[hsl(var(--launcher-bg-tertiary)/0.4)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.6)]'
                              : 'bg-[hsl(var(--launcher-bg-tertiary)/0.2)] hover:bg-[hsl(var(--launcher-bg-tertiary)/0.4)]'
                          }`}
                        >
                          {/* Main row */}
                          <div className="flex items-center justify-between p-2.5">
                            <div className="flex items-center gap-2 flex-1 min-w-0">
                              <button
                                onClick={() => onToggleStar(model.id)}
                                className="flex-shrink-0 text-[hsl(var(--launcher-text-muted))] hover:text-[hsl(var(--launcher-accent-primary))] transition-colors"
                              >
                                <Star className="w-4 h-4" fill={isStarred ? 'currentColor' : 'none'} />
                              </button>
                              <div className="flex-1 min-w-0">
                                <span
                                  className={`text-sm font-medium block truncate ${
                                    isLinked
                                      ? 'text-[hsl(var(--launcher-text-primary))]'
                                      : 'text-[hsl(var(--launcher-text-secondary))]'
                                  }`}
                                >
                                  {model.name}
                                </span>
                                {/* Metadata row */}
                                <div className="flex items-center gap-3 mt-1 text-xs text-[hsl(var(--launcher-text-muted))]">
                                  {model.size && (
                                    <span className="flex items-center gap-1">
                                      <HardDrive className="w-3 h-3" />
                                      {formatSize(model.size)}
                                    </span>
                                  )}
                                  {model.date && (
                                    <span className="flex items-center gap-1">
                                      <Calendar className="w-3 h-3" />
                                      {formatDate(model.date)}
                                    </span>
                                  )}
                                </div>
                              </div>
                            </div>
                            <button
                              onClick={() => onToggleLink(model.id)}
                              className={`flex-shrink-0 transition-colors cursor-pointer ${
                                isLinked
                                  ? 'text-[hsl(var(--launcher-accent-primary))] hover:text-[hsl(var(--launcher-accent-primary)/0.8)]'
                                  : 'text-[hsl(var(--launcher-text-muted))] group-hover:text-[hsl(var(--launcher-accent-primary))]'
                              }`}
                              title={isLinked ? `Linked to ${selectedAppId || 'app'}` : 'Link to current app'}
                            >
                              <ExternalLink className="w-4 h-4" />
                            </button>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              ))
            )}
          </div>
        )}
      </div>
    </div>
  );
};
