import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { VersionRelease } from './useVersions';
import { useReleaseSizeCalculation } from './useReleaseSizeCalculation';

const { calculateReleaseSizeMock } = vi.hoisted(() => ({
  calculateReleaseSizeMock: vi.fn<
    (
      _tagName: string,
      _forceRefresh?: boolean,
      _appId?: string
    ) => Promise<unknown>
  >(),
}));

vi.mock('../api/adapter', () => ({
  api: {
    calculate_release_size: calculateReleaseSizeMock,
  },
}));

function createRelease(overrides: Partial<VersionRelease> = {}): VersionRelease {
  return {
    tagName: 'v1.0.0',
    name: 'v1.0.0',
    publishedAt: '2026-01-01T00:00:00Z',
    prerelease: false,
    body: '',
    htmlUrl: 'https://example.test/releases/v1.0.0',
    totalSize: null,
    ...overrides,
  };
}

describe('useReleaseSizeCalculation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    calculateReleaseSizeMock.mockResolvedValue({
      success: true,
      size: 1024,
    });
  });

  it('calculates missing release sizes and refreshes version state when opened', async () => {
    const onRefreshAll = vi.fn().mockResolvedValue(undefined);

    renderHook(() => useReleaseSizeCalculation({
      appId: 'comfyui',
      availableVersions: [
        createRelease({ tagName: 'v1.0.0', totalSize: null }),
        createRelease({ tagName: 'v1.1.0', totalSize: 2048 }),
      ],
      isOpen: true,
      onRefreshAll,
    }));

    await waitFor(() => {
      expect(calculateReleaseSizeMock).toHaveBeenCalledWith('v1.0.0', false, 'comfyui');
      expect(onRefreshAll).toHaveBeenCalledWith(false);
    });
    expect(calculateReleaseSizeMock).toHaveBeenCalledTimes(1);
  });

  it('does not calculate sizes while closed or when all sizes are known', async () => {
    const onRefreshAll = vi.fn().mockResolvedValue(undefined);

    renderHook(() => useReleaseSizeCalculation({
      availableVersions: [createRelease({ totalSize: null })],
      isOpen: false,
      onRefreshAll,
    }));
    renderHook(() => useReleaseSizeCalculation({
      availableVersions: [createRelease({ totalSize: 1024 })],
      isOpen: true,
      onRefreshAll,
    }));

    await waitFor(() => {
      expect(calculateReleaseSizeMock).not.toHaveBeenCalled();
      expect(onRefreshAll).not.toHaveBeenCalled();
    });
  });

  it('runs only once per open session and resets after close', async () => {
    const onRefreshAll = vi.fn().mockResolvedValue(undefined);
    const versions = [createRelease({ tagName: 'v1.0.0', totalSize: null })];
    const { rerender } = renderHook(
      ({ isOpen, availableVersions }) => useReleaseSizeCalculation({
        availableVersions,
        isOpen,
        onRefreshAll,
      }),
      {
        initialProps: {
          isOpen: true,
          availableVersions: versions,
        },
      }
    );

    await waitFor(() => {
      expect(calculateReleaseSizeMock).toHaveBeenCalledTimes(1);
    });

    rerender({
      isOpen: true,
      availableVersions: [createRelease({ tagName: 'v1.1.0', totalSize: null })],
    });
    expect(calculateReleaseSizeMock).toHaveBeenCalledTimes(1);

    rerender({
      isOpen: false,
      availableVersions: versions,
    });
    rerender({
      isOpen: true,
      availableVersions: versions,
    });

    await waitFor(() => {
      expect(calculateReleaseSizeMock).toHaveBeenCalledTimes(2);
    });
  });
});
