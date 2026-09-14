import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { InstallDialog } from './InstallDialog';
import { APIError } from '../errors';
import type { InstallationProgress, VersionRelease } from '../hooks/useVersions';

const ollamaPatchReleases: VersionRelease[] = [
  {
    tagName: 'v0.22.1',
    name: 'Ollama 0.22.1',
    publishedAt: '2026-04-12T00:00:00Z',
    prerelease: false,
  },
  {
    tagName: 'v0.22.0',
    name: 'Ollama 0.22.0',
    publishedAt: '2026-04-11T00:00:00Z',
    prerelease: false,
  },
  {
    tagName: 'v0.20.7',
    name: 'Ollama 0.20.7',
    publishedAt: '2026-04-10T00:00:00Z',
    prerelease: false,
  },
  {
    tagName: 'v0.20.6',
    name: 'Ollama 0.20.6',
    publishedAt: '2026-04-09T00:00:00Z',
    prerelease: false,
  },
];

const activeProgress: InstallationProgress = {
  tag: 'v0.22.1',
  started_at: '2026-04-12T00:00:00Z',
  stage: 'download',
  stage_progress: 50,
  overall_progress: 25,
  current_item: 'archive.zip',
  download_speed: 1024,
  eta_seconds: 30,
  total_size: 4096,
  downloaded_bytes: 1024,
  dependency_count: null,
  completed_dependencies: 0,
  completed_items: [],
  error: null,
};

