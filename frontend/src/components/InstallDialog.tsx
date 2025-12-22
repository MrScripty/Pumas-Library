import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Download, Check, AlertCircle, Loader2, ChevronDown, ChevronUp, Package, FolderArchive, Settings, CheckCircle2, Clock, ExternalLink, File, Settings as Gear } from 'lucide-react';
import { VersionRelease, InstallationProgress } from '../hooks/useVersions';

interface InstallDialogProps {
  isOpen: boolean;
  onClose: () => void;
  availableVersions: VersionRelease[];
  installedVersions: string[];
  isLoading: boolean;
  onInstallVersion: (tag: string) => Promise<boolean>;
  onRefreshAll: (forceRefresh?: boolean) => Promise<void>;
  onRemoveVersion: (tag: string) => Promise<boolean>;
  displayMode?: 'modal' | 'page';
  installingTag?: string | null;
  installationProgress?: InstallationProgress | null;
  onRefreshProgress?: () => Promise<void>;
}

const STAGE_LABELS = {
  download: 'Downloading',
  extract: 'Extracting',
  venv: 'Creating Environment',
  dependencies: 'Installing Dependencies',
  setup: 'Final Setup'
};

const STAGE_ICONS = {
  download: Download,
  extract: FolderArchive,
  venv: Settings,
  dependencies: Package,
  setup: CheckCircle2
};

// Helper function to format bytes to human-readable size
function formatSize(bytes: number | null | undefined): string {
  if (!bytes || bytes === 0) return '';

  if (bytes < 1024) {
    return `${bytes} B`;
  } else if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  } else if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  } else {
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }
}

const formatGB = (bytes: number | null | undefined): string => {
  if (!bytes || bytes <= 0) return '0.00 GB';
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
};

const getReleaseUrl = (release: VersionRelease) =>
  release.html_url || `https://github.com/comfyanonymous/ComfyUI/releases/tag/${release.tag_name}`;

