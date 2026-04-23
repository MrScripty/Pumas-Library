import { useCallback, type RefObject } from 'react';
import { animate, type MotionValue } from 'framer-motion';
import {
  clamp,
  ICON_SIZE,
  LIST_TOP_PADDING,
  TOTAL_HEIGHT,
  type FloatingState,
} from './physicsDragUtils';

interface UsePhysicsDragSettleOptions {
  dragOriginRef: RefObject<{ x: number; y: number } | null>;
  dragX: MotionValue<number>;
  dragY: MotionValue<number>;
  listRef: RefObject<HTMLDivElement | null>;
  resetFloating: () => void;
  setFloatingId: (appId: string | null) => void;
  setFloatingState: (state: FloatingState) => void;
  setPlaceholderIndex: (index: number | null) => void;
}

export function usePhysicsDragSettle({
  dragOriginRef,
  dragX,
  dragY,
  listRef,
  resetFloating,
  setFloatingId,
  setFloatingState,
  setPlaceholderIndex,
}: UsePhysicsDragSettleOptions) {
  return useCallback(
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
    [
      dragOriginRef,
      dragX,
      dragY,
      listRef,
      resetFloating,
      setFloatingId,
      setFloatingState,
      setPlaceholderIndex,
    ]
  );
}
