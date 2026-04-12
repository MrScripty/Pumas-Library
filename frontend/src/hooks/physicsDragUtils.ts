import type { MotionValue } from 'framer-motion';
import type { RefObject, PointerEvent as ReactPointerEvent } from 'react';
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

export type FloatingState = 'dragging' | 'settling' | 'deleting' | null;
export type PointerPhase = 'idle' | 'pending' | 'dragging';

export interface UndoSnapshot {
  apps: AppConfig[];
  selectedAppId: string | null;
  selectedIndex: number;
}

export interface PendingDrag {
  appId: string;
  pointerId: number;
  startX: number;
  startY: number;
  element: HTMLElement;
  elementRect: DOMRect;
}

export interface UsePhysicsDragOptions {
  apps: AppConfig[];
  selectedAppId: string | null;
  onSelectApp: (appId: string | null) => void;
  onReorderApps?: (reorderedApps: AppConfig[]) => void;
  onDeleteApp?: (appId: string) => void;
  listRef: RefObject<HTMLDivElement | null>;
}

export interface PhysicsDragState {
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

export const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

export function resolveSelection(snapshot: UndoSnapshot) {
  if (snapshot.selectedAppId === null) {
    return null;
  }

  if (snapshot.selectedAppId) {
    const exists = snapshot.apps.some((app) => app.id === snapshot.selectedAppId);
    if (exists) {
      return snapshot.selectedAppId;
    }
  }

  if (snapshot.apps.length === 0) {
    return null;
  }

  const safeIndex = clamp(snapshot.selectedIndex, 0, snapshot.apps.length - 1);
  return snapshot.apps[safeIndex]?.id ?? snapshot.apps[0]?.id ?? null;
}

export function computeAnchorIndex(
  localY: number,
  anchorStart: number,
  count: number,
  previousIndex: number
) {
  if (count <= 0) {
    return 0;
  }

  const rawIndex = Math.round((localY - anchorStart) / TOTAL_HEIGHT);
  const clampedIndex = clamp(rawIndex, 0, count - 1);
  const threshold = HYSTERESIS_THRESHOLD * TOTAL_HEIGHT;
  const lower = anchorStart + (previousIndex - 0.5) * TOTAL_HEIGHT - threshold;
  const upper = anchorStart + (previousIndex + 0.5) * TOTAL_HEIGHT + threshold;

  return localY >= lower && localY <= upper ? previousIndex : clampedIndex;
}

export function reorderAppsAtIndices(
  apps: AppConfig[],
  currentIndex: number,
  targetIndex: number
) {
  const reordered = [...apps];
  const [removed] = reordered.splice(currentIndex, 1);
  if (!removed) {
    return null;
  }

  reordered.splice(targetIndex, 0, removed);
  return reordered;
}

export function isEditableElement(activeElement: HTMLElement | null) {
  return Boolean(
    activeElement?.tagName === 'INPUT'
    || activeElement?.tagName === 'TEXTAREA'
    || activeElement?.isContentEditable
  );
}

export function calculateDragFrame({
  anchorIndex,
  clientX,
  clientY,
  deleteIntensity,
  grabOffset,
  listRect,
  origin,
  proximity,
  smoothed,
}: {
  anchorIndex: number;
  clientX: number;
  clientY: number;
  deleteIntensity: number;
  grabOffset: { x: number; y: number };
  listRect: DOMRect;
  origin: { x: number; y: number };
  proximity: number;
  smoothed: { x: number; y: number };
}) {
  const anchorStart = LIST_TOP_PADDING + ICON_SIZE / 2;
  const anchorX = listRect.left + (listRect.width / 2);
  const anchorY = listRect.top + anchorStart + anchorIndex * TOTAL_HEIGHT;

  const dx = clientX - anchorX;
  const dy = clientY - anchorY;
  const distance = Math.hypot(dx, dy);
  const inSnapRange = Math.abs(dx) <= SNAP_RANGE && Math.abs(dy) <= SNAP_RANGE;
  const nextRawProximity = inSnapRange ? 1 - Math.min(1, distance / SNAP_RANGE) : 0;
  const horizontalBias = Math.min(1, Math.abs(dx) / (SNAP_RANGE * CURSOR_INFLUENCE));
  const snapWeight = inSnapRange
    ? Math.min(0.85, nextRawProximity * (1 - horizontalBias))
    : 0;
  const cursorWeight = 1 - snapWeight;

  const snapTopLeftX = anchorX - ICON_SIZE / 2;
  const snapTopLeftY = anchorY - ICON_SIZE / 2;
  const cursorTopLeftX = clientX - grabOffset.x;
  const cursorTopLeftY = clientY - grabOffset.y;
  const targetTopLeftX = snapTopLeftX * snapWeight + cursorTopLeftX * cursorWeight;
  const targetTopLeftY = snapTopLeftY * snapWeight + cursorTopLeftY * cursorWeight;

  const targetX = targetTopLeftX - origin.x;
  const targetY = targetTopLeftY - origin.y;
  const smoothing = 1 - ACCELERATION;
  const nextX = smoothed.x + (targetX - smoothed.x) * smoothing;
  const nextY = smoothed.y + (targetY - smoothed.y) * smoothing;

  const outsideLeft = listRect.left - clientX;
  const outsideRight = clientX - listRect.right;
  const outsideDistance = Math.max(0, outsideLeft, outsideRight);
  const inDeleteZone = outsideDistance > DELETE_DISTANCE;
  const rawDeleteIntensity = inDeleteZone
    ? Math.min(1, (outsideDistance - DELETE_DISTANCE) / (DELETE_DISTANCE * 1.2))
    : 0;
  const easedDeleteIntensity =
    rawDeleteIntensity * rawDeleteIntensity * (3 - 2 * rawDeleteIntensity);
  const smoothedDeleteIntensity =
    deleteIntensity + (easedDeleteIntensity - deleteIntensity) * 0.18;

  const proximitySmoothing = 0.25;
  const nextProximity = proximity + (nextRawProximity - proximity) * proximitySmoothing;

  return {
    anchorIndex,
    inDeleteZone,
    inSnapRange,
    nextProximity,
    nextX,
    nextY,
    resistanceShakeIntensity: inSnapRange ? Math.max(0, 1 - nextRawProximity) : 0,
    settlingShakeIntensity: inSnapRange ? Math.sin(nextRawProximity * Math.PI) : 0,
    smoothedDeleteIntensity,
  };
}
