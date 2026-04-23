import { useEffect } from 'react';
import type { UndoSnapshot, UsePhysicsDragOptions } from './physicsDragUtils';

export function useDeleteFallbackCleanup(deleteFallbackRef: { current: number | null }) {
  useEffect(() => {
    return () => {
      if (deleteFallbackRef.current) {
        window.clearTimeout(deleteFallbackRef.current);
      }
    };
  }, [deleteFallbackRef]);
}

export function writePendingUndoSnapshot(
  pendingUndoRef: { current: UndoSnapshot | null },
  apps: UsePhysicsDragOptions['apps'],
  selectedAppId: UsePhysicsDragOptions['selectedAppId']
) {
  const selectedIndex = apps.findIndex(app => app.id === selectedAppId);
  pendingUndoRef.current = {
    apps,
    selectedAppId,
    selectedIndex: selectedIndex === -1 ? 0 : selectedIndex,
  };
}
