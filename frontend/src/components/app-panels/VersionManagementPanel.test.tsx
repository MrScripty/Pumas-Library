import { useState } from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppVersionState } from '../../utils/appVersionState';
import type { TorchStartupTrialOutcome } from '../../types/torch-install';
import { UNSUPPORTED_VERSION_STATE } from '../../utils/appVersionState';
import { VersionManagementPanel } from './VersionManagementPanel';
import { createTorchTrialLifecycleStore } from '../TorchTrialLifecycleStore';

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
  const { TorchStartupTrial } = await import('../TorchStartupTrial');
  return {
    TorchRuntimeProbePanel: ({ tag, trialStore }: {
      tag: string;
      trialStore?: ReturnType<typeof createTorchTrialLifecycleStore>;
    }) => {
      useEffect(() => {
        mockGetTorchRuntimeProbe(tag);
      }, [tag]);
      return <div role="region" aria-label={`Torch probe results for ${tag}`}>
        {trialStore && <TorchStartupTrial tag={tag} store={trialStore} />}
      </div>;
    },
  };
});

function Harness({
  isLoading,
  refreshAll,
  installedVersions = [],
  activeVersion = null,
  trialStore,
}: {
  isLoading: boolean;
  refreshAll: AppVersionState['refreshAll'];
  installedVersions?: string[];
  activeVersion?: string | null;
  trialStore?: ReturnType<typeof createTorchTrialLifecycleStore>;
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
      trialStore={trialStore}
    />
  );
}

beforeEach(() => mockGetTorchRuntimeProbe.mockClear());
afterEach(() => vi.unstubAllGlobals());

describe('VersionManagementPanel refresh lifecycle', () => {
  it('keeps an in-flight trial serialized across complete version panel remounts', async () => {
    let finishTrial!: (value: TorchStartupTrialOutcome) => void;
    const trial = vi.fn(() => new Promise<TorchStartupTrialOutcome>((resolve) => { finishTrial = resolve; }));
    const stop = vi.fn().mockResolvedValue({ success: true, stopped: true });
    const trialStore = createTorchTrialLifecycleStore({ trial_torch_runtime: trial, stop_runtime_profile_if_generation: stop });
    vi.stubGlobal('electronAPI', { get_runtime_profiles_snapshot: vi.fn().mockResolvedValue({
      success: true, snapshot: { profiles: [{
        profile_id: 'managed-torch', name: 'Managed Torch', provider: 'torch',
        provider_mode: 'torch_serve', management_mode: 'managed', enabled: true,
      }] },
    }) });
    const props = {
      isLoading: false, refreshAll: vi.fn(async () => undefined),
      installedVersions: ['v2.10.0'], activeVersion: 'v2.10.0', trialStore,
    };
    const panel = render(<Harness {...props} />);
    fireEvent.click(screen.getByRole('button', { name: 'Inspect active Torch runtime' }));
    fireEvent.change(await screen.findByRole('combobox', { name: 'Managed Torch profile' }), { target: { value: 'managed-torch' } });
    fireEvent.click(screen.getByRole('button', { name: 'Trial startup' }));
    expect(trial).toHaveBeenCalledTimes(1);

    panel.unmount();
    render(<Harness {...props} />);
    fireEvent.click(screen.getByRole('button', { name: 'Inspect active Torch runtime' }));
    expect(screen.getByRole('button', { name: 'Starting trial…' })).toBeDisabled();
    await act(async () => {
      finishTrial({
        success: true, tag: 'v2.10.0', profileId: 'managed-torch', startupStatus: 'passed',
        healthStatus: 'passed', protocol: 3, capabilities: [], generation: 'generation-42',
        startedByTrial: true, cleanup: 'not_needed',
      });
    });

    expect(await screen.findByRole('button', { name: 'Stop trial profile' })).toBeEnabled();
    fireEvent.click(screen.getByRole('button', { name: 'Stop trial profile' }));
    await waitFor(() => expect(stop).toHaveBeenCalledWith('managed-torch', 'generation-42'));
    expect(trial).toHaveBeenCalledTimes(1);
  });

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
