import React, { useCallback, useEffect, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { APIError } from '../errors';
import type { InstallationProgress } from '../hooks/useVersions';
import { getLogger } from '../utils/logger';
import { VersionSelectorDropdown } from './VersionSelectorDropdown';
import { VersionSelectorTrigger } from './VersionSelectorTrigger';

const logger = getLogger('VersionSelector');

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

  const handleToggleDefault = useCallback(() => {
    if (!onMakeDefault || !activeVersion) {
      return;
    }
    const isDefault = defaultVersion === activeVersion;
    const target = isDefault ? null : activeVersion;
    onMakeDefault(target).catch((error: unknown) => {
      if (error instanceof APIError) {
        logger.error('API error toggling default version', {
          error: error.message,
          endpoint: error.endpoint,
          version: activeVersion,
        });
      } else if (error instanceof Error) {
        logger.error('Failed to toggle default version', {
          error: error.message,
          version: activeVersion,
        });
      } else {
        logger.error('Unknown error toggling default version', {
          error: String(error),
          version: activeVersion,
        });
      }
    });
  }, [activeVersion, defaultVersion, onMakeDefault]);

  const handleToggleOpen = useCallback(() => {
    logger.debug('Version selector clicked', { hasVersionsToShow, isOpen });
    if (hasVersionsToShow) {
      setIsOpen(!isOpen);
      logger.debug('Dropdown toggled', { newState: !isOpen });
    }
  }, [hasVersionsToShow, isOpen]);

  const handleOpenVersionManager = useCallback((event: React.MouseEvent) => {
    logger.info('Opening version manager');
    event.stopPropagation();
    onOpenVersionManager();
  }, [onOpenVersionManager]);

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
    void refreshShortcutStates();
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
      <VersionSelectorTrigger
        hasVersionsToShow={hasVersionsToShow}
        hasInstalledVersions={hasInstalledVersions}
        installingVersion={installingVersion}
        isLoading={isLoading}
        isSwitching={isSwitching}
        activeVersion={activeVersion}
        defaultVersion={defaultVersion ?? null}
        displayVersion={displayVersion}
        showOpenedIndicator={showOpenedIndicator}
        isOpeningPath={isOpeningPath}
        folderIconColor={folderIconColor}
        emphasizeInstall={emphasizeInstall}
        hasNewVersion={hasNewVersion}
        latestVersion={latestVersion}
        installNetworkStatus={installNetworkStatus}
        hasInstallActivity={hasInstallActivity}
        isInstallPending={isInstallPending}
        isInstallFailed={isInstallFailed}
        ringDegrees={ringDegrees}
        onToggleOpen={handleToggleOpen}
        onToggleDefault={handleToggleDefault}
        onOpenActiveInstall={handleOpenActiveInstall}
        onOpenVersionManager={handleOpenVersionManager}
        canMakeDefault={Boolean(onMakeDefault && activeVersion)}
      />

      <VersionSelectorDropdown
        isOpen={isOpen}
        hasVersionsToShow={hasVersionsToShow}
        combinedVersions={combinedVersions}
        activeVersion={activeVersion}
        shortcutState={shortcutState}
        supportsShortcuts={supportsShortcuts}
        installingVersion={installingVersion}
        installedVersions={installedVersions}
        isInstallComplete={isInstallComplete}
        defaultVersion={defaultVersion ?? null}
        isSwitching={isSwitching}
        isLoading={isLoading}
        onMakeDefault={onMakeDefault}
        onSwitchVersion={handleVersionSwitch}
        onToggleShortcuts={handleToggleShortcuts}
      />
    </div>
  );
}
