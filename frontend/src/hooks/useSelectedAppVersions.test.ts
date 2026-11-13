import { act, renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TEST_RUNTIME_PROVIDERS, testRuntimeProvidersResponse } from '../test/runtimeProviders';
import type { RuntimeProvidersResponse } from '../types/api-runtime-profiles';
import { resetRuntimeProvidersForTests } from './useRuntimeProviders';
import type { UseVersionsResult } from './useVersions';
import { useVersions } from './useVersions';
import { useSelectedAppVersions } from './useSelectedAppVersions';

const { getInstalledVersionsMock, getRuntimeProvidersMock } = vi.hoisted(() => ({
  getInstalledVersionsMock: vi.fn<(appId?: string) => Promise<{ success: true; versions: string[] }>>(),
  getRuntimeProvidersMock: vi.fn<() => Promise<RuntimeProvidersResponse>>(),
}));

vi.mock('./useVersions', () => ({
  useVersions: vi.fn(),
}));

vi.mock('../api/adapter', () => {
  const bridge = {
    get_installed_versions: getInstalledVersionsMock,
    get_runtime_providers: getRuntimeProvidersMock,
  };
  return {
    api: bridge,
    getElectronAPI: () => bridge,
    isAPIAvailable: () => true,
  };
});

const useVersionsMock = vi.mocked(useVersions);

function createVersions(overrides: Partial<UseVersionsResult> = {}): UseVersionsResult {
  return {
    activeVersion: null,
    availableVersions: [],
    cacheStatus: {
      has_cache: false,
      is_fetching: false,
      is_valid: false,
    },
    defaultVersion: null,
    error: null,
    getVersionInfo: vi.fn(),
    installNetworkStatus: 'idle',
    installationProgress: null,
    installedVersions: [],
    installingTag: null,
    installVersion: vi.fn(),
    cancelInstallation: vi.fn(),
    isLoading: false,
    isRateLimited: false,
    openActiveInstall: vi.fn(),
    openPath: vi.fn(),
    rateLimitRetryAfter: null,
    refreshAll: vi.fn(),
    refreshAvailableVersions: vi.fn(),
    removeVersion: vi.fn(),
    setDefaultVersion: vi.fn(),
    switchVersion: vi.fn(),
    versionStatus: null,
    ...overrides,
  };
}

// useVersions keeps its lists in state, so each app's result keeps its
// identity across renders; a fresh array per render would read as a change.
const versionsByAppId = new Map<string | undefined, UseVersionsResult>();

function stableVersions(appId: string | undefined): UseVersionsResult {
  let versions = versionsByAppId.get(appId);
  if (!versions) {
    versions = createVersions({
      activeVersion: `${appId}-active`,
      installedVersions: [`${appId}-installed`],
    });
    versionsByAppId.set(appId, versions);
  }
  return versions;
}

