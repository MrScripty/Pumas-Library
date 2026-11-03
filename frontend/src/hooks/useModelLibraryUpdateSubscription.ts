import { useEffect, useRef } from 'react';
import { getElectronAPI } from '../api/adapter';
import type { ModelLibraryUpdateNotification } from '../types/api-package-facts';
import { getLogger } from '../utils/logger';
import type { UpdateStreamDelivery } from './updateStreamDelivery';

const logger = getLogger('useModelLibraryUpdateSubscription');

export const MODEL_LIBRARY_UPDATE_REFRESH_DEBOUNCE_MS = 250;

export function useModelLibraryUpdateSubscription(
  onUpdate: (delivery: UpdateStreamDelivery<ModelLibraryUpdateNotification>) => void,
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

    let latestDelivery: UpdateStreamDelivery<ModelLibraryUpdateNotification> | null = null;
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;

    const clearRefreshTimer = () => {
      if (refreshTimer) {
        clearTimeout(refreshTimer);
        refreshTimer = null;
      }
    };

    const deliver = (delivery: UpdateStreamDelivery<ModelLibraryUpdateNotification>) => {
      // A pending resync is never replaced by a later notification.
      if (latestDelivery?.kind !== 'resync') latestDelivery = delivery;
      clearRefreshTimer();
      refreshTimer = setTimeout(() => {
        refreshTimer = null;
        const pending = latestDelivery;
        latestDelivery = null;
        if (pending) onUpdateRef.current(pending);
      }, debounceMs);
    };

    const unsubscribe = electronAPI.onModelLibraryUpdate(
      (notification) => deliver({ kind: 'notification', notification }),
      (reason) => {
        logger.warn('Model-library update not delivered; reloading', { reason });
        deliver({ kind: 'resync', reason });
      }
    );

    return () => {
      clearRefreshTimer();
      unsubscribe();
    };
  }, [debounceMs]);
}
