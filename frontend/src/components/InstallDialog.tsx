/**
 * Install Dialog Component (Refactored)
 *
 * Modal/page for installing app versions with progress tracking.
 * Reduced from 939 lines to orchestrator component by extracting:
 * - useInstallationProgress hook (progress polling)
 * - useInstallationState hook (UI state management)
 * - ProgressDetailsView component (detailed progress display)
 * - VersionListItem component (version cards)
 * - installationFormatters utility (formatting helpers)
 */

import { useState, useEffect, useRef } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { motion, AnimatePresence } from 'framer-motion';
import { X, Loader2, AlertCircle } from 'lucide-react';
import type { VersionRelease, InstallationProgress } from '../hooks/useVersions';
import { useInstallationProgress } from '../hooks/useInstallationProgress';
import { useInstallationState } from '../hooks/useInstallationState';
import { ProgressDetailsView } from './ProgressDetailsView';
import { VersionListItem } from './VersionListItem';
import { getLogger } from '../utils/logger';
import { APIError, NetworkError } from '../errors';

const logger = getLogger('InstallDialog');

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
  appDisplayName?: string;
  appId?: string;
  /** True when GitHub API rate limit was hit */
  isRateLimited?: boolean;
  /** Seconds until rate limit resets (if known) */
  rateLimitRetryAfter?: number | null;
}

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
  appDisplayName = 'ComfyUI',
  appId,
  isRateLimited = false,
  rateLimitRetryAfter,
}: InstallDialogProps) {
  logger.debug('Component rendered', { isOpen, availableVersionsCount: availableVersions.length, displayMode });

  const isPageMode = displayMode === 'page';
  const [installingVersion, setInstallingVersion] = useState<string | null>(installingTag || null);
  const [errorVersion, setErrorVersion] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const cancellationRef = useRef(false);

  // Custom hooks
  const {
    progress,
    cancellationNotice,
    failedInstall,
    setFailedInstall,
    showCancellationNotice,
  } = useInstallationProgress({
    appId,
    installingVersion,
    externalProgress: installationProgress,
    onRefreshProgress,
  });

  const {
    showPreReleases,
    showInstalled,
    viewMode,
    setViewMode,
    showCompletedItems,
    setShowCompletedItems,
    hoveredTag,
    setHoveredTag,
    cancelHoverTag,
    setCancelHoverTag,
  } = useInstallationState({
    isOpen,
    installingVersion,
    progress,
  });

  // Sync installingVersion with external tag
  useEffect(() => {
    if (installingTag) {
      setInstallingVersion(installingTag);
      if (failedInstall?.tag === installingTag) {
        setFailedInstall(null);
      }
    } else if (!installationProgress || installationProgress.completed_at) {
      setInstallingVersion(null);
    }
  }, [installingTag, installationProgress, failedInstall, setFailedInstall]);

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
      setShowCompletedItems(false);
      setViewMode('list');
    }, resetDelay);

    return () => clearTimeout(timer);
  }, [progress, onRefreshProgress, setShowCompletedItems, setViewMode]);

  // Calculate sizes for releases in the background when dialog opens
  useEffect(() => {
    if (!isOpen || availableVersions.length === 0) {
      return;
    }

    const releasesNeedingSize = availableVersions.filter(
      release =>
        release.tag_name && // Skip releases without valid tag_name
        (release.total_size === null || release.total_size === undefined)
    );

    if (releasesNeedingSize.length === 0) {
      return;
    }

    const calculateSizes = async () => {
      logger.info('Starting background size calculation', { releaseCount: releasesNeedingSize.length });

      for (const release of releasesNeedingSize) {
        try {
          await api.calculate_release_size(release.tag_name, false, appId);
        } catch (error) {
          if (error instanceof APIError) {
            logger.error('API error calculating release size', { error: error.message, endpoint: error.endpoint, tag: release.tag_name });
          } else if (error instanceof Error) {
            logger.error('Failed to calculate release size', { error: error.message, tag: release.tag_name });
          } else {
            logger.error('Unknown error calculating release size', { error, tag: release.tag_name });
          }
        }
      }

      logger.info('Size calculation complete, refreshing versions');
      await onRefreshAll(false);
    };

    calculateSizes().catch(error => {
      if (error instanceof APIError) {
        logger.error('API error during background size calculation', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Error during background size calculation', { error: error.message });
      } else {
        logger.error('Unknown error during background size calculation', { error });
      }
    });
  }, [appId, isOpen, availableVersions, onRefreshAll]);

  // Filter versions based on user preferences
  const filteredVersions = availableVersions.filter((release) => {
    if (!showPreReleases && release.prerelease) {
      return false;
    }
    if (!showInstalled && installedVersions.includes(release.tag_name)) {
      return false;
    }
    return true;
  });

  const failedTag = progress && progress.completed_at && !progress.success ? progress.tag : null;
  const failedLogPath = progress && progress.completed_at && !progress.success ? progress.log_path || null : null;
  const stickyFailedTag = failedTag || failedInstall?.tag || null;
  const stickyFailedLogPath = failedLogPath || failedInstall?.log || null;

  const openLogPath = async (path?: string | null) => {
    if (!path || !isAPIAvailable()) return;
    try {
      await api.open_path(path);
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening log path', { error: error.message, endpoint: error.endpoint, path });
      } else if (error instanceof Error) {
        logger.error('Failed to open log path', { error: error.message, path });
      } else {
        logger.error('Unknown error opening log path', { error, path });
      }
    }
  };

  const openReleaseLink = async (url: string) => {
    try {
      if (isAPIAvailable()) {
        const result = await api.open_url(url);
        if (!result?.success) {
          logger.warn('API failed to open URL, falling back to window.open', { url });
          window.open(url, '_blank');
        }
      } else {
        window.open(url, '_blank');
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening release link', { error: error.message, endpoint: error.endpoint, url });
      } else if (error instanceof Error) {
        logger.error('Failed to open release link', { error: error.message, url });
      } else {
        logger.error('Unknown error opening release link', { error, url });
      }
      window.open(url, '_blank');
    }
  };

  const handleInstall = async (tag: string) => {
    logger.info('Starting installation', { tag });
    setInstallingVersion(tag);
    setErrorVersion(null);
    setErrorMessage(null);
    setShowCompletedItems(false);
    setViewMode('list');
    cancellationRef.current = false;

    try {
      await onInstallVersion(tag);
      if (onRefreshProgress) {
        await onRefreshProgress();
      }
      logger.info('Installation initiated successfully', { tag });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const isCancellation = cancellationRef.current || message.toLowerCase().includes('cancel');

      if (!isCancellation) {
        if (error instanceof APIError) {
          logger.error('API error during installation', { error: error.message, endpoint: error.endpoint, tag });
        } else if (error instanceof NetworkError) {
          logger.error('Network error during installation', { error: error.message, url: error.url ?? undefined, status: error.status ?? undefined, tag });
        } else if (error instanceof Error) {
          logger.error('Installation failed', { error: error.message, tag });
        } else {
          logger.error('Unknown error during installation', { error, tag });
        }
        setErrorVersion(tag);
        setErrorMessage(message);
      } else {
        logger.info('Installation cancelled by user', { tag });
        showCancellationNotice();
      }

      setInstallingVersion(null);
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
      logger.info('Cancelling installation');
      const result = await api.cancel_installation();
      if (result.success) {
        logger.info('Installation cancelled successfully');
        cancellationRef.current = true;
        showCancellationNotice();
        setInstallingVersion(null);
        setShowCompletedItems(false);
        setViewMode('list');
        setCancelHoverTag(null);
      } else {
        logger.error('Failed to cancel installation', { error: result.error });
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error cancelling installation', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Error cancelling installation', { error: error.message });
      } else {
        logger.error('Unknown error cancelling installation', { error });
      }
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
    return undefined;
  }, [isOpen, isPageMode, onClose]);

  const showProgressDetails = viewMode === 'details' && Boolean(installingVersion && progress);
  const containerClasses = isPageMode
    ? 'w-full h-full flex flex-col'
    : 'w-full max-w-3xl max-h-[80vh] flex flex-col';

  const dialogContent = (
    <div className={containerClasses} onClick={(e) => !isPageMode && e.stopPropagation()}>
      {/* Header */}
      {!isPageMode && (
        <div className="flex items-center justify-between p-4 border-b border-[hsl(var(--border-default))]">
          <div className="flex items-center gap-3">
            <h2 className="text-xl font-semibold text-[hsl(var(--text-primary))]">
              {installingVersion ? `Installing ${installingVersion}` : `Install ${appDisplayName} Version`}
            </h2>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={onClose}
              className="p-1 rounded hover:bg-[hsl(var(--surface-interactive-hover))] transition-colors"
            >
              <X size={20} className="text-[hsl(var(--launcher-text-muted))]" />
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

        {/* Rate limit banner */}
        <AnimatePresence>
          {isRateLimited && (
            <motion.div
              initial={{ opacity: 0, y: -6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              className="mb-3 mx-4 rounded border border-[hsl(var(--accent-warning))]/30 bg-[hsl(var(--accent-warning))]/10 px-3 py-2 text-sm text-[hsl(var(--accent-warning))]"
            >
              <div className="flex flex-col gap-1">
                <div className="flex items-center gap-2 font-medium">
                  <AlertCircle size={14} />
                  <span>GitHub Rate Limit Reached</span>
                </div>
                <p className="text-xs opacity-80 ml-5">
                  Showing cached version data.
                  {rateLimitRetryAfter != null && rateLimitRetryAfter > 0 && (
                    <> Rate limit resets in {Math.ceil(rateLimitRetryAfter / 60)} minute{Math.ceil(rateLimitRetryAfter / 60) !== 1 ? 's' : ''}.</>
                  )}
                </p>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {showProgressDetails ? (
          <ProgressDetailsView
            progress={progress!}
            installingVersion={installingVersion}
            showCompletedItems={showCompletedItems}
            onToggleCompletedItems={() => setShowCompletedItems(!showCompletedItems)}
            onBackToList={() => setViewMode('list')}
            onOpenLogPath={openLogPath}
          />
        ) : isLoading ? (
          <div className="flex items-center justify-center py-12 px-4">
            <Loader2 size={32} className="text-[hsl(var(--launcher-text-muted))] animate-spin" />
          </div>
        ) : filteredVersions.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-12 px-4 text-[hsl(var(--launcher-text-muted))]">
            <AlertCircle size={48} className="mb-3" />
            <p>No versions available</p>
            <p className="text-sm mt-1">Try adjusting the filters above</p>
          </div>
        ) : (
          <div className="space-y-3">
            {filteredVersions.map((release) => {
              const isInstalled = installedVersions.includes(release.tag_name);
              const isInstalling = installingVersion === release.tag_name;
              const currentProgress = isInstalling ? progress : null;
              const hasError = errorVersion === release.tag_name;

              return (
                <VersionListItem
                  key={release.tag_name}
                  release={release}
                  isInstalled={isInstalled}
                  isInstalling={isInstalling}
                  progress={currentProgress}
                  hasError={hasError}
                  errorMessage={errorMessage}
                  isHovered={hoveredTag === release.tag_name}
                  isCancelHovered={isInstalling && cancelHoverTag === release.tag_name}
                  installNetworkStatus={installNetworkStatus}
                  failedLogPath={stickyFailedTag === release.tag_name ? stickyFailedLogPath : null}
                  onInstall={() => handleInstall(release.tag_name)}
                  onRemove={() => onRemoveVersion(release.tag_name).catch(error => {
                    if (error instanceof APIError) {
                      logger.error('API error removing version', { error: error.message, endpoint: error.endpoint, tag: release.tag_name });
                    } else if (error instanceof Error) {
                      logger.error('Failed to remove version', { error: error.message, tag: release.tag_name });
                    } else {
                      logger.error('Unknown error removing version', { error, tag: release.tag_name });
                    }
                  })}
                  onCancel={handleCancelInstallation}
                  onOpenUrl={openReleaseLink}
                  onOpenLogPath={openLogPath}
                  onMouseEnter={() => setHoveredTag(release.tag_name)}
                  onMouseLeave={() => setHoveredTag(null)}
                  onCancelMouseEnter={() => setCancelHoverTag(release.tag_name)}
                  onCancelMouseLeave={() => setCancelHoverTag(null)}
                />
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
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
            className="fixed inset-0 bg-black/70 z-50"
          />
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
