import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAppWindowActions } from './useAppWindowActions';

const {
  closeWindowMock,
  isApiAvailableMock,
  minimizeMock,
  openPathMock,
} = vi.hoisted(() => ({
  closeWindowMock: vi.fn<() => Promise<void>>(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  minimizeMock: vi.fn<() => Promise<void>>(),
  openPathMock: vi.fn<(_path: string) => Promise<{ success: boolean; error?: string }>>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    close_window: closeWindowMock,
    open_path: openPathMock,
  },
  isAPIAvailable: isApiAvailableMock,
  windowAPI: {
    minimize: minimizeMock,
  },
}));

describe('useAppWindowActions', () => {
  beforeEach(() => {
    isApiAvailableMock.mockReturnValue(true);
    openPathMock.mockResolvedValue({ success: true });
    closeWindowMock.mockResolvedValue(undefined);
    minimizeMock.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('opens the shared models root through the API when available', async () => {
    const { result } = renderHook(() => useAppWindowActions());

    await act(async () => {
      await result.current.openModelsRoot();
    });

    expect(openPathMock).toHaveBeenCalledWith('shared-resources/models');
  });

  it('routes minimize and close actions through the backend API when available', async () => {
    const { result } = renderHook(() => useAppWindowActions());

    await act(async () => {
      result.current.minimizeWindow();
      result.current.closeWindow();
    });

    expect(minimizeMock).toHaveBeenCalledTimes(1);
    expect(closeWindowMock).toHaveBeenCalledTimes(1);
  });

  it('falls back to window.close when the backend API is unavailable', async () => {
    isApiAvailableMock.mockReturnValue(false);
    const windowCloseSpy = vi.spyOn(window, 'close').mockImplementation(() => undefined);
    const { result } = renderHook(() => useAppWindowActions());

    await act(async () => {
      await result.current.openModelsRoot();
      result.current.closeWindow();
    });

    expect(openPathMock).not.toHaveBeenCalled();
    expect(closeWindowMock).not.toHaveBeenCalled();
    expect(windowCloseSpy).toHaveBeenCalledTimes(1);

    windowCloseSpy.mockRestore();
  });
});
