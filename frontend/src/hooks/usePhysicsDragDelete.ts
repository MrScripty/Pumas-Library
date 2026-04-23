import { useCallback, useEffect, type RefObject } from 'react';
import type { AppConfig } from '../types/apps';
import type { FloatingState } from './physicsDragUtils';

interface UsePhysicsDragDeleteOptions {
  apps: AppConfig[];
  appsRef: RefObject<AppConfig[]>;
  deleteFallbackRef: RefObject<number | null>;
  deletingIdRef: RefObject<string | null>;
  floatingId: string | null;
  floatingState: FloatingState;
  onDeleteApp?: ((appId: string) => void) | undefined;
  onSelectApp: (appId: string | null) => void;
  resetFloating: () => void;
}

export function usePhysicsDragDelete({
  apps,
  appsRef,
  deleteFallbackRef,
  deletingIdRef,
  floatingId,
  floatingState,
  onDeleteApp,
  onSelectApp,
  resetFloating,
}: UsePhysicsDragDeleteOptions) {
  const completeDelete = useCallback(() => {
    const deletingId = deletingIdRef.current;
    if (!deletingId) return;

    onDeleteApp?.(deletingId);
    onSelectApp(null);

    deletingIdRef.current = null;
    if (deleteFallbackRef.current) {
      window.clearTimeout(deleteFallbackRef.current);
    }
    deleteFallbackRef.current = window.setTimeout(() => {
      if (floatingState !== 'deleting' || !floatingId) return;
      const stillExists = appsRef.current.some(app => app.id === floatingId);
      if (stillExists) {
        resetFloating();
      }
    }, 350);
  }, [
    appsRef,
    deleteFallbackRef,
    deletingIdRef,
    floatingId,
    floatingState,
    onDeleteApp,
    onSelectApp,
    resetFloating,
  ]);

  useEffect(() => {
    if (floatingState !== 'deleting' || !floatingId) return;
    const stillExists = apps.some(app => app.id === floatingId);
    if (stillExists) return;

    if (deleteFallbackRef.current) {
      window.clearTimeout(deleteFallbackRef.current);
      deleteFallbackRef.current = null;
    }
    resetFloating();
  }, [apps, deleteFallbackRef, floatingId, floatingState, resetFloating]);

  return completeDelete;
}
