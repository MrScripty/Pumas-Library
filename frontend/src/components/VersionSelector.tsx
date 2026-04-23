import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useVersionShortcutState } from '../hooks/useVersionShortcutState';
import type { InstallationProgress } from '../hooks/useVersions';
import { getLogger } from '../utils/logger';
import { VersionSelectorDropdown } from './VersionSelectorDropdown';
import {
  getVersionSelectorDisplayState,
  reportOpenActiveInstallError,
  reportToggleDefaultError,
  reportVersionSwitchError,
} from './VersionSelectorState';
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
  const containerRef = useRef<HTMLDivElement | null>(null);
  const { shortcutState, toggleShortcuts } = useVersionShortcutState({
    activeShortcutState,
    activeVersion,
    installedVersions,
    supportsShortcuts,
  });

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
      reportVersionSwitchError(error, tag);
    } finally {
      setIsSwitching(false);
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
      reportOpenActiveInstallError(error, activeVersion);
    } finally {
      setIsOpeningPath(false);
    }
  };

  const {
    combinedVersions,
    displayVersion,
    emphasizeInstall,
    folderIconColor,
    hasInstallActivity,
    hasInstalledVersions,
    hasVersionsToShow,
    isInstallComplete,
    isInstallFailed,
    isInstallPending,
    ringDegrees,
  } = React.useMemo(() => getVersionSelectorDisplayState({
    activeVersion,
    diskSpacePercent,
    hasNewVersion,
    installedVersions,
    installNetworkStatus,
    installationProgress,
    installingVersion,
    isLoading,
    latestVersion,
  }), [
    activeVersion,
    diskSpacePercent,
    hasNewVersion,
    installedVersions,
    installNetworkStatus,
    installationProgress,
    installingVersion,
    isLoading,
    latestVersion,
  ]);

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

  const handleToggleDefault = useCallback(() => {
    if (!onMakeDefault || !activeVersion) {
      return;
    }
    const isDefault = defaultVersion === activeVersion;
    const target = isDefault ? null : activeVersion;
    onMakeDefault(target).catch((error: unknown) => {
      reportToggleDefaultError(error, activeVersion);
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
        onToggleShortcuts={toggleShortcuts}
      />
    </div>
  );
}
