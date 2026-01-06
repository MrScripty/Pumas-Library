import React, { useState, useEffect, useRef } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Download, Check, AlertCircle, Loader2, ChevronDown, ChevronUp, Package, FolderArchive, Settings, CheckCircle2, Clock, ExternalLink, Settings as Gear, ArrowLeft, FileText, XCircle } from 'lucide-react';
import { VersionRelease, InstallationProgress } from '../hooks/useVersions';
import { ProgressRing } from './ProgressRing';
import { formatBytes, formatSpeed } from '../utils/formatters';

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
  installNetworkStatus?: 'idle' | 'downloading' | 'stalled' | 'failed';
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
  installNetworkStatus = 'idle',
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
  const [viewMode, setViewMode] = useState<'list' | 'details'>('list');
  const [showCompletedItems, setShowCompletedItems] = useState(false);
  const [pollInterval, setPollInterval] = useState<NodeJS.Timeout | null>(null);
  const [cancellationNotice, setCancellationNotice] = useState<string | null>(null);
  const [noticeTimeout, setNoticeTimeout] = useState<NodeJS.Timeout | null>(null);
  const cancellationRef = useRef(false);
  const [hoveredTag, setHoveredTag] = useState<string | null>(null);
  const [cancelHoverTag, setCancelHoverTag] = useState<string | null>(null);
  const [failedInstall, setFailedInstall] = useState<{ tag: string; log: string | null } | null>(null);

  // Sync local state with external installation progress/tag
  useEffect(() => {
    if (installationProgress) {
      setProgress(installationProgress);
      if (installationProgress.tag) {
        setInstallingVersion(installationProgress.tag);
      }

      const isCancelled = installationProgress.error?.toLowerCase().includes('cancel');
      if (installationProgress.completed_at && !installationProgress.success && installationProgress.tag && !isCancelled) {
        setFailedInstall({
          tag: installationProgress.tag,
          log: installationProgress.log_path || null,
        });
      } else if (installationProgress.completed_at && installationProgress.success && installationProgress.tag && failedInstall?.tag === installationProgress.tag) {
        setFailedInstall(null);
      }
    }
  }, [installationProgress, failedInstall]);

  useEffect(() => {
    if (installingTag) {
      setInstallingVersion(installingTag);
      if (failedInstall?.tag === installingTag) {
        setFailedInstall(null);
      }
    } else if (!installationProgress || installationProgress.completed_at) {
      setInstallingVersion(null);
    }
  }, [installingTag, installationProgress, failedInstall]);

  useEffect(() => {
    if (isOpen) {
      setViewMode('list');
    }
  }, [isOpen]);

  useEffect(() => {
    if (viewMode === 'details' && (!installingVersion || !progress)) {
      setViewMode('list');
    }
  }, [viewMode, installingVersion, progress]);

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

  const failedTag = progress && progress.completed_at && !progress.success ? progress.tag : null;
  const failedLogPath = progress && progress.completed_at && !progress.success ? progress.log_path || null : null;
  const stickyFailedTag = failedTag || failedInstall?.tag || null;
  const stickyFailedLogPath = failedLogPath || failedInstall?.log || null;

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
              setShowCompletedItems(false);
              setViewMode('list');
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
      setShowCompletedItems(false);
      setViewMode('list');
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

  const openLogPath = async (path?: string | null) => {
    if (!path || !(window as any).pywebview?.api?.open_path) return;
    try {
      await (window as any).pywebview.api.open_path(path);
    } catch (err) {
      console.error('Failed to open log path', err);
    }
  };

  const openDetailView = () => {
    if (!progress || !installingVersion) {
      return;
    }
    setViewMode('details');
    if (onRefreshProgress) {
      void onRefreshProgress();
    }
  };

  const handleInstall = async (tag: string) => {
    setInstallingVersion(tag);
    setErrorVersion(null);
    setErrorMessage(null);
    setShowCompletedItems(false);
    setViewMode('list');
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

    cancellationRef.current = true;
    setErrorVersion(null);
    setErrorMessage(null);

    try {
      console.log('Cancelling installation...');
      const result = await (window as any).pywebview.api.cancel_installation();
      if (result.success) {
        console.log('Installation cancelled successfully');
        cancellationRef.current = true;
        showCancellationNotice();
        setInstallingVersion(null);
        setProgress(null);
        setShowCompletedItems(false);
        setViewMode('list');
        setCancelHoverTag(null);
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

  const formatETA = (seconds: number): string => {
    const maxEtaSeconds = 48 * 3600 + 59 * 60;
    const clampedSeconds = Math.min(seconds, maxEtaSeconds);
    if (clampedSeconds < 60) return `${Math.round(clampedSeconds)}s`;
    if (clampedSeconds < 3600) return `${Math.floor(clampedSeconds / 60)}m ${Math.round(clampedSeconds % 60)}s`;
    return `${Math.floor(clampedSeconds / 3600)}h ${Math.floor((clampedSeconds % 3600) / 60)}m`;
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
  const showProgressDetails = viewMode === 'details' && Boolean(installingVersion && progress);

  const containerClasses = isPageMode
    ? 'w-full h-full flex flex-col'
    : 'w-full max-w-3xl max-h-[80vh] flex flex-col';

  const dialogContent = (
    <div className={containerClasses} onClick={(e) => !isPageMode && e.stopPropagation()}>
      {/* Header */}
      { !isPageMode && (
        <div className="flex items-center justify-between p-4 border-b border-[hsl(var(--border-default))]">
          <div className="flex items-center gap-3">
            <h2 className="text-xl font-semibold text-[hsl(var(--text-primary))]">
              {installingVersion ? `Installing ${installingVersion}` : 'Install ComfyUI Version'}
            </h2>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={onClose}
              className="p-1 rounded hover:bg-[hsl(var(--surface-interactive-hover))] transition-colors"
            >
              <X size={20} className="text-[hsl(var(--text-muted))]" />
            </button>
          </div>
        </div>
      )}

      {/* Version List or Installation Progress */}
      <div className="flex-1 overflow-y-auto py-4 px-0">
        <AnimatePresence>
          {cancellationNotice && (
            <motion.div
              initial={{ opacity: 0, y: -6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              className="mb-3 mx-4 rounded border border-[hsl(var(--accent-warning))]/30 bg-[hsl(var(--accent-warning))]/10 px-3 py-2 text-sm text-[hsl(var(--accent-warning))]"
            >
              <div className="flex items-center gap-2">
                <AlertCircle size={14} />
                <span>{cancellationNotice}</span>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {showProgressDetails ? (
          /* Installation Progress View */
          <div className="space-y-4 px-4">
            <div className="flex items-center justify-between gap-3">
              <button
                onClick={() => setViewMode('list')}
                className="flex items-center gap-2 px-3 py-2 rounded border border-[hsl(var(--surface-low))] bg-[hsl(var(--surface-lowest))] hover:bg-[hsl(var(--surface-low))] text-[hsl(var(--text-primary))] text-sm transition-colors"
              >
                <ArrowLeft size={14} />
                <span>Back to versions</span>
              </button>
              <div className="text-sm text-[hsl(var(--text-muted))] truncate">
                {installingVersion ? `Installing ${installingVersion}` : 'Installation details'}
              </div>
            </div>

            {/* Overall Progress */}
            <div>
              <div className="flex items-center justify-between mb-2">
                <span className="text-sm font-medium text-[hsl(var(--text-secondary))]">Overall Progress</span>
                <span className="text-sm font-semibold text-[hsl(var(--text-primary))]">{progress.overall_progress}%</span>
              </div>
              <div className="w-full h-2 bg-[hsl(var(--surface-low))] rounded-full overflow-hidden">
                <motion.div
                  className="h-full bg-[hsl(var(--accent-success))] rounded-full"
                  initial={{ width: 0 }}
                  animate={{ width: `${progress.overall_progress}%` }}
                  transition={{ duration: 0.3 }}
                />
              </div>
            </div>

            {/* Current Stage */}
            <div className="bg-[hsl(var(--surface-low))] rounded-lg p-4">
              <div className="flex items-start gap-3">
                <div className="p-2 bg-[hsl(var(--accent-success))]/10 rounded-lg">
                  <CurrentStageIcon size={24} className="text-[hsl(var(--accent-success))]" />
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center justify-between mb-1">
                    <h3 className="text-[hsl(var(--text-primary))] font-medium">
                      {STAGE_LABELS[progress.stage]}
                    </h3>
                    <span className="text-sm text-[hsl(var(--text-muted))]">
                      {progress.stage_progress}%
                    </span>
                  </div>
                  {progress.current_item && (
                    <p className="text-sm text-[hsl(var(--text-muted))] truncate">
                      {progress.current_item}
                    </p>
                  )}
                  <div className="w-full h-1.5 bg-[hsl(var(--surface-lowest))] rounded-full overflow-hidden mt-2">
                    <motion.div
                      className="h-full bg-[hsl(var(--accent-success))]/50 rounded-full"
                      initial={{ width: 0 }}
                      animate={{ width: `${progress.stage_progress}%` }}
                      transition={{ duration: 0.3 }}
                    />
                  </div>
                </div>
              </div>
            </div>

            {/* Stage-specific Stats */}
            {progress.download_speed !== null && (
              <div className="bg-[hsl(var(--surface-low))] rounded-lg p-3">
                <div className="flex items-center gap-2 mb-1">
                  <Download size={14} className="text-[hsl(var(--text-muted))]" />
                  <span className="text-xs text-[hsl(var(--text-muted))]">Speed</span>
                </div>
                <span className="text-base font-semibold text-[hsl(var(--text-primary))]">
                  {formatSpeed(progress.download_speed)}
                </span>
              </div>
            )}

            {progress.stage === 'dependencies' && progress.dependency_count !== null && (
              <div className="bg-[hsl(var(--surface-low))] rounded-lg p-3">
                <div className="flex items-center gap-2 mb-1">
                  <Package size={14} className="text-[hsl(var(--text-muted))]" />
                  <span className="text-xs text-[hsl(var(--text-muted))]">Packages</span>
                </div>
                <span className="text-base font-semibold text-[hsl(var(--text-primary))]">
                  {progress.completed_dependencies} / {progress.dependency_count}
                </span>
              </div>
            )}

            {/* Elapsed Time */}
            <div className="flex items-center gap-2 text-sm text-[hsl(var(--text-muted))]">
              <Clock size={14} />
              <span>Elapsed: {formatElapsedTime(progress.started_at)}</span>
            </div>

            {/* Expandable Details */}
            {progress.completed_items.length > 0 && (
              <div className="bg-[hsl(var(--surface-low))] rounded-lg overflow-hidden">
                <button
                  onClick={() => setShowCompletedItems(!showCompletedItems)}
                  className="w-full flex items-center justify-between p-3 hover:bg-[hsl(var(--surface-tertiary))] transition-colors"
                >
                  <div className="flex items-center gap-2">
                    <CheckCircle2 size={14} className="text-[hsl(var(--accent-success))]" />
                    <span className="text-sm font-medium text-[hsl(var(--text-primary))]">
                      Completed Items ({progress.completed_items.length})
                    </span>
                  </div>
                  {showCompletedItems ? (
                    <ChevronUp size={14} className="text-[hsl(var(--text-muted))]" />
                  ) : (
                    <ChevronDown size={14} className="text-[hsl(var(--text-muted))]" />
                  )}
                </button>
                <AnimatePresence>
                  {showCompletedItems && (
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
                            className="flex items-center justify-between text-xs py-1 px-2 rounded hover:bg-[hsl(var(--surface-lowest))]"
                          >
                            <span className="text-[hsl(var(--text-secondary))] truncate flex-1">
                              {item.name}
                            </span>
                            {item.size !== null && (
                              <span className="text-[hsl(var(--text-muted))] text-xs ml-2">
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
                  <div className="bg-[hsl(var(--accent-warning))]/10 border border-[hsl(var(--accent-warning))]/30 rounded-lg p-3 flex items-center gap-3">
                    <AlertCircle size={20} className="text-[hsl(var(--accent-warning))]" />
                    <div>
                      <p className="text-[hsl(var(--accent-warning))] font-medium text-sm">Installation Cancelled</p>
                      <p className="text-xs text-[hsl(var(--text-muted))] mt-1">
                        The installation was stopped and incomplete files have been removed
                      </p>
                    </div>
                  </div>
                ) : (
                  /* Error Message */
                  <div className="bg-[hsl(var(--accent-error))]/10 border border-[hsl(var(--accent-error))]/30 rounded-lg p-3 flex items-center gap-3">
                    <AlertCircle size={20} className="text-[hsl(var(--accent-error))]" />
                  <div>
                    <p className="text-[hsl(var(--accent-error))] font-medium text-sm">Installation Failed</p>
                    <p className="text-xs text-[hsl(var(--text-muted))] mt-1">{progress.error}</p>
                    {progress.log_path && (
                      <button
                        onClick={() => openLogPath(progress.log_path)}
                        className="mt-2 inline-flex items-center gap-2 px-2 py-1 rounded border border-[hsl(var(--accent-error))]/40 text-xs text-[hsl(var(--accent-error))] hover:bg-[hsl(var(--accent-error))]/10"
                      >
                        <FileText size={12} />
                        <span>Open log</span>
                      </button>
                    )}
                  </div>
                </div>
              )}
            </>
          )}

            {/* Success Message */}
            {progress.completed_at && progress.success && (
              <div className="bg-[hsl(var(--accent-success))]/10 border border-[hsl(var(--accent-success))]/30 rounded-lg p-3 flex items-center gap-3">
                <CheckCircle2 size={20} className="text-[hsl(var(--accent-success))]" />
                <div>
                  <p className="text-[hsl(var(--accent-success))] font-medium text-sm">Installation Complete!</p>
                  <p className="text-xs text-[hsl(var(--text-muted))] mt-1">
                    {installingVersion} has been successfully installed
                  </p>
                </div>
              </div>
            )}
          </div>
        ) : isLoading ? (
          /* Loading State */
          <div className="flex items-center justify-center py-12 px-4">
            <Loader2 size={32} className="text-[hsl(var(--text-muted))] animate-spin" />
          </div>
        ) : filteredVersions.length === 0 ? (
          /* Empty State */
          <div className="flex flex-col items-center justify-center py-12 px-4 text-[hsl(var(--text-muted))]">
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
              const currentProgress = isCurrent ? progress : null;
              const isComplete = installed || (isCurrent && currentProgress?.success && !!currentProgress?.completed_at);
              const totalBytes = (currentProgress ? currentProgress.total_size : null) ?? release.total_size ?? null;
              const releaseUrl = getReleaseUrl(release);
              const isHovering = hoveredTag === release.tag_name;
              const showUninstall = installed && !isCurrent && isHovering;
              const overallPercent = currentProgress ? Math.round(currentProgress.overall_progress || 0) : null;
              const downloadPercent =
                currentProgress && currentProgress.total_size && currentProgress.total_size > 0
                  ? Math.min(
                      100,
                      Math.round((currentProgress.downloaded_bytes / currentProgress.total_size) * 100)
                    )
                  : null;
              const stagePercent = currentProgress ? currentProgress.stage_progress : null;
              const ringPercent =
                currentProgress && (currentProgress.stage === 'download' || currentProgress.stage === 'dependencies')
                  ? downloadPercent ?? stagePercent ?? overallPercent
                  : overallPercent ?? stagePercent;
              const speedLabel = currentProgress?.download_speed !== null && currentProgress?.download_speed !== undefined
                ? formatSpeed(currentProgress.download_speed)
                : 'Waiting...';
              const packageLabel = currentProgress?.dependency_count !== null && currentProgress?.dependency_count !== undefined && currentProgress?.completed_dependencies !== null
                ? `${currentProgress.completed_dependencies}/${currentProgress.dependency_count}`
                : currentProgress?.stage === 'dependencies'
                  ? 'Installing...'
                  : 'Downloading...';
              const ringColor = currentProgress?.error ? 'hsl(var(--accent-error))' : 'hsl(var(--accent-success))';
              const isCancelHover = isCurrent && cancelHoverTag === release.tag_name;
              const downloadIconClass =
                installNetworkStatus === 'stalled'
                  ? 'animate-pulse text-[hsl(var(--accent-warning))]'
                  : installNetworkStatus === 'failed'
                    ? 'animate-pulse text-[hsl(var(--accent-error))]'
                    : 'animate-pulse text-[hsl(var(--accent-success))]';
              const downloadIconStyle =
                installNetworkStatus === 'stalled'
                  ? { filter: 'drop-shadow(0 0 6px hsl(var(--accent-warning)))' }
                  : installNetworkStatus === 'failed'
                    ? { filter: 'drop-shadow(0 0 6px hsl(var(--accent-error)))' }
                    : { filter: 'drop-shadow(0 0 6px hsl(var(--accent-success)))' };

              return (
                <motion.div
                  key={release.tag_name}
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  onMouseEnter={() => setHoveredTag(release.tag_name)}
                  onMouseLeave={() => setHoveredTag(null)}
                  className="w-full p-3 transition-colors"
                >
                  <div className="flex items-center justify-between gap-3">
                    {/* Version Info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <div className="flex flex-col min-w-0">
                          <div className="flex items-center gap-2 min-w-0">
                            <h3 className="text-[hsl(var(--text-primary))] font-medium truncate">
                              {displayTag}
                            </h3>
                          <button
                              onClick={(e) => {
                                e.stopPropagation();
                                openReleaseLink(releaseUrl);
                              }}
                              className="p-1 rounded hover:bg-[hsl(var(--surface-interactive-hover))] transition-colors flex-shrink-0"
                              title="Open release notes"
                            >
                              <ExternalLink size={14} className="text-[hsl(var(--text-secondary))]" />
                            </button>
                            {stickyFailedTag === release.tag_name && stickyFailedLogPath && (
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  openLogPath(stickyFailedLogPath);
                                }}
                                className="p-1 rounded hover:bg-[hsl(var(--surface-interactive-hover))] transition-colors flex-shrink-0"
                                title="Open last install log"
                              >
                                <FileText size={14} className="text-[hsl(var(--accent-error))]" />
                              </button>
                            )}
                            {release.prerelease && (
                              <span className="px-2 py-0.5 bg-[hsl(var(--accent-warning))]/20 text-[hsl(var(--accent-warning))] text-[11px] rounded-full">
                                Pre
                              </span>
                            )}
                          </div>
                          <div className="flex items-center gap-1 text-xs text-[hsl(var(--text-muted))]">
                            <span>{formatDate(release.published_at)}</span>
                          </div>
                        </div>
                      </div>

                      {/* Error message */}
                      {hasError && errorMessage && (
                        <div className="mt-1 flex items-start gap-2 text-sm text-[hsl(var(--accent-error))] bg-[hsl(var(--accent-error))]/10 rounded p-2">
                          <AlertCircle size={16} className="flex-shrink-0 mt-0.5" />
                          <span>{errorMessage}</span>
                        </div>
                      )}
                    </div>

                    {/* Compact Install Button */}
                    <div className="flex items-center gap-2 flex-shrink-0">
                      <motion.button
                        onClick={() => {
                          if (isCurrent) {
                            handleCancelInstallation();
                            return;
                          }
                          if (installed && !isCurrent) {
                            onRemoveVersion(release.tag_name).catch(err => console.error('Remove failed', err));
                          } else {
                            handleInstall(release.tag_name);
                          }
                        }}
                        onMouseEnter={() => {
                          if (isCurrent) {
                            setCancelHoverTag(release.tag_name);
                          }
                        }}
                        onMouseLeave={() => {
                          if (isCurrent) {
                            setCancelHoverTag(null);
                          }
                        }}
                        whileHover={!isCurrent ? { scale: 1.05 } : {}}
                        whileTap={!isCurrent ? { scale: 0.96 } : {}}
                        className={`flex items-center gap-2 px-3 py-2 rounded text-sm font-medium transition-colors border w-[120px] min-w-[120px] overflow-hidden ${
                          isCurrent
                            ? isCancelHover
                              ? 'bg-[hsl(var(--surface-lowest))] border-[hsl(var(--accent-error))] text-[hsl(var(--accent-error))]'
                              : 'bg-[hsl(var(--surface-control))] border-[hsl(var(--border-control))] text-[hsl(var(--text-primary))]'
                            : showUninstall
                              ? 'bg-[hsl(var(--surface-lowest))] border-[hsl(var(--accent-error))] text-[hsl(var(--accent-error))]'
                              : installed
                                ? 'bg-[hsl(var(--accent-success))]/20 border-[hsl(var(--accent-success))]/60 text-[hsl(var(--text-primary))]'
                                : isComplete
                                  ? 'bg-[hsl(var(--accent-success))]/20 border-[hsl(var(--accent-success))]/60 text-[hsl(var(--text-primary))]'
                                  : 'bg-[hsl(var(--surface-control))] border-[hsl(var(--border-control))] text-[hsl(var(--text-primary))] hover:border-[hsl(var(--accent-success))] hover:text-[hsl(var(--accent-success))]'
                        }`}
                      >
                        {installed && !showUninstall ? (
                          <>
                            <Check size={16} className="text-[hsl(var(--text-primary))]" />
                            <span className="text-xs font-semibold text-[hsl(var(--text-primary))] truncate whitespace-nowrap flex-1 min-w-0">Ready</span>
                          </>
                        ) : showUninstall ? (
                          <>
                            <X size={16} className="text-[hsl(var(--accent-error))]" />
                            <span className="text-xs font-semibold text-[hsl(var(--accent-error))] truncate whitespace-nowrap flex-1 min-w-0">Uninstall</span>
                          </>
                        ) : isCurrent ? (
                          <>
                            {isCancelHover ? (
                              <XCircle size={16} className="text-[hsl(var(--accent-error))]" />
                            ) : (
                              <ProgressRing
                                progress={ringPercent ?? 0}
                                size={18}
                                strokeWidth={2}
                                trackColor="hsl(var(--surface-control))"
                                indicatorColor={ringColor}
                              >
                                <Download size={14} className={downloadIconClass} style={downloadIconStyle} />
                              </ProgressRing>
                            )}
                            {isCancelHover ? (
                              <span className="text-xs font-semibold text-[hsl(var(--accent-error))] truncate whitespace-nowrap flex-1 min-w-0">Cancel</span>
                            ) : (
                              <span className="text-xs font-semibold truncate whitespace-nowrap flex-1 min-w-0">{packageLabel}</span>
                            )}
                          </>
                        ) : (
                          <>
                            <Download size={16} />
                            <span className="text-xs truncate whitespace-nowrap flex-1 min-w-0">
                              {totalBytes ? formatGB(totalBytes) : 'Size TBD'}
                            </span>
                          </>
                        )}
                      </motion.button>
                      <Gear size={16} className="text-[hsl(var(--text-muted))]" />
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
