/**
 * Version List Item Component
 *
 * Individual version card with install/uninstall/progress display.
 * Extracted from InstallDialog.tsx
 */

import { motion } from 'framer-motion';
import { Settings as Gear } from 'lucide-react';
import type { VersionRelease, InstallationProgress } from '../hooks/useVersions';
import { IconButton } from './ui';
import { VersionListItemButton } from './VersionListItemButton';
import { VersionListItemInfo } from './VersionListItemInfo';
import { getVersionInstallDisplayState } from './VersionListItemState';
import type { TorchInstalledConfig } from '../types/torch-install';

interface VersionListItemProps {
  appId?: string;
  release: VersionRelease;
  torchInstalledConfig?: TorchInstalledConfig;
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
  onInspectTorchProbe?: () => void;
  onRemove: () => void;
  onCancel: () => void;
  onOpenUrl: (url: string) => void;
  onOpenLogPath: (path: string) => void;
  onHoverStart: () => void;
  onHoverEnd: () => void;
  onCancelMouseEnter: () => void;
  onCancelMouseLeave: () => void;
}

export function VersionListItem({
  appId,
  release,
  torchInstalledConfig,
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
  onInspectTorchProbe,
  onRemove,
  onCancel,
  onOpenUrl,
  onOpenLogPath,
  onHoverStart,
  onHoverEnd,
  onCancelMouseEnter,
  onCancelMouseLeave,
}: VersionListItemProps) {
  const displayState = getVersionInstallDisplayState({
    appId,
    installNetworkStatus,
    isHovered,
    isInstalled,
    isInstalling,
    progress,
    release,
  });

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      onPointerEnter={onHoverStart}
      onPointerLeave={onHoverEnd}
      className="w-full p-2 transition-colors"
    >
      <div className="flex items-center justify-between gap-2">
        <VersionListItemInfo
          appId={appId}
          displayTag={displayState.displayTag}
          errorMessage={errorMessage}
          failedLogPath={failedLogPath}
          hasError={hasError}
          release={release}
          isInstalled={isInstalled}
          torchInstalledConfig={torchInstalledConfig}
          onOpenLogPath={onOpenLogPath}
          onOpenUrl={onOpenUrl}
        />

        <div className="flex items-center gap-2 flex-shrink-0">
          <VersionListItemButton
            displayState={displayState}
            isCancelHovered={isCancelHovered}
            isInstalled={isInstalled}
            isInstalling={isInstalling}
            onCancel={onCancel}
            onCancelMouseEnter={onCancelMouseEnter}
            onCancelMouseLeave={onCancelMouseLeave}
            onInstall={onInstall}
            onRemove={onRemove}
          />
          <IconButton icon={<Gear />} tooltip={onInspectTorchProbe ? 'Inspect saved checks' : 'Settings'} size="sm" onClick={onInspectTorchProbe} />
        </div>
      </div>
    </motion.div>
  );
}
