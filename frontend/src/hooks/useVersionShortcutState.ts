import { useCallback, useEffect, useState } from 'react';
import { api, isAPIAvailable } from '../api/adapter';
import { APIError } from '../errors';
import { getLogger } from '../utils/logger';

const logger = getLogger('useVersionShortcutState');

export interface VersionShortcutState {
  menu: boolean;
  desktop: boolean;
}

interface UseVersionShortcutStateOptions {
  activeShortcutState?: VersionShortcutState;
  activeVersion: string | null;
  installedVersions: string[];
  supportsShortcuts: boolean;
}

export function useVersionShortcutState({
  activeShortcutState,
  activeVersion,
  installedVersions,
  supportsShortcuts,
}: UseVersionShortcutStateOptions) {
  const [shortcutState, setShortcutState] = useState<Record<string, VersionShortcutState>>({});

  const refreshShortcutStates = useCallback(async () => {
    if (!isAPIAvailable() || !supportsShortcuts) {
      return;
    }

    try {
      const result = await api.get_all_shortcut_states();
      const states = result?.states?.states;
      if (result.success && states) {
        const mapped: Record<string, VersionShortcutState> = {};
        Object.entries(states).forEach(([tag, state]) => {
          const typedState = state as VersionShortcutState & { tag?: string };
          mapped[tag] = {
            menu: Boolean(typedState.menu),
            desktop: Boolean(typedState.desktop),
          };
        });
        setShortcutState(mapped);
        logger.debug('Shortcut states refreshed', { stateCount: Object.keys(mapped).length });
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error fetching shortcut states', {
          error: error.message,
          endpoint: error.endpoint,
        });
      } else if (error instanceof Error) {
        logger.error('Failed to fetch shortcut states', { error: error.message });
      } else {
        logger.error('Unknown error fetching shortcut states', { error });
      }
    }
  }, [supportsShortcuts]);

  const toggleShortcuts = useCallback(async (version: string, next: boolean) => {
    if (!isAPIAvailable() || !supportsShortcuts) {
      logger.warn('Shortcut API not available');
      return;
    }

    logger.info('Toggling shortcuts', { version, enabled: next });
    setShortcutState((prev) => ({
      ...prev,
      [version]: { menu: next, desktop: next },
    }));

    try {
      const result = await api.set_version_shortcuts(version, next);
      if (result?.state) {
        setShortcutState((prev) => ({
          ...prev,
          [version]: {
            menu: Boolean(result.state.menu),
            desktop: Boolean(result.state.desktop),
          },
        }));
        logger.info('Shortcuts toggled successfully', { version, state: result.state });
      }
    } catch (error) {
      if (error instanceof APIError) {
        logger.error('API error toggling shortcuts', {
          error: error.message,
          endpoint: error.endpoint,
          version,
        });
      } else if (error instanceof Error) {
        logger.error('Failed to toggle shortcuts', { error: error.message, version });
      } else {
        logger.error('Unknown error toggling shortcuts', { error, version });
      }

      const isEnabled = !next;
      setShortcutState((prev) => ({
        ...prev,
        [version]: { menu: isEnabled, desktop: isEnabled },
      }));
    }
  }, [supportsShortcuts]);

  useEffect(() => {
    if (!supportsShortcuts || !installedVersions.length) {
      setShortcutState({});
      return;
    }
    void refreshShortcutStates();
  }, [installedVersions, refreshShortcutStates, supportsShortcuts]);

  useEffect(() => {
    if (!supportsShortcuts || !activeVersion || !activeShortcutState) {
      return;
    }

    setShortcutState((prev) => ({
      ...prev,
      [activeVersion]: {
        menu: activeShortcutState.menu,
        desktop: activeShortcutState.desktop,
      },
    }));
  }, [activeVersion, activeShortcutState?.desktop, activeShortcutState?.menu, supportsShortcuts]);

  return {
    shortcutState,
    toggleShortcuts,
  };
}
