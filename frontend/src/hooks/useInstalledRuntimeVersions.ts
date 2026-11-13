import { useCallback, useEffect, useRef, useState } from 'react';
import { getElectronAPI } from '../api/adapter';
import { getLogger } from '../utils/logger';

const logger = getLogger('useInstalledRuntimeVersions');

export interface InstalledRuntimeVersionsState {
  versions: string[];
  isLoading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

/**
 * Installed versions of the runtime app a managed profile launches (the
 * provider's backend-reported `app_id`), as reported by the backend version
 * manager. Null when that app is not version managed: there is nothing to
 * list. The backend validates any pin on save and launch; this list only
 * feeds the editor's choices.
 */
export function useInstalledRuntimeVersions(
  appId: string | null
): InstalledRuntimeVersionsState {
  const [versions, setVersions] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);

  const refresh = useCallback(async () => {
    const electronAPI = getElectronAPI();
    if (appId === null || !electronAPI?.get_installed_versions) {
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
