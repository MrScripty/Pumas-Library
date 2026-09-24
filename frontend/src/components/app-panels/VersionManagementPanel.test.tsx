import { useState } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { AppVersionState } from '../../utils/appVersionState';
import { UNSUPPORTED_VERSION_STATE } from '../../utils/appVersionState';
import { VersionManagementPanel } from './VersionManagementPanel';

const { mockGetTorchRuntimeProbe } = vi.hoisted(() => ({ mockGetTorchRuntimeProbe: vi.fn() }));

vi.mock('../VersionSelector', () => ({
  VersionSelector: ({ onOpenVersionManager }: { onOpenVersionManager: () => void }) => (
    <button type="button" onClick={onOpenVersionManager}>Open versions</button>
  ),
}));

vi.mock('../InstallDialog', () => ({
  InstallDialog: ({
    isLoading,
    onClose,
  }: {
    isLoading: boolean;
    onClose: () => void;
  }) => (
    <div>
      <span>Manager loading: {String(isLoading)}</span>
      <button type="button" onClick={onClose}>Close manager</button>
    </div>
  ),
}));

vi.mock('../TorchRuntimeProbePanel', async () => {
  const { useEffect } = await import('react');
  return {
    TorchRuntimeProbePanel: ({ tag }: { tag: string }) => {
      useEffect(() => {
        mockGetTorchRuntimeProbe(tag);
      }, [tag]);
      return <div role="region" aria-label={`Torch probe results for ${tag}`} />;
    },
  };
});

function Harness({
  isLoading,
  refreshAll,
  installedVersions = [],
  activeVersion = null,
}: {
  isLoading: boolean;
  refreshAll: AppVersionState['refreshAll'];
  installedVersions?: string[];
  activeVersion?: string | null;
}) {
  const [showManager, setShowManager] = useState(false);
  const versions: AppVersionState = {
    ...UNSUPPORTED_VERSION_STATE,
    appId: 'torch',
    isSupported: true,
    isLoading,
    refreshAll,
    installedVersions,
    activeVersion,
  };

  return (
    <VersionManagementPanel
      appDisplayName="Torch"
      versions={versions}
      showManager={showManager}
      onShowManager={setShowManager}
    />
  );
}

describe('VersionManagementPanel refresh lifecycle', () => {
  it('requires explicit inspection of an active Torch version again after returning from the manager', async () => {
    const refreshAll = vi.fn(async () => undefined);
    const panel = render(<Harness isLoading={false} refreshAll={refreshAll} installedVersions={['v2.10.0']} activeVersion="v2.10.0" />);

    expect(screen.queryByRole('region', { name: 'Torch probe results for v2.10.0' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Inspect active Torch runtime' }));
    expect(screen.getByRole('region', { name: 'Torch probe results for v2.10.0' })).toBeInTheDocument();
    expect(mockGetTorchRuntimeProbe).toHaveBeenCalledTimes(1);

    panel.rerender(<Harness isLoading={false} refreshAll={refreshAll} installedVersions={['v2.9.0', 'v2.10.0']} activeVersion="v2.9.0" />);
    expect(screen.queryByRole('region', { name: 'Torch probe results for v2.10.0' })).not.toBeInTheDocument();
    panel.rerender(<Harness isLoading={false} refreshAll={refreshAll} installedVersions={['v2.9.0', 'v2.10.0']} activeVersion="v2.10.0" />);
    expect(screen.queryByRole('region', { name: 'Torch probe results for v2.10.0' })).not.toBeInTheDocument();
    expect(mockGetTorchRuntimeProbe).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole('button', { name: 'Open versions' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close manager' }));
    expect(screen.queryByRole('region', { name: 'Torch probe results for v2.10.0' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Inspect active Torch runtime' }));
    expect(screen.getByRole('region', { name: 'Torch probe results for v2.10.0' })).toBeInTheDocument();
    expect(mockGetTorchRuntimeProbe).toHaveBeenCalledTimes(2);

    panel.rerender(<Harness isLoading={false} refreshAll={refreshAll} installedVersions={['v2.10.0']} activeVersion={null} />);
    expect(screen.queryByRole('button', { name: 'Inspect active Torch runtime' })).not.toBeInTheDocument();

    panel.rerender(<Harness isLoading={false} refreshAll={refreshAll} installedVersions={['v2.10.0']} activeVersion="v2.10.0" />);
    expect(screen.queryByRole('region', { name: 'Torch probe results for v2.10.0' })).not.toBeInTheDocument();
  });

  it('waits for initial loading and keeps the manager pending until its forced refresh finishes', async () => {
    let finishRefresh!: () => void;
    const refreshAll = vi.fn(() => new Promise<void>((resolve) => {
      finishRefresh = resolve;
    }));
    const panel = render(<Harness isLoading refreshAll={refreshAll} />);

    fireEvent.click(screen.getByRole('button', { name: 'Open versions' }));

    expect(refreshAll).not.toHaveBeenCalled();
    expect(screen.getByText('Manager loading: true')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeDisabled();

    panel.rerender(<Harness isLoading={false} refreshAll={refreshAll} />);

    await waitFor(() => expect(refreshAll).toHaveBeenCalledWith(true));
    expect(refreshAll).toHaveBeenCalledTimes(1);
    expect(screen.getByText('Manager loading: true')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Refresh' })).toBeDisabled();

    finishRefresh();

    await waitFor(() => {
      expect(screen.getByText('Manager loading: false')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Refresh' })).toBeEnabled();
    });
  });

  it('cancels a queued refresh when closed and schedules a new one when reopened', async () => {
    const refreshAll = vi.fn(async () => undefined);
    const panel = render(<Harness isLoading refreshAll={refreshAll} />);

    fireEvent.click(screen.getByRole('button', { name: 'Open versions' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close manager' }));
    panel.rerender(<Harness isLoading={false} refreshAll={refreshAll} />);

    expect(refreshAll).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Open versions' }));

    await waitFor(() => expect(refreshAll).toHaveBeenCalledTimes(1));
    expect(refreshAll).toHaveBeenCalledWith(true);
  });

  it('recovers from a rejected automatic refresh and allows a manual retry', async () => {
    const refreshAll = vi.fn()
      .mockRejectedValueOnce(new Error('catalog unavailable'))
      .mockResolvedValueOnce(undefined);

    render(<Harness isLoading={false} refreshAll={refreshAll} />);
    fireEvent.click(screen.getByRole('button', { name: 'Open versions' }));

    await waitFor(() => {
      expect(refreshAll).toHaveBeenCalledTimes(1);
      expect(screen.getByText('Manager loading: false')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));

    await waitFor(() => expect(refreshAll).toHaveBeenCalledTimes(2));
    expect(refreshAll).toHaveBeenLastCalledWith(true);
  });
});
