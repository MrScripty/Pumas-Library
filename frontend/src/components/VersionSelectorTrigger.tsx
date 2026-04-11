import React, { useEffect, useState } from 'react';
import { motion } from 'framer-motion';
import {
  Anchor,
  CheckCircle2,
  CircleX,
  FolderOpen,
  Globe,
  Loader2,
} from 'lucide-react';
import { useHover } from '@react-aria/interactions';

interface VersionSelectorTriggerProps {
  hasVersionsToShow: boolean;
  hasInstalledVersions: boolean;
  installingVersion: string | null | undefined;
  isLoading: boolean;
  isSwitching: boolean;
  activeVersion: string | null;
  defaultVersion: string | null;
  displayVersion: string;
  showOpenedIndicator: boolean;
  isOpeningPath: boolean;
  folderIconColor: string;
  emphasizeInstall: boolean;
  hasNewVersion: boolean;
  latestVersion: string | null;
  installNetworkStatus: 'idle' | 'downloading' | 'stalled' | 'failed';
  hasInstallActivity: boolean;
  isInstallPending: boolean;
  isInstallFailed: boolean;
  ringDegrees: number;
  onToggleOpen: () => void;
  onToggleDefault: () => void;
  onOpenActiveInstall: (event: React.MouseEvent) => void;
  onOpenVersionManager: (event: React.MouseEvent) => void;
  canMakeDefault: boolean;
}

