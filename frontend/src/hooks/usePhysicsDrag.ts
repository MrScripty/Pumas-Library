import { useCallback, useEffect, useRef, useState, type RefObject, type PointerEvent as ReactPointerEvent } from 'react';
import { animate, useMotionValue } from 'framer-motion';
import type { MotionValue } from 'framer-motion';
import type { AppConfig } from '../types/apps';

export const ICON_SIZE = 60;
export const ICON_GAP = 12;
export const TOTAL_HEIGHT = ICON_SIZE + ICON_GAP;
export const LIST_TOP_PADDING = 12;

export const SNAP_RANGE = 30;
export const CURSOR_INFLUENCE = 1.2;
export const DELETE_DISTANCE = 50;
export const ACCELERATION = 0.8;
export const HYSTERESIS_THRESHOLD = 0.2;
export const DRAG_START_DISTANCE = 6;

const MAX_SNAP_WEIGHT = 0.85;

type FloatingState = 'dragging' | 'settling' | 'deleting' | null;
type PointerPhase = 'idle' | 'pending' | 'dragging';

interface UndoSnapshot {
  apps: AppConfig[];
  selectedAppId: string | null;
  selectedIndex: number;
}

interface PendingDrag {
  appId: string;
  pointerId: number;
  startX: number;
  startY: number;
  element: HTMLElement;
  elementRect: DOMRect;
}

interface UsePhysicsDragOptions {
  apps: AppConfig[];
  selectedAppId: string | null;
  onSelectApp: (appId: string | null) => void;
  onReorderApps?: (reorderedApps: AppConfig[]) => void;
  onDeleteApp?: (appId: string) => void;
  listRef: RefObject<HTMLDivElement | null>;
}

interface PhysicsDragState {
  draggedId: string | null;
  floatingId: string | null;
  floatingState: FloatingState;
  activeAnchorIndex: number;
  placeholderIndex: number | null;
  isInSnapRange: boolean;
  isInDeleteZone: boolean;
  deleteZoneShakeIntensity: number;
  snapProximity: number;
  settlingShakeIntensity: number;
  resistanceShakeIntensity: number;
  dragVelocity: number;
  dragX: MotionValue<number>;
  dragY: MotionValue<number>;
  dragOrigin: { x: number; y: number } | null;
  onPointerDown: (appId: string, event: ReactPointerEvent<HTMLElement>) => void;
  completeDelete: () => void;
}

const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

const resolveSelection = (snapshot: UndoSnapshot) => {
  if (snapshot.selectedAppId === null) {
    return null;
  }

  if (snapshot.selectedAppId) {
    const exists = snapshot.apps.some(app => app.id === snapshot.selectedAppId);
    if (exists) {
      return snapshot.selectedAppId;
    }
  }

  if (snapshot.apps.length === 0) {
    return null;
  }

  const safeIndex = clamp(snapshot.selectedIndex, 0, snapshot.apps.length - 1);
  return snapshot.apps[safeIndex]?.id ?? snapshot.apps[0]?.id ?? null;
};

