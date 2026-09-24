/**
 * Install Dialog Component (Refactored)
 *
 * Modal/page for installing app versions with progress tracking.
 * Reduced from 939 lines to orchestrator component by extracting:
 * - useInstallationProgress hook (dialog presentation state)
 * - useInstallationState hook (UI state management)
 * - ProgressDetailsView component (detailed progress display)
 * - VersionListItem component (version cards)
 * - installationFormatters utility (formatting helpers)
 */

import { useState, useEffect, useRef } from 'react';
import type { VersionRelease, InstallationProgress } from '../hooks/useVersions';
import { useInstallationProgress } from '../hooks/useInstallationProgress';
import { useInstallDialogLinks } from '../hooks/useInstallDialogLinks';
import { useReleaseSizeCalculation } from '../hooks/useReleaseSizeCalculation';
import { useInstallationState } from '../hooks/useInstallationState';
import { ConfirmationDialog } from './ConfirmationDialog';
import { InstallDialogContent } from './InstallDialogContent';
import { InstallDialogFrame } from './InstallDialogFrame';
import { TorchInstallPreview } from './TorchInstallPreview';
import { TorchRuntimeProbePanel } from './TorchRuntimeProbePanel';
import { api } from '../api/adapter';
import type { TorchInstalledConfig } from '../types/torch-install';
import {
  filterVersions,
  getErrorMessage,
  getStickyFailure,
  isInstallationCancellation,
  reportCancelError,
  reportInstallationError,
} from './InstallDialogHelpers';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('InstallDialog');

