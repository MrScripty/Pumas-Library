import type { UseVersionsResult } from '../hooks/useVersions';

export interface AppVersionState extends UseVersionsResult {
  appId: string | null;
  isSupported: boolean;
  supportsShortcuts: boolean;
}

const noopBoolean = async (..._args: unknown[]) => false;
const noopVoid = async (..._args: unknown[]) => {};
const noopInfo = async (..._args: unknown[]) => null;

export const UNSUPPORTED_VERSION_STATE: AppVersionState = {
  appId: null,
  isSupported: false,
  supportsShortcuts: false,
  installedVersions: [],
  activeVersion: null,
  availableVersions: [],
  versionStatus: null,
  isLoading: false,
  error: null,
  isRateLimited: false,
  rateLimitRetryAfter: null,
  installingTag: null,
  installationProgress: null,
  defaultVersion: null,
  installNetworkStatus: 'idle',
  cacheStatus: {
    has_cache: false,
    is_valid: false,
    is_fetching: false,
  },
  switchVersion: noopBoolean,
  installVersion: noopBoolean,
  cancelInstallation: noopBoolean,
  removeVersion: noopBoolean,
  getVersionInfo: noopInfo,
  refreshAll: noopVoid,
  refreshAvailableVersions: noopVoid,
  openPath: noopBoolean,
  openActiveInstall: noopBoolean,
  setDefaultVersion: noopVoid,
};

/**
 * Shown while the backend's runtime provider list is still loading, so panels
 * read as loading rather than as an app without version management.
 */
export const LOADING_VERSION_STATE: AppVersionState = {
  ...UNSUPPORTED_VERSION_STATE,
  isLoading: true,
};

/** Whether `appId` is one the backend version-manages (see `useRuntimeProviders`). */
export function isVersionSupportedAppId(
  appId: string | null,
  versionManagedAppIds: readonly string[]
): boolean {
  if (!appId) return false;
  return versionManagedAppIds.includes(appId);
}

export function supportsVersionShortcuts(appId: string | null): boolean {
  if (!appId) return false;
  return false;
}

export function getAppVersionState(
  appId: string | null,
  isSupported: boolean,
  versions: UseVersionsResult
): AppVersionState {
  if (appId && isSupported) {
    return {
      ...versions,
      appId,
      isSupported: true,
      supportsShortcuts: supportsVersionShortcuts(appId),
    };
  }
  return UNSUPPORTED_VERSION_STATE;
}
