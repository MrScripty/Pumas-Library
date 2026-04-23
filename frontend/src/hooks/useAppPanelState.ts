import { useCallback, useEffect, useState } from 'react';

export interface AppPanelState {
  showVersionManager: boolean;
}

const DEFAULT_PANEL_STATE: AppPanelState = {
  showVersionManager: false,
};

const ensureStateForApps = (
  appIds: string[],
  current: Record<string, AppPanelState>
) => {
  const missingAppIds = appIds.filter((appId) => current[appId] === undefined);
  if (missingAppIds.length === 0) {
    return current;
  }

  const next = { ...current };
  missingAppIds.forEach((appId) => {
    next[appId] = { ...DEFAULT_PANEL_STATE };
  });
  return next;
};

export function useAppPanelState(appIds: string[]) {
  const [stateById, setStateById] = useState<Record<string, AppPanelState>>(
    () =>
      appIds.reduce<Record<string, AppPanelState>>((acc, appId) => {
        acc[appId] = { ...DEFAULT_PANEL_STATE };
        return acc;
      }, {})
  );

  useEffect(() => {
    setStateById((current) => ensureStateForApps(appIds, current));
  }, [appIds]);

  const getPanelState = useCallback(
    (appId: string | null) => {
      if (!appId) {
        return DEFAULT_PANEL_STATE;
      }
      return stateById[appId] ?? DEFAULT_PANEL_STATE;
    },
    [stateById]
  );

  const setShowVersionManager = useCallback((appId: string, show: boolean) => {
    setStateById((current) => ({
      ...current,
      [appId]: {
        ...(current[appId] ?? DEFAULT_PANEL_STATE),
        showVersionManager: show,
      },
    }));
  }, []);

  return {
    getPanelState,
    setShowVersionManager,
  };
}
