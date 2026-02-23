/**
 * Version List Item Component
 *
 * Individual version card with install/uninstall/progress display.
 * Extracted from InstallDialog.tsx
 */

import type { CSSProperties } from 'react';
import { motion } from 'framer-motion';
import {
  Download,
  Pause,
  Check,
  ExternalLink,
  FileText,
  Settings as Gear,
  X,
  XCircle,
} from 'lucide-react';
import type { VersionRelease, InstallationProgress } from '../hooks/useVersions';
import { ProgressRing } from './ProgressRing';
import { formatVersionDate, formatGB } from '../utils/installationFormatters';
import { IconButton } from './ui';

interface VersionListItemProps {
  release: VersionRelease;
  isInstalled: boolean;
  isInstalling: boolean;
  progress: InstallationProgress | null;
  hasError: boolean;
  errorMessage: string | null;
  isHovered: boolean;
  isCancelHovered: boolean;
  installNetworkStatus: 'idle' | 'downloading' | 'stalled' | 'failed';
  failedLogPath: string | null;
  onInstall: () => void;
  onRemove: () => void;
  onCancel: () => void;
  onOpenUrl: (url: string) => void;
  onOpenLogPath: (path: string) => void;
  onMouseEnter: () => void;
  onMouseLeave: () => void;
  onCancelMouseEnter: () => void;
  onCancelMouseLeave: () => void;
}