describe('InstallDialog', () => {
  it('keeps installed runtimes manageable when remote releases are unavailable', () => {
    render(<InstallDialog
      isOpen={true} onClose={vi.fn()} availableVersions={[]}
      installedVersions={['torch-runtime-0.1.0']} isLoading={false}
      onInstallVersion={vi.fn().mockResolvedValue(true)}
      onCancelInstallation={vi.fn().mockResolvedValue(true)}
      onRefreshAll={vi.fn().mockResolvedValue(undefined)}
      onRemoveVersion={vi.fn().mockResolvedValue(true)}
    />);
    expect(screen.getByText('torch-runtime-0.1.0')).toBeInTheDocument();
    expect(screen.queryByText('No versions available')).not.toBeInTheDocument();
  });

  it('renders modal mode as a named dialog and closes from backdrop or Escape key', () => {
    const onClose = vi.fn();

    render(
      <InstallDialog
        isOpen={true}
        onClose={onClose}
        availableVersions={[]}
        installedVersions={[]}
        isLoading={false}
        onInstallVersion={vi.fn().mockResolvedValue(true)}
        onCancelInstallation={vi.fn().mockResolvedValue(true)}
        onRefreshAll={vi.fn().mockResolvedValue(undefined)}
        onRemoveVersion={vi.fn().mockResolvedValue(true)}
      />
    );

    const dialog = screen.getByRole('dialog', { name: 'Install Application Version' });
    const backdrop = dialog.parentElement?.querySelector<HTMLElement>('[data-modal-backdrop]');
    if (!backdrop) {
      throw new TypeError('Expected install dialog backdrop');
    }

    fireEvent.mouseDown(backdrop);
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('only shows the latest patch for each Ollama minor release', () => {
    render(
      <InstallDialog
        isOpen={true}
        onClose={vi.fn()}
        availableVersions={ollamaPatchReleases}
        installedVersions={[]}
        isLoading={false}
        onInstallVersion={vi.fn().mockResolvedValue(true)}
        onCancelInstallation={vi.fn().mockResolvedValue(true)}
        onRefreshAll={vi.fn().mockResolvedValue(undefined)}
        onRemoveVersion={vi.fn().mockResolvedValue(true)}
        appDisplayName="Ollama"
      />
    );

    expect(screen.getByText('0.22.1')).toBeInTheDocument();
    expect(screen.queryByText('0.22.0')).not.toBeInTheDocument();
    expect(screen.getByText('0.20.7')).toBeInTheDocument();
    expect(screen.queryByText('0.20.6')).not.toBeInTheDocument();
  });

  it('presents terminal success with its install tag until the outcome timer expires', () => {
    vi.useFakeTimers();
    try {
      const { rerender } = render(
        <InstallDialog
          isOpen={true}
          onClose={vi.fn()}
          availableVersions={ollamaPatchReleases}
          installedVersions={[]}
          isLoading={false}
          onInstallVersion={vi.fn().mockResolvedValue(true)}
          onCancelInstallation={vi.fn().mockResolvedValue(true)}
          onRefreshAll={vi.fn().mockResolvedValue(undefined)}
          onRemoveVersion={vi.fn().mockResolvedValue(true)}
          installingTag="v0.22.1"
          installationProgress={activeProgress}
        />
      );

      expect(
        screen.getByRole('progressbar', { name: 'Overall installation progress' })
      ).toHaveAttribute('aria-valuenow', '25');

      rerender(
        <InstallDialog
          isOpen={true}
          onClose={vi.fn()}
          availableVersions={ollamaPatchReleases}
          installedVersions={['v0.22.1']}
          isLoading={false}
          onInstallVersion={vi.fn().mockResolvedValue(true)}
          onCancelInstallation={vi.fn().mockResolvedValue(true)}
          onRefreshAll={vi.fn().mockResolvedValue(undefined)}
          onRemoveVersion={vi.fn().mockResolvedValue(true)}
          installingTag={null}
          installationProgress={{
            ...activeProgress,
            completed_at: '2026-04-12T00:05:00Z',
            success: true,
          }}
        />
      );

      expect(screen.getByRole('status')).toHaveTextContent(
        'v0.22.1 has been successfully installed'
      );

      act(() => {
        vi.advanceTimersByTime(2999);
      });
      expect(screen.getByRole('status')).toBeInTheDocument();

      act(() => {
        vi.advanceTimersByTime(1);
      });
      expect(screen.queryByRole('status')).not.toBeInTheDocument();
      expect(screen.getByText('0.22.1')).toBeInTheDocument();
    } finally {
      vi.clearAllTimers();
      vi.useRealTimers();
    }
  });

  it('routes cancellation through its owner and shows successful cancellation', async () => {
    const onCancelInstallation = vi.fn().mockResolvedValue(true);
    render(
      <InstallDialog
        isOpen={true}
        onClose={vi.fn()}
        availableVersions={ollamaPatchReleases}
        installedVersions={[]}
        isLoading={false}
        onInstallVersion={vi.fn().mockResolvedValue(true)}
        onCancelInstallation={onCancelInstallation}
        onRefreshAll={vi.fn().mockResolvedValue(undefined)}
        onRemoveVersion={vi.fn().mockResolvedValue(true)}
        installingTag="v0.22.1"
        installationProgress={activeProgress}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    fireEvent.click(screen.getByRole('button', { name: '25%' }));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel installation' }));

    await waitFor(() => expect(onCancelInstallation).toHaveBeenCalledTimes(1));
    expect(screen.getByText('Installation canceled')).toBeInTheDocument();
  });

  it('shows a cancellation failure instead of silently logging it', async () => {
    const onCancelInstallation = vi.fn().mockRejectedValue(
      new APIError('Cancellation unavailable', 'cancel_installation')
    );
    render(
      <InstallDialog
        isOpen={true}
        onClose={vi.fn()}
        availableVersions={ollamaPatchReleases}
        installedVersions={[]}
        isLoading={false}
        onInstallVersion={vi.fn().mockResolvedValue(true)}
        onCancelInstallation={onCancelInstallation}
        onRefreshAll={vi.fn().mockResolvedValue(undefined)}
        onRemoveVersion={vi.fn().mockResolvedValue(true)}
        installingTag="v0.22.1"
        installationProgress={activeProgress}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    fireEvent.click(screen.getByRole('button', { name: '25%' }));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel installation' }));

    expect(await screen.findByText('Cancellation unavailable')).toBeInTheDocument();
    expect(screen.queryByText('Installation canceled')).not.toBeInTheDocument();
  });
});
