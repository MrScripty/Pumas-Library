import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const {
  getAvailableVersionsMock,
  getGithubCacheStatusMock,
  isApiAvailableMock,
  resetBackgroundFetchFlagMock,
  shouldUpdateUiFromBackgroundFetchMock,
} = vi.hoisted(() => ({
  getAvailableVersionsMock: vi.fn(),
  getGithubCacheStatusMock: vi.fn(),
  isApiAvailableMock: vi.fn<() => boolean>(),
  resetBackgroundFetchFlagMock: vi.fn(),
  shouldUpdateUiFromBackgroundFetchMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    get_available_versions: getAvailableVersionsMock,
    get_github_cache_status: getGithubCacheStatusMock,
    reset_background_fetch_flag: resetBackgroundFetchFlagMock,
    should_update_ui_from_background_fetch: shouldUpdateUiFromBackgroundFetchMock,
  },
  isAPIAvailable: isApiAvailableMock,
}));

import { useAvailableVersionState } from './useAvailableVersionState';

describe('useAvailableVersionState', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    isApiAvailableMock.mockReturnValue(true);
    getAvailableVersionsMock.mockResolvedValue({
      success: true,
      versions: [
        {
          tagName: 'v1.2.3',
          name: 'Version 1.2.3',
          publishedAt: '2026-04-12T00:00:00Z',
          prerelease: false,
          body: '',
          htmlUrl: 'https://github.com/example/app/releases/tag/v1.2.3',
          totalSize: 4096,
          archiveSize: 2048,
          dependenciesSize: 2048,
          installing: true,
          assets: [],
        },
      ],
    });
    getGithubCacheStatusMock.mockResolvedValue({
      has_cache: true,
      is_valid: true,
      is_fetching: false,
      age_seconds: 10,
      last_fetched: null,
      releases_count: null,
    });
    shouldUpdateUiFromBackgroundFetchMock.mockResolvedValue(false);
    resetBackgroundFetchFlagMock.mockResolvedValue({ success: true });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('maps available versions and schedules follow-up refreshes', async () => {
    const { result } = renderHook(() => useAvailableVersionState({
      isEnabled: true,
      resolvedAppId: 'ollama',
      trackAvailableVersions: true,
    }));

    await act(async () => {
      await result.current.fetchAvailableVersions(true);
    });

    expect(getAvailableVersionsMock).toHaveBeenCalledWith(true, 'ollama');
    expect(result.current.availableVersions).toEqual([
      expect.objectContaining({
        tagName: 'v1.2.3',
        htmlUrl: 'https://github.com/example/app/releases/tag/v1.2.3',
        totalSize: 4096,
        installing: true,
      }),
    ]);
    await act(async () => {
      vi.advanceTimersByTime(1500);
    });

    expect(getAvailableVersionsMock).toHaveBeenNthCalledWith(2, false, 'ollama');
  });

  it('tracks rate-limit state when version fetching is throttled', async () => {
    getAvailableVersionsMock.mockResolvedValue({
      success: false,
      rate_limited: true,
      retry_after_secs: 120,
      error: 'Rate limited',
    });

    const { result } = renderHook(() => useAvailableVersionState({
      isEnabled: true,
      resolvedAppId: 'ollama',
      trackAvailableVersions: true,
    }));

    await act(async () => {
      await result.current.fetchAvailableVersions(false);
    });

    expect(result.current.isRateLimited).toBe(true);
    expect(result.current.rateLimitRetryAfter).toBe(120);
  });

  it('does not run the discontinued native-app background refresh path', async () => {
    shouldUpdateUiFromBackgroundFetchMock
      .mockResolvedValueOnce(true)
      .mockResolvedValue(false);

    renderHook(() => useAvailableVersionState({
      isEnabled: true,
      resolvedAppId: 'ollama',
      trackAvailableVersions: true,
    }));

    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    expect(getGithubCacheStatusMock).toHaveBeenCalledWith('ollama');
    expect(shouldUpdateUiFromBackgroundFetchMock).not.toHaveBeenCalled();
    expect(resetBackgroundFetchFlagMock).not.toHaveBeenCalled();
  });

  it('preserves exact producer strings and maps nullable presentation fields', async () => {
    getAvailableVersionsMock.mockResolvedValue({
      success: true,
      versions: [
        {
          tagName: ' v0.8.0 λ ',
          name: '',
          publishedAt: '2026-04-13T00:00:00Z',
          prerelease: false,
          htmlUrl: 'https://github.com/ollama/ollama/releases/tag/v0.8.0',
          totalSize: 1024,
          archiveSize: 512,
          dependenciesSize: 512,
          installing: null,
          body: null,
          assets: [],
        },
      ],
    });

    const { result } = renderHook(() => useAvailableVersionState({
      isEnabled: true,
      resolvedAppId: 'ollama',
      trackAvailableVersions: true,
    }));

    await act(async () => {
      await result.current.fetchAvailableVersions(false);
    });

    expect(result.current.availableVersions).toEqual([
      expect.objectContaining({
        tagName: ' v0.8.0 λ ',
        name: '',
        publishedAt: '2026-04-13T00:00:00Z',
        htmlUrl: 'https://github.com/ollama/ollama/releases/tag/v0.8.0',
        totalSize: 1024,
        body: undefined,
        installing: false,
      }),
    ]);
  });

  it('does not poll background cache state when available-version tracking is disabled', async () => {
    renderHook(() => useAvailableVersionState({
      isEnabled: true,
      resolvedAppId: 'ollama',
      trackAvailableVersions: false,
    }));

    await act(async () => {
      vi.advanceTimersByTime(4000);
    });

    expect(getGithubCacheStatusMock).not.toHaveBeenCalled();
  });
});
