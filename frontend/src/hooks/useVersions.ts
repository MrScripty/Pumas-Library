/**
 * useVersions Hook (Refactored)
 *
 * Lightweight orchestrator for version management.
 * Reduced from 728 lines by extracting:
 * - Type definitions → types/versions.ts
 * - Network status monitoring → utils/networkStatusMonitor.ts
 * - Version fetching logic → hooks/useVersionFetching.ts
 * - Installation management → hooks/useInstallationManager.ts
 */

import { useEffect, useState } from 'react';
import { useVersionFetching } from './useVersionFetching';
import { useInstallationManager } from './useInstallationManager';
import type {
  VersionRelease,
  VersionStatus,
  VersionInfo,
  InstallationProgress,
  InstallNetworkStatus,
  CacheStatus,
} from '../types/versions';

interface UseVersionsResult {
  // State
  installedVersions: string[];
  activeVersion: string | null;
  availableVersions: VersionRelease[];
  versionStatus: VersionStatus | null;
  isLoading: boolean;
  error: string | null;
  installingTag: string | null;
  installationProgress: InstallationProgress | null;
  defaultVersion: string | null;
  installNetworkStatus: InstallNetworkStatus;
  cacheStatus: CacheStatus;

  // Actions
  switchVersion: (tag: string) => Promise<boolean>;
  installVersion: (tag: string) => Promise<boolean>;
  removeVersion: (tag: string) => Promise<boolean>;
  getVersionInfo: (tag: string) => Promise<VersionInfo | null>;
  refreshAll: (forceRefresh?: boolean) => Promise<void>;
  refreshAvailableVersions: (forceRefresh?: boolean) => Promise<void>;
  openPath: (path: string) => Promise<boolean>;
  openActiveInstall: () => Promise<boolean>;
  fetchInstallationProgress: () => Promise<InstallationProgress | null>;
  setDefaultVersion: (tag: string | null) => Promise<void>;
}

export function useVersions(): UseVersionsResult {
  const [localInstallingTag, setLocalInstallingTag] = useState<string | null>(null);

  // Version fetching hook
  const {
    installedVersions,
    activeVersion,
    availableVersions,
    versionStatus,
    defaultVersion,
    cacheStatus,
    isLoading,
    error,
    fetchInstalledVersions,
    fetchActiveVersion,
    fetchVersionStatus,
    refreshAll,
    fetchAvailableVersions,
    setDefaultVersion,
  } = useVersionFetching({
    onInstallingTagUpdate: setLocalInstallingTag,
  });

  // Installation management hook
  const {
    installingTag: installManagerTag,
    installationProgress,
    installNetworkStatus,
    switchVersion,
    installVersion,
    removeVersion,
    getVersionInfo,
    openPath,
    openActiveInstall,
    fetchInstallationProgress,
  } = useInstallationManager({
    availableVersions,
    onRefreshVersions: async () => {
      await Promise.all([
        fetchInstalledVersions(),
        fetchActiveVersion(),
        fetchVersionStatus(),
      ]);
    },
  });

  // Merge installing tags from both sources
  const installingTag = installManagerTag || localInstallingTag;

  // Initial load
  useEffect(() => {
    let waitTimeout: NodeJS.Timeout | null = null;

    const waitForApi = () => {
      if (window.pywebview?.api) {
        void refreshAll();
        return;
      }
      waitTimeout = setTimeout(waitForApi, 100);
    };

    waitForApi();

    return () => {
      if (waitTimeout) clearTimeout(waitTimeout);
    };
  }, [refreshAll]);

  return {
    // State
    installedVersions,
    activeVersion,
    availableVersions,
    versionStatus,
    isLoading,
    error,
    installingTag,
    installationProgress,
    defaultVersion,
    installNetworkStatus,
    cacheStatus,

    // Actions
    switchVersion,
    installVersion,
    removeVersion,
    getVersionInfo,
    refreshAll,
    refreshAvailableVersions: fetchAvailableVersions,
    openPath,
    openActiveInstall,
    fetchInstallationProgress,
    setDefaultVersion,
  };
}

// Re-export types for convenience
export type {
  VersionRelease,
  VersionStatus,
  VersionInfo,
  InstallationProgress,
  InstallNetworkStatus,
  CacheStatus,
} from '../types/versions';