export function VersionListItem({
  release,
  isInstalled,
  isInstalling,
  progress,
  hasError,
  errorMessage,
  isHovered,
  isCancelHovered,
  installNetworkStatus,
  failedLogPath,
  onInstall,
  onRemove,
  onCancel,
  onOpenUrl,
  onOpenLogPath,
  onMouseEnter,
  onMouseLeave,
  onCancelMouseEnter,
  onCancelMouseLeave,
}: VersionListItemProps) {
  const displayTag = release.tagName?.replace(/^v/i, '') || release.tagName;
  const releaseUrl = release.htmlUrl;
  const showUninstall = isInstalled && !isInstalling && isHovered;
  const totalBytes = (progress ? progress.total_size : null) ?? release.totalSize ?? release.archiveSize ?? null;
  const isComplete = isInstalled || (isInstalling && progress?.success && !!progress?.completed_at);

  // Progress calculations
  const overallPercent = progress ? Math.round(progress.overall_progress || 0) : null;
  const downloadPercent =
    progress && progress.total_size && progress.total_size > 0
      ? Math.min(100, Math.round((progress.downloaded_bytes / progress.total_size) * 100))
      : null;
  const stagePercent = progress ? progress.stage_progress : null;
  const ringPercent =
    progress && (progress.stage === 'download' || progress.stage === 'dependencies')
      ? downloadPercent ?? stagePercent ?? overallPercent
      : overallPercent ?? stagePercent;

  const packageLabel = progress?.dependency_count !== null && progress?.dependency_count !== undefined && progress?.completed_dependencies !== null
    ? `${progress.completed_dependencies}/${progress.dependency_count}`
    : progress?.stage === 'dependencies'
      ? 'Installing...'
      : 'Downloading...';

  const isInstallFailed = installNetworkStatus === 'failed' || Boolean(progress?.error);
  const isDownloadPending = isInstalling && !isInstallFailed && (
    !progress || (
      progress.stage === 'download'
      && (progress.downloaded_bytes ?? 0) <= 0
      && (progress.download_speed ?? 0) <= 0
    )
  );

  const ringColor = isInstallFailed ? 'hsl(var(--accent-error))' : 'hsl(var(--accent-success))';

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
  const StatusIcon = isInstallFailed ? Pause : Download;

  const handleButtonClick = () => {
    if (isInstalling) {
      onCancel();
      return;
    }
    if (isInstalled && !isInstalling) {
      onRemove();
    } else {
      onInstall();
    }
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      className="w-full p-2 transition-colors"
    >
      <div className="flex items-center justify-between gap-2">
        {/* Version Info */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <div className="flex flex-col min-w-0">
              <div className="flex items-center gap-2 min-w-0">
                <h3 className="text-[hsl(var(--text-primary))] font-medium truncate">
                  {displayTag}
                </h3>
                {releaseUrl && (
                  <IconButton
                    icon={<ExternalLink />}
                    tooltip="Release notes"
                    onClick={() => onOpenUrl(releaseUrl)}
                    size="sm"
                  />
                )}
                {failedLogPath && (
                  <IconButton
                    icon={<FileText className="text-[hsl(var(--accent-error))]" />}
                    tooltip="View log"
                    onClick={() => onOpenLogPath(failedLogPath)}
                    size="sm"
                  />
                )}
                {release.prerelease && (
                  <span className="px-2 py-0.5 bg-[hsl(var(--accent-warning))]/20 text-[hsl(var(--accent-warning))] text-[11px] rounded-full">
                    Pre
                  </span>
                )}
              </div>
              <div className="flex items-center gap-1 text-xs text-[hsl(var(--text-muted))]">
                <span>{formatVersionDate(release.publishedAt)}</span>
              </div>
            </div>
          </div>

          {/* Error message */}
          {hasError && errorMessage && (
            <div className="mt-1 flex items-start gap-2 text-sm text-[hsl(var(--accent-error))] bg-[hsl(var(--accent-error))]/10 rounded p-2">
              <span>{errorMessage}</span>
            </div>
          )}
        </div>

        {/* Install Button */}
        <div className="flex items-center gap-2 flex-shrink-0">
          <motion.button
            onClick={handleButtonClick}
            onMouseEnter={isInstalling ? onCancelMouseEnter : undefined}
            onMouseLeave={isInstalling ? onCancelMouseLeave : undefined}
            whileHover={!isInstalling ? { scale: 1.05 } : {}}
            whileTap={!isInstalling ? { scale: 0.96 } : {}}
            className={`flex items-center gap-2 px-3 py-2 rounded text-sm font-medium transition-colors border w-[120px] min-w-[120px] overflow-hidden ${
              isInstalling
                ? isCancelHovered
                  ? 'bg-[hsl(var(--surface-lowest))] border-[hsl(var(--accent-error))] text-[hsl(var(--accent-error))]'
                  : 'bg-[hsl(var(--surface-control))] border-[hsl(var(--border-control))] text-[hsl(var(--text-primary))]'
                : showUninstall
                  ? 'bg-[hsl(var(--surface-lowest))] border-[hsl(var(--accent-error))] text-[hsl(var(--accent-error))]'
                  : isInstalled
                    ? 'bg-[hsl(var(--accent-success))]/20 border-[hsl(var(--accent-success))]/60 text-[hsl(var(--text-primary))]'
                    : isComplete
                      ? 'bg-[hsl(var(--accent-success))]/20 border-[hsl(var(--accent-success))]/60 text-[hsl(var(--text-primary))]'
                      : 'bg-[hsl(var(--surface-control))] border-[hsl(var(--border-control))] text-[hsl(var(--text-primary))] hover:border-[hsl(var(--accent-success))] hover:text-[hsl(var(--accent-success))]'
            }`}
          >
            {isInstalled && !showUninstall ? (
              <>
                <Check size={16} className="text-[hsl(var(--text-primary))]" />
                <span className="text-xs font-semibold text-[hsl(var(--text-primary))] truncate whitespace-nowrap flex-1 min-w-0">Ready</span>
              </>
            ) : showUninstall ? (
              <>
                <X size={16} className="text-[hsl(var(--accent-error))]" />
                <span className="text-xs font-semibold text-[hsl(var(--accent-error))] truncate whitespace-nowrap flex-1 min-w-0">Uninstall</span>
              </>
            ) : isInstalling ? (
              <>
                {isCancelHovered ? (
                  <XCircle size={16} className="text-[hsl(var(--accent-error))]" />
                ) : isDownloadPending ? (
                  <span className="relative flex h-[18px] w-[18px] items-center justify-center">
                    <span
                      className="download-progress-ring is-waiting"
                      style={
                        {
                          '--progress': '60deg',
                        } as CSSProperties
                      }
                    />
                    <StatusIcon size={14} className={downloadIconClass} style={downloadIconStyle} />
                  </span>
                ) : (
                  <ProgressRing
                    progress={ringPercent ?? 0}
                    size={18}
                    strokeWidth={2}
                    trackColor="hsl(var(--surface-control))"
                    indicatorColor={ringColor}
                  >
                    <StatusIcon size={14} className={downloadIconClass} style={downloadIconStyle} />
                  </ProgressRing>
                )}
                {isCancelHovered ? (
                  <span className="text-xs font-semibold text-[hsl(var(--accent-error))] truncate whitespace-nowrap flex-1 min-w-0">Cancel</span>
                ) : (
                  <span className="text-xs font-semibold truncate whitespace-nowrap flex-1 min-w-0">{packageLabel}</span>
                )}
              </>
            ) : (
              <>
                <Download size={16} />
                {totalBytes && (
                  <span className="text-xs truncate whitespace-nowrap flex-1 min-w-0">
                    {formatGB(totalBytes)}
                  </span>
                )}
              </>
            )}
          </motion.button>
          <IconButton
            icon={<Gear />}
            tooltip="Settings"
            size="sm"
          />
        </div>
      </div>
    </motion.div>
  );
}
