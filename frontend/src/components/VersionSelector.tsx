import React, { useCallback, useEffect, useRef, useState } from 'react';
import type { InstallationProgress } from '../hooks/useVersions';
import { APIError } from '../errors';
import { getLogger } from '../utils/logger';
import { VersionSelectorDropdown } from './VersionSelectorDropdown';
import {
  getVersionSelectorDisplayState,
  reportOpenActiveInstallError,
  reportToggleDefaultError,
  reportVersionSwitchError,
} from './VersionSelectorState';
import { VersionSelectorTrigger } from './VersionSelectorTrigger';
import { Popover } from './ui';

const logger = getLogger('VersionSelector');

interface VersionSelectorProps {
  appId?: string | null;
  installedVersions: string[];
  activeVersion: string | null;
  isLoading: boolean;
  switchVersion: (tag: string) => Promise<boolean>;
  openActiveInstall: () => Promise<boolean>;
  onOpenVersionManager: () => void;
  installingVersion?: string | null;
  installationProgress?: InstallationProgress | null;
  installNetworkStatus?: 'idle' | 'downloading' | 'stalled' | 'failed';
  defaultVersion?: string | null;
  onMakeDefault?: (tag: string | null) => Promise<boolean>;
  diskSpacePercent?: number;
  hasNewVersion?: boolean;
  latestVersion?: string | null;
}

export function VersionSelector({
  appId,
  installedVersions,
  activeVersion,
  isLoading,
  switchVersion,
  openActiveInstall,
  onOpenVersionManager,
  installingVersion,
  installationProgress,
  installNetworkStatus = 'idle',
  defaultVersion,
  onMakeDefault,
  diskSpacePercent = 0,
  hasNewVersion = false,
  latestVersion = null,
}: VersionSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isSwitching, setIsSwitching] = useState(false);
  const [switchError, setSwitchError] = useState<string | null>(null);
  const [isOpeningPath, setIsOpeningPath] = useState(false);
  const [showOpenedIndicator, setShowOpenedIndicator] = useState(false);
  const openedIndicatorTimeout = useRef<ReturnType<typeof setTimeout> | null>(null);
  const initialVersionFocusRef = useRef<HTMLButtonElement>(null);

  const handleVersionSwitch = async (tag: string) => {
    if (tag === activeVersion) {
      setIsOpen(false);
      return;
    }

    logger.info('Switching version', { from: activeVersion, to: tag });
    setIsSwitching(true);
    setSwitchError(null);
    try {
      if (!await switchVersion(tag)) throw new APIError('The runtime version could not be changed', 'switch_version');
      logger.info('Version switched successfully', { version: tag });
      setIsOpen(false);
    } catch (error) {
      reportVersionSwitchError(error, tag);
      setSwitchError(error instanceof Error ? error.message : 'The runtime version could not be changed');
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
    appId,
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
    appId,
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

  const handleOpenChange = useCallback((nextIsOpen: boolean) => {
    logger.debug('Version selector state changed', { hasVersionsToShow, nextIsOpen });
    if (hasVersionsToShow || !nextIsOpen) {
      setIsOpen(nextIsOpen);
    }
  }, [hasVersionsToShow]);

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

  useEffect(() => {
    if (!hasVersionsToShow) {
      setIsOpen(false);
    }
  }, [hasVersionsToShow]);

  return (
    <>
    <Popover
      contentClassName="absolute left-0 right-0 top-full z-50 mt-1 overflow-hidden rounded bg-[hsl(var(--surface-overlay))]/80 backdrop-blur-sm"
      initialFocusRef={initialVersionFocusRef}
      isOpen={isOpen}
      label="Version actions"
      onOpenChange={handleOpenChange}
      rootClassName="relative w-full"
      trigger={(popoverTriggerProps) => (
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
          popoverTriggerProps={popoverTriggerProps}
          onToggleDefault={handleToggleDefault}
          onOpenActiveInstall={handleOpenActiveInstall}
          onOpenVersionManager={handleOpenVersionManager}
          canMakeDefault={Boolean(onMakeDefault && activeVersion)}
        />
      )}
    >
      <VersionSelectorDropdown
        combinedVersions={combinedVersions}
        activeVersion={activeVersion}
        installingVersion={installingVersion}
        installedVersions={installedVersions}
        isInstallComplete={isInstallComplete}
        defaultVersion={defaultVersion ?? null}
        isSwitching={isSwitching}
        isLoading={isLoading}
        initialFocusRef={initialVersionFocusRef}
        onMakeDefault={onMakeDefault}
        onSwitchVersion={handleVersionSwitch}
      />
    </Popover>
    {switchError && <p role="alert" className="mt-2 text-sm text-red-400">{switchError}</p>}
    </>
  );
}
