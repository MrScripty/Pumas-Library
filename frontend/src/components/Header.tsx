import React from 'react';
import type { SystemResources } from '../types/apps';
import type { ActiveModelDownload } from '../hooks/useActiveModelDownload';
import { HeaderStatusBadge, HeaderUpdateControls, HeaderWindowControls } from './HeaderControls';
import { HeaderResourceStrip } from './HeaderResourceStrip';
import { getHeaderStatusInfo, type InstallationProgress } from './HeaderStatus';

interface HeaderProps {
  systemResources?: SystemResources;
  appResources?: {
    gpu_memory?: number;
    ram_memory?: number;
  };
  launcherUpdateAvailable: boolean;
  launcherLatestVersion?: string | null;
  isCheckingLauncherUpdates?: boolean;
  onCheckLauncherUpdates?: () => void;
  onDownloadLauncherUpdate?: () => void;
  onMinimize: () => void;
  onClose: () => void;
  networkAvailable?: boolean | null;
  modelLibraryLoaded?: boolean | null;
  installationProgress?: InstallationProgress | null;
  activeModelDownload?: ActiveModelDownload | null;
  activeModelDownloadCount?: number;
}

export const Header: React.FC<HeaderProps> = ({
  systemResources,
  appResources,
  launcherUpdateAvailable,
  launcherLatestVersion,
  isCheckingLauncherUpdates = false,
  onCheckLauncherUpdates,
  onDownloadLauncherUpdate,
  onMinimize,
  onClose,
  networkAvailable,
  modelLibraryLoaded,
  installationProgress,
  activeModelDownload,
  activeModelDownloadCount = 0,
}) => {
  const status = getHeaderStatusInfo({
    activeModelDownload,
    activeModelDownloadCount,
    installationProgress,
    modelLibraryLoaded,
    networkAvailable,
  });
  return (
    <div className="border-b border-[hsl(var(--launcher-border))] bg-[hsl(var(--launcher-bg-secondary)/0.3)] backdrop-blur-sm relative z-10 app-region-drag">
      <div className="h-8 px-3 pt-1 flex items-center justify-between gap-3">
        <HeaderUpdateControls
          isCheckingLauncherUpdates={isCheckingLauncherUpdates}
          launcherLatestVersion={launcherLatestVersion}
          launcherUpdateAvailable={launcherUpdateAvailable}
          onCheckLauncherUpdates={onCheckLauncherUpdates}
          onDownloadLauncherUpdate={onDownloadLauncherUpdate}
        />
        <HeaderStatusBadge status={status} />
        <HeaderWindowControls onClose={onClose} onMinimize={onMinimize} />
      </div>

      <HeaderResourceStrip appResources={appResources} systemResources={systemResources} />
    </div>
  );
};
