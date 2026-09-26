import { AnimatePresence, motion } from 'framer-motion';
import { AlertCircle, Loader2 } from 'lucide-react';
import type { InstallationProgress, VersionRelease } from '../hooks/useVersions';
import { ProgressDetailsView } from './ProgressDetailsView';
import { VersionListItem } from './VersionListItem';
import type { TorchInstalledConfig } from '../types/torch-install';
import { getInstallActivityPresentation } from '../utils/installActivityPresentation';

interface InstallDialogContentProps {
  appId?: string;
  cancellationNotice: string | null;
  cancelHoverTag: string | null;
  errorMessage: string | null;
  errorVersion: string | null;
  filteredVersions: VersionRelease[];
  hoveredTag: string | null;
  installNetworkStatus: 'idle' | 'downloading' | 'stalled' | 'failed';
  installedVersions: string[];
  torchInstalledConfigs?: TorchInstalledConfig[];
  installingVersion: string | null;
  isLoading: boolean;
  isRateLimited: boolean;
  progress: InstallationProgress | null;
  rateLimitRetryAfter?: number | null;
  showCompletedItems: boolean;
  showProgressDetails: boolean;
  stickyFailedLogPath: string | null;
  stickyFailedTag: string | null;
  onCancelInstallation: () => void;
  onOpenLogPath: (path?: string | null) => Promise<void>;
  onOpenReleaseLink: (url: string) => Promise<void>;
  onRemoveVersion: (tag: string) => Promise<unknown>;
  onSetCancelHoverTag: (tag: string | null) => void;
  onSetHoveredTag: (tag: string | null) => void;
  onToggleCompletedItems: () => void;
  onBackToList: () => void;
  onInstallVersion: (tag: string) => void;
  onInspectTorchProbe?: (tag: string) => void;
  onReportRemoveError: (tag: string, error: unknown) => void;
}

export function InstallDialogContent({
  appId,
  cancellationNotice,
  cancelHoverTag,
  errorMessage,
  errorVersion,
  filteredVersions,
  hoveredTag,
  installNetworkStatus,
  installedVersions,
  torchInstalledConfigs = [],
  installingVersion,
  isLoading,
  isRateLimited,
  progress,
  rateLimitRetryAfter,
  showCompletedItems,
  showProgressDetails,
  stickyFailedLogPath,
  stickyFailedTag,
  onCancelInstallation,
  onOpenLogPath,
  onOpenReleaseLink,
  onRemoveVersion,
  onSetCancelHoverTag,
  onSetHoveredTag,
  onToggleCompletedItems,
  onBackToList,
  onInstallVersion,
  onInspectTorchProbe,
  onReportRemoveError,
}: InstallDialogContentProps) {
  const activity = getInstallActivityPresentation({ appId, installingTag: installingVersion, progress });
  return (
    <div className="flex-1 min-h-0 overflow-y-auto py-4 px-0">
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

      {activity.active && (appId === 'torch' || !progress) && (
        <div className="mx-4 mb-3 flex items-center justify-between gap-3 rounded border border-[hsl(var(--border-default))] p-3 text-sm">
          <span role="status" className="min-w-0 break-words">{activity.phase}</span>
          <button type="button" onClick={onCancelInstallation} className="shrink-0 rounded border px-3 py-1" aria-label="Cancel current installation">Cancel</button>
        </div>
      )}

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

      {showProgressDetails && progress ? (
        <ProgressDetailsView
          appId={appId}
          progress={progress}
          installingVersion={installingVersion}
          showCompletedItems={showCompletedItems}
          onToggleCompletedItems={onToggleCompletedItems}
          onBackToList={onBackToList}
          onOpenLogPath={onOpenLogPath}
        />
      ) : isLoading ? (
        <div className="flex items-center justify-center py-12 px-4">
          <Loader2 size={32} className="text-[hsl(var(--text-muted))] animate-spin" />
        </div>
      ) : filteredVersions.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-12 px-4 text-[hsl(var(--text-muted))]">
          <AlertCircle size={48} className="mb-3" />
          <p>No versions available</p>
          <p className="text-sm mt-1">Try adjusting the filters above</p>
        </div>
      ) : (
        <div className="space-y-3">
          {filteredVersions.map((release) => {
            const isInstalled = installedVersions.includes(release.tagName);
            const isInstalling = installingVersion === release.tagName;
            const currentProgress = isInstalling ? progress : null;
            const hasError = errorVersion === release.tagName;

            return (
              <VersionListItem
                appId={appId}
                key={release.tagName}
                release={release}
                torchInstalledConfig={torchInstalledConfigs.find((config) => config.tag === release.tagName)}
                isInstalled={isInstalled}
                isInstalling={isInstalling}
                progress={currentProgress}
                hasError={hasError}
                errorMessage={errorMessage}
                isHovered={hoveredTag === release.tagName}
                isCancelHovered={isInstalling && cancelHoverTag === release.tagName}
                installNetworkStatus={installNetworkStatus}
                failedLogPath={stickyFailedTag === release.tagName ? stickyFailedLogPath : null}
                onInstall={() => onInstallVersion(release.tagName)}
                onInspectTorchProbe={appId === 'torch' && isInstalled ? () => onInspectTorchProbe?.(release.tagName) : undefined}
                onRemove={() => onRemoveVersion(release.tagName).catch((error: unknown) => {
                  onReportRemoveError(release.tagName, error);
                })}
                onCancel={onCancelInstallation}
                onOpenUrl={onOpenReleaseLink}
                onOpenLogPath={onOpenLogPath}
                onHoverStart={() => onSetHoveredTag(release.tagName)}
                onHoverEnd={() => onSetHoveredTag(null)}
                onCancelMouseEnter={() => onSetCancelHoverTag(release.tagName)}
                onCancelMouseLeave={() => onSetCancelHoverTag(null)}
              />
            );
          })}
        </div>
      )}
    </div>
  );
}
