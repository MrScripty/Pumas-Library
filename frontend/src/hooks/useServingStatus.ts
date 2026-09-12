import { useCallback, useEffect, useRef, useState } from 'react';
import { getElectronAPI } from '../api/adapter';
import type { ServingStatusSnapshot } from '../types/api-serving';
import { getLogger } from '../utils/logger';

const logger = getLogger('useServingStatus');
const SERVING_STATUS_SUBSCRIPTION_UNAVAILABLE =
  'Serving status push subscription unavailable';
const SERVING_STATUS_OBSERVATION_INVALID = 'Serving status response was malformed';

export type ServingControlStatus = Pick<
  ServingStatusSnapshot['served_models'][number],
  'model_id' | 'model_alias' | 'provider' | 'profile_id' | 'load_state'
>;

export type ServingControlObservation =
  | { kind: 'known'; rows: ServingControlStatus[] }
  | { kind: 'unavailable'; message: string };

const PROVIDERS = new Set(['ollama', 'llama_cpp', 'onnx_runtime']);
const LOAD_STATES = new Set(['requested', 'loading', 'loaded', 'unloading', 'unloaded', 'failed']);

function readControlObservation(value: unknown): ServingControlStatus[] | null {
  if (!value || typeof value !== 'object') return null;
  const snapshot = value as Record<string, unknown>;
  if (
    snapshot['schema_version'] !== 1 ||
    typeof snapshot['cursor'] !== 'string' ||
    !Array.isArray(snapshot['served_models'])
  ) {
    return null;
  }
  const rows: ServingControlStatus[] = [];
  for (const valueRow of snapshot['served_models']) {
    if (!valueRow || typeof valueRow !== 'object') return null;
    const row = valueRow as Record<string, unknown>;
    if (
      typeof row['model_id'] !== 'string' || !row['model_id'] ||
      typeof row['profile_id'] !== 'string' || !row['profile_id'] ||
      typeof row['provider'] !== 'string' || !PROVIDERS.has(row['provider']) ||
      typeof row['load_state'] !== 'string' || !LOAD_STATES.has(row['load_state']) ||
      !(
        row['model_alias'] === undefined ||
        row['model_alias'] === null ||
        typeof row['model_alias'] === 'string'
      )
    ) return null;
    rows.push(row as unknown as ServingControlStatus);
  }
  return rows;
}

export function useServingStatus() {
  const [snapshot, setSnapshot] = useState<ServingStatusSnapshot | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [subscriptionError, setSubscriptionError] = useState<string | null>(null);
  const [controlRows, setControlRows] = useState<ServingControlStatus[] | null>(null);
  const refreshSequenceRef = useRef(0);
  const cursorRef = useRef<string | null>(null);
  const subscriptionFailedRef = useRef(false);

  const refreshServingStatus = useCallback(async () => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.get_serving_status) {
      return;
    }

    const currentSequence = ++refreshSequenceRef.current;
    try {
      const response = await electronAPI.get_serving_status();
      if (currentSequence !== refreshSequenceRef.current) {
        return;
      }
      const responseWire = response as unknown as Record<string, unknown>;
      if (responseWire['success'] === true) {
        const admittedRows = readControlObservation(response.snapshot);
        if (!admittedRows) {
          setReadError(SERVING_STATUS_OBSERVATION_INVALID);
          setControlRows(null);
          return;
        }
        cursorRef.current = response.snapshot.cursor;
        setSnapshot(response.snapshot);
        setControlRows(admittedRows);
        setReadError(null);
      } else {
        setReadError(response.error ?? 'Failed to load serving status');
        setControlRows(null);
      }
    } catch (caught) {
      if (currentSequence !== refreshSequenceRef.current) {
        return;
      }
      const message = caught instanceof Error ? caught.message : 'Failed to load serving status';
      logger.error('Failed to refresh serving status', { error: message });
      setReadError(message);
      setControlRows(null);
    }
  }, []);

  useEffect(() => {
    void refreshServingStatus();
    return () => {
      refreshSequenceRef.current += 1;
    };
  }, [refreshServingStatus]);

  useEffect(() => {
    const electronAPI = getElectronAPI();
    if (!electronAPI?.onServingStatusUpdate) {
      setSubscriptionError(SERVING_STATUS_SUBSCRIPTION_UNAVAILABLE);
      return undefined;
    }

    let isSubscribed = true;
    const unsubscribe = electronAPI.onServingStatusUpdate(
      (feed) => {
        if (!isSubscribed) {
          return;
        }
        if (subscriptionFailedRef.current) {
          subscriptionFailedRef.current = false;
          setSubscriptionError(null);
          setControlRows(null);
          void refreshServingStatus();
          return;
        }
        setSubscriptionError(null);
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
        subscriptionFailedRef.current = true;
        refreshSequenceRef.current += 1;
        setControlRows(null);
        setSubscriptionError(message);
      }
    );

    return () => {
      isSubscribed = false;
      unsubscribe();
    };
  }, [refreshServingStatus]);

  const error = readError ?? subscriptionError;
  const controlObservation: ServingControlObservation =
    error || !controlRows
      ? { kind: 'unavailable', message: error ?? 'Serving status is loading' }
      : { kind: 'known', rows: controlRows };

  return {
    snapshot,
    servedModels: snapshot?.served_models ?? [],
    endpoint: snapshot?.endpoint ?? null,
    cursor: snapshot?.cursor ?? null,
    error,
    controlObservation,
    refreshServingStatus,
  };
}