interface InstallDialogProps {
  isOpen: boolean;
  onClose: () => void;
  availableVersions: VersionRelease[];
  installedVersions: string[];
  isLoading: boolean;
  onInstallVersion: (tag: string, previewId?: string) => Promise<boolean>;
  onCancelInstallation: () => Promise<boolean>;
  onRefreshAll: (forceRefresh?: boolean) => Promise<void>;
  onRemoveVersion: (tag: string) => Promise<boolean>;
  displayMode?: 'modal' | 'page';
  installingTag?: string | null;
  installationProgress?: InstallationProgress | null;
  installNetworkStatus?: 'idle' | 'downloading' | 'stalled' | 'failed';
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
  onCancelInstallation,
  onRefreshAll,
  onRemoveVersion,
  displayMode = 'modal',
  installingTag,
  installationProgress,
  installNetworkStatus = 'idle',
  appDisplayName = 'Application',
  appId,
  isRateLimited = false,
  rateLimitRetryAfter,
}: InstallDialogProps) {
  logger.debug('Component rendered', { isOpen, availableVersionsCount: availableVersions.length, displayMode });

  const isPageMode = displayMode === 'page';
  const [installingVersion, setInstallingVersion] = useState<string | null>(installingTag || null);
  const [errorVersion, setErrorVersion] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [showCancelConfirmation, setShowCancelConfirmation] = useState(false);
  const [torchPreviewTag, setTorchPreviewTag] = useState<string | null>(null);
  const [torchProbeTag, setTorchProbeTag] = useState<string | null>(null);
  const [torchInstalledConfigs, setTorchInstalledConfigs] = useState<TorchInstalledConfig[]>([]);
  const cancellationRef = useRef(false);
  const installedKey = installedVersions.join('\0');

  useEffect(() => {
    if (!isOpen || appId !== 'torch') return;
    let active = true;
    setTorchInstalledConfigs([]);
    void api.get_torch_runtime_options().then((options) => {
      if (active) setTorchInstalledConfigs(options.installed);
    }).catch(() => {
      if (active) setTorchInstalledConfigs([]);
    });
    return () => { active = false; };
  }, [appId, installedKey, isOpen]);

  // Custom hooks
  const {
    progress,
    cancellationNotice,
    failedInstall,
    setFailedInstall,
    showCancellationNotice,
  } = useInstallationProgress({
    externalProgress: installationProgress,
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
  const { openLogPath, openReleaseLink } = useInstallDialogLinks();

  useReleaseSizeCalculation({
    appId,
    availableVersions,
    isOpen,
    onRefreshAll,
  });

  // Sync installingVersion with external tag
  useEffect(() => {
    if (installingTag) {
      setInstallingVersion(installingTag);
      if (failedInstall?.tag === installingTag) {
        setFailedInstall(null);
      }
    } else if (!installationProgress) {
      setInstallingVersion(null);
    }
  }, [installingTag, installationProgress, failedInstall, setFailedInstall]);

  // Reset dialog-only state after manager-owned terminal progress is visible.
  useEffect(() => {
    if (!progress || !progress.completed_at) {
      return;
    }

    const wasCancelled = progress.error?.toLowerCase().includes('cancel');
    const resetDelay = wasCancelled ? 1500 : 3000;

    const timer = setTimeout(() => {
      setInstallingVersion(null);
      setShowCompletedItems(false);
      setViewMode('list');
    }, resetDelay);

    return () => clearTimeout(timer);
  }, [progress, setShowCompletedItems, setViewMode]);

  const filteredVersions = filterVersions(
    availableVersions,
    installedVersions,
    showPreReleases,
    showInstalled,
    appId === 'torch'
  );
  const stickyFailure = getStickyFailure(progress, failedInstall);

  const handleInstall = async (tag: string, previewId?: string) => {
    logger.info('Starting installation', { tag });
    setInstallingVersion(tag);
    setErrorVersion(null);
    setErrorMessage(null);
    setShowCompletedItems(false);
    setViewMode('list');
    cancellationRef.current = false;

    try {
      await onInstallVersion(tag, previewId);
      logger.info('Installation initiated successfully', { tag });
    } catch (error) {
      const isCancellation = isInstallationCancellation(error, cancellationRef.current);

      if (!isCancellation) {
        reportInstallationError(tag, error);
        setErrorVersion(tag);
        setErrorMessage(getErrorMessage(error));
      } else {
        logger.info('Installation cancelled by user', { tag });
        showCancellationNotice();
      }

      setInstallingVersion(null);
    }
  };

  const confirmCancelInstallation = async () => {
    setShowCancelConfirmation(false);
    cancellationRef.current = true;
    setErrorVersion(null);
    setErrorMessage(null);

    try {
      logger.info('Cancelling installation');
      const wasCancelled = await onCancelInstallation();
      if (wasCancelled) {
        logger.info('Installation cancelled successfully');
        cancellationRef.current = true;
        showCancellationNotice();
        setInstallingVersion(null);
        setShowCompletedItems(false);
        setViewMode('list');
        setCancelHoverTag(null);
      }
    } catch (error) {
      reportCancelError(error);
      cancellationRef.current = false;
      if (installingVersion) {
        setErrorVersion(installingVersion);
        setErrorMessage(getErrorMessage(error));
      }
    }
  };

  const handleCancelInstallation = () => {
    setShowCancelConfirmation(true);
  };

  const showProgressDetails = viewMode === 'details' && Boolean(installingVersion && progress);
  const dialogTitle = installingVersion
    ? `Installing ${installingVersion}`
    : `Install ${appDisplayName} Version`;

  const reportRemoveError = (tag: string, error: unknown) => {
    if (error instanceof APIError) {
      logger.error('API error removing version', { error: error.message, endpoint: error.endpoint, tag });
    } else if (error instanceof Error) {
      logger.error('Failed to remove version', { error: error.message, tag });
    } else {
      logger.error('Unknown error removing version', { error: String(error), tag });
    }
  };

  return (
    <InstallDialogFrame
      isOpen={isOpen}
      isPageMode={isPageMode}
      onClose={onClose}
      title={dialogTitle}
    >
      {appId === 'torch' && torchProbeTag && !installingVersion ? (
        <div className="flex-1 min-h-0 overflow-y-auto px-4 py-3">
          <button type="button" onClick={() => setTorchProbeTag(null)} className="text-sm underline">All versions</button>
          <TorchRuntimeProbePanel tag={torchProbeTag} />
        </div>
      ) : appId === 'torch' && torchPreviewTag && !installingVersion ? (
        <TorchInstallPreview
          key={torchPreviewTag}
          tag={torchPreviewTag}
          onBack={() => setTorchPreviewTag(null)}
          onInstall={(previewId) => {
            const tag = torchPreviewTag;
            setTorchPreviewTag(null);
            void handleInstall(tag, previewId);
          }}
        />
      ) : <InstallDialogContent
        appId={appId}
        cancellationNotice={cancellationNotice}
        cancelHoverTag={cancelHoverTag}
        errorMessage={errorMessage}
        errorVersion={errorVersion}
        filteredVersions={filteredVersions}
        hoveredTag={hoveredTag}
        installNetworkStatus={installNetworkStatus}
        installedVersions={installedVersions}
        torchInstalledConfigs={torchInstalledConfigs}
        installingVersion={installingVersion}
        isLoading={isLoading}
        isRateLimited={isRateLimited}
        progress={progress}
        rateLimitRetryAfter={rateLimitRetryAfter}
        showCompletedItems={showCompletedItems}
        showProgressDetails={showProgressDetails}
        stickyFailedLogPath={stickyFailure.log}
        stickyFailedTag={stickyFailure.tag}
        onCancelInstallation={handleCancelInstallation}
        onOpenLogPath={openLogPath}
        onOpenReleaseLink={openReleaseLink}
        onRemoveVersion={onRemoveVersion}
        onSetCancelHoverTag={setCancelHoverTag}
        onSetHoveredTag={setHoveredTag}
        onToggleCompletedItems={() => setShowCompletedItems(!showCompletedItems)}
        onBackToList={() => setViewMode('list')}
        onInstallVersion={(tag) => {
          if (appId === 'torch') {
            setTorchPreviewTag(tag);
          } else {
            void handleInstall(tag);
          }
        }}
        onInspectTorchProbe={(tag) => setTorchProbeTag(tag)}
        onReportRemoveError={reportRemoveError}
      />}

      <ConfirmationDialog
        isOpen={showCancelConfirmation}
        title="Cancel installation"
        message="This will stop the process and remove any partially installed files."
        confirmLabel="Cancel installation"
        onCancel={() => setShowCancelConfirmation(false)}
        onConfirm={() => void confirmCancelInstallation()}
      />
    </InstallDialogFrame>
  );
}
