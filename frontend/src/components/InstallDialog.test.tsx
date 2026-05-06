import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { InstallDialog } from './InstallDialog';
import type { VersionRelease } from '../hooks/useVersions';

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

describe('InstallDialog', () => {
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
        onRefreshAll={vi.fn().mockResolvedValue(undefined)}
        onRemoveVersion={vi.fn().mockResolvedValue(true)}
      />
    );

    expect(screen.getByRole('dialog', { name: 'Install ComfyUI Version' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Dismiss install dialog' }));
    expect(onClose).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(window, { key: 'Escape' });
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
});
