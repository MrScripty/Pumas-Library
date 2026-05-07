import { useCallback, useSyncExternalStore } from 'react';
import { api, getElectronAPI, isAPIAvailable } from '../api/adapter';
import { APIError } from '../errors';
import type { StatusResponse, StatusTelemetrySnapshot } from '../types/api';
import { getLogger } from '../utils/logger';

const logger = getLogger('statusTelemetryStore');

type StatusTelemetryState = {
  snapshot: StatusTelemetrySnapshot | null;
  error: string | null;
};

type SubscriptionOptions = {
  loadInitial?: boolean;
};

const ENRICHED_STATUS_FIELDS: Array<keyof StatusResponse> = [
  'deps_ready',
  'patched',
  'menu_shortcut',
  'desktop_shortcut',
  'shortcut_version',
];

let state: StatusTelemetryState = {
  snapshot: null,
  error: null,
};
let started = false;
let hasLoadedSnapshot = false;
let loadingPromise: Promise<void> | null = null;
let pendingRefresh = false;
let waitTimeout: NodeJS.Timeout | null = null;
let unsubscribeTelemetry: (() => void) | null = null;

const listeners = new Set<() => void>();

function emitChange() {
  listeners.forEach((listener) => listener());
}

function setState(nextState: StatusTelemetryState) {
  state = nextState;
  emitChange();
}

function mergeSnapshot(
  nextSnapshot: StatusTelemetrySnapshot,
  options: { preserveEnrichedStatusFields?: boolean } = {},
): StatusTelemetrySnapshot {
  const previousStatus = state.snapshot?.status;
  if (!previousStatus || !options.preserveEnrichedStatusFields) {
    return nextSnapshot;
  }

  const nextStatus = { ...nextSnapshot.status } as Partial<StatusResponse>;
  ENRICHED_STATUS_FIELDS.forEach((field) => {
    nextStatus[field] = previousStatus[field] as never;
  });

  return {
    ...nextSnapshot,
    status: nextStatus as StatusResponse,
  };
}

function applySnapshot(
  snapshot: StatusTelemetrySnapshot,
  options: { preserveEnrichedStatusFields?: boolean } = {},
) {
  hasLoadedSnapshot = true;
  setState({
    snapshot: mergeSnapshot(snapshot, options),
    error: null,
  });
}

function recordError(error: unknown) {
  const message = error instanceof Error ? error.message : 'Unknown error';
  if (error instanceof APIError) {
    logger.error('API error fetching status telemetry', {
      error: error.message,
      endpoint: error.endpoint,
    });
  } else {
    logger.error('Error fetching status telemetry', { error: message });
  }
  setState({
    ...state,
    error: message,
  });
}

function clearApiWait() {
  if (waitTimeout) {
    clearTimeout(waitTimeout);
    waitTimeout = null;
  }
}

function startTelemetry(loadInitial: boolean) {
  if (!isAPIAvailable()) {
    waitTimeout = setTimeout(() => startTelemetry(loadInitial), 100);
    return;
  }

  clearApiWait();

  const electronAPI = getElectronAPI();
  if (!unsubscribeTelemetry && electronAPI?.onStatusTelemetryUpdate) {
    unsubscribeTelemetry = electronAPI.onStatusTelemetryUpdate((notification) => {
      applySnapshot(notification.snapshot, { preserveEnrichedStatusFields: true });
      if (notification.snapshot_required) {
        void fetchStatusTelemetrySnapshot({ queueIfInFlight: true });
      }
    });
  }

  if (loadInitial && !hasLoadedSnapshot) {
    void fetchStatusTelemetrySnapshot();
  }
}

function ensureStarted(loadInitial: boolean) {
  if (started) {
    if (loadInitial && !hasLoadedSnapshot) {
      void fetchStatusTelemetrySnapshot();
    }
    return;
  }

  started = true;
  startTelemetry(loadInitial);
}

function stopTelemetryIfIdle() {
  if (listeners.size > 0) {
    return;
  }

  started = false;
  hasLoadedSnapshot = false;
  state = {
    snapshot: null,
    error: null,
  };
  clearApiWait();
  unsubscribeTelemetry?.();
  unsubscribeTelemetry = null;
}

export async function fetchStatusTelemetrySnapshot(
  options: { queueIfInFlight?: boolean } = {},
): Promise<void> {
  if (loadingPromise) {
    if (options.queueIfInFlight) {
      pendingRefresh = true;
    }
    await loadingPromise;
    return;
  }

  if (!isAPIAvailable()) {
    setState({
      ...state,
      error: null,
    });
    return;
  }

  loadingPromise = api.get_status_telemetry_snapshot()
    .then(applySnapshot)
    .catch(recordError)
    .finally(() => {
      loadingPromise = null;
      if (pendingRefresh) {
        pendingRefresh = false;
        void fetchStatusTelemetrySnapshot({ queueIfInFlight: true });
      }
    });

  await loadingPromise;
}

export function subscribeStatusTelemetry(
  listener: () => void,
  options: SubscriptionOptions = {},
): () => void {
  listeners.add(listener);
  ensureStarted(options.loadInitial ?? true);

  return () => {
    listeners.delete(listener);
    stopTelemetryIfIdle();
  };
}

export function getStatusTelemetryState(): StatusTelemetryState {
  return state;
}

export function useStatusTelemetry(options: SubscriptionOptions = {}) {
  const loadInitial = options.loadInitial ?? true;
  const subscribe = useCallback(
    (listener: () => void) => subscribeStatusTelemetry(listener, { loadInitial }),
    [loadInitial],
  );
  const telemetryState = useSyncExternalStore(
    subscribe,
    getStatusTelemetryState,
    getStatusTelemetryState,
  );

  return {
    ...telemetryState,
    refetch: () => fetchStatusTelemetrySnapshot({ queueIfInFlight: true }),
  };
}
