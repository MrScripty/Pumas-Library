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
          tag_name: 'v1.2.3',
          name: 'Version 1.2.3',
          published_at: '2026-04-12T00:00:00Z',
          prerelease: false,
          body: '',
          html_url: 'https://github.com/example/app/releases/tag/v1.2.3',
          total_size: 4096,
          archive_size: 2048,
          dependencies_size: 2048,
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
    });
    shouldUpdateUiFromBackgroundFetchMock.mockResolvedValue(false);
    resetBackgroundFetchFlagMock.mockResolvedValue({ success: true });
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('maps available versions, notifies about installing tags, and schedules follow-up refreshes', async () => {
    const onInstallingTagUpdate = vi.fn();

    const { result } = renderHook(() => useAvailableVersionState({
      isEnabled: true,
      onInstallingTagUpdate,
      resolvedAppId: 'comfyui',
      trackAvailableVersions: true,
    }));

    await act(async () => {
      await result.current.fetchAvailableVersions(true);
    });

    expect(getAvailableVersionsMock).toHaveBeenCalledWith(true, 'comfyui');
    expect(result.current.availableVersions).toEqual([
      expect.objectContaining({
        tagName: 'v1.2.3',
        htmlUrl: 'https://github.com/example/app/releases/tag/v1.2.3',
        totalSize: 4096,
        installing: true,
      }),
    ]);
    expect(onInstallingTagUpdate).toHaveBeenCalledWith('v1.2.3');

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });

    expect(getAvailableVersionsMock).toHaveBeenNthCalledWith(2, false, 'comfyui');
  });

  it('tracks rate-limit state when version fetching is throttled', async () => {
    getAvailableVersionsMock.mockResolvedValue({
      success: false,
      rate_limited: true,
      retry_after_secs: 120,
      versions: [],
    });

    const { result } = renderHook(() => useAvailableVersionState({
      isEnabled: true,
      resolvedAppId: 'comfyui',
      trackAvailableVersions: true,
    }));

    await act(async () => {
      await result.current.fetchAvailableVersions(false);
    });

    expect(result.current.isRateLimited).toBe(true);
    expect(result.current.rateLimitRetryAfter).toBe(120);
  });

  it('refreshes cached versions when background fetch completion is signaled', async () => {
    shouldUpdateUiFromBackgroundFetchMock
      .mockResolvedValueOnce(true)
      .mockResolvedValue(false);

    renderHook(() => useAvailableVersionState({
      isEnabled: true,
      resolvedAppId: 'comfyui',
      trackAvailableVersions: true,
    }));

    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    expect(getGithubCacheStatusMock).toHaveBeenCalledWith('comfyui');
    expect(shouldUpdateUiFromBackgroundFetchMock).toHaveBeenCalledTimes(2);
    expect(resetBackgroundFetchFlagMock).toHaveBeenCalledTimes(1);
    expect(getAvailableVersionsMock).toHaveBeenCalledTimes(1);
    expect(getAvailableVersionsMock).toHaveBeenCalledWith(false, 'comfyui');
  });

  it('does not poll background cache state when available-version tracking is disabled', async () => {
    renderHook(() => useAvailableVersionState({
      isEnabled: true,
      resolvedAppId: 'comfyui',
      trackAvailableVersions: false,
    }));

    await act(async () => {
      vi.advanceTimersByTime(4000);
    });

    expect(getGithubCacheStatusMock).not.toHaveBeenCalled();
  });
});
