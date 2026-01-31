import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { motion, AnimatePresence } from 'framer-motion';
import { Check, Loader2, Globe, FolderOpen, CheckCircle2, Link2, Anchor, CircleX } from 'lucide-react';
import { useHover } from '@react-aria/interactions';
import type { InstallationProgress } from '../hooks/useVersions';
import { getLogger } from '../utils/logger';
import { APIError } from '../errors';

const logger = getLogger('VersionSelector');

// Component for individual version dropdown items to handle hover state
interface VersionDropdownItemProps {
  version: string;
  isActive: boolean;
  isInstalling: boolean;
  isSwitching: boolean;
  isLoading: boolean;
  isDefault: boolean;
  isEnabled: boolean;
  supportsShortcuts: boolean;
  onMakeDefault?: (tag: string | null) => Promise<boolean>;
  onSwitchVersion: (tag: string) => void;
  onToggleShortcuts?: (version: string, enabled: boolean) => Promise<void>;
}

const VersionDropdownItem: React.FC<VersionDropdownItemProps> = ({
  version,
  isActive,
  isInstalling,
  isSwitching,
  isLoading,
  isDefault,
  isEnabled,
  supportsShortcuts,
  onMakeDefault,
  onSwitchVersion,
  onToggleShortcuts,
}) => {
  // Hover states for the row and anchor button
  const { hoverProps: rowHoverProps, isHovered: isRowHovered } = useHover({});
  const { hoverProps: anchorHoverProps, isHovered: isAnchorHovered } = useHover({});
  const [anchorHoverStartedAsDefault, setAnchorHoverStartedAsDefault] = useState(false);

  // Track when hover starts on the anchor button
  useEffect(() => {
    if (isAnchorHovered) {
      setAnchorHoverStartedAsDefault(isDefault);
    }
  }, [isAnchorHovered, isDefault]);

  return (
    <div
      {...rowHoverProps}
      role="button"
      tabIndex={0}
      onClick={() => {
        if (isInstalling) return;
        onSwitchVersion(version);
      }}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          if (isInstalling) return;
          onSwitchVersion(version);
        }
      }}
      className={`relative w-full px-3 py-2 text-left text-sm flex items-center justify-between transition-colors ${
        isActive
          ? 'bg-[hsl(var(--surface-interactive-hover))] text-[hsl(var(--accent-success))]'
          : isInstalling
            ? 'text-[hsl(var(--text-tertiary))] bg-[hsl(var(--surface-interactive))]'
            : 'text-[hsl(var(--text-secondary))] hover:bg-[hsl(var(--surface-interactive-hover))] hover:text-[hsl(var(--text-primary))]'
      } ${isSwitching || isInstalling ? 'opacity-50 cursor-not-allowed' : ''}`}
    >
      <div className="flex items-center gap-2 min-w-0">
        {/* Anchor on the left of version */}
        <div className="w-4 flex items-center justify-center flex-shrink-0">
          {onMakeDefault ? (
            <button
              {...anchorHoverProps}
              onClick={(e) => {
                e.stopPropagation();
                if (!onMakeDefault) return;
                if (isDefault) {
                  onMakeDefault(null).catch((error) => {
                    if (error instanceof APIError) {
                      logger.error('API error clearing default version', { error: error.message, endpoint: error.endpoint, version });
                    } else if (error instanceof Error) {
                      logger.error('Failed to clear default version', { error: error.message, version });
                    } else {
                      logger.error('Unknown error clearing default version', { error, version });
                    }
                  });
                } else {
                  onMakeDefault(version).catch((error) => {
                    if (error instanceof APIError) {
                      logger.error('API error setting default version', { error: error.message, endpoint: error.endpoint, version });
                    } else if (error instanceof Error) {
                      logger.error('Failed to set default version', { error: error.message, version });
                    } else {
                      logger.error('Unknown error setting default version', { error, version });
                    }
                  });
                }
              }}
              className="flex items-center justify-center"
              title={
                isDefault
                  ? 'Click to unset as default'
                  : 'Click to set as default'
              }
              disabled={isSwitching || isLoading}
            >
              {isDefault && isAnchorHovered && anchorHoverStartedAsDefault ? (
                <CircleX size={14} className="text-[hsl(var(--text-tertiary))]" />
              ) : isDefault ? (
                <Anchor
                  size={14}
                  className="text-[hsl(var(--accent-success))]"
                />
              ) : isRowHovered ? (
                <Anchor size={14} className="text-[hsl(var(--text-tertiary))]" />
              ) : (
                <Anchor size={14} className="text-transparent" />
              )}
            </button>
          ) : (
            <div className="w-4" />
          )}
        </div>
        <span className="font-medium truncate">{version}</span>
        {isInstalling && (
          <span className="px-1.5 py-[2px] text-[10px] rounded-full bg-amber-500/20 border border-amber-400/60 text-amber-200">
            Installing
          </span>
        )}
      </div>
      <div className="flex items-center gap-2 pr-12">
        {supportsShortcuts && !isInstalling && (isRowHovered || isEnabled) && onToggleShortcuts && (
          <button
            onClick={async (e) => {
              e.stopPropagation();
              const next = !isEnabled;
              await onToggleShortcuts(version, next);
            }}
            disabled={isSwitching || isLoading}
            className="absolute right-8 top-1/2 -translate-y-1/2 flex items-center justify-center transition-colors"
            title={isEnabled ? 'Shortcuts enabled (click to disable)' : 'Shortcuts disabled (click to enable)'}
          >
            <Link2
              size={14}
              className={isEnabled ? 'text-[hsl(var(--accent-link))]' : 'text-[hsl(var(--text-tertiary))]'}
              style={{ opacity: 1 }}
              aria-hidden
            />
          </button>
        )}
        {isActive && (
          <span className="absolute right-2 top-1/2 -translate-y-1/2">
            <Check size={14} className="text-[hsl(var(--accent-success))]" />
          </span>
        )}
      </div>
    </div>
  );
};

