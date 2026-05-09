import React, { useEffect, useState } from 'react';
import { motion } from 'framer-motion';
import { useHover } from '@react-aria/interactions';
import {
  Anchor,
  CheckCircle2,
  CircleX,
  FolderOpen,
  Globe,
  Loader2,
} from 'lucide-react';

type InstallNetworkStatus = 'idle' | 'downloading' | 'stalled' | 'failed';

interface TriggerDefaultButtonProps {
  activeVersion: string | null;
  defaultVersion: string | null;
  isLoading: boolean;
  onToggleDefault: () => void;
}

function TriggerDefaultIcon({
  hoverStartedAsDefault,
  isDefault,
  isHovered,
}: {
  hoverStartedAsDefault: boolean;
  isDefault: boolean;
  isHovered: boolean;
}) {
  if (isDefault && isHovered && hoverStartedAsDefault) {
    return <CircleX size={14} className="text-[hsl(var(--text-tertiary))]" />;
  }
  if (isDefault) {
    return <Anchor size={14} className="text-[hsl(var(--accent-success))]" />;
  }
  return <div className="h-2 w-2 rounded-full bg-[hsl(var(--accent-success))]" />;
}

export function TriggerDefaultButton({
  activeVersion,
  defaultVersion,
  isLoading,
  onToggleDefault,
}: TriggerDefaultButtonProps) {
  const { hoverProps, isHovered } = useHover({});
  const [hoverStartedAsDefault, setHoverStartedAsDefault] = useState(false);
  const isDefault = defaultVersion === activeVersion;

  useEffect(() => {
    if (isHovered) {
      setHoverStartedAsDefault(isDefault);
    }
  }, [isHovered, isDefault]);

  return (
    <button
      {...hoverProps}
      onClick={(event) => {
        event.stopPropagation();
        onToggleDefault();
      }}
      className="flex h-4 w-4 items-center justify-center"
      title={isDefault ? 'Click to unset as default' : 'Click to set as default'}
      disabled={isLoading}
    >
      <TriggerDefaultIcon
        hoverStartedAsDefault={hoverStartedAsDefault}
        isDefault={isDefault}
        isHovered={isHovered}
      />
    </button>
  );
}

export function FirstVersionInstallButton({
  isLoading: _isLoading,
  onOpenVersionManager,
}: {
  isLoading: boolean;
  onOpenVersionManager: (event: React.MouseEvent) => void;
}) {
  return (
    <motion.button
      onClick={onOpenVersionManager}
      className="rounded p-2 transition-colors hover:bg-[hsl(var(--surface-interactive-hover))]"
      whileHover={{ scale: 1.2 }}
      whileTap={{ scale: 0.9 }}
      title="Install your first version"
    >
      <Globe
        size={20}
        className="animate-pulse text-[hsl(var(--accent-success))]"
        style={{ filter: 'drop-shadow(0 0 8px hsl(var(--accent-success)))' }}
      />
    </motion.button>
  );
}

function ActiveInstallIcon({
  folderIconColor,
  isOpeningPath,
  showOpenedIndicator,
}: {
  folderIconColor: string;
  isOpeningPath: boolean;
  showOpenedIndicator: boolean;
}) {
  if (showOpenedIndicator) {
    return <CheckCircle2 size={14} className="text-[hsl(var(--accent-success))]" />;
  }
  if (isOpeningPath) {
    return <Loader2 size={14} className="animate-spin text-[hsl(var(--text-tertiary))]" />;
  }
  return <FolderOpen size={14} className={folderIconColor} />;
}

export function OpenActiveInstallButton({
  activeVersion,
  folderIconColor,
  isLoading,
  isOpeningPath,
  onOpenActiveInstall,
  showOpenedIndicator,
}: {
  activeVersion: string | null;
  folderIconColor: string;
  isLoading: boolean;
  isOpeningPath: boolean;
  onOpenActiveInstall: (event: React.MouseEvent) => void;
  showOpenedIndicator: boolean;
}) {
  return (
    <motion.button
      onClick={onOpenActiveInstall}
      disabled={!activeVersion || isOpeningPath || isLoading}
      className="rounded p-1 transition-colors hover:bg-[hsl(var(--surface-interactive-hover))] disabled:opacity-50"
      whileHover={{ scale: activeVersion ? 1.1 : 1 }}
      whileTap={{ scale: activeVersion ? 0.9 : 1 }}
      title={activeVersion ? 'Open active version in file manager' : 'No active version to open'}
    >
      <ActiveInstallIcon
        folderIconColor={folderIconColor}
        isOpeningPath={isOpeningPath}
        showOpenedIndicator={showOpenedIndicator}
      />
    </motion.button>
  );
}

