import { useCallback, useEffect, useRef, useState } from 'react';
import { getElectronAPI } from '../api/adapter';
import type { RuntimeProviderId } from '../types/api-runtime-profiles';
import { getLogger } from '../utils/logger';
import { getRuntimeProviderDescriptor } from '../utils/runtimeProviderDescriptors';

const logger = getLogger('useInstalledRuntimeVersions');

export interface InstalledRuntimeVersionsState {
  versions: string[];
  isLoading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

/**
 * Installed versions of the runtime a managed profile of `provider` launches,
 * as reported by the backend version manager. The backend validates any pin on
 * save and launch; this list only feeds the editor's choices.
 */
export function useInstalledRuntimeVersions(
  provider: RuntimeProviderId
): InstalledRuntimeVersionsState {
  const appId = getRuntimeProviderDescriptor(provider).managedRuntimeAppId;
  const [versions, setVersions] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);

  const refresh = useCallback(async () => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.get_installed_versions) {
      return;
    }

    const request = ++requestRef.current;
    setIsLoading(true);
    setError(null);
    try {
      const result = await electronAPI.get_installed_versions(appId);
      if (request === requestRef.current) {
        setVersions([...result.versions]);
      }
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : String(caught);
      logger.error('Failed to load installed runtime versions', { appId, error: message });
      if (request === requestRef.current) {
        setVersions([]);
        setError(message);
      }
    } finally {
      if (request === requestRef.current) {
        setIsLoading(false);
      }
    }
  }, [appId]);

  useEffect(() => {
    setVersions([]);
    void refresh();
  }, [refresh]);

  return { versions, isLoading, error, refresh };
}
