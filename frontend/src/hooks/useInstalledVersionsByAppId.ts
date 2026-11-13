import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { getLogger } from '../utils/logger';

const logger = getLogger('useInstalledVersionsByAppId');

const NO_VERSIONS: readonly string[] = Object.freeze([]);

export type InstalledVersionsByAppId = Readonly<Record<string, readonly string[]>>;

/**
 * Installed versions of every version-managed app, keyed by app id, for
 * surfaces (sidebar icons) that only need to know what is installed.
 *
 * Every app is re-read when the set of apps or the selection changes, and the
 * selected app again whenever its full version state reports a new installed
 * list (an install or removal finished).
 */
export function useInstalledVersionsByAppId(
  appIds: readonly string[],
  selectedAppId: string | null,
  selectedInstalledVersions: readonly string[] | null
): InstalledVersionsByAppId {
  const [fetched, setFetched] = useState<Record<string, readonly string[]>>({});
  const sequenceRef = useRef(new Map<string, number>());
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const refreshApp = useCallback(async (appId: string) => {
    if (!isAPIAvailable()) {
      return;
    }
    const sequence = (sequenceRef.current.get(appId) ?? 0) + 1;
    sequenceRef.current.set(appId, sequence);
    try {
      const result = await api.get_installed_versions(appId);
      if (!mountedRef.current || sequenceRef.current.get(appId) !== sequence) {
        return;
      }
      setFetched((current) => ({ ...current, [appId]: [...result.versions] }));
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : String(caught);
      logger.warn('Failed to load installed versions', { appId, error: message });
    }
  }, []);

  useEffect(() => {
    for (const appId of appIds) {
      void refreshApp(appId);
    }
  }, [appIds, refreshApp, selectedAppId]);

  useEffect(() => {
    if (selectedAppId && selectedInstalledVersions && appIds.includes(selectedAppId)) {
      void refreshApp(selectedAppId);
    }
    // Keyed on the list's identity: a new list means the selected app's
    // installs changed, so the sidebar copy is re-read from the backend.
  }, [selectedInstalledVersions]);

  return useMemo(
    () => Object.fromEntries(appIds.map((appId) => [appId, fetched[appId] ?? NO_VERSIONS])),
    [appIds, fetched]
  );
}
