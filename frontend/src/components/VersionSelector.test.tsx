import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { VersionSelector } from './VersionSelector';
import type { InstallationProgress } from '../types/versions';

const torchSetupProgress: InstallationProgress = {
  tag: 'v2.14.0', started_at: '2026-04-12T00:00:00Z', stage: 'setup',
  stage_progress: 0, overall_progress: 95, current_item: 'Creating managed Python environment',
  download_speed: null, eta_seconds: null, total_size: null, downloaded_bytes: 0,
  dependency_count: null, completed_dependencies: 0, completed_items: [], error: null,
};

describe('VersionSelector popup', () => {
  it('keeps the Torch install indicator indeterminate during setup', () => {
    const { container, rerender } = render(<VersionSelector
      appId="torch" activeVersion={null} installedVersions={['v2.9.1']}
      isLoading={false} onOpenVersionManager={vi.fn()}
      openActiveInstall={vi.fn().mockResolvedValue(true)} switchVersion={vi.fn().mockResolvedValue(true)}
      installingVersion="v2.14.0" installationProgress={torchSetupProgress}
    />);
    const ring = container.querySelector('.download-progress-ring.is-waiting');
    expect(ring).toBeInTheDocument();
    expect(ring).toHaveStyle({ '--progress': '60deg' });

    rerender(<VersionSelector
      appId="torch" activeVersion={null} installedVersions={['v2.9.1']}
      isLoading={false} onOpenVersionManager={vi.fn()}
      openActiveInstall={vi.fn().mockResolvedValue(true)} switchVersion={vi.fn().mockResolvedValue(true)}
      installingVersion="v2.14.0" installationProgress={{ ...torchSetupProgress, stage_progress: 40 }}
    />);
    expect(container.querySelector('.download-progress-ring.is-waiting')).not.toBeInTheDocument();
  });
  it('shows a failed switch and keeps the active version selected', async () => {
    render(<VersionSelector
      activeVersion="v1.0.0" installedVersions={['v1.0.0', 'v1.1.0']}
      isLoading={false} onOpenVersionManager={vi.fn()}
      openActiveInstall={vi.fn().mockResolvedValue(true)}
      switchVersion={vi.fn().mockRejectedValue(new Error('Stop the running runtime first'))}
    />);
    fireEvent.click(screen.getByRole('button', { name: 'v1.0.0' }));
    fireEvent.click(screen.getByRole('button', { name: 'Switch to v1.1.0' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Stop the running runtime first');
    expect(screen.getByRole('button', { name: 'v1.0.0' })).toBeInTheDocument();
  });

  it('exposes a named action dialog and restores the version trigger after Escape', async () => {
    render(
      <VersionSelector
        activeVersion="v1.0.0"
        installedVersions={['v1.0.0', 'v1.1.0']}
        isLoading={false}
        onOpenVersionManager={vi.fn()}
        openActiveInstall={vi.fn().mockResolvedValue(true)}
        switchVersion={vi.fn().mockResolvedValue(true)}
      />
    );

    const trigger = screen.getByRole('button', { name: 'v1.0.0' });
    trigger.focus();
    fireEvent.click(trigger);
    const popup = screen.getByRole('dialog', { name: 'Version actions' });
    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    expect(trigger).toHaveAttribute('aria-controls', popup.id);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Switch to v1.0.0' })).toHaveFocus();
    });

    fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: 'Version actions' })).not.toBeInTheDocument();
    });
    expect(trigger).toHaveAttribute('aria-expanded', 'false');
    expect(trigger).toHaveFocus();
  });
});