export function InstallDialog({
  isOpen,
  onClose,
  availableVersions,
  installedVersions,
  isLoading,
  onInstallVersion,
  onRefreshAll,
  onRemoveVersion,
  displayMode = 'modal',
  installingTag,
  installationProgress,
  onRefreshProgress,
}: InstallDialogProps) {
  console.log('InstallDialog render - isOpen:', isOpen, 'availableVersions:', availableVersions.length);

  const isPageMode = displayMode === 'page';
  const [showPreReleases, setShowPreReleases] = useState(true);
  const [showInstalled, setShowInstalled] = useState(true);
  const [installingVersion, setInstallingVersion] = useState<string | null>(installingTag || null);
  const [progress, setProgress] = useState<InstallationProgress | null>(installationProgress || null);
  const [errorVersion, setErrorVersion] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [showDetails, setShowDetails] = useState(false);
  const [pollInterval, setPollInterval] = useState<NodeJS.Timeout | null>(null);
  const [cancellationNotice, setCancellationNotice] = useState<string | null>(null);
  const [noticeTimeout, setNoticeTimeout] = useState<NodeJS.Timeout | null>(null);
  const cancellationRef = useRef(false);
  const [hoveredTag, setHoveredTag] = useState<string | null>(null);

  // Sync local state with external installation progress/tag
  useEffect(() => {
    if (installationProgress) {
      setProgress(installationProgress);
      if (installationProgress.tag) {
        setInstallingVersion(installationProgress.tag);
      }
    }
  }, [installationProgress]);

  useEffect(() => {
    if (installingTag) {
      setInstallingVersion(installingTag);
    } else if (!installationProgress || installationProgress.completed_at) {
      setInstallingVersion(null);
    }
  }, [installingTag, installationProgress]);

  // Filter versions based on user preferences
  const filteredVersions = availableVersions.filter((release) => {
    // Filter pre-releases
    if (!showPreReleases && release.prerelease) {
      return false;
    }

    // Filter installed versions
    if (!showInstalled && installedVersions.includes(release.tag_name)) {
      return false;
    }

    return true;
  });

  // Poll for progress updates when installing
  useEffect(() => {
    // When external polling is provided, rely on that source and skip local polling
    if (onRefreshProgress) {
      return;
    }

    if (!installingVersion) {
      if (pollInterval) {
        clearInterval(pollInterval);
        setPollInterval(null);
      }
      setProgress(null);
      return;
    }

    const fetchProgress = async () => {
      try {
        const result = await (window as any).pywebview.api.get_installation_progress();
        setProgress(result);

        // Stop polling if installation is complete
        if (result?.completed_at) {
          setTimeout(() => {
            if (pollInterval) {
              clearInterval(pollInterval);
              setPollInterval(null);
            }
            // Check if this was a cancellation vs a real failure
            const wasCancelled = result?.error?.toLowerCase().includes('cancel');
            // Reset quickly for cancellations (1.5s), slower for success/failure (3s)
            const resetDelay = wasCancelled ? 1500 : 3000;

            setTimeout(() => {
              setInstallingVersion(null);
              setProgress(null);
              setShowDetails(false);
            }, resetDelay);
          }, 1000);
        }

        if (result?.error?.toLowerCase().includes('cancel')) {
          cancellationRef.current = true;
          showCancellationNotice();
        }
      } catch (error) {
        console.error('Failed to fetch installation progress:', error);
      }
    };

    // Initial fetch
    fetchProgress();

    // Poll every second
    const interval = setInterval(fetchProgress, 1000);
    setPollInterval(interval);

    return () => {
      clearInterval(interval);
    };
  }, [installingVersion]);

  useEffect(() => {
    return () => {
      if (noticeTimeout) {
        clearTimeout(noticeTimeout);
      }
    };
  }, [noticeTimeout]);

  // Handle completion when progress is driven externally (hook polling)
  useEffect(() => {
    if (!onRefreshProgress) {
      return;
    }

    if (!progress || !progress.completed_at) {
      return;
    }

    const wasCancelled = progress?.error?.toLowerCase().includes('cancel');
    const resetDelay = wasCancelled ? 1500 : 3000;

    const timer = setTimeout(() => {
      setInstallingVersion(null);
      setProgress(null);
      setShowDetails(false);
    }, resetDelay);

    return () => clearTimeout(timer);
  }, [progress, onRefreshProgress]);

  const showCancellationNotice = () => {
    if (noticeTimeout) {
      clearTimeout(noticeTimeout);
    }

    setCancellationNotice('Installation canceled');
    const timeoutId = setTimeout(() => setCancellationNotice(null), 3000);
    setNoticeTimeout(timeoutId);
  };

  const handleInstall = async (tag: string) => {
    setInstallingVersion(tag);
    setErrorVersion(null);
    setErrorMessage(null);
    setShowDetails(false);
    cancellationRef.current = false;
    if (noticeTimeout) {
      clearTimeout(noticeTimeout);
      setNoticeTimeout(null);
    }
    setCancellationNotice(null);

    try {
      await onInstallVersion(tag);
      if (onRefreshProgress) {
        await onRefreshProgress();
      }
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);

      // Check if this was a cancellation - if so, don't show error in version list
      // The progress UI will already show the cancellation message
      const isCancellation = cancellationRef.current || message.toLowerCase().includes('cancel');

      if (!isCancellation) {
        setErrorVersion(tag);
        setErrorMessage(message);
      } else {
        showCancellationNotice();
      }

      setInstallingVersion(null);
      setProgress(null);
    }
  };

  const handleCancelInstallation = async () => {
    if (!window.confirm('Are you sure you want to cancel the installation? This will stop the process and remove any partially installed files.')) {
      return;
    }

    try {
      console.log('Cancelling installation...');
      const result = await (window as any).pywebview.api.cancel_installation();
      if (result.success) {
        console.log('Installation cancelled successfully');
        cancellationRef.current = true;
        showCancellationNotice();
      } else {
        console.error('Failed to cancel installation:', result.error);
      }
    } catch (error) {
      console.error('Error cancelling installation:', error);
    }
  };

  // Calculate sizes for releases in the background when dialog opens (Phase 6.2.5c)
  useEffect(() => {
    if (!isOpen || availableVersions.length === 0) {
      return;
    }

    // Check if we have any releases without size data
    const releasesNeedingSize = availableVersions.filter(
      release => release.total_size === null || release.total_size === undefined
    );

    if (releasesNeedingSize.length === 0) {
      return; // All releases already have size data
    }

    // Calculate sizes in the background for releases that need it
    const calculateSizes = async () => {
      console.log(`Calculating sizes for ${releasesNeedingSize.length} releases in background...`);

      for (const release of releasesNeedingSize) {
        try {
          // Calculate size for this release (non-blocking)
          await (window as any).pywebview.api.calculate_release_size(release.tag_name, false);
        } catch (error) {
          console.error(`Failed to calculate size for ${release.tag_name}:`, error);
        }
      }

      // Refresh the available versions to get updated size data
      console.log('Size calculation complete, refreshing versions...');
      await onRefreshAll(false);
    };

    // Start calculation in background (non-blocking)
    calculateSizes().catch(error => {
      console.error('Error during background size calculation:', error);
    });
  }, [isOpen, availableVersions, onRefreshAll]);

  const isInstalled = (tag: string) => installedVersions.includes(tag);

  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    return date.toLocaleDateString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric'
    });
  };

  const formatBytes = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  const formatSpeed = (bytesPerSec: number): string => {
    return `${formatBytes(bytesPerSec)}/s`;
  };

  const formatETA = (seconds: number): string => {
    if (seconds < 60) return `${Math.round(seconds)}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${Math.round(seconds % 60)}s`;
    return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
  };

  const formatElapsedTime = (startedAt: string): string => {
    const start = new Date(startedAt);
    const now = new Date();
    const elapsed = Math.floor((now.getTime() - start.getTime()) / 1000);
    return formatETA(elapsed);
  };

  const openReleaseLink = async (url: string) => {
    try {
      if ((window as any).pywebview?.api?.open_url) {
        const result = await (window as any).pywebview.api.open_url(url);
        if (!result?.success) {
          window.open(url, '_blank');
        }
      } else {
        window.open(url, '_blank');
      }
    } catch (err) {
      console.error('Failed to open release link:', err);
      window.open(url, '_blank');
    }
  };

  // Close on escape key
  useEffect(() => {
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };

    if (isOpen && !isPageMode) {
      window.addEventListener('keydown', handleEscape);
      return () => window.removeEventListener('keydown', handleEscape);
    }
  }, [isOpen, isPageMode, onClose]);

  const CurrentStageIcon = progress ? STAGE_ICONS[progress.stage] : Loader2;

  const containerClasses = isPageMode
    ? 'bg-[#2a2a2a] border border-[#444] rounded-lg shadow-inner w-full h-full flex flex-col'
    : 'bg-[#2a2a2a] border border-[#444] rounded-lg shadow-2xl w-full max-w-3xl max-h-[80vh] flex flex-col';

  const dialogContent = (
    <div className={containerClasses} onClick={(e) => !isPageMode && e.stopPropagation()}>
      {/* Header */}
      { !isPageMode && (
        <div className="flex items-center justify-between p-4 border-b border-[#444]">
          <div className="flex items-center gap-3">
            <h2 className="text-xl font-semibold text-white">
              {installingVersion ? `Installing ${installingVersion}` : 'Install ComfyUI Version'}
            </h2>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={onClose}
              className="p-1 rounded hover:bg-[#444] transition-colors"
            >
              <X size={20} className="text-gray-400" />
            </button>
          </div>
        </div>
      )}

      {/* Version List or Installation Progress */}
      <div className="flex-1 overflow-y-auto p-4 pt-0">
        <AnimatePresence>
          {cancellationNotice && (
            <motion.div
              initial={{ opacity: 0, y: -6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              className="mb-3 rounded border border-yellow-500/30 bg-yellow-500/10 px-3 py-2 text-sm text-yellow-400"
            >
              <div className="flex items-center gap-2">
                <AlertCircle size={14} />
                <span>{cancellationNotice}</span>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {installingVersion && progress ? (
          /* Installation Progress View */
          <div className="space-y-4">
            {/* Overall Progress */}
            <div>
              <div className="flex items-center justify-between mb-2">
                <span className="text-sm font-medium text-gray-300">Overall Progress</span>
                <span className="text-sm font-semibold text-white">{progress.overall_progress}%</span>
              </div>
              <div className="w-full h-2 bg-[#333] rounded-full overflow-hidden">
                <motion.div
                  className="h-full bg-[#55ff55] rounded-full"
                  initial={{ width: 0 }}
                  animate={{ width: `${progress.overall_progress}%` }}
                  transition={{ duration: 0.3 }}
                />
              </div>
            </div>

            {/* Current Stage */}
            <div className="bg-[#333] rounded-lg p-4">
              <div className="flex items-start gap-3">
                <div className="p-2 bg-[#55ff55]/10 rounded-lg">
                  <CurrentStageIcon size={24} className="text-[#55ff55]" />
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center justify-between mb-1">
                    <h3 className="text-white font-medium">
                      {STAGE_LABELS[progress.stage]}
                    </h3>
                    <span className="text-sm text-gray-400">
                      {progress.stage_progress}%
                    </span>
                  </div>
                  {progress.current_item && (
                    <p className="text-sm text-gray-400 truncate">
                      {progress.current_item}
                    </p>
                  )}
                  <div className="w-full h-1.5 bg-[#222] rounded-full overflow-hidden mt-2">
                    <motion.div
                      className="h-full bg-[#55ff55]/50 rounded-full"
                      initial={{ width: 0 }}
                      animate={{ width: `${progress.stage_progress}%` }}
                      transition={{ duration: 0.3 }}
                    />
                  </div>
                </div>
              </div>
            </div>

            {/* Stage-specific Stats */}
            {(progress.download_speed !== null || progress.eta_seconds !== null) && (
              <div className="grid grid-cols-2 gap-3">
                {progress.download_speed !== null && (
                  <div className="bg-[#333] rounded-lg p-3">
                    <div className="flex items-center gap-2 mb-1">
                      <Download size={14} className="text-gray-400" />
                      <span className="text-xs text-gray-400">Speed</span>
                    </div>
                    <span className="text-base font-semibold text-white">
                      {formatSpeed(progress.download_speed)}
                    </span>
                  </div>
                )}
                {progress.eta_seconds !== null && (
                  <div className="bg-[#333] rounded-lg p-3">
                    <div className="flex items-center gap-2 mb-1">
                      <Clock size={14} className="text-gray-400" />
                      <span className="text-xs text-gray-400">ETA</span>
                    </div>
                    <span className="text-base font-semibold text-white">
                      {formatETA(progress.eta_seconds)}
                    </span>
                  </div>
                )}
              </div>
            )}

            {progress.stage === 'dependencies' && progress.dependency_count !== null && (
              <div className="bg-[#333] rounded-lg p-3">
                <div className="flex items-center gap-2 mb-1">
                  <Package size={14} className="text-gray-400" />
                  <span className="text-xs text-gray-400">Packages</span>
                </div>
                <span className="text-base font-semibold text-white">
                  {progress.completed_dependencies} / {progress.dependency_count}
                </span>
              </div>
            )}

            {/* Elapsed Time */}
            <div className="flex items-center gap-2 text-sm text-gray-400">
              <Clock size={14} />
              <span>Elapsed: {formatElapsedTime(progress.started_at)}</span>
            </div>

            {/* Expandable Details */}
            {progress.completed_items.length > 0 && (
              <div className="bg-[#333] rounded-lg overflow-hidden">
                <button
                  onClick={() => setShowDetails(!showDetails)}
                  className="w-full flex items-center justify-between p-3 hover:bg-[#3a3a3a] transition-colors"
                >
                  <div className="flex items-center gap-2">
                    <CheckCircle2 size={14} className="text-[#55ff55]" />
                    <span className="text-sm font-medium text-white">
                      Completed Items ({progress.completed_items.length})
                    </span>
                  </div>
                  {showDetails ? (
                    <ChevronUp size={14} className="text-gray-400" />
                  ) : (
                    <ChevronDown size={14} className="text-gray-400" />
                  )}
                </button>
                <AnimatePresence>
                  {showDetails && (
                    <motion.div
                      initial={{ height: 0 }}
                      animate={{ height: 'auto' }}
                      exit={{ height: 0 }}
                      className="overflow-hidden"
                    >
                      <div className="max-h-40 overflow-y-auto p-3 pt-0 space-y-1">
                        {progress.completed_items.map((item, index) => (
                          <div
                            key={index}
                            className="flex items-center justify-between text-xs py-1 px-2 rounded hover:bg-[#2a2a2a]"
                          >
                            <span className="text-gray-300 truncate flex-1">
                              {item.name}
                            </span>
                            {item.size !== null && (
                              <span className="text-gray-500 text-xs ml-2">
                                {formatBytes(item.size)}
                              </span>
                            )}
                          </div>
                        ))}
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            )}

            {/* Error or Cancellation Message */}
            {progress.error && (
              <>
                {progress.error.toLowerCase().includes('cancel') ? (
                  /* Cancellation Message */
                  <div className="bg-yellow-500/10 border border-yellow-500/30 rounded-lg p-3 flex items-center gap-3">
                    <AlertCircle size={20} className="text-yellow-400" />
                    <div>
                      <p className="text-yellow-400 font-medium text-sm">Installation Cancelled</p>
                      <p className="text-xs text-gray-400 mt-1">
                        The installation was stopped and incomplete files have been removed
                      </p>
                    </div>
                  </div>
                ) : (
                  /* Error Message */
                  <div className="bg-red-500/10 border border-red-500/30 rounded-lg p-3 flex items-center gap-3">
                    <AlertCircle size={20} className="text-red-400" />
                    <div>
                      <p className="text-red-400 font-medium text-sm">Installation Failed</p>
                      <p className="text-xs text-gray-400 mt-1">{progress.error}</p>
                    </div>
                  </div>
                )}
              </>
            )}

            {/* Success Message */}
            {progress.completed_at && progress.success && (
              <div className="bg-[#55ff55]/10 border border-[#55ff55]/30 rounded-lg p-3 flex items-center gap-3">
                <CheckCircle2 size={20} className="text-[#55ff55]" />
                <div>
                  <p className="text-[#55ff55] font-medium text-sm">Installation Complete!</p>
                  <p className="text-xs text-gray-400 mt-1">
                    {installingVersion} has been successfully installed
                  </p>
                </div>
              </div>
            )}
          </div>
        ) : isLoading ? (
          /* Loading State */
          <div className="flex items-center justify-center py-12">
            <Loader2 size={32} className="text-gray-400 animate-spin" />
          </div>
        ) : filteredVersions.length === 0 ? (
          /* Empty State */
          <div className="flex flex-col items-center justify-center py-12 text-gray-500">
            <AlertCircle size={48} className="mb-3" />
            <p>No versions available</p>
            <p className="text-sm mt-1">Try adjusting the filters above</p>
          </div>
    ) : (
          /* Version List */
          <div className="space-y-3">
            {filteredVersions.map((release) => {
              const displayTag = release.tag_name?.replace(/^v/i, '') || release.tag_name;
              const installed = isInstalled(release.tag_name);
              const hasError = errorVersion === release.tag_name;
              const isCurrent = installingVersion === release.tag_name;
              const isDownloading = isCurrent && progress && progress.stage === 'download' && !progress.completed_at;
              const isInstalling = isCurrent && progress && progress.stage !== 'download' && !progress.completed_at;
              const isComplete = installed || (isCurrent && progress?.success && !!progress?.completed_at);
              const totalBytes = (isCurrent ? progress?.total_size : null) ?? release.total_size ?? null;
              const downloadedBytes = isCurrent ? progress?.downloaded_bytes ?? 0 : 0;
              const releaseUrl = getReleaseUrl(release);
              const isHovering = hoveredTag === release.tag_name;
              const showUninstall = installed && !isCurrent && isHovering;

              const downloadLabel =
                isDownloading && totalBytes
                  ? `${formatGB(downloadedBytes)} / ${formatGB(totalBytes)}`
                  : totalBytes
                  ? formatGB(totalBytes)
                  : '...';

              return (
                <motion.div
                  key={release.tag_name}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  onMouseEnter={() => setHoveredTag(release.tag_name)}
                  onMouseLeave={() => setHoveredTag(null)}
                  className={`bg-[#333] border rounded-lg p-3 transition-colors ${
                    installed
                      ? 'border-[#55ff55]/40'
                      : hasError
                      ? 'border-red-500/50'
                      : 'border-[#444] hover:border-[#555]'
                  }`}
                >
                  <div className="flex items-center justify-between gap-3">
                    {/* Version Info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <div className="flex flex-col min-w-0">
                          <div className="flex items-center gap-2 min-w-0">
                            <h3 className="text-white font-medium truncate">
                              {displayTag}
                            </h3>
                            {isCurrent && !isComplete && (
                              <span className="px-2 py-0.5 bg-amber-500/20 text-amber-300 text-[11px] rounded-full flex items-center gap-1">
                                <Loader2 size={12} className="animate-spin" />
                                Installing
                              </span>
                            )}
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                openReleaseLink(releaseUrl);
                              }}
                              className="p-1 rounded hover:bg-[#444] transition-colors flex-shrink-0"
                              title="Open release notes"
                            >
                              <ExternalLink size={14} className="text-gray-300" />
                            </button>
                            {release.prerelease && (
                              <span className="px-2 py-0.5 bg-yellow-500/20 text-yellow-400 text-[11px] rounded-full">
                                Pre
                              </span>
                            )}
                          </div>
                          <div className="flex items-center gap-1 text-xs text-gray-400">
                            <span>{formatDate(release.published_at)}</span>
                          </div>
                        </div>
                      </div>

                      {/* Error message */}
                      {hasError && errorMessage && (
                        <div className="mt-1 flex items-start gap-2 text-sm text-red-400 bg-red-500/10 rounded p-2">
                          <AlertCircle size={16} className="flex-shrink-0 mt-0.5" />
                          <span>{errorMessage}</span>
                        </div>
                      )}
                    </div>

                    {/* Compact Install Button */}
                    <div className="flex items-center gap-2 flex-shrink-0">
                      <motion.button
                        onClick={() => {
                          if (installed && !isCurrent) {
                            onRemoveVersion(release.tag_name).catch(err => console.error('Remove failed', err));
                          } else {
                            handleInstall(release.tag_name);
                          }
                        }}
                        disabled={isCurrent}
                        whileHover={!isCurrent ? { scale: 1.05 } : {}}
                        whileTap={!isCurrent ? { scale: 0.96 } : {}}
                        className={`flex items-center gap-2 px-3 py-2 rounded text-sm font-medium transition-colors border ${
                          isCurrent
                            ? 'bg-[#1f1f1f] border-[#888]/40 text-gray-200 cursor-not-allowed opacity-80'
                            : showUninstall
                              ? 'bg-[#2a1a1a] border-[#ff6666] text-[#ffcccc]'
                              : installed
                                ? 'bg-[#55ff55]/20 border-[#55ff55]/60 text-[#0f2b0f]'
                                : isDownloading
                                  ? 'bg-[#122812] border-[#55ff55]/70 text-[#55ff55]'
                                  : isInstalling
                                    ? 'bg-[#1f1f1f] border-[#888]/40 text-gray-200'
                                    : isComplete
                                      ? 'bg-[#55ff55]/20 border-[#55ff55]/60 text-[#0f2b0f]'
                                      : 'bg-[#2f2f2f] border-[#555] text-white hover:border-[#66ff66] hover:text-[#66ff66]'
                        }`}
                      >
                        {installed && !showUninstall ? (
                          <>
                            <Check size={16} className="text-[#0f2b0f]" />
                            <span className="text-xs font-semibold text-[#0f2b0f]">Installed</span>
                          </>
                        ) : showUninstall ? (
                          <>
                            <X size={16} className="text-[#ffcccc]" />
                            <span className="text-xs font-semibold text-[#ffcccc]">Uninstall</span>
                          </>
                        ) : isDownloading ? (
                          <>
                            <Download
                              size={16}
                              className="text-[#55ff55] animate-pulse"
                            />
                            <span className="text-xs font-semibold">{downloadLabel}</span>
                          </>
                        ) : isInstalling ? (
                          <>
                            <File size={16} className="text-[#55ff55] animate-pulse" />
                            <span className="text-xs text-gray-300">
                              Installing…
                            </span>
                          </>
                        ) : (
                          <>
                            <Download size={16} />
                            <span className="text-xs">
                              {totalBytes ? formatGB(totalBytes) : 'Size TBD'}
                            </span>
                          </>
                        )}
                      </motion.button>
                      <Gear size={16} className="text-gray-400" />
                    </div>
                  </div>
                </motion.div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );

  if (!isOpen) {
    return null;
  }

  if (isPageMode) {
    return (
      <div className="w-full h-full flex flex-col">
        {dialogContent}
      </div>
    );
  }

  return (
    <AnimatePresence>
      {isOpen && (
        <>
          {/* Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
            className="fixed inset-0 bg-black/70 z-50"
          />

          {/* Dialog */}
          <motion.div
            initial={{ opacity: 0, scale: 0.95, y: 20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.95, y: 20 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-50 flex items-center justify-center p-4"
          >
            {dialogContent}
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
