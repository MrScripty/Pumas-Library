import { useMemo } from 'react';
import {
  getAppVersionState,
  isVersionSupportedAppId,
  LOADING_VERSION_STATE,
  type AppVersionState,
} from '../utils/appVersionState';
import {
  useInstalledVersionsByAppId,
  type InstalledVersionsByAppId,
} from './useInstalledVersionsByAppId';
import { useRuntimeProviders, versionManagedAppIds } from './useRuntimeProviders';
import { useVersions, type UseVersionsResult } from './useVersions';

export interface UseSelectedAppVersionsResult {
  appVersions: AppVersionState;
  installationProgress: UseVersionsResult['installationProgress'];
  /** Installed versions of every app the backend version-manages. */
  installedVersionsByAppId: InstalledVersionsByAppId;
}

/**
 * Full version state for the selected app when the backend version-manages
 * it, plus installed-version lists for every version-managed app.
 */
export function useSelectedAppVersions(selectedAppId: string | null): UseSelectedAppVersionsResult {
  const { providers, error: providersError } = useRuntimeProviders();
  const managedAppIds = useMemo(() => versionManagedAppIds(providers), [providers]);
  const isSupported = isVersionSupportedAppId(selectedAppId, managedAppIds);

  const selectedVersions = useVersions({
    appId: isSupported && selectedAppId ? selectedAppId : undefined,
    enabled: isSupported,
    trackAvailableVersions: isSupported,
  });
  const installedVersionsByAppId = useInstalledVersionsByAppId(
    managedAppIds,
    selectedAppId,
    isSupported ? selectedVersions.installedVersions : null
  );

  // Until the backend has said which apps it version-manages, read as loading
  // rather than unsupported; a failed lookup falls back to unsupported.
  const appVersions =
    providers === null && providersError === null && selectedAppId !== null
      ? LOADING_VERSION_STATE
      : getAppVersionState(selectedAppId, isSupported, selectedVersions);

  return {
    appVersions,
    installationProgress: appVersions.installationProgress,
    installedVersionsByAppId,
  };
}
