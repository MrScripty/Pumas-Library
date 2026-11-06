import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useInstallDialogLinks } from './useInstallDialogLinks';

const {
  isApiAvailableMock,
  openPathMock,
  openUrlMock,
} = vi.hoisted(() => ({
  isApiAvailableMock: vi.fn<() => boolean>(),
  openPathMock: vi.fn<(_path: string) => Promise<unknown>>(),
  openUrlMock: vi.fn<(_url: string) => Promise<{ success?: boolean }>>(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    open_path: openPathMock,
    open_url: openUrlMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

describe('useInstallDialogLinks', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isApiAvailableMock.mockReturnValue(true);
    openPathMock.mockResolvedValue(undefined);
    openUrlMock.mockResolvedValue({ success: true });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('opens log paths through the backend bridge when available', async () => {
    const { result } = renderHook(() => useInstallDialogLinks());

    await act(async () => {
      await result.current.openLogPath('/tmp/install.log');
    });

    expect(openPathMock).toHaveBeenCalledWith('/tmp/install.log');
  });

  it('skips empty log paths and unavailable backend bridge calls', async () => {
    isApiAvailableMock.mockReturnValue(false);
    const { result } = renderHook(() => useInstallDialogLinks());

    await act(async () => {
      await result.current.openLogPath(null);
      await result.current.openLogPath('/tmp/install.log');
    });

    expect(openPathMock).not.toHaveBeenCalled();
  });

  it('opens release links through the backend bridge when successful', async () => {
    const { result } = renderHook(() => useInstallDialogLinks());

    await act(async () => {
      await result.current.openReleaseLink('https://example.test/release');
    });

    expect(openUrlMock).toHaveBeenCalledWith('https://example.test/release');
  });

  it('never opens an in-app window when the bridge is unavailable, refuses, or the URL is not http(s)', async () => {
    const openWindow = vi.fn();
    vi.stubGlobal('open', openWindow);
    const { result } = renderHook(() => useInstallDialogLinks());

    isApiAvailableMock.mockReturnValue(false);
    await act(async () => {
      await result.current.openReleaseLink('https://example.test/no-bridge');
    });
    expect(openUrlMock).not.toHaveBeenCalled();

    isApiAvailableMock.mockReturnValue(true);
    openUrlMock.mockResolvedValue({ success: false });
    await act(async () => {
      await result.current.openReleaseLink('https://example.test/rejected');
    });
    expect(openUrlMock).toHaveBeenCalledWith('https://example.test/rejected');

    openUrlMock.mockClear();
    await act(async () => {
      await result.current.openReleaseLink('javascript:alert(1)');
    });
    expect(openUrlMock).not.toHaveBeenCalled();

    openUrlMock.mockRejectedValue(new Error('bridge failed'));
    await act(async () => {
      await result.current.openReleaseLink('https://example.test/throws');
    });
    expect(openWindow).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });
});