interface VersionSelectorProps {
  installedVersions: string[];
  activeVersion: string | null;
  isLoading: boolean;
  switchVersion: (tag: string) => Promise<boolean>;
  openActiveInstall: () => Promise<boolean>;
  onOpenVersionManager: () => void;
  activeShortcutState?: { menu: boolean; desktop: boolean };
  installingVersion?: string | null;
  installationProgress?: InstallationProgress | null;
  installNetworkStatus?: 'idle' | 'downloading' | 'stalled' | 'failed';
  defaultVersion?: string | null;
  onMakeDefault?: (tag: string | null) => Promise<boolean>;
  diskSpacePercent?: number;
  hasNewVersion?: boolean;
  latestVersion?: string | null;
  supportsShortcuts?: boolean;
}

export function VersionSelector({
  installedVersions,
  activeVersion,
  isLoading,
  switchVersion,
  openActiveInstall,
  onOpenVersionManager,
  activeShortcutState,
  installingVersion,
  installationProgress,
  installNetworkStatus = 'idle',
  defaultVersion,
  onMakeDefault,
  diskSpacePercent = 0,
  hasNewVersion = false,
  latestVersion = null,
  supportsShortcuts = true,
}: VersionSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isSwitching, setIsSwitching] = useState(false);
  const [isOpeningPath, setIsOpeningPath] = useState(false);
  const [showOpenedIndicator, setShowOpenedIndicator] = useState(false);
  const openedIndicatorTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [shortcutState, setShortcutState] = useState<Record<string, { menu: boolean; desktop: boolean }>>({});
  const containerRef = useRef<HTMLDivElement | null>(null);

  // React Aria hover hook for the selector anchor button
  const { hoverProps: selectorAnchorHoverProps, isHovered: isSelectorAnchorHovered } = useHover({});

  const refreshShortcutStates = useCallback(async () => {
    if (!isAPIAvailable() || !supportsShortcuts) {
      return;
    }

    try {
      const result = await api.get_all_shortcut_states();
      const states = result?.states?.states;
      if (result.success && states) {
        const mapped: Record<string, { menu: boolean; desktop: boolean }> = {};
        Object.entries(states).forEach(([tag, state]) => {
          const typedState = state as { menu: boolean; desktop: boolean; tag?: string };
          mapped[tag] = {
            menu: Boolean(typedState.menu),
            desktop: Boolean(typedState.desktop),
          };
        });
        setShortcutState(mapped);
        logger.debug('Shortcut states refreshed', { stateCount: Object.keys(mapped).length });
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching shortcut states', { error: error.message, endpoint: error.endpoint });
      } else if (error instanceof Error) {
        logger.error('Failed to fetch shortcut states', { error: error.message });
      } else {
        logger.error('Unknown error fetching shortcut states', { error });
      }
    }
  }, [supportsShortcuts]);

  const handleVersionSwitch = async (tag: string) => {
    if (tag === activeVersion) {
      setIsOpen(false);
      return;
    }

    logger.info('Switching version', { from: activeVersion, to: tag });
    setIsSwitching(true);
    try {
      await switchVersion(tag);
      logger.info('Version switched successfully', { version: tag });
      setIsOpen(false);
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error switching version', { error: error.message, endpoint: error.endpoint, tag });
      } else if (error instanceof Error) {
        logger.error('Failed to switch version', { error: error.message, tag });
      } else {
        logger.error('Unknown error switching version', { error, tag });
      }
    } finally {
      setIsSwitching(false);
    }
  };

  const handleToggleShortcuts = async (version: string, next: boolean) => {
    if (!isAPIAvailable() || !supportsShortcuts) {
      logger.warn('Shortcut API not available');
      return;
    }

    logger.info('Toggling shortcuts', { version, enabled: next });
    setShortcutState((prev) => ({
      ...prev,
      [version]: { menu: next, desktop: next },
    }));

    try {
      if (isAPIAvailable()) {
        const result = await api.set_version_shortcuts(version, next);
        if (result?.state) {
          setShortcutState((prev) => ({
            ...prev,
            [version]: {
              menu: Boolean(result.state.menu),
              desktop: Boolean(result.state.desktop),
            },
          }));
          logger.info('Shortcuts toggled successfully', { version, state: result.state });
        }
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error toggling shortcuts', { error: error.message, endpoint: error.endpoint, version });
      } else if (error instanceof Error) {
        logger.error('Failed to toggle shortcuts', { error: error.message, version });
      } else {
        logger.error('Unknown error toggling shortcuts', { error, version });
      }
      // revert on failure
      const isEnabled = !next;
      setShortcutState((prev) => ({
        ...prev,
        [version]: { menu: isEnabled, desktop: isEnabled },
      }));
    }
  };

  const handleOpenActiveInstall = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!activeVersion) {
      return;
    }

    logger.info('Opening active installation path', { version: activeVersion });
    setIsOpeningPath(true);
    setShowOpenedIndicator(false);
    try {
      await openActiveInstall();
      logger.info('Active installation path opened successfully', { version: activeVersion });
      setShowOpenedIndicator(true);
      if (openedIndicatorTimeout.current) {
        clearTimeout(openedIndicatorTimeout.current);
      }
      openedIndicatorTimeout.current = setTimeout(() => setShowOpenedIndicator(false), 2000);
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error opening installation path', { error: error.message, endpoint: error.endpoint, version: activeVersion });
      } else if (error instanceof Error) {
        logger.error('Failed to open installation path', { error: error.message, version: activeVersion });
      } else {
        logger.error('Unknown error opening installation path', { error, version: activeVersion });
      }
    } finally {
      setIsOpeningPath(false);
    }
  };

  const combinedVersions = React.useMemo(() => {
    const unique = new Set(installedVersions);
    const merged = [...installedVersions];
    if (installingVersion && !unique.has(installingVersion)) {
      merged.push(installingVersion);
    }
    // Sort by version number - most recent first (reverse sort)
    return merged.sort((a, b) => b.localeCompare(a, undefined, { numeric: true, sensitivity: 'base' }));
  }, [installedVersions, installingVersion]);

  const displayVersion = activeVersion || 'No version selected';
  const hasInstalledVersions = installedVersions.length > 0;
  const hasVersionsToShow = combinedVersions.length > 0;
  const emphasizeInstall = !hasInstalledVersions && !isLoading;
  // Check both installingVersion and that installation is not complete
  const isInstallComplete = Boolean(installationProgress?.completed_at);
  const hasInstallActivity = Boolean(installingVersion) && !isInstallComplete;
  const isInstallFailed = installNetworkStatus === 'failed';
  const isInstallPending = hasInstallActivity && !isInstallFailed && (
    !installationProgress
    || (
      installationProgress.stage === 'download'
      && (installationProgress.downloaded_bytes ?? 0) <= 0
      && (installationProgress.download_speed ?? 0) <= 0
      && !installationProgress.error
    )
  );
  const progressPercent = installationProgress?.overall_progress ?? 0;
  const progressDegrees = Math.min(
    360,
    Math.max(0, Math.round((Math.min(100, Math.max(0, progressPercent)) / 100) * 360))
  );
  const ringDegrees = isInstallPending ? 60 : progressDegrees;

  // Debug logging for new version detection
  React.useEffect(() => {
    logger.debug('Version state updated', {
      hasNewVersion,
      latestVersion,
      activeVersion,
      installNetworkStatus,
      emphasizeInstall,
      isLoading,
      shouldPulse: (emphasizeInstall || hasNewVersion) && installNetworkStatus === 'idle'
    });
  }, [hasNewVersion, latestVersion, activeVersion, installNetworkStatus, emphasizeInstall, isLoading]);

  // Determine folder icon color based on disk space
  const getFolderIconColor = () => {
    if (diskSpacePercent >= 95) return 'text-accent-error';
    if (diskSpacePercent >= 85) return 'text-accent-warning';
    return 'text-tertiary';
  };

  const folderIconColor = getFolderIconColor();

  useEffect(() => {
    return () => {
      if (openedIndicatorTimeout.current) {
        clearTimeout(openedIndicatorTimeout.current);
      }
    };
  }, []);

  // Refresh shortcut states when installed versions change
  useEffect(() => {
    if (!supportsShortcuts) {
      setShortcutState({});
      return;
    }
    if (!installedVersions.length) {
      setShortcutState({});
      return;
    }
    refreshShortcutStates();
  }, [installedVersions, refreshShortcutStates, supportsShortcuts]);

  // Keep active version shortcut in sync with main toggle state
  useEffect(() => {
    if (!supportsShortcuts) {
      return;
    }
    if (activeVersion && activeShortcutState) {
      setShortcutState((prev) => ({
        ...prev,
        [activeVersion]: {
          menu: activeShortcutState.menu,
          desktop: activeShortcutState.desktop,
        },
      }));
    }
  }, [activeVersion, activeShortcutState?.menu, activeShortcutState?.desktop, supportsShortcuts]);

  // Close dropdown on outside click
  useEffect(() => {
    const handleOutsideClick = (event: MouseEvent | TouchEvent) => {
      if (!isOpen) return;
      const target = event.target as Node | null;
      if (containerRef.current && target && !containerRef.current.contains(target)) {
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleOutsideClick);
    document.addEventListener('touchstart', handleOutsideClick);
    return () => {
      document.removeEventListener('mousedown', handleOutsideClick);
      document.removeEventListener('touchstart', handleOutsideClick);
    };
  }, [isOpen]);

  return (
    <div className="relative w-full" ref={containerRef}>
      {/* Version Selector Container - Changed from button to div to allow nested buttons */}
      <div
        className={`w-full h-10 bg-[hsl(var(--surface-interactive))] border border-[hsl(var(--border-control))] rounded flex items-center justify-center transition-colors ${
          !hasVersionsToShow || isLoading || isSwitching ? 'opacity-50' : ''
        }`}
      >
        {/* No versions installed - show centered download button */}
        {!hasInstalledVersions && !installingVersion ? (
          <motion.button
            onClick={(e) => {
              e.stopPropagation();
              onOpenVersionManager();
            }}
            disabled={isLoading}
            className="p-2 rounded hover:bg-[hsl(var(--surface-interactive-hover))] transition-colors disabled:opacity-50"
            whileHover={{ scale: 1.2 }}
            whileTap={{ scale: 0.9 }}
            title="Install your first version"
          >
            <Globe
              size={20}
              className="text-[hsl(var(--accent-success))] animate-pulse"
              style={{ filter: 'drop-shadow(0 0 8px hsl(var(--accent-success)))' }}
            />
          </motion.button>
        ) : (
          <>
            {/* Left side - clickable area for version selector */}
            <button
              onClick={() => {
                logger.debug('Version selector clicked', { hasVersionsToShow, isOpen });
                if (hasVersionsToShow) {
                  setIsOpen(!isOpen);
                  logger.debug('Dropdown toggled', { newState: !isOpen });
                }
              }}
              disabled={!hasVersionsToShow || isLoading || isSwitching}
              className="flex items-center gap-2 flex-1 hover:opacity-80 transition-opacity disabled:cursor-not-allowed px-3"
            >
              <span className="inline-flex items-center justify-center w-4">
                {isSwitching ? (
                  <Loader2 size={14} className="text-[hsl(var(--text-tertiary))] animate-spin" />
                ) : (
                  <button
                    {...selectorAnchorHoverProps}
                    onClick={(e) => {
                      e.stopPropagation();
                      if (!onMakeDefault || !activeVersion) return;
                      const isDefault = defaultVersion === activeVersion;
                      const target = isDefault ? null : activeVersion;
                      onMakeDefault(target).catch((error) => {
                        if (error instanceof APIError) {
                          logger.error('API error toggling default version', { error: error.message, endpoint: error.endpoint, version: activeVersion });
                        } else if (error instanceof Error) {
                          logger.error('Failed to toggle default version', { error: error.message, version: activeVersion });
                        } else {
                          logger.error('Unknown error toggling default version', { error, version: activeVersion });
                        }
                      });
                    }}
                    className="flex items-center justify-center w-4 h-4"
                    title={
                      defaultVersion === activeVersion
                        ? 'Click to unset as default'
                        : 'Click to set as default'
                    }
                    disabled={!onMakeDefault || isLoading}
                  >
                    {defaultVersion === activeVersion ? (
                      isSelectorAnchorHovered ? (
                        <CircleX size={14} className="text-[hsl(var(--text-tertiary))]" />
                      ) : (
                        <Anchor
                          size={14}
                          className="text-[hsl(var(--accent-success))]"
                        />
                      )
                    ) : (
                      <div className="w-2 h-2 rounded-full bg-[hsl(var(--accent-success))]" />
                    )}
                  </button>
                )}
              </span>
              <span className="text-sm text-[hsl(var(--text-primary))] font-medium">
                {displayVersion}
              </span>
            </button>

            {/* Right side - action buttons */}
            <div className="flex items-center gap-2 px-3">
              {/* Open in File Manager */}
              <motion.button
                onClick={handleOpenActiveInstall}
                disabled={!activeVersion || isOpeningPath || isLoading}
                className="p-1 rounded hover:bg-[hsl(var(--surface-interactive-hover))] transition-colors disabled:opacity-50"
                whileHover={{ scale: activeVersion ? 1.1 : 1 }}
                whileTap={{ scale: activeVersion ? 0.9 : 1 }}
                title={activeVersion ? 'Open active version in file manager' : 'No active version to open'}
              >
                {showOpenedIndicator ? (
                  <CheckCircle2 size={14} className="text-[hsl(var(--accent-success))]" />
                ) : isOpeningPath ? (
                  <Loader2 size={14} className="text-[hsl(var(--text-tertiary))] animate-spin" />
                ) : (
                  <FolderOpen size={14} className={folderIconColor} />
                )}
              </motion.button>

              {/* Download Button */}
              <motion.button
                onClick={(e) => {
                  logger.info('Opening version manager');
                  e.stopPropagation();
                  onOpenVersionManager();
                }}
                disabled={isLoading}
                className={`p-1 rounded hover:bg-[hsl(var(--surface-interactive-hover))] disabled:opacity-50 ${
                  (emphasizeInstall || hasNewVersion) && installNetworkStatus === 'idle' ? 'ring-1 ring-[hsl(var(--accent-success))]/60' : ''
                }`}
                animate={{
                  opacity: (emphasizeInstall || hasNewVersion) && installNetworkStatus === 'idle' ? [1, 0.7, 1] : 1,
                }}
                transition={{
                  duration: 2,
                  repeat: (emphasizeInstall || hasNewVersion) && installNetworkStatus === 'idle' ? Infinity : 0,
                  ease: "easeInOut"
                }}
                whileHover={{ scale: 1.1 }}
                whileTap={{ scale: 0.9 }}
                title={hasNewVersion ? `New version available: ${latestVersion}` : "Install new version"}
              >
                <span className="relative flex h-4 w-4 items-center justify-center">
                  {hasInstallActivity && (
                    <>
                      <span
                        className={`download-progress-ring ${isInstallPending ? 'is-waiting' : ''}`}
                        style={
                          {
                            '--progress': `${ringDegrees}deg`,
                          } as React.CSSProperties
                        }
                      />
                      {!isInstallPending && !isInstallFailed && <span className="download-scan-ring" />}
                    </>
                  )}
                  <Globe
                    size={14}
                    className={`${
                      installNetworkStatus === 'downloading'
                        ? 'text-[hsl(var(--accent-success))]'
                        : installNetworkStatus === 'stalled'
                          ? 'animate-pulse text-accent-warning'
                          : installNetworkStatus === 'failed'
                            ? 'animate-pulse text-[hsl(var(--accent-error))]'
                            : hasNewVersion
                              ? 'text-[hsl(var(--accent-success))]'
                              : 'text-[hsl(var(--text-tertiary))]'
                    }`}
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

              {/* Refresh Button moved to manager; dropdown arrow removed for cleaner look */}
            </div>
          </>
        )}
      </div>

      {/* Dropdown Menu */}
      <AnimatePresence>
        {isOpen && hasVersionsToShow && (
          <motion.div
            initial={{ opacity: 0, y: -10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -10 }}
            transition={{ duration: 0.2 }}
            className="absolute top-full left-0 right-0 mt-1 bg-[hsl(var(--surface-overlay))]/80 backdrop-blur-sm rounded overflow-hidden z-50"
          >
            <div className="max-h-64 overflow-y-auto">
              {combinedVersions.map((version) => {
                const isActive = version === activeVersion;
                const toggles = shortcutState[version] || { menu: false, desktop: false };
                const isEnabled = supportsShortcuts && toggles.menu && toggles.desktop;
                const isInstalling = installingVersion === version && !installedVersions.includes(version) && !isInstallComplete;
                const isDefault = defaultVersion === version;
                return (
                  <VersionDropdownItem
                    key={version}
                    version={version}
                    isActive={isActive}
                    isInstalling={isInstalling}
                    isSwitching={isSwitching}
                    isLoading={isLoading}
                    isDefault={isDefault}
                    isEnabled={isEnabled}
                    supportsShortcuts={supportsShortcuts}
                    onMakeDefault={onMakeDefault}
                    onSwitchVersion={handleVersionSwitch}
                    onToggleShortcuts={handleToggleShortcuts}
                  />
                );
              })}
            </div>

            {installedVersions.length === 0 && (
              <div className="px-3 py-4 text-sm text-[hsl(var(--text-tertiary))] text-center">
                No versions installed
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>

    </div>
  );
}
