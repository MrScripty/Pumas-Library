import { afterEach, describe, expect, it, vi } from 'vitest';
import { APIError } from '../errors';
import type { ElectronAPI } from '../types/api';
import {
  api,
  detectEnvironment,
  getElectronAPI,
  isAPIAvailable,
  safeAPICall,
  windowAPI,
} from './adapter';

interface InstalledBridge {
  bridge: ElectronAPI;
  getThemeMock: ReturnType<typeof vi.fn<() => Promise<'dark' | 'light'>>>;
  maximizeWindowMock: ReturnType<typeof vi.fn<() => Promise<void>>>;
  minimizeWindowMock: ReturnType<typeof vi.fn<() => Promise<void>>>;
}

function installBridge(overrides: Partial<ElectronAPI> = {}): InstalledBridge {
  const minimizeWindowMock = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const maximizeWindowMock = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const getThemeMock = vi.fn<() => Promise<'dark' | 'light'>>().mockResolvedValue('light');

  const bridge = {
    minimizeWindow: minimizeWindowMock,
    maximizeWindow: maximizeWindowMock,
    getTheme: getThemeMock,
    getPathForFile: vi.fn<(file: File) => string>().mockReturnValue('/tmp/model.gguf'),
    get_status: vi.fn(),
    ...overrides,
  } as unknown as ElectronAPI;

  window.electronAPI = bridge;
  return {
    bridge,
    getThemeMock,
    maximizeWindowMock,
    minimizeWindowMock,
  };
}

describe('api adapter', () => {
  afterEach(() => {
    delete window.electronAPI;
    vi.clearAllMocks();
  });

  it('reports browser mode and returns safe fallbacks without the Electron bridge', async () => {
    delete window.electronAPI;

    await expect(api.get_status()).rejects.toBeInstanceOf(APIError);
    await expect(safeAPICall(async () => 'backend-value', 'fallback-value')).resolves.toBe(
      'fallback-value'
    );

    expect(detectEnvironment()).toBe('browser');
    expect(getElectronAPI()).toBeNull();
    expect(isAPIAvailable()).toBe(false);
    await expect(windowAPI.getTheme()).resolves.toBe('dark');
  });

  it('routes Electron-specific calls through the canonical bridge', async () => {
    const { bridge, getThemeMock, maximizeWindowMock, minimizeWindowMock } = installBridge();

    await windowAPI.minimize();
    await windowAPI.maximize();
    await expect(windowAPI.getTheme()).resolves.toBe('light');

    expect(detectEnvironment()).toBe('electron');
    expect(getElectronAPI()).toBe(bridge);
    expect(isAPIAvailable()).toBe(true);
    expect(minimizeWindowMock).toHaveBeenCalledTimes(1);
    expect(maximizeWindowMock).toHaveBeenCalledTimes(1);
    expect(getThemeMock).toHaveBeenCalledTimes(1);
  });
});
