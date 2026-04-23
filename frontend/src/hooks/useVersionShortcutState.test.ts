import { act, renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { APIError } from '../errors';

const {
  getAllShortcutStatesMock,
  isApiAvailableMock,
  setVersionShortcutsMock,
} = vi.hoisted(() => ({
  getAllShortcutStatesMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  setVersionShortcutsMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_all_shortcut_states: getAllShortcutStatesMock,
    set_version_shortcuts: setVersionShortcutsMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

import { useVersionShortcutState } from './useVersionShortcutState';

async function flushMicrotasks() {
  await act(async () => {
    await Promise.resolve();
  });
}

describe('useVersionShortcutState', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
    getAllShortcutStatesMock.mockResolvedValue({
      success: true,
      states: {
        states: {},
      },
    });
    setVersionShortcutsMock.mockResolvedValue({
      success: true,
      state: {
        menu: true,
        desktop: true,
      },
    });
  });

  it('refreshes and normalizes shortcut states for installed versions', async () => {
    getAllShortcutStatesMock.mockResolvedValueOnce({
      success: true,
      states: {
        states: {
          'v1.0.0': {
            menu: 1,
            desktop: 0,
          },
          'v2.0.0': {
            menu: false,
            desktop: true,
          },
        },
      },
    });

    const { result } = renderHook(() => useVersionShortcutState({
      activeVersion: null,
      installedVersions: ['v1.0.0', 'v2.0.0'],
      supportsShortcuts: true,
    }));

    await flushMicrotasks();

    expect(getAllShortcutStatesMock).toHaveBeenCalledTimes(1);
    expect(result.current.shortcutState).toEqual({
      'v1.0.0': {
        menu: true,
        desktop: false,
      },
      'v2.0.0': {
        menu: false,
        desktop: true,
      },
    });
  });

  it('clears local shortcut state when shortcut support is disabled', async () => {
    const { result, rerender } = renderHook(
      (props: { supportsShortcuts: boolean }) => useVersionShortcutState({
        activeVersion: 'v1.0.0',
        activeShortcutState: { menu: true, desktop: false },
        installedVersions: ['v1.0.0'],
        supportsShortcuts: props.supportsShortcuts,
      }),
      {
        initialProps: { supportsShortcuts: true },
      }
    );

    await flushMicrotasks();

    expect(result.current.shortcutState).toEqual({
      'v1.0.0': {
        menu: true,
        desktop: false,
      },
    });

    rerender({ supportsShortcuts: false });

    await flushMicrotasks();

    expect(result.current.shortcutState).toEqual({});
  });

  it('syncs active version shortcut state into the local map', async () => {
    const { result, rerender } = renderHook(
      (props: { activeShortcutState?: { menu: boolean; desktop: boolean } }) => useVersionShortcutState({
        activeVersion: 'v1.0.0',
        activeShortcutState: props.activeShortcutState,
        installedVersions: ['v1.0.0'],
        supportsShortcuts: true,
      }),
      {
        initialProps: {
          activeShortcutState: undefined as { menu: boolean; desktop: boolean } | undefined,
        },
      }
    );

    await flushMicrotasks();

    expect(result.current.shortcutState).toEqual({});

    rerender({
      activeShortcutState: {
        menu: false,
        desktop: true,
      },
    });

    await flushMicrotasks();

    expect(result.current.shortcutState).toEqual({
      'v1.0.0': {
        menu: false,
        desktop: true,
      },
    });
  });

  it('applies optimistic shortcut toggles and reconciles with backend state', async () => {
    setVersionShortcutsMock.mockResolvedValueOnce({
      success: true,
      state: {
        menu: true,
        desktop: false,
      },
    });

    const { result } = renderHook(() => useVersionShortcutState({
      activeVersion: 'v1.0.0',
      activeShortcutState: { menu: false, desktop: false },
      installedVersions: ['v1.0.0'],
      supportsShortcuts: true,
    }));

    await flushMicrotasks();

    await act(async () => {
      await result.current.toggleShortcuts('v1.0.0', true);
    });

    expect(setVersionShortcutsMock).toHaveBeenCalledWith('v1.0.0', true);
    expect(result.current.shortcutState).toEqual({
      'v1.0.0': {
        menu: true,
        desktop: false,
      },
    });
  });

  it('restores the exact previous shortcut state when a toggle request fails', async () => {
    setVersionShortcutsMock.mockRejectedValueOnce(
      new APIError('toggle failed', 'set_version_shortcuts')
    );

    const { result } = renderHook(() => useVersionShortcutState({
      activeVersion: 'v1.0.0',
      activeShortcutState: { menu: true, desktop: false },
      installedVersions: ['v1.0.0'],
      supportsShortcuts: true,
    }));

    await flushMicrotasks();

    await act(async () => {
      await result.current.toggleShortcuts('v1.0.0', false);
    });

    expect(setVersionShortcutsMock).toHaveBeenCalledWith('v1.0.0', false);
    expect(result.current.shortcutState).toEqual({
      'v1.0.0': {
        menu: true,
        desktop: false,
      },
    });
  });

  it('restores the exact previous shortcut state when a toggle response is unsuccessful', async () => {
    setVersionShortcutsMock.mockResolvedValueOnce({
      success: false,
      error: 'toggle rejected',
      state: {
        menu: false,
        desktop: false,
      },
    });

    const { result } = renderHook(() => useVersionShortcutState({
      activeVersion: 'v1.0.0',
      activeShortcutState: { menu: true, desktop: false },
      installedVersions: ['v1.0.0'],
      supportsShortcuts: true,
    }));

    await flushMicrotasks();

    await act(async () => {
      await result.current.toggleShortcuts('v1.0.0', false);
    });

    expect(setVersionShortcutsMock).toHaveBeenCalledWith('v1.0.0', false);
    expect(result.current.shortcutState).toEqual({
      'v1.0.0': {
        menu: true,
        desktop: false,
      },
    });
  });
});
