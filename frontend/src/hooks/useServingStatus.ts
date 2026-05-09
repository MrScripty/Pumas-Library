import { useCallback, useEffect, useRef, useState } from 'react';
import { getElectronAPI } from '../api/adapter';
import type { ServingStatusSnapshot } from '../types/api-serving';
import { getLogger } from '../utils/logger';

const logger = getLogger('useServingStatus');
const SERVING_STATUS_SUBSCRIPTION_UNAVAILABLE =
  'Serving status push subscription unavailable';

export function useServingStatus() {
  const [snapshot, setSnapshot] = useState<ServingStatusSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const refreshSequenceRef = useRef(0);
  const cursorRef = useRef<string | null>(null);

  const refreshServingStatus = useCallback(async () => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.get_serving_status) {
      return;
    }

    const currentSequence = ++refreshSequenceRef.current;
    setError(null);

    try {
      const response = await electronAPI.get_serving_status();
      if (currentSequence !== refreshSequenceRef.current) {
        return;
      }
      if (response.success) {
        cursorRef.current = response.snapshot.cursor;
        setSnapshot(response.snapshot);
      } else {
        setError(response.error ?? 'Failed to load serving status');
      }
    } catch (caught) {
      if (currentSequence !== refreshSequenceRef.current) {
        return;
      }
      const message = caught instanceof Error ? caught.message : 'Failed to load serving status';
      logger.error('Failed to refresh serving status', { error: message });
      setError(message);
    }
  }, []);

  useEffect(() => {
    void refreshServingStatus();
  }, [refreshServingStatus]);

  useEffect(() => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.onServingStatusUpdate) {
      setError(SERVING_STATUS_SUBSCRIPTION_UNAVAILABLE);
      return undefined;
    }

    let isSubscribed = true;
    const unsubscribe = electronAPI.onServingStatusUpdate(
      (feed) => {
        if (!isSubscribed) {
          return;
        }
        cursorRef.current = feed.cursor;
        if (feed.snapshot_required || feed.stale_cursor || feed.events.length > 0) {
          void refreshServingStatus();
        }
      },
      (message) => {
        if (!isSubscribed) {
          return;
        }
        logger.error('Serving status push subscription failed', { error: message });
        setError(message);
      }
    );

    return () => {
      isSubscribed = false;
      unsubscribe();
    };
  }, [refreshServingStatus]);

  return {
    snapshot,
    servedModels: snapshot?.served_models ?? [],
    endpoint: snapshot?.endpoint ?? null,
    cursor: snapshot?.cursor ?? null,
    error,
    refreshServingStatus,
  };
}
