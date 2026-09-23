import { useState } from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { AppVersionState } from '../../utils/appVersionState';
import { UNSUPPORTED_VERSION_STATE } from '../../utils/appVersionState';
import { VersionManagementPanel } from './VersionManagementPanel';

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

function Harness({
  isLoading,
  refreshAll,
}: {
  isLoading: boolean;
  refreshAll: AppVersionState['refreshAll'];
}) {
  const [showManager, setShowManager] = useState(false);
  const versions: AppVersionState = {
    ...UNSUPPORTED_VERSION_STATE,
    appId: 'torch',
    isSupported: true,
    isLoading,
    refreshAll,
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
