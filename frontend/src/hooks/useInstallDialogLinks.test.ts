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
    vi.spyOn(window, 'open').mockImplementation(() => null);
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
    expect(window.open).not.toHaveBeenCalled();
  });

  it('falls back to window.open when the bridge is unavailable or rejects a URL', async () => {
    const { result } = renderHook(() => useInstallDialogLinks());

    isApiAvailableMock.mockReturnValue(false);
    await act(async () => {
      await result.current.openReleaseLink('https://example.test/no-bridge');
    });

    isApiAvailableMock.mockReturnValue(true);
    openUrlMock.mockResolvedValue({ success: false });
    await act(async () => {
      await result.current.openReleaseLink('https://example.test/rejected');
    });

    expect(window.open).toHaveBeenCalledWith('https://example.test/no-bridge', '_blank');
    expect(window.open).toHaveBeenCalledWith('https://example.test/rejected', '_blank');
  });
});
