import { useCallback, useEffect, useRef, useState } from 'react';
import { useMotionValue } from 'framer-motion';
import {
  calculateDragFrame,
  computeAnchorIndex,
  ICON_SIZE,
  LIST_TOP_PADDING,
  reorderAppsAtIndices,
  type FloatingState,
  type PendingDrag,
  type PhysicsDragState,
  type PointerPhase,
  type UndoSnapshot,
  type UsePhysicsDragOptions,
} from './physicsDragUtils';
import { usePhysicsDragDelete } from './usePhysicsDragDelete';
import { usePhysicsDragPointerEvents } from './usePhysicsDragPointerEvents';
import { usePhysicsDragSettle } from './usePhysicsDragSettle';
import { usePhysicsDragUndo } from './usePhysicsDragUndo';

export {
  DELETE_DISTANCE,
  DRAG_START_DISTANCE,
  ICON_GAP,
  ICON_SIZE,
  LIST_TOP_PADDING,
  TOTAL_HEIGHT,
} from './physicsDragUtils';

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
      const prevIndex = activeAnchorRef.current;
      const nextIndex = computeAnchorIndex(localY, anchorStart, count, prevIndex);

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
      const frame = calculateDragFrame({
        anchorIndex,
        clientX,
        clientY,
        deleteIntensity: deleteShakeRef.current,
        grabOffset: grabOffsetRef.current,
        listRect,
        origin,
        proximity: snapProximityRef.current,
        smoothed: smoothedRef.current,
      });

      smoothedRef.current = { x: frame.nextX, y: frame.nextY };
      dragX.set(frame.nextX);
      dragY.set(frame.nextY);
      deleteShakeRef.current = frame.smoothedDeleteIntensity;
      setIsInSnapRange(frame.inSnapRange);
      snapProximityRef.current = frame.nextProximity;
      setSnapProximity(frame.nextProximity);
      setIsInDeleteZone(frame.inDeleteZone);
      setDeleteZoneShakeIntensity(frame.smoothedDeleteIntensity);
      setSettlingShakeIntensity(frame.settlingShakeIntensity);
      setResistanceShakeIntensity(frame.resistanceShakeIntensity);

      return { inDeleteZone: frame.inDeleteZone, anchorIndex: frame.anchorIndex };
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

  usePhysicsDragUndo({
    draggedId,
    floatingState,
    undoSnapshotRef,
    onReorderApps,
    onSelectApp,
  });

  const settleToAnchor = usePhysicsDragSettle({
    dragOriginRef,
    dragX,
    dragY,
    listRef,
    resetFloating,
    setFloatingId,
    setFloatingState,
    setPlaceholderIndex,
  });

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
        const reordered = reorderAppsAtIndices(apps, currentIndex, targetIndex);
        if (reordered) {
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

  const completeDelete = usePhysicsDragDelete({
    apps,
    appsRef,
    deleteFallbackRef,
    deletingIdRef,
    floatingId,
    floatingState,
    onDeleteApp,
    onSelectApp,
    resetFloating,
  });

  const onPointerDown = usePhysicsDragPointerEvents({
    beginDrag,
    draggedIdRef,
    endDrag,
    floatingState,
    lastPointerRef,
    pendingDragRef,
    pointerIdRef,
    pointerPhase,
    pointerTargetRef,
    setDragVelocity,
    setPointerPhase,
    updateDragPhysics,
  });

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
    onPointerDown,
    completeDelete,
  };
};