export function VersionSelectorTrigger({
  hasVersionsToShow,
  hasInstalledVersions,
  installingVersion,
  isLoading,
  isSwitching,
  activeVersion,
  defaultVersion,
  displayVersion,
  showOpenedIndicator,
  isOpeningPath,
  folderIconColor,
  emphasizeInstall,
  hasNewVersion,
  latestVersion,
  installNetworkStatus,
  hasInstallActivity,
  isInstallPending,
  isInstallFailed,
  ringDegrees,
  onToggleOpen,
  onToggleDefault,
  onOpenActiveInstall,
  onOpenVersionManager,
  canMakeDefault,
}: VersionSelectorTriggerProps) {
  const { hoverProps: selectorAnchorHoverProps, isHovered: isSelectorAnchorHovered } = useHover({});
  const [anchorHoverStartedAsDefault, setAnchorHoverStartedAsDefault] = useState(false);

  useEffect(() => {
    if (isSelectorAnchorHovered) {
      setAnchorHoverStartedAsDefault(defaultVersion === activeVersion);
    }
  }, [activeVersion, defaultVersion, isSelectorAnchorHovered]);

  return (
    <div
      className={`flex h-10 w-full items-center justify-center rounded border border-[hsl(var(--border-control))] bg-[hsl(var(--surface-interactive))] transition-colors ${
        !hasVersionsToShow || isLoading || isSwitching ? 'opacity-50' : ''
      }`}
    >
      {!hasInstalledVersions && !installingVersion ? (
        <motion.button
          onClick={onOpenVersionManager}
          disabled={isLoading}
          className="rounded p-2 transition-colors hover:bg-[hsl(var(--surface-interactive-hover))] disabled:opacity-50"
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
      ) : (
        <>
          <button
            onClick={onToggleOpen}
            disabled={!hasVersionsToShow || isLoading || isSwitching}
            className="flex flex-1 items-center gap-2 px-3 transition-opacity hover:opacity-80 disabled:cursor-not-allowed"
          >
            <span className="inline-flex w-4 items-center justify-center">
              {isSwitching ? (
                <Loader2 size={14} className="animate-spin text-[hsl(var(--text-tertiary))]" />
              ) : canMakeDefault ? (
                <button
                  {...selectorAnchorHoverProps}
                  onClick={(event) => {
                    event.stopPropagation();
                    onToggleDefault();
                  }}
                  className="flex h-4 w-4 items-center justify-center"
                  title={
                    defaultVersion === activeVersion
                      ? 'Click to unset as default'
                      : 'Click to set as default'
                  }
                  disabled={isLoading}
                >
                  {defaultVersion === activeVersion ? (
                    isSelectorAnchorHovered && anchorHoverStartedAsDefault ? (
                      <CircleX size={14} className="text-[hsl(var(--text-tertiary))]" />
                    ) : (
                      <Anchor size={14} className="text-[hsl(var(--accent-success))]" />
                    )
                  ) : (
                    <div className="h-2 w-2 rounded-full bg-[hsl(var(--accent-success))]" />
                  )}
                </button>
              ) : (
                <div className="w-4" />
              )}
            </span>
            <span className="text-sm font-medium text-[hsl(var(--text-primary))]">
              {displayVersion}
            </span>
          </button>

          <div className="flex items-center gap-2 px-3">
            <motion.button
              onClick={onOpenActiveInstall}
              disabled={!activeVersion || isOpeningPath || isLoading}
              className="rounded p-1 transition-colors hover:bg-[hsl(var(--surface-interactive-hover))] disabled:opacity-50"
              whileHover={{ scale: activeVersion ? 1.1 : 1 }}
              whileTap={{ scale: activeVersion ? 0.9 : 1 }}
              title={activeVersion ? 'Open active version in file manager' : 'No active version to open'}
            >
              {showOpenedIndicator ? (
                <CheckCircle2 size={14} className="text-[hsl(var(--accent-success))]" />
              ) : isOpeningPath ? (
                <Loader2 size={14} className="animate-spin text-[hsl(var(--text-tertiary))]" />
              ) : (
                <FolderOpen size={14} className={folderIconColor} />
              )}
            </motion.button>

            <motion.button
              onClick={onOpenVersionManager}
              disabled={isLoading}
              className={`rounded p-1 hover:bg-[hsl(var(--surface-interactive-hover))] disabled:opacity-50 ${
                (emphasizeInstall || hasNewVersion) && installNetworkStatus === 'idle'
                  ? 'ring-1 ring-[hsl(var(--accent-success))]/60'
                  : ''
              }`}
              animate={{
                opacity:
                  (emphasizeInstall || hasNewVersion) && installNetworkStatus === 'idle'
                    ? [1, 0.7, 1]
                    : 1,
              }}
              transition={{
                duration: 2,
                repeat:
                  (emphasizeInstall || hasNewVersion) && installNetworkStatus === 'idle'
                    ? Infinity
                    : 0,
                ease: 'easeInOut',
              }}
              whileHover={{ scale: 1.1 }}
              whileTap={{ scale: 0.9 }}
              title={hasNewVersion ? `New version available: ${latestVersion}` : 'Install new version'}
            >
              <span className="relative flex h-4 w-4 items-center justify-center">
                {hasInstallActivity && (
                  <>
                    <span
                      className={`download-progress-ring ${isInstallPending ? 'is-waiting' : ''}`}
                      style={{ '--progress': `${ringDegrees}deg` } as React.CSSProperties}
                    />
                    {!isInstallPending && !isInstallFailed && <span className="download-scan-ring" />}
                  </>
                )}
                <Globe
                  size={14}
                  className={
                    installNetworkStatus === 'downloading'
                      ? 'text-[hsl(var(--accent-success))]'
                      : installNetworkStatus === 'stalled'
                        ? 'animate-pulse text-accent-warning'
                        : installNetworkStatus === 'failed'
                          ? 'animate-pulse text-[hsl(var(--accent-error))]'
                          : hasNewVersion
                            ? 'text-[hsl(var(--accent-success))]'
                            : 'text-[hsl(var(--text-tertiary))]'
                  }
                  style={
                    installNetworkStatus === 'downloading'
                      ? { filter: 'drop-shadow(0 0 6px hsl(var(--accent-success)))' }
                      : installNetworkStatus === 'stalled'
                        ? { filter: 'drop-shadow(0 0 6px rgb(251 146 60))' }
                        : installNetworkStatus === 'failed'
                          ? { filter: 'drop-shadow(0 0 6px hsl(var(--accent-error)))' }
                          : hasNewVersion
                            ? { filter: 'drop-shadow(0 0 8px hsl(var(--accent-success)))' }
                            : undefined
                  }
                />
              </span>
            </motion.button>
          </div>
        </>
      )}
    </div>
  );
}
