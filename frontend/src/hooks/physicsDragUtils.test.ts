import { Box } from 'lucide-react';
import { describe, expect, it } from 'vitest';
import type { AppConfig } from '../types/apps';
import {
  DELETE_DISTANCE,
  ICON_SIZE,
  LIST_TOP_PADDING,
  TOTAL_HEIGHT,
  calculateDragFrame,
  clamp,
  computeAnchorIndex,
  isEditableElement,
  reorderAppsAtIndices,
  resolveSelection,
} from './physicsDragUtils';

const mockApps: AppConfig[] = [
  {
    id: 'comfyui',
    name: 'comfyui',
    displayName: 'ComfyUI',
    icon: Box,
    status: 'running',
    iconState: 'running',
  },
  {
    id: 'openwebui',
    name: 'openwebui',
    displayName: 'OpenWebUI',
    icon: Box,
    status: 'idle',
    iconState: 'offline',
  },
  {
    id: 'invoke',
    name: 'invoke',
    displayName: 'Invoke',
    icon: Box,
    status: 'idle',
    iconState: 'uninstalled',
  },
];

const createRect = (left: number, top: number, width: number, height: number): DOMRect => ({
  left,
  top,
  width,
  height,
  right: left + width,
  bottom: top + height,
  x: left,
  y: top,
  toJSON: () => ({}),
});

describe('physicsDragUtils', () => {
  it('clamps values and resolves selection from valid ids or safe fallback indices', () => {
    expect(clamp(12, 0, 10)).toBe(10);
    expect(clamp(-2, 0, 10)).toBe(0);

    expect(resolveSelection({
      apps: mockApps,
      selectedAppId: 'openwebui',
      selectedIndex: 0,
    })).toBe('openwebui');

    expect(resolveSelection({
      apps: mockApps,
      selectedAppId: 'missing',
      selectedIndex: 99,
    })).toBe('invoke');

    expect(resolveSelection({
      apps: [],
      selectedAppId: 'missing',
      selectedIndex: 0,
    })).toBeNull();
  });

  it('keeps the previous anchor inside the hysteresis band and moves once the pointer leaves it', () => {
    const anchorStart = LIST_TOP_PADDING + ICON_SIZE / 2;

    expect(computeAnchorIndex(
      anchorStart + TOTAL_HEIGHT * 1.6,
      anchorStart,
      4,
      1
    )).toBe(1);

    expect(computeAnchorIndex(
      anchorStart + TOTAL_HEIGHT * 1.8,
      anchorStart,
      4,
      1
    )).toBe(2);

    expect(computeAnchorIndex(anchorStart, anchorStart, 0, 2)).toBe(0);
  });

  it('reorders app arrays and reports invalid removals with null', () => {
    expect(reorderAppsAtIndices(mockApps, 0, 2)?.map((app) => app.id)).toEqual([
      'openwebui',
      'invoke',
      'comfyui',
    ]);

    expect(reorderAppsAtIndices(mockApps, 9, 1)).toBeNull();
  });

  it('detects editable targets for input, textarea, and contenteditable elements', () => {
    const input = document.createElement('input');
    const textarea = document.createElement('textarea');
    const contentEditable = document.createElement('div');
    Object.defineProperty(contentEditable, 'isContentEditable', { value: true });

    expect(isEditableElement(input)).toBe(true);
    expect(isEditableElement(textarea)).toBe(true);
    expect(isEditableElement(contentEditable)).toBe(true);
    expect(isEditableElement(document.createElement('button'))).toBe(false);
    expect(isEditableElement(null)).toBe(false);
  });

  it('calculates drag frames that snap near anchors and ease into delete-zone intensity', () => {
    const listRect = createRect(0, 0, 60, 300);
    const anchorX = listRect.left + listRect.width / 2;
    const anchorY = listRect.top + LIST_TOP_PADDING + ICON_SIZE / 2;

    const snapped = calculateDragFrame({
      anchorIndex: 0,
      clientX: anchorX,
      clientY: anchorY,
      deleteIntensity: 0,
      grabOffset: { x: ICON_SIZE / 2, y: ICON_SIZE / 2 },
      listRect,
      origin: { x: 0, y: 0 },
      proximity: 0,
      smoothed: { x: 0, y: 0 },
    });

    expect(snapped.inSnapRange).toBe(true);
    expect(snapped.inDeleteZone).toBe(false);
    expect(snapped.nextX).toBe(0);
    expect(snapped.nextY).toBeCloseTo(2.4, 5);
    expect(snapped.nextProximity).toBeCloseTo(0.25, 5);

    const deleting = calculateDragFrame({
      anchorIndex: 0,
      clientX: listRect.right + DELETE_DISTANCE + 30,
      clientY: anchorY,
      deleteIntensity: 0,
      grabOffset: { x: ICON_SIZE / 2, y: ICON_SIZE / 2 },
      listRect,
      origin: { x: 0, y: 0 },
      proximity: 0,
      smoothed: { x: 0, y: 0 },
    });

    expect(deleting.inSnapRange).toBe(false);
    expect(deleting.inDeleteZone).toBe(true);
    expect(deleting.smoothedDeleteIntensity).toBeGreaterThan(0);
    expect(deleting.nextX).toBeGreaterThan(0);
  });
});
