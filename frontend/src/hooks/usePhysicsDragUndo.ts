import { useEffect, type RefObject } from 'react';
import type { AppConfig } from '../types/apps';
import {
  isEditableElement,
  resolveSelection,
  type FloatingState,
  type UndoSnapshot,
} from './physicsDragUtils';

interface UsePhysicsDragUndoOptions {
  draggedId: string | null;
  floatingState: FloatingState;
  undoSnapshotRef: RefObject<UndoSnapshot | null>;
  onReorderApps?: ((reorderedApps: AppConfig[]) => void) | undefined;
  onSelectApp: (appId: string | null) => void;
}

export function usePhysicsDragUndo({
  draggedId,
  floatingState,
  undoSnapshotRef,
  onReorderApps,
  onSelectApp,
}: UsePhysicsDragUndoOptions) {
  useEffect(() => {
    const handleUndo = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z' && !event.shiftKey) {
        const activeElement = document.activeElement as HTMLElement | null;
        if (
          draggedId
          || floatingState
          || isEditableElement(activeElement)
          || !undoSnapshotRef.current
          || !onReorderApps
        ) {
          return;
        }

        event.preventDefault();

        const snapshot = undoSnapshotRef.current;
        onReorderApps(snapshot.apps);
        onSelectApp(resolveSelection(snapshot));
        undoSnapshotRef.current = null;
      }
    };

    window.addEventListener('keydown', handleUndo);
    return () => window.removeEventListener('keydown', handleUndo);
  }, [draggedId, floatingState, onReorderApps, onSelectApp, undoSnapshotRef]);
}
