import type { CSSProperties } from 'react';
import { motion } from 'framer-motion';
import { Check, Download, Pause, X, XCircle } from 'lucide-react';
import { formatGB } from '../utils/installationFormatters';
import { ProgressRing } from './ProgressRing';
import type { VersionInstallDisplayState } from './VersionListItemState';

interface VersionListItemButtonProps {
  displayState: VersionInstallDisplayState;
  isCancelHovered: boolean;
  isInstalled: boolean;
  isInstalling: boolean;
  onCancel: () => void;
  onCancelMouseEnter: () => void;
  onCancelMouseLeave: () => void;
  onInstall: () => void;
  onRemove: () => void;
}

function getInstallButtonClassName({
  displayState,
  isCancelHovered,
  isInstalled,
  isInstalling,
}: Pick<VersionListItemButtonProps, 'displayState' | 'isCancelHovered' | 'isInstalled' | 'isInstalling'>): string {
  const baseClass =
    'flex items-center gap-2 px-3 py-2 rounded text-sm font-medium transition-colors border w-[120px] min-w-[120px] overflow-hidden';

  if (isInstalling) {
    return `${baseClass} ${
      isCancelHovered
        ? 'bg-[hsl(var(--surface-lowest))] border-[hsl(var(--accent-error))] text-[hsl(var(--accent-error))]'
        : 'bg-[hsl(var(--surface-control))] border-[hsl(var(--border-control))] text-[hsl(var(--text-primary))]'
    }`;
  }
  if (displayState.showUninstall) {
    return `${baseClass} bg-[hsl(var(--surface-lowest))] border-[hsl(var(--accent-error))] text-[hsl(var(--accent-error))]`;
  }
  if (isInstalled || displayState.isComplete) {
    return `${baseClass} bg-[hsl(var(--accent-success))]/20 border-[hsl(var(--accent-success))]/60 text-[hsl(var(--text-primary))]`;
  }
  return `${baseClass} bg-[hsl(var(--surface-control))] border-[hsl(var(--border-control))] text-[hsl(var(--text-primary))] hover:border-[hsl(var(--accent-success))] hover:text-[hsl(var(--accent-success))]`;
}

function ReadyButtonContent() {
  return (
    <>
      <Check size={16} className="text-[hsl(var(--text-primary))]" />
      <span className="text-xs font-semibold text-[hsl(var(--text-primary))] truncate whitespace-nowrap flex-1 min-w-0">
        Ready
      </span>
    </>
  );
}

function UninstallButtonContent() {
  return (
    <>
      <X size={16} className="text-[hsl(var(--accent-error))]" />
      <span className="text-xs font-semibold text-[hsl(var(--accent-error))] truncate whitespace-nowrap flex-1 min-w-0">
        Uninstall
      </span>
    </>
  );
}

function PendingDownloadIcon({
  displayState,
}: {
  displayState: VersionInstallDisplayState;
}) {
  const StatusIcon = displayState.isInstallFailed ? Pause : Download;

  return (
    <span className="relative flex h-[18px] w-[18px] items-center justify-center">
      <span
        className="download-progress-ring is-waiting"
        style={{ '--progress': '60deg' } as CSSProperties}
      />
      <StatusIcon
        size={14}
        className={displayState.downloadIconClass}
        style={displayState.downloadIconStyle}
      />
    </span>
  );
}

function ProgressDownloadIcon({
  displayState,
}: {
  displayState: VersionInstallDisplayState;
}) {
  const StatusIcon = displayState.isInstallFailed ? Pause : Download;

  return (
    <ProgressRing
      progress={displayState.ringPercent ?? 0}
      size={18}
      strokeWidth={2}
      trackColor="hsl(var(--surface-control))"
      indicatorColor={displayState.ringColor}
    >
      <StatusIcon
        size={14}
        className={displayState.downloadIconClass}
        style={displayState.downloadIconStyle}
      />
    </ProgressRing>
  );
}

function InstallingButtonContent({
  displayState,
  isCancelHovered,
}: {
  displayState: VersionInstallDisplayState;
  isCancelHovered: boolean;
}) {
  return (
    <>
      {isCancelHovered ? (
        <XCircle size={16} className="text-[hsl(var(--accent-error))]" />
      ) : displayState.isDownloadPending ? (
        <PendingDownloadIcon displayState={displayState} />
      ) : (
        <ProgressDownloadIcon displayState={displayState} />
      )}
      {isCancelHovered ? (
        <span className="text-xs font-semibold text-[hsl(var(--accent-error))] truncate whitespace-nowrap flex-1 min-w-0">
          Cancel
        </span>
      ) : (
        <span className="text-xs font-semibold truncate whitespace-nowrap flex-1 min-w-0">
          {displayState.packageLabel}
        </span>
      )}
    </>
  );
}

function InstallButtonContent({
  displayState,
}: {
  displayState: VersionInstallDisplayState;
}) {
  return (
    <>
      <Download size={16} />
      {displayState.totalBytes && (
        <span className="text-xs truncate whitespace-nowrap flex-1 min-w-0">
          {formatGB(displayState.totalBytes)}
        </span>
      )}
    </>
  );
}

function getButtonContent({
  displayState,
  isCancelHovered,
  isInstalled,
  isInstalling,
}: Pick<VersionListItemButtonProps, 'displayState' | 'isCancelHovered' | 'isInstalled' | 'isInstalling'>) {
  if (isInstalled && !displayState.showUninstall) {
    return <ReadyButtonContent />;
  }
  if (displayState.showUninstall) {
    return <UninstallButtonContent />;
  }
  if (isInstalling) {
    return <InstallingButtonContent displayState={displayState} isCancelHovered={isCancelHovered} />;
  }
  return <InstallButtonContent displayState={displayState} />;
}

export function VersionListItemButton({
  displayState,
  isCancelHovered,
  isInstalled,
  isInstalling,
  onCancel,
  onCancelMouseEnter,
  onCancelMouseLeave,
  onInstall,
  onRemove,
}: VersionListItemButtonProps) {
  const handleButtonClick = () => {
    if (isInstalling) {
      onCancel();
      return;
    }
    if (isInstalled && !isInstalling) {
      onRemove();
      return;
    }
    onInstall();
  };

  return (
    <motion.button
      onClick={handleButtonClick}
      onPointerEnter={isInstalling ? onCancelMouseEnter : undefined}
      onPointerLeave={isInstalling ? onCancelMouseLeave : undefined}
      whileHover={!isInstalling ? { scale: 1.05 } : {}}
      whileTap={!isInstalling ? { scale: 0.96 } : {}}
      className={getInstallButtonClassName({
        displayState,
        isCancelHovered,
        isInstalled,
        isInstalling,
      })}
    >
      {getButtonContent({
        displayState,
        isCancelHovered,
        isInstalled,
        isInstalling,
      })}
    </motion.button>
  );
}
