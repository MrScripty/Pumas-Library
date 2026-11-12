import { useCallback, useLayoutEffect, useRef } from 'react';

/**
 * Reports whether a reply requested for `requestedAppId` still belongs to the
 * app this hook instance serves. One instance follows the selected app, so a
 * reply that arrives after the selection changed must be dropped instead of
 * replacing the new app's state.
 */
export function useCurrentAppGuard(appId: string): (requestedAppId: string) => boolean {
  const current = useRef(appId);
  useLayoutEffect(() => {
    current.current = appId;
  }, [appId]);
  return useCallback((requestedAppId: string) => current.current === requestedAppId, []);
}