describe('useSelectedAppVersions', () => {
  beforeEach(() => {
    resetRuntimeProvidersForTests();
    versionsByAppId.clear();
    useVersionsMock.mockReset();
    useVersionsMock.mockImplementation((options = {}) => stableVersions(options.appId));
    getRuntimeProvidersMock.mockReset();
    getRuntimeProvidersMock.mockResolvedValue(testRuntimeProvidersResponse());
    getInstalledVersionsMock.mockReset();
    getInstalledVersionsMock.mockImplementation((appId) =>
      Promise.resolve({ success: true, versions: [`${appId}-listed`] })
    );
  });

  it('reads as loading, not unsupported, until the backend reports its providers', async () => {
    let resolveProviders!: (value: RuntimeProvidersResponse) => void;
    getRuntimeProvidersMock.mockReturnValue(
      new Promise((resolve) => {
        resolveProviders = resolve;
      })
    );

    const { result } = renderHook(() => useSelectedAppVersions('ollama'));

    expect(result.current.appVersions.isSupported).toBe(false);
    expect(result.current.appVersions.isLoading).toBe(true);
    expect(useVersionsMock).toHaveBeenLastCalledWith({
      appId: undefined,
      enabled: false,
      trackAvailableVersions: false,
    });

    await act(async () => {
      resolveProviders(testRuntimeProvidersResponse());
      await Promise.resolve();
    });

    expect(result.current.appVersions.isSupported).toBe(true);
    expect(result.current.appVersions.appId).toBe('ollama');
  });

  it('keeps full version state only for the selected version-managed app', async () => {
    const { result } = renderHook(() => useSelectedAppVersions('ollama'));

    await waitFor(() => {
      expect(result.current.appVersions.appId).toBe('ollama');
    });
    expect(useVersionsMock).toHaveBeenLastCalledWith({
      appId: 'ollama',
      enabled: true,
      trackAvailableVersions: true,
    });
    expect(
      new Set(useVersionsMock.mock.calls.map(([options]) => options?.appId))
    ).toEqual(new Set([undefined, 'ollama']));
    expect(result.current.appVersions.activeVersion).toBe('ollama-active');
  });

  it('lists installed versions of every app the backend version-manages', async () => {
    const { result } = renderHook(() => useSelectedAppVersions('ollama'));

    await waitFor(() => {
      expect(result.current.installedVersionsByAppId).toEqual({
        ollama: ['ollama-listed'],
        'llama-cpp': ['llama-cpp-listed'],
        'onnx-runtime': ['onnx-runtime-listed'],
        torch: ['torch-listed'],
      });
    });
  });

  it('derives version support from the backend instead of a local app list', async () => {
    getRuntimeProvidersMock.mockResolvedValue(
      testRuntimeProvidersResponse(
        TEST_RUNTIME_PROVIDERS.map((info) =>
          info.provider === 'torch' ? { ...info, version_managed: false } : info
        )
      )
    );

    const { result, rerender } = renderHook(
      ({ appId }: { appId: string | null }) => useSelectedAppVersions(appId),
      { initialProps: { appId: 'torch' as string | null } }
    );

    await waitFor(() => {
      expect(result.current.appVersions.isLoading).toBe(false);
    });
    expect(result.current.appVersions.isSupported).toBe(false);
    await waitFor(() => {
      expect(Object.keys(result.current.installedVersionsByAppId)).toEqual([
        'ollama',
        'llama-cpp',
        'onnx-runtime',
      ]);
    });
    expect(getInstalledVersionsMock).not.toHaveBeenCalledWith('torch');

    rerender({ appId: 'onnx-runtime' });
    expect(result.current.appVersions.isSupported).toBe(true);
    expect(result.current.appVersions.appId).toBe('onnx-runtime');
    expect(result.current.appVersions.activeVersion).toBe('onnx-runtime-active');
  });

  it('returns unsupported version state for no selection', async () => {
    const { result } = renderHook(() => useSelectedAppVersions(null));

    expect(result.current.appVersions.isSupported).toBe(false);
    expect(result.current.appVersions.isLoading).toBe(false);
    await waitFor(() => {
      expect(getRuntimeProvidersMock).toHaveBeenCalledTimes(1);
    });
  });

  it('falls back to unsupported when the provider lookup fails', async () => {
    getRuntimeProvidersMock.mockRejectedValue(new Error('backend unavailable'));

    const { result } = renderHook(() => useSelectedAppVersions('ollama'));

    await waitFor(() => {
      expect(result.current.appVersions.isLoading).toBe(false);
    });
    expect(result.current.appVersions.isSupported).toBe(false);
    expect(result.current.installedVersionsByAppId).toEqual({});
  });

  it('re-reads the selected app installed list when its version state changes', async () => {
    const { result, rerender } = renderHook(() => useSelectedAppVersions('llama-cpp'));
    await waitFor(() => {
      expect(result.current.installedVersionsByAppId['llama-cpp']).toEqual(['llama-cpp-listed']);
    });

    getInstalledVersionsMock.mockImplementation((appId) =>
      Promise.resolve({ success: true, versions: [`${appId}-after-install`] })
    );
    versionsByAppId.set(
      'llama-cpp',
      createVersions({ installedVersions: ['llama-cpp-installed', 'new'] })
    );
    rerender();

    await waitFor(() => {
      expect(result.current.installedVersionsByAppId['llama-cpp']).toEqual([
        'llama-cpp-after-install',
      ]);
    });
    expect(result.current.installedVersionsByAppId['ollama']).toEqual(['ollama-listed']);
  });
});
