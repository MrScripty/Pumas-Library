import { Download, Minus, RefreshCw, X } from 'lucide-react';
import { IconButton } from './ui';
import type { HeaderStatusInfo } from './HeaderStatus';

export function HeaderUpdateControls({
  isCheckingLauncherUpdates,
  launcherLatestVersion,
  launcherUpdateAvailable,
  onCheckLauncherUpdates,
  onDownloadLauncherUpdate,
}: {
  isCheckingLauncherUpdates: boolean;
  launcherLatestVersion?: string | null;
  launcherUpdateAvailable: boolean;
  onCheckLauncherUpdates?: () => void;
  onDownloadLauncherUpdate?: () => void;
}) {
  return (
    <div className="flex items-center gap-2 flex-shrink-0">
      <IconButton
        icon={<RefreshCw className={isCheckingLauncherUpdates ? 'animate-spin' : ''} />}
        tooltip={isCheckingLauncherUpdates ? 'Checking GitHub releases...' : 'Check GitHub releases for updates'}
        onClick={onCheckLauncherUpdates}
        size="sm"
        disabled={isCheckingLauncherUpdates || !onCheckLauncherUpdates}
        className="app-region-no-drag"
      />
      {launcherUpdateAvailable && (
        <IconButton
          icon={<Download className="text-[hsl(var(--accent-success))]" />}
          tooltip={launcherLatestVersion
            ? `Download ${launcherLatestVersion} from GitHub`
            : 'Download update from GitHub'}
          onClick={onDownloadLauncherUpdate}
          size="sm"
          disabled={!onDownloadLauncherUpdate}
          className="app-region-no-drag"
        />
      )}
    </div>
  );
}

export function HeaderStatusBadge({ status }: { status: HeaderStatusInfo }) {
  const StatusIcon = status.icon;

  return (
    <div className="flex-1 flex items-center justify-center min-w-0">
      <div className="flex items-center gap-1.5 px-2 py-0.5 bg-[hsl(var(--accent-success)/0.15)] rounded text-[10px] text-[hsl(var(--text-secondary))]">
        <StatusIcon className={`w-3 h-3 flex-shrink-0 ${status.spinning ? 'animate-spin' : ''}`} />
        <span className="truncate whitespace-nowrap">{status.text}</span>
      </div>
    </div>
  );
}

export function HeaderWindowControls({
  onClose,
  onMinimize,
}: {
  onClose: () => void;
  onMinimize: () => void;
}) {
  return (
    <div className="flex items-center gap-0.5">
      <IconButton
        icon={<Minus />}
        tooltip="Minimize"
        onClick={onMinimize}
        size="sm"
        className="app-region-no-drag"
      />
      <IconButton
        icon={<X className="group-hover:text-[hsl(var(--accent-error))] transition-colors" />}
        tooltip="Close"
        onClick={onClose}
        size="sm"
        className="app-region-no-drag"
      />
    </div>
  );
}
