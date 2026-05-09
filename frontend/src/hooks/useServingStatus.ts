import { useCallback, useEffect, useRef, useState } from 'react';
import { getElectronAPI } from '../api/adapter';
import type { ServingStatusSnapshot } from '../types/api-serving';
import { getLogger } from '../utils/logger';

const logger = getLogger('useServingStatus');
const SERVING_STATUS_UPDATE_INTERVAL_MS = 1500;

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
    if (!electronAPI?.list_serving_status_updates_since) {
      return undefined;
    }

    let isDisposed = false;
    let isChecking = false;
    const interval = setInterval(() => {
      if (isChecking) {
        return;
      }
      isChecking = true;
      void electronAPI
        .list_serving_status_updates_since(cursorRef.current)
        .then((response) => {
          if (isDisposed || !response.success) {
            return;
          }
          cursorRef.current = response.feed.cursor;
          if (response.feed.snapshot_required || response.feed.events.length > 0) {
            void refreshServingStatus();
          }
        })
        .catch((caught: unknown) => {
          const message =
            caught instanceof Error ? caught.message : 'Failed to check serving status updates';
          logger.warn('Failed to check serving status updates', { error: message });
        })
        .finally(() => {
          isChecking = false;
        });
    }, SERVING_STATUS_UPDATE_INTERVAL_MS);

    return () => {
      isDisposed = true;
      clearInterval(interval);
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