export const usePhysicsDrag = ({
  apps,
  selectedAppId,
  onSelectApp,
  onReorderApps,
  onDeleteApp,
  listRef,
}: UsePhysicsDragOptions): PhysicsDragState => {
  const [draggedId, setDraggedId] = useState<string | null>(null);
  const [floatingId, setFloatingId] = useState<string | null>(null);
  const [floatingState, setFloatingState] = useState<FloatingState>(null);
  const [activeAnchorIndex, setActiveAnchorIndex] = useState(0);
  const [placeholderIndex, setPlaceholderIndex] = useState<number | null>(null);
  const [isInSnapRange, setIsInSnapRange] = useState(false);
  const [isInDeleteZone, setIsInDeleteZone] = useState(false);
  const [deleteZoneShakeIntensity, setDeleteZoneShakeIntensity] = useState(0);
  const [snapProximity, setSnapProximity] = useState(0);
  const [settlingShakeIntensity, setSettlingShakeIntensity] = useState(0);
  const [resistanceShakeIntensity, setResistanceShakeIntensity] = useState(0);
  const [dragVelocity, setDragVelocity] = useState(0);
  const [dragOrigin, setDragOrigin] = useState<{ x: number; y: number } | null>(null);
  const [pointerPhase, setPointerPhase] = useState<PointerPhase>('idle');

  const dragX = useMotionValue(0);
  const dragY = useMotionValue(0);

  const activeAnchorRef = useRef(0);
  const draggedIdRef = useRef<string | null>(null);
  const dragOriginRef = useRef<{ x: number; y: number } | null>(null);
  const lastPointerRef = useRef<{ x: number; y: number; time: number } | null>(null);
  const smoothedRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  const pendingUndoRef = useRef<UndoSnapshot | null>(null);
  const undoSnapshotRef = useRef<UndoSnapshot | null>(null);
  const deletingIdRef = useRef<string | null>(null);
  const deleteFallbackRef = useRef<number | null>(null);
  const appsRef = useRef(apps);
  const pointerIdRef = useRef<number | null>(null);
  const pointerTargetRef = useRef<HTMLElement | null>(null);
  const grabOffsetRef = useRef<{ x: number; y: number }>({ x: ICON_SIZE / 2, y: ICON_SIZE / 2 });
  const deleteShakeRef = useRef(0);
  const snapProximityRef = useRef(0);
  const pendingDragRef = useRef<PendingDrag | null>(null);

  useEffect(() => {
    appsRef.current = apps;
  }, [apps]);

  useEffect(() => {
    return () => {
      if (deleteFallbackRef.current) {
        window.clearTimeout(deleteFallbackRef.current);
      }
    };
  }, []);

  const resetFloating = useCallback(() => {
    setFloatingId(null);
    setFloatingState(null);
    dragOriginRef.current = null;
    setDragOrigin(null);
    setPlaceholderIndex(null);
    dragX.set(0);
    dragY.set(0);
    setDragVelocity(0);
    deleteShakeRef.current = 0;
    setDeleteZoneShakeIntensity(0);
    snapProximityRef.current = 0;
    setSnapProximity(0);
  }, [dragX, dragY]);

  const updateActiveAnchor = useCallback(
    (localY: number, anchorStart = 0, count: number) => {
      if (count <= 0) return 0;
      const rawIndex = Math.round((localY - anchorStart) / TOTAL_HEIGHT);
      const clampedIndex = clamp(rawIndex, 0, count - 1);
      const threshold = HYSTERESIS_THRESHOLD * TOTAL_HEIGHT;

      const prevIndex = activeAnchorRef.current;
      const lower = anchorStart + (prevIndex - 0.5) * TOTAL_HEIGHT - threshold;
      const upper = anchorStart + (prevIndex + 0.5) * TOTAL_HEIGHT + threshold;

      const nextIndex =
        localY >= lower && localY <= upper ? prevIndex : clampedIndex;

      if (nextIndex !== activeAnchorRef.current) {
        activeAnchorRef.current = nextIndex;
        setActiveAnchorIndex(nextIndex);
      }

      return nextIndex;
    },
    [setActiveAnchorIndex]
  );

  const updateDragPhysics = useCallback(
    (clientX: number, clientY: number): { inDeleteZone: boolean; anchorIndex: number } | null => {
      const listElement = listRef.current;
      const origin = dragOriginRef.current;
      if (!listElement || !origin) return null;

      const listRect = listElement.getBoundingClientRect();
      const localY = clientY - listRect.top;

      const anchorStart = LIST_TOP_PADDING + ICON_SIZE / 2;
      const anchorIndex = updateActiveAnchor(localY, anchorStart, apps.length);
      const anchorX = listRect.left + (listRect.width / 2);
      const anchorY = listRect.top + anchorStart + anchorIndex * TOTAL_HEIGHT;

      const dx = clientX - anchorX;
      const dy = clientY - anchorY;
      const distance = Math.hypot(dx, dy);
      const inSnapRange = Math.abs(dx) <= SNAP_RANGE && Math.abs(dy) <= SNAP_RANGE;
      const proximity = inSnapRange ? 1 - Math.min(1, distance / SNAP_RANGE) : 0;
      const horizontalBias = Math.min(1, Math.abs(dx) / (SNAP_RANGE * CURSOR_INFLUENCE));
      const snapWeight = inSnapRange
        ? Math.min(MAX_SNAP_WEIGHT, proximity * (1 - horizontalBias))
        : 0;
      const cursorWeight = 1 - snapWeight;

      const snapTopLeftX = anchorX - ICON_SIZE / 2;
      const snapTopLeftY = anchorY - ICON_SIZE / 2;
      const cursorTopLeftX = clientX - grabOffsetRef.current.x;
      const cursorTopLeftY = clientY - grabOffsetRef.current.y;
      const targetTopLeftX = snapTopLeftX * snapWeight + cursorTopLeftX * cursorWeight;
      const targetTopLeftY = snapTopLeftY * snapWeight + cursorTopLeftY * cursorWeight;

      const targetX = targetTopLeftX - origin.x;
      const targetY = targetTopLeftY - origin.y;

      const smoothing = 1 - ACCELERATION;
      const nextX = smoothedRef.current.x + (targetX - smoothedRef.current.x) * smoothing;
      const nextY = smoothedRef.current.y + (targetY - smoothedRef.current.y) * smoothing;
      smoothedRef.current = { x: nextX, y: nextY };
      dragX.set(nextX);
      dragY.set(nextY);

      const outsideLeft = listRect.left - clientX;
      const outsideRight = clientX - listRect.right;
      const outsideDistance = Math.max(0, outsideLeft, outsideRight);
      const inDeleteZone = outsideDistance > DELETE_DISTANCE;
      const rawDeleteIntensity = inDeleteZone
        ? Math.min(1, (outsideDistance - DELETE_DISTANCE) / (DELETE_DISTANCE * 1.2))
        : 0;
      const easedDeleteIntensity = rawDeleteIntensity * rawDeleteIntensity * (3 - 2 * rawDeleteIntensity);
      const smoothedDeleteIntensity =
        deleteShakeRef.current + (easedDeleteIntensity - deleteShakeRef.current) * 0.18;
      deleteShakeRef.current = smoothedDeleteIntensity;

      setIsInSnapRange(inSnapRange);
      const proximitySmoothing = 0.25;
      const nextProximity =
        snapProximityRef.current + (proximity - snapProximityRef.current) * proximitySmoothing;
      snapProximityRef.current = nextProximity;
      setSnapProximity(nextProximity);
      setIsInDeleteZone(inDeleteZone);
      setDeleteZoneShakeIntensity(smoothedDeleteIntensity);
      setSettlingShakeIntensity(inSnapRange ? Math.sin(proximity * Math.PI) : 0);
      setResistanceShakeIntensity(inSnapRange ? Math.max(0, 1 - proximity) : 0);

      return { inDeleteZone, anchorIndex };
    },
    [apps.length, dragX, dragY, listRef, updateActiveAnchor],
  );

  const registerUndoSnapshot = useCallback(() => {
    const selectedIndex = apps.findIndex(app => app.id === selectedAppId);
    pendingUndoRef.current = {
      apps,
      selectedAppId,
      selectedIndex: selectedIndex === -1 ? 0 : selectedIndex,
    };
  }, [apps, selectedAppId]);

  const beginDrag = useCallback(
    (pending: PendingDrag, currentX: number, currentY: number) => {
      registerUndoSnapshot();
      deleteShakeRef.current = 0;
      setDeleteZoneShakeIntensity(0);
      snapProximityRef.current = 0;
      setSnapProximity(0);

      const rawIndex = apps.findIndex(app => app.id === pending.appId);
      const currentIndex = rawIndex === -1 ? 0 : rawIndex;

      setDraggedId(pending.appId);
      draggedIdRef.current = pending.appId;
      setFloatingId(pending.appId);
      setFloatingState('dragging');
      setDragVelocity(0);
      setPlaceholderIndex(currentIndex);
      activeAnchorRef.current = currentIndex;
      setActiveAnchorIndex(activeAnchorRef.current);

      pointerIdRef.current = pending.pointerId;
      pointerTargetRef.current = pending.element;
      pending.element.setPointerCapture?.(pending.pointerId);

      const origin = {
        x: pending.elementRect.left,
        y: pending.elementRect.top,
      };
      dragOriginRef.current = origin;
      grabOffsetRef.current = {
        x: pending.startX - pending.elementRect.left,
        y: pending.startY - pending.elementRect.top,
      };
      setDragOrigin(origin);
      smoothedRef.current = { x: 0, y: 0 };
      dragX.set(0);
      dragY.set(0);
      lastPointerRef.current = { x: currentX, y: currentY, time: performance.now() };
      updateDragPhysics(currentX, currentY);
    },
    [apps, dragX, dragY, registerUndoSnapshot, updateDragPhysics],
  );

  const commitUndoSnapshot = useCallback(() => {
    if (!pendingUndoRef.current) return;
    undoSnapshotRef.current = pendingUndoRef.current;
    pendingUndoRef.current = null;
  }, []);

  useEffect(() => {
    const handleUndo = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z' && !event.shiftKey) {
        const activeElement = document.activeElement as HTMLElement | null;
        const isEditable =
          activeElement?.tagName === 'INPUT' ||
          activeElement?.tagName === 'TEXTAREA' ||
          activeElement?.isContentEditable;

        if (draggedId || floatingState || isEditable || !undoSnapshotRef.current || !onReorderApps) {
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
  }, [draggedId, floatingState, onReorderApps, onSelectApp]);

  const settleToAnchor = useCallback(
    (appId: string, anchorIndex: number) => {
      const listElement = listRef.current;
      const origin = dragOriginRef.current;
      if (!listElement || !origin) return;

      const listRect = listElement.getBoundingClientRect();
      const anchorStart = LIST_TOP_PADDING + ICON_SIZE / 2;
      const anchorX = listRect.left + (listRect.width / 2);
      const anchorY = listRect.top + anchorStart + anchorIndex * TOTAL_HEIGHT;

      const targetTopLeftX = anchorX - ICON_SIZE / 2;
      const targetTopLeftY = anchorY - ICON_SIZE / 2;
      const targetX = targetTopLeftX - origin.x;
      const targetY = targetTopLeftY - origin.y;

      const distance = Math.hypot(targetX - dragX.get(), targetY - dragY.get());
      const stiffness = clamp(600 - distance * 1.5, 220, 700);
      const damping = clamp(32 + distance * 0.08, 24, 50);

      setFloatingId(appId);
      setFloatingState('settling');
      setPlaceholderIndex(anchorIndex);

      const xAnim = animate(dragX, targetX, { type: 'spring', stiffness, damping });
      const yAnim = animate(dragY, targetY, { type: 'spring', stiffness, damping });

      void Promise.all([xAnim.finished, yAnim.finished]).then(() => {
        resetFloating();
      });
    },
    [dragX, dragY, listRef, resetFloating],
  );

  const endDrag = useCallback(
    (appId: string, clientX: number, clientY: number) => {
      if (!dragOriginRef.current) {
        setDraggedId(null);
        resetFloating();
        return;
      }

      const metrics = updateDragPhysics(clientX, clientY);

      const currentIndex = apps.findIndex(app => app.id === appId);
      const targetIndex = metrics?.anchorIndex ?? activeAnchorRef.current;
      const inDeleteZone = metrics?.inDeleteZone ?? isInDeleteZone;

      setDraggedId(null);
      setDragVelocity(0);

      if (inDeleteZone) {
        commitUndoSnapshot();
        deletingIdRef.current = appId;
        setFloatingId(appId);
        setFloatingState('deleting');
        setPlaceholderIndex(null);
        return;
      }

      if (currentIndex !== -1 && targetIndex !== currentIndex && onReorderApps) {
        const reordered = [...apps];
        const [removed] = reordered.splice(currentIndex, 1);
        if (removed) {
          reordered.splice(targetIndex, 0, removed);
          commitUndoSnapshot();
          onReorderApps(reordered);
        }
      } else {
        pendingUndoRef.current = null;
      }

      settleToAnchor(appId, targetIndex);
    },
    [
      apps,
      commitUndoSnapshot,
      isInDeleteZone,
      onReorderApps,
      resetFloating,
      settleToAnchor,
      updateDragPhysics,
    ],
  );

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
  }, [floatingId, floatingState, onDeleteApp, onSelectApp, resetFloating]);

  useEffect(() => {
    if (floatingState !== 'deleting' || !floatingId) return;
    const stillExists = apps.some(app => app.id === floatingId);
    if (stillExists) return;

    if (deleteFallbackRef.current) {
      window.clearTimeout(deleteFallbackRef.current);
      deleteFallbackRef.current = null;
    }
    resetFloating();
  }, [apps, floatingId, floatingState, resetFloating]);

  useEffect(() => {
    if (pointerPhase === 'idle') return;

    const handlePointerMove = (event: PointerEvent) => {
      if (pointerIdRef.current !== null && event.pointerId !== pointerIdRef.current) return;
      if (event.pointerType && event.pointerType !== 'mouse') return;

      if (pointerPhase === 'pending' && pendingDragRef.current) {
        const dx = event.clientX - pendingDragRef.current.startX;
        const dy = event.clientY - pendingDragRef.current.startY;
        if (Math.hypot(dx, dy) >= DRAG_START_DISTANCE) {
          const pending = pendingDragRef.current;
          pendingDragRef.current = null;
          beginDrag(pending, event.clientX, event.clientY);
          setPointerPhase('dragging');
        }
        return;
      }

      if (floatingState !== 'dragging') return;

      event.preventDefault();
      updateDragPhysics(event.clientX, event.clientY);

      const now = performance.now();
      if (lastPointerRef.current) {
        const { x, y, time } = lastPointerRef.current;
        const distance = Math.hypot(event.clientX - x, event.clientY - y);
        const delta = Math.max(1, now - time);
        setDragVelocity((distance / delta) * 1000);
      }

      lastPointerRef.current = { x: event.clientX, y: event.clientY, time: now };
    };

    const handlePointerEnd = (event: PointerEvent) => {
      if (pointerIdRef.current !== null && event.pointerId !== pointerIdRef.current) return;
      if (event.pointerType && event.pointerType !== 'mouse') return;

      if (pointerPhase === 'pending') {
        pendingDragRef.current = null;
        pointerIdRef.current = null;
        pointerTargetRef.current = null;
        setPointerPhase('idle');
        return;
      }

      event.preventDefault();
      const activeId = draggedIdRef.current;
      if (!activeId) return;

      pointerIdRef.current = null;
      pointerTargetRef.current?.releasePointerCapture?.(event.pointerId);
      pointerTargetRef.current = null;

      endDrag(activeId, event.clientX, event.clientY);
      draggedIdRef.current = null;
      setPointerPhase('idle');
    };

    const handleWindowBlur = () => {
      if (pointerPhase === 'pending') {
        pendingDragRef.current = null;
        pointerIdRef.current = null;
        pointerTargetRef.current = null;
        setPointerPhase('idle');
        return;
      }
      const activeId = draggedIdRef.current;
      if (!activeId || !lastPointerRef.current) return;
      endDrag(activeId, lastPointerRef.current.x, lastPointerRef.current.y);
      draggedIdRef.current = null;
      pointerIdRef.current = null;
      pointerTargetRef.current = null;
      setPointerPhase('idle');
    };

    window.addEventListener('pointermove', handlePointerMove, { passive: false });
    window.addEventListener('pointerup', handlePointerEnd, { passive: false });
    window.addEventListener('pointercancel', handlePointerEnd, { passive: false });
    window.addEventListener('blur', handleWindowBlur);

    return () => {
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', handlePointerEnd);
      window.removeEventListener('pointercancel', handlePointerEnd);
      window.removeEventListener('blur', handleWindowBlur);
    };
  }, [beginDrag, endDrag, floatingState, pointerPhase, updateDragPhysics]);

  useEffect(() => {
    if (floatingState === 'dragging') {
      setPlaceholderIndex(activeAnchorIndex);
    }
  }, [activeAnchorIndex, floatingState]);

  return {
    draggedId,
    floatingId,
    floatingState,
    activeAnchorIndex,
    placeholderIndex,
    isInSnapRange,
    isInDeleteZone,
    deleteZoneShakeIntensity,
    snapProximity,
    settlingShakeIntensity,
    resistanceShakeIntensity,
    dragVelocity,
    dragX,
    dragY,
    dragOrigin,
    onPointerDown: (appId: string, event: ReactPointerEvent<HTMLElement>) => {
      if (floatingState !== null || draggedIdRef.current || pointerPhase !== 'idle') return;
      if (event.pointerType && event.pointerType !== 'mouse') return;
      if ('button' in event && event.button !== 0) return;

      const element = event.currentTarget as HTMLElement | null;
      const elementRect = element?.getBoundingClientRect();

      if (!elementRect || !element) return;

      pendingDragRef.current = {
        appId,
        pointerId: event.pointerId,
        startX: event.clientX,
        startY: event.clientY,
        element,
        elementRect,
      };
      pointerIdRef.current = event.pointerId;
      pointerTargetRef.current = element;
      setPointerPhase('pending');
    },
    completeDelete,
  };
};
