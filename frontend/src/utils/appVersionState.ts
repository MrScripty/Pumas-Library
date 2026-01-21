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
  removeVersion: noopBoolean,
  getVersionInfo: noopInfo,
  refreshAll: noopVoid,
  refreshAvailableVersions: noopVoid,
  openPath: noopBoolean,
  openActiveInstall: noopBoolean,
  fetchInstallationProgress: noopInfo,
  setDefaultVersion: noopVoid,
};

const VERSION_SUPPORTED_APP_IDS = new Set(['comfyui', 'ollama']);
const SHORTCUT_SUPPORTED_APP_IDS = new Set(['comfyui']);

export function isVersionSupportedAppId(appId: string | null): boolean {
  if (!appId) return false;
  return VERSION_SUPPORTED_APP_IDS.has(appId);
}

export function supportsVersionShortcuts(appId: string | null): boolean {
  if (!appId) return false;
  return SHORTCUT_SUPPORTED_APP_IDS.has(appId);
}

export function getAppVersionState(
  appId: string | null,
  versions: UseVersionsResult
): AppVersionState {
  if (isVersionSupportedAppId(appId)) {
    return {
      ...versions,
      appId,
      isSupported: true,
      supportsShortcuts: supportsVersionShortcuts(appId),
    };
  }
  return UNSUPPORTED_VERSION_STATE;
}
