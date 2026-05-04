import { useEffect, useRef } from 'react';
import { getElectronAPI } from '../api/adapter';
import {
  isModelLibraryUpdateNotification,
  type ModelLibraryUpdateNotification,
} from '../types/api-package-facts';
import { getLogger } from '../utils/logger';

const logger = getLogger('useModelLibraryUpdateSubscription');

export const MODEL_LIBRARY_UPDATE_REFRESH_DEBOUNCE_MS = 250;

export function useModelLibraryUpdateSubscription(
  onUpdate: (notification: ModelLibraryUpdateNotification) => void,
  debounceMs = MODEL_LIBRARY_UPDATE_REFRESH_DEBOUNCE_MS
): void {
  const onUpdateRef = useRef(onUpdate);

  useEffect(() => {
    onUpdateRef.current = onUpdate;
  }, [onUpdate]);

  useEffect(() => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.onModelLibraryUpdate) {
      return undefined;
    }

    let latestNotification: ModelLibraryUpdateNotification | null = null;
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;

    const clearRefreshTimer = () => {
      if (refreshTimer) {
        clearTimeout(refreshTimer);
        refreshTimer = null;
      }
    };

    const unsubscribe = electronAPI.onModelLibraryUpdate((notification) => {
      if (!isModelLibraryUpdateNotification(notification)) {
        logger.warn('Ignoring invalid model-library update notification');
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
