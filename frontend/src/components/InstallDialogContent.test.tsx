import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { InstallDialogContent } from './InstallDialogContent';
import type { InstallationProgress, VersionRelease } from '../hooks/useVersions';

const baseReleases: VersionRelease[] = [
  {
    tagName: 'v1.2.3',
    name: 'Version 1.2.3',
    publishedAt: '2026-04-10T00:00:00Z',
    prerelease: false,
    htmlUrl: 'https://github.com/example/app/releases/tag/v1.2.3',
    totalSize: 1024 ** 3,
  },
  {
    tagName: 'v2.0.0',
    name: 'Version 2.0.0',
    publishedAt: '2026-04-11T00:00:00Z',
    prerelease: false,
    totalSize: 2 * 1024 ** 3,
  },
];

const progress: InstallationProgress = {
  tag: 'v3.0.0',
  started_at: '2026-04-12T00:00:00Z',
  stage: 'dependencies',
  stage_progress: 55,
  overall_progress: 78,
  current_item: 'torch',
  download_speed: 5 * 1024 * 1024,
  eta_seconds: 30,
  total_size: 1024 ** 3,
  downloaded_bytes: 512 * 1024 ** 2,
  dependency_count: 4,
  completed_dependencies: 2,
  completed_items: [
    {
      name: 'wheel.whl',
      type: 'dependency',
      size: 128 * 1024,
      completed_at: '2026-04-12T00:02:00Z',
    },
  ],
  error: 'Dependency install failed',
  log_path: '/tmp/install.log',
};

function getClosestButton(label: string): HTMLButtonElement {
  const button = screen.getByText(label).closest('button');
  if (!(button instanceof HTMLButtonElement)) {
    throw new TypeError(`Expected ${label} to have a button ancestor`);
  }

  return button;
}

function renderInstallDialogContent(overrides: Partial<React.ComponentProps<typeof InstallDialogContent>> = {}) {
  const props: React.ComponentProps<typeof InstallDialogContent> = {
    cancellationNotice: null,
    cancelHoverTag: null,
    errorMessage: 'Remove failed',
    errorVersion: 'v2.0.0',
    filteredVersions: baseReleases,
    hoveredTag: null,
    installNetworkStatus: 'idle',
    installedVersions: [],
    installingVersion: null,
    isLoading: false,
    isRateLimited: false,
    progress: null,
    rateLimitRetryAfter: null,
    showCompletedItems: false,
    showProgressDetails: false,
    stickyFailedLogPath: null,
    stickyFailedTag: null,
    onCancelInstallation: vi.fn(),
    onOpenLogPath: vi.fn().mockResolvedValue(undefined),
    onOpenReleaseLink: vi.fn().mockResolvedValue(undefined),
    onRemoveVersion: vi.fn().mockResolvedValue(true),
    onSetCancelHoverTag: vi.fn(),
    onSetHoveredTag: vi.fn(),
    onToggleCompletedItems: vi.fn(),
    onBackToList: vi.fn(),
    onInstallVersion: vi.fn(),
    onReportRemoveError: vi.fn(),
    ...overrides,
  };

  return {
    ...render(<InstallDialogContent {...props} />),
    props,
  };
}

describe('InstallDialogContent', () => {
  it('renders notices and routes install, release-link, and remove-error actions', async () => {
    const removeError = new Error('remove failed');
    const { props } = renderInstallDialogContent({
      cancellationNotice: 'Cancelling v0.9.0',
      hoveredTag: 'v2.0.0',
      isRateLimited: true,
      rateLimitRetryAfter: 120,
      installedVersions: ['v2.0.0'],
      onRemoveVersion: vi.fn().mockRejectedValue(removeError),
    });

    expect(screen.getByText('Cancelling v0.9.0')).toBeInTheDocument();
    expect(screen.getByText('GitHub Rate Limit Reached')).toBeInTheDocument();
    expect(screen.getByText(/Rate limit resets in 2 minutes/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Release notes' }));
    expect(props.onOpenReleaseLink).toHaveBeenCalledWith(
      'https://github.com/example/app/releases/tag/v1.2.3'
    );

    fireEvent.click(getClosestButton('1.00 GB'));
    expect(props.onInstallVersion).toHaveBeenCalledWith('v1.2.3');

    fireEvent.click(getClosestButton('Uninstall'));
    await waitFor(() => {
      expect(props.onReportRemoveError).toHaveBeenCalledWith('v2.0.0', removeError);
    });
  });

  it('renders the progress details branch and routes detail-view callbacks', () => {
    const { props } = renderInstallDialogContent({
      filteredVersions: [],
      installingVersion: 'v3.0.0',
      progress,
      showProgressDetails: true,
      stickyFailedLogPath: '/tmp/install.log',
      stickyFailedTag: 'v3.0.0',
    });

    expect(screen.getByText('Installing v3.0.0')).toBeInTheDocument();
    expect(screen.getByText('Overall Progress')).toBeInTheDocument();
    expect(screen.getByText('2 / 4')).toBeInTheDocument();
    expect(screen.getByText('Dependency install failed')).toBeInTheDocument();
    expect(screen.getByText('Completed Items (1)')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(props.onBackToList).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText('Completed Items (1)'));
    expect(props.onToggleCompletedItems).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: /open log/i }));
    expect(props.onOpenLogPath).toHaveBeenCalledWith('/tmp/install.log');
  });

  it('renders the loading state while versions are being fetched', () => {
    renderInstallDialogContent({
      filteredVersions: [],
      isLoading: true,
    });

    expect(screen.queryByText('No versions available')).not.toBeInTheDocument();
    expect(document.querySelector('.animate-spin')).not.toBeNull();
  });

  it('renders the empty state when no versions match the filters', () => {
    renderInstallDialogContent({
      filteredVersions: [],
    });

    expect(screen.getByText('No versions available')).toBeInTheDocument();
    expect(screen.getByText('Try adjusting the filters above')).toBeInTheDocument();
  });
});
