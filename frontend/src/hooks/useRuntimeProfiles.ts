import { useCallback, useEffect, useRef, useState } from 'react';
import { getElectronAPI } from '../api/adapter';
import {
  isRuntimeProfileUpdateFeed,
  type RuntimeProfileUpdateFeed,
  type RuntimeProfilesSnapshot,
} from '../types/api-runtime-profiles';
import { getLogger } from '../utils/logger';

const logger = getLogger('useRuntimeProfiles');

export const RUNTIME_PROFILE_UPDATE_REFRESH_DEBOUNCE_MS = 250;

export function useRuntimeProfileUpdateSubscription(
  onUpdate: (notification: RuntimeProfileUpdateFeed) => void,
  debounceMs = RUNTIME_PROFILE_UPDATE_REFRESH_DEBOUNCE_MS
): void {
  const onUpdateRef = useRef(onUpdate);

  useEffect(() => {
    onUpdateRef.current = onUpdate;
  }, [onUpdate]);

  useEffect(() => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.onRuntimeProfileUpdate) {
      return undefined;
    }

    let latestNotification: RuntimeProfileUpdateFeed | null = null;
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;

    const clearRefreshTimer = () => {
      if (refreshTimer) {
        clearTimeout(refreshTimer);
        refreshTimer = null;
      }
    };

    const unsubscribe = electronAPI.onRuntimeProfileUpdate((notification) => {
      if (!isRuntimeProfileUpdateFeed(notification)) {
        logger.warn('Ignoring invalid runtime-profile update notification');
        return;
      }

      latestNotification = notification;
      clearRefreshTimer();
      refreshTimer = setTimeout(() => {
        refreshTimer = null;
        if (latestNotification) {
          onUpdateRef.current(latestNotification);
        }
      }, debounceMs);
    });

    return () => {
      clearRefreshTimer();
      unsubscribe();
    };
  }, [debounceMs]);
}

export function useRuntimeProfiles() {
  const [snapshot, setSnapshot] = useState<RuntimeProfilesSnapshot | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const refreshSequenceRef = useRef(0);

  const refreshRuntimeProfiles = useCallback(async () => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.get_runtime_profiles_snapshot) {
      return;
    }

    const currentSequence = ++refreshSequenceRef.current;
    setIsLoading(true);
    setError(null);

    try {
      const response = await electronAPI.get_runtime_profiles_snapshot();
      if (currentSequence !== refreshSequenceRef.current) {
        return;
      }

      if (response.success) {
        setSnapshot(response.snapshot);
      } else {
        setError(response.error ?? 'Failed to load runtime profiles');
      }
    } catch (caught) {
      if (currentSequence !== refreshSequenceRef.current) {
        return;
      }
      const message = caught instanceof Error ? caught.message : 'Failed to load runtime profiles';
      logger.error('Failed to refresh runtime profiles', { error: message });
      setError(message);
    } finally {
      if (currentSequence === refreshSequenceRef.current) {
        setIsLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    void refreshRuntimeProfiles();
  }, [refreshRuntimeProfiles]);

  useRuntimeProfileUpdateSubscription(
    useCallback(
      (notification) => {
        if (notification.snapshot_required || notification.events.length > 0) {
          void refreshRuntimeProfiles();
        }
      },
      [refreshRuntimeProfiles]
    )
  );

  return {
    snapshot,
    profiles: snapshot?.profiles ?? [],
    routes: snapshot?.routes ?? [],
    statuses: snapshot?.statuses ?? [],
    defaultProfileId: snapshot?.default_profile_id ?? null,
    cursor: snapshot?.cursor ?? null,
    isLoading,
    error,
    refreshRuntimeProfiles,
  };
}
