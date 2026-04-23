import {
  useCallback,
  useEffect,
  type Dispatch,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
  type SetStateAction,
} from 'react';
import {
  DRAG_START_DISTANCE,
  releasePointerCaptureIfAvailable,
  type FloatingState,
  type PendingDrag,
  type PointerPhase,
} from './physicsDragUtils';

interface UsePhysicsDragPointerEventsOptions {
  beginDrag: (pending: PendingDrag, currentX: number, currentY: number) => void;
  draggedIdRef: RefObject<string | null>;
  endDrag: (appId: string, clientX: number, clientY: number) => void;
  floatingState: FloatingState;
  lastPointerRef: RefObject<{ x: number; y: number; time: number } | null>;
  pendingDragRef: RefObject<PendingDrag | null>;
  pointerIdRef: RefObject<number | null>;
  pointerPhase: PointerPhase;
  pointerTargetRef: RefObject<HTMLElement | null>;
  setDragVelocity: Dispatch<SetStateAction<number>>;
  setPointerPhase: Dispatch<SetStateAction<PointerPhase>>;
  updateDragPhysics: (clientX: number, clientY: number) => unknown;
}

function isNonMousePointerType(pointerType: string | undefined) {
  return pointerType !== undefined && pointerType !== '' && pointerType !== 'mouse';
}

export function usePhysicsDragPointerEvents({
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
}: UsePhysicsDragPointerEventsOptions) {
  useEffect(() => {
    if (pointerPhase === 'idle') return;

    const handlePointerMove = (event: PointerEvent) => {
      if (pointerIdRef.current !== null && event.pointerId !== pointerIdRef.current) return;
      if (isNonMousePointerType(event.pointerType)) return;

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
      if (isNonMousePointerType(event.pointerType)) return;

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
      const pointerTarget = pointerTargetRef.current;
      if (pointerTarget) {
        releasePointerCaptureIfAvailable(pointerTarget, event.pointerId);
      }
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
  }, [
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
  ]);

  return useCallback(
    (appId: string, event: ReactPointerEvent<HTMLElement>) => {
      if (floatingState !== null || draggedIdRef.current || pointerPhase !== 'idle') return;
      if (isNonMousePointerType(event.pointerType)) return;
      if ('button' in event && event.button !== 0) return;

      const element = event.currentTarget;
      const elementRect = element.getBoundingClientRect();

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
    [
      draggedIdRef,
      floatingState,
      pendingDragRef,
      pointerIdRef,
      pointerPhase,
      pointerTargetRef,
      setPointerPhase,
    ]
  );
}
