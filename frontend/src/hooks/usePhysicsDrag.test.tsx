import { describe, it, expect, vi } from 'vitest';
import { act, render, waitFor, screen } from '@testing-library/react';
import { useRef, useEffect, type PointerEvent as ReactPointerEvent } from 'react';
import type { AppConfig } from '../types/apps';
import { Box } from 'lucide-react';
import {
  usePhysicsDrag,
  LIST_TOP_PADDING,
  TOTAL_HEIGHT,
  DELETE_DISTANCE,
  DRAG_START_DISTANCE,
} from './usePhysicsDrag';

const mockApps: AppConfig[] = [
  {
    id: 'comfyui',
    name: 'comfyui',
    displayName: 'ComfyUI',
    icon: Box,
    status: 'running',
    iconState: 'running',
    ramUsage: 60,
    gpuUsage: 40,
  },
  {
    id: 'openwebui',
    name: 'openwebui',
    displayName: 'OpenWebUI',
    icon: Box,
    status: 'idle',
    iconState: 'offline',
    ramUsage: 0,
    gpuUsage: 0,
  },
  {
    id: 'invoke',
    name: 'invoke',
    displayName: 'Invoke',
    icon: Box,
    status: 'idle',
    iconState: 'uninstalled',
    ramUsage: 0,
    gpuUsage: 0,
  },
];

const createMockRect = (left: number, top: number, width: number, height: number): DOMRect => ({
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

const createPointerEvent = (type: string, options: { clientX: number; clientY: number; pointerId?: number }) => {
  const event = new Event(type, { bubbles: true, cancelable: true }) as PointerEvent;
  Object.assign(event, {
    pointerId: options.pointerId ?? 1,
    pointerType: 'mouse',
    clientX: options.clientX,
    clientY: options.clientY,
  });
  return event;
};

const createPointerDownEvent = (currentTarget: HTMLElement, clientX: number, clientY: number) =>
  ({
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
    currentTarget,
    pointerId: 1,
    pointerType: 'mouse',
    clientX,
    clientY,
  }) as unknown as ReactPointerEvent<HTMLElement>;

interface HarnessProps {
  apps: AppConfig[];
  onSelectApp: (appId: string | null) => void;
  onReorderApps?: (apps: AppConfig[]) => void;
  onDeleteApp?: (appId: string) => void;
  onReady: (api: ReturnType<typeof usePhysicsDrag>) => void;
}

const Harness = ({ apps, onSelectApp, onReorderApps, onDeleteApp, onReady }: HarnessProps) => {
  const listRef = useRef<HTMLDivElement>(null);
  const api = usePhysicsDrag({
    apps,
    selectedAppId: null,
    onSelectApp,
    onReorderApps,
    onDeleteApp,
    listRef,
  });

  useEffect(() => {
    onReady(api);
  }, [api, onReady]);

  return (
    <div ref={listRef} data-testid="list">
      {apps.map((app) => (
        <div key={app.id} data-testid={`item-${app.id}`} />
      ))}
    </div>
  );
};

describe('usePhysicsDrag', () => {
  it('reorders apps on drag end', async () => {
    const onReorderApps = vi.fn();
    const onSelectApp = vi.fn();
    let api: ReturnType<typeof usePhysicsDrag> | null = null;

    render(
      <Harness
        apps={mockApps}
        onSelectApp={onSelectApp}
        onReorderApps={onReorderApps}
        onReady={(instance) => {
          api = instance;
        }}
      />
    );

    await waitFor(() => expect(api).not.toBeNull());

    const list = screen.getByTestId('list');
    const item = screen.getByTestId('item-comfyui');

    list.getBoundingClientRect = () => createMockRect(0, 0, 64, 500);
    item.getBoundingClientRect = () => createMockRect(0, 0, 60, 60);

    const startPoint = { x: 32, y: LIST_TOP_PADDING + 30 };
    const endPoint = { x: 32, y: LIST_TOP_PADDING + 30 + (TOTAL_HEIGHT * 2) };

    act(() => {
      api?.onPointerDown('comfyui', createPointerDownEvent(item, startPoint.x, startPoint.y));
    });

    act(() => {
      window.dispatchEvent(createPointerEvent('pointermove', {
        clientX: startPoint.x + DRAG_START_DISTANCE + 1,
        clientY: startPoint.y,
        pointerId: 1,
      }));
    });

    await waitFor(() => {
      expect(api?.floatingState).toBe('dragging');
    });

    act(() => {
      window.dispatchEvent(createPointerEvent('pointermove', {
        clientX: endPoint.x,
        clientY: endPoint.y,
        pointerId: 1,
      }));
      window.dispatchEvent(createPointerEvent('pointerup', {
        clientX: endPoint.x,
        clientY: endPoint.y,
        pointerId: 1,
      }));
    });

    expect(onReorderApps).toHaveBeenCalled();
    const reordered = onReorderApps.mock.calls[0]?.[0] as AppConfig[];
    expect(reordered[2]?.id).toBe('comfyui');
  });

  it('deletes when released beyond delete distance', async () => {
    const onDeleteApp = vi.fn();
    const onSelectApp = vi.fn();
    let api: ReturnType<typeof usePhysicsDrag> | null = null;

    render(
      <Harness
        apps={mockApps}
        onSelectApp={onSelectApp}
        onDeleteApp={onDeleteApp}
        onReady={(instance) => {
          api = instance;
        }}
      />
    );

    await waitFor(() => expect(api).not.toBeNull());

    const list = screen.getByTestId('list');
    const item = screen.getByTestId('item-openwebui');

    list.getBoundingClientRect = () => createMockRect(0, 0, 64, 500);
    item.getBoundingClientRect = () => createMockRect(0, TOTAL_HEIGHT, 60, 60);

    const startPoint = { x: 32, y: LIST_TOP_PADDING + 30 + TOTAL_HEIGHT };
    const endPoint = { x: 64 + DELETE_DISTANCE + 20, y: startPoint.y };

    act(() => {
      api?.onPointerDown('openwebui', createPointerDownEvent(item, startPoint.x, startPoint.y));
    });

    act(() => {
      window.dispatchEvent(createPointerEvent('pointermove', {
        clientX: startPoint.x + DRAG_START_DISTANCE + 1,
        clientY: startPoint.y,
        pointerId: 1,
      }));
    });

    await waitFor(() => {
      expect(api?.floatingState).toBe('dragging');
    });

    act(() => {
      window.dispatchEvent(createPointerEvent('pointermove', {
        clientX: endPoint.x,
        clientY: endPoint.y,
        pointerId: 1,
      }));
      window.dispatchEvent(createPointerEvent('pointerup', {
        clientX: endPoint.x,
        clientY: endPoint.y,
        pointerId: 1,
      }));
      api?.completeDelete();
    });

    expect(onDeleteApp).toHaveBeenCalledWith('openwebui');
    expect(onSelectApp).toHaveBeenCalledWith(null);
  });
});
