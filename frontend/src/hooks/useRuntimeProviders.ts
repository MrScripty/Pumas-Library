import { useEffect, useSyncExternalStore } from 'react';
import { getElectronAPI } from '../api/adapter';
import type {
  RuntimeManagementMode,
  RuntimeProviderId,
  RuntimeProviderInfo,
} from '../types/api-runtime-profiles';
import { getLogger } from '../utils/logger';

const logger = getLogger('useRuntimeProviders');

/**
 * The backend's runtime provider registry (`get_runtime_providers`): which
 * providers exist, the runtime app each launches, whether that app is version
 * managed, and the modes their profiles may use. Fetched once per renderer
 * and shared by every consumer; `refresh` re-asks on demand.
 */
export interface RuntimeProvidersState {
  /** Null until the backend has answered once. */
  providers: readonly RuntimeProviderInfo[] | null;
  /** True while a request is in flight. */
  isLoading: boolean;
  error: string | null;
}

export interface UseRuntimeProvidersResult extends RuntimeProvidersState {
  refresh: () => Promise<void>;
}

const INITIAL_STATE: RuntimeProvidersState = { providers: null, isLoading: false, error: null };

let currentState: RuntimeProvidersState = INITIAL_STATE;
let inFlight: Promise<void> | null = null;
const listeners = new Set<() => void>();

function setState(next: RuntimeProvidersState) {
  currentState = next;
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot() {
  return currentState;
}

export function refreshRuntimeProviders(): Promise<void> {
  if (inFlight) {
    return inFlight;
  }
  const electronAPI = getElectronAPI();
  if (!electronAPI?.get_runtime_providers) {
    setState({ ...currentState, isLoading: false, error: 'Runtime provider API unavailable' });
    return Promise.resolve();
  }

  setState({ ...currentState, isLoading: true, error: null });
  inFlight = (async () => {
    try {
      const response = await electronAPI.get_runtime_providers();
      setState({ providers: response.providers, isLoading: false, error: null });
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : String(caught);
      logger.error('Failed to load runtime providers', { error: message });
      setState({ ...currentState, isLoading: false, error: message });
    } finally {
      inFlight = null;
    }
  })();
  return inFlight;
}

/** Test-only: forget the shared registry so each test starts unloaded. */
export function resetRuntimeProvidersForTests() {
  inFlight = null;
  currentState = INITIAL_STATE;
  listeners.clear();
}

export function useRuntimeProviders(): UseRuntimeProvidersResult {
  const state = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  useEffect(() => {
    if (getSnapshot().providers === null && !inFlight) {
      void refreshRuntimeProviders();
    }
  }, []);

  return { ...state, refresh: refreshRuntimeProviders };
}

/** App ids the backend version-manages, in registry order, without repeats. */
export function versionManagedAppIds(
  providers: readonly RuntimeProviderInfo[] | null
): string[] {
  const appIds: string[] = [];
  for (const provider of providers ?? []) {
    if (provider.version_managed && !appIds.includes(provider.app_id)) {
      appIds.push(provider.app_id);
    }
  }
  return appIds;
}

export function findRuntimeProvider(
  providers: readonly RuntimeProviderInfo[] | null,
  provider: RuntimeProviderId
): RuntimeProviderInfo | null {
  return providers?.find((entry) => entry.provider === provider) ?? null;
}

export function runtimeProviderManagementModes(
  provider: RuntimeProviderInfo
): RuntimeManagementMode[] {
  const modes: RuntimeManagementMode[] = [];
  if (provider.supports_managed_profiles) modes.push('managed');
  if (provider.supports_external_profiles) modes.push('external');
  return modes;
}