function getInstallIconVisual(
  installNetworkStatus: InstallNetworkStatus,
  hasNewVersion: boolean
): { className: string; style?: React.CSSProperties } {
  switch (installNetworkStatus) {
    case 'downloading':
      return {
        className: 'text-[hsl(var(--accent-success))]',
        style: { filter: 'drop-shadow(0 0 6px hsl(var(--accent-success)))' },
      };
    case 'stalled':
      return {
        className: 'animate-pulse text-accent-warning',
        style: { filter: 'drop-shadow(0 0 6px rgb(251 146 60))' },
      };
    case 'failed':
      return {
        className: 'animate-pulse text-[hsl(var(--accent-error))]',
        style: { filter: 'drop-shadow(0 0 6px hsl(var(--accent-error)))' },
      };
    case 'idle':
      break;
  }

  if (hasNewVersion) {
    return {
      className: 'text-[hsl(var(--accent-success))]',
      style: { filter: 'drop-shadow(0 0 8px hsl(var(--accent-success)))' },
    };
  }
  return { className: 'text-[hsl(var(--text-tertiary))]' };
}

function InstallActivityRings({
  isInstallFailed,
  isInstallPending,
  ringDegrees,
}: {
  isInstallFailed: boolean;
  isInstallPending: boolean;
  ringDegrees: number;
}) {
  return (
    <>
      <span
        className={`download-progress-ring ${isInstallPending ? 'is-waiting' : ''}`}
        style={{ '--progress': `${ringDegrees}deg` } as React.CSSProperties}
      />
      {!isInstallPending && !isInstallFailed && <span className="download-scan-ring" />}
    </>
  );
}

function getVersionManagerTitle(hasNewVersion: boolean, latestVersion: string | null): string {
  if (hasNewVersion) {
    return `New version available: ${latestVersion}`;
  }
  return 'Install new version';
}

export function VersionManagerButton({
  emphasizeInstall,
  hasInstallActivity,
  hasNewVersion,
  installNetworkStatus,
  isInstallFailed,
  isInstallPending,
  isLoading: _isLoading,
  latestVersion,
  onOpenVersionManager,
  ringDegrees,
}: {
  emphasizeInstall: boolean;
  hasInstallActivity: boolean;
  hasNewVersion: boolean;
  installNetworkStatus: InstallNetworkStatus;
  isInstallFailed: boolean;
  isInstallPending: boolean;
  isLoading: boolean;
  latestVersion: string | null;
  onOpenVersionManager: (event: React.MouseEvent) => void;
  ringDegrees: number;
}) {
  const isEmphasized = (emphasizeInstall || hasNewVersion) && installNetworkStatus === 'idle';
  const iconVisual = getInstallIconVisual(installNetworkStatus, hasNewVersion);

  return (
    <motion.button
      onClick={onOpenVersionManager}
      className={`rounded p-1 hover:bg-[hsl(var(--surface-interactive-hover))] disabled:opacity-50 ${
        isEmphasized ? 'ring-1 ring-[hsl(var(--accent-success))]/60' : ''
      }`}
      animate={{ opacity: isEmphasized ? [1, 0.7, 1] : 1 }}
      transition={{
        duration: 2,
        repeat: isEmphasized ? Infinity : 0,
        ease: 'easeInOut',
      }}
      whileHover={{ scale: 1.1 }}
      whileTap={{ scale: 0.9 }}
      title={getVersionManagerTitle(hasNewVersion, latestVersion)}
    >
      <span className="relative flex h-4 w-4 items-center justify-center">
        {hasInstallActivity && (
          <InstallActivityRings
            isInstallFailed={isInstallFailed}
            isInstallPending={isInstallPending}
            ringDegrees={ringDegrees}
          />
        )}
        <Globe size={14} className={iconVisual.className} style={iconVisual.style} />
      </span>
    </motion.button>
  );
}
