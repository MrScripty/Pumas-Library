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

function installBridge(overrides: Partial<ElectronAPI> = {}): ElectronAPI {
  const bridge = {
    minimizeWindow: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    maximizeWindow: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    getTheme: vi.fn<() => Promise<'dark' | 'light'>>().mockResolvedValue('light'),
    getPathForFile: vi.fn<(file: File) => string>().mockReturnValue('/tmp/model.gguf'),
    get_status: vi.fn(),
    ...overrides,
  } as unknown as ElectronAPI;

  window.electronAPI = bridge;
  return bridge;
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
    const bridge = installBridge();

    await windowAPI.minimize();
    await windowAPI.maximize();
    await expect(windowAPI.getTheme()).resolves.toBe('light');

    expect(detectEnvironment()).toBe('electron');
    expect(getElectronAPI()).toBe(bridge);
    expect(isAPIAvailable()).toBe(true);
    expect(bridge.minimizeWindow).toHaveBeenCalledTimes(1);
    expect(bridge.maximizeWindow).toHaveBeenCalledTimes(1);
    expect(bridge.getTheme).toHaveBeenCalledTimes(1);
  });
});
