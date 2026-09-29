import { useState } from 'react';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { TorchRuntimeProbePanel } from './TorchRuntimeProbePanel';
import { createTorchTrialLifecycleStore } from './TorchTrialLifecycleStore';
import type { TorchRuntimeOptions, TorchRuntimeProbeReport, TorchStartupTrialOutcome } from '../types/torch-install';

const getProbe = vi.fn<(tag: string) => Promise<TorchRuntimeProbeReport>>();
const getOptions = vi.fn<() => Promise<TorchRuntimeOptions>>();
const trial = vi.fn<(tag: string, profileId: string) => Promise<TorchStartupTrialOutcome>>();
const conditionalStop = vi.fn<(profileId: string, generation: string) => Promise<{ success: boolean; stopped: boolean }>>();
const unconditionalStop = vi.fn<(profileId: string) => Promise<unknown>>();
const getProfiles = vi.fn<() => Promise<unknown>>().mockResolvedValue({ success: true, snapshot: { profiles: [] } });
const baseOptions: TorchRuntimeOptions = {
  builds: ['cpu'], pythons: [], adapters: ['none'],
  bundledPresetAvailable: false, defaultAdapter: 'none',
  preset: { tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'bundled' },
  installed: [],
};
vi.mock('../api/adapter', () => ({
  api: {
    get_torch_runtime_probe: (tag: string) => getProbe(tag),
    get_torch_runtime_options: () => getOptions(),
    get_runtime_profiles_snapshot: () => getProfiles(),
    trial_torch_runtime: (tag: string, profileId: string) => trial(tag, profileId),
    stop_runtime_profile_if_generation: (profileId: string, generation: string) => conditionalStop(profileId, generation),
    stop_runtime_profile: (profileId: string) => unconditionalStop(profileId),
  },
}));

describe('TorchRuntimeProbePanel', () => {
  it('attributes an earlier trial to its release while a different release is inspected', async () => {
    getOptions.mockResolvedValue(baseOptions);
    getProbe.mockResolvedValue({
      status: 'passed', core_status: 'passed', adapter_status: 'not selected', capabilities: {},
    });
    getProfiles.mockResolvedValue({ success: true, snapshot: { profiles: [{
      profile_id: 'managed-torch', name: 'Managed Torch', provider: 'torch',
      provider_mode: 'torch_serve', management_mode: 'managed', enabled: true,
    }] } });
    let finishTrial!: (outcome: TorchStartupTrialOutcome) => void;
    trial.mockImplementation(() => new Promise<TorchStartupTrialOutcome>((resolve) => { finishTrial = resolve; }));
    conditionalStop.mockResolvedValue({ success: true, stopped: true });
    const store = createTorchTrialLifecycleStore();

    function Harness() {
      const [tag, setTag] = useState('v2.10.0');
      return <>
        <button type="button" onClick={() => setTag((current) => current === 'v2.10.0' ? 'v2.11.0' : 'v2.10.0')}>Switch inspected release</button>
        <TorchRuntimeProbePanel tag={tag} trialStore={store} />
      </>;
    }

    render(<Harness />);
    fireEvent.change(await screen.findByRole('combobox', { name: 'Managed Torch profile' }), { target: { value: 'managed-torch' } });
    fireEvent.click(screen.getByRole('button', { name: 'Trial startup' }));
    fireEvent.click(screen.getByRole('button', { name: 'Switch inspected release' }));
    expect(screen.getByRole('button', { name: 'Starting trial for v2.10.0…' })).toBeDisabled();

    await act(async () => {
      finishTrial({
        success: true, tag: 'v2.10.0', profileId: 'managed-torch', startupStatus: 'passed',
        healthStatus: 'passed', protocol: 3, capabilities: [], generation: 'generation-42',
        startedByTrial: true, cleanup: 'not_needed',
      });
    });
    expect(screen.getByText(/trial profile for v2.10.0 is still running/)).toBeInTheDocument();
    expect(screen.queryByText(/Startup trial passed/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Stop trial profile' }));
    await waitFor(() => expect(store.getSnapshot().stopResult).toBe('stopped'));
    expect(conditionalStop).toHaveBeenCalledWith('managed-torch', 'generation-42');
    expect(screen.queryByText('Trial profile stopped.')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Switch inspected release' }));
    expect(screen.getByText(/Startup trial passed · Health: passed · Protocol: 3/)).toBeInTheDocument();
    expect(await screen.findByText('Trial profile stopped.')).toBeInTheDocument();
  });

  it('keeps a successful trial stop available after the probe panel remounts', async () => {
    getOptions.mockResolvedValue(baseOptions);
    getProbe.mockResolvedValue({
      status: 'passed', core_status: 'passed', adapter_status: 'not selected', capabilities: {},
    });
    getProfiles.mockResolvedValue({ success: true, snapshot: { profiles: [{
      profile_id: 'managed-torch', name: 'Managed Torch', provider: 'torch',
      provider_mode: 'torch_serve', management_mode: 'managed', enabled: true,
    }] } });
    trial.mockResolvedValue({
      success: true, tag: 'v2.10.0', profileId: 'managed-torch', startupStatus: 'passed',
      healthStatus: 'passed', protocol: 3, capabilities: [], generation: 'generation-42',
      startedByTrial: true, cleanup: 'not_needed',
    });
    conditionalStop.mockResolvedValue({ success: true, stopped: true });
    const store = createTorchTrialLifecycleStore();

    function Harness() {
      const [visible, setVisible] = useState(true);
      return <>
        <button type="button" onClick={() => setVisible((current) => !current)}>Toggle probe</button>
        {visible && <TorchRuntimeProbePanel tag="v2.10.0" trialStore={store} />}
      </>;
    }

    render(<Harness />);
    fireEvent.change(await screen.findByRole('combobox', { name: 'Managed Torch profile' }), { target: { value: 'managed-torch' } });
    fireEvent.click(screen.getByRole('button', { name: 'Trial startup' }));
    expect(await screen.findByText(/Startup trial passed/)).toBeInTheDocument();
    expect(trial).toHaveBeenCalledWith('v2.10.0', 'managed-torch');

    fireEvent.click(screen.getByRole('button', { name: 'Toggle probe' }));
    expect(screen.queryByRole('button', { name: 'Stop trial profile' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Toggle probe' }));
    expect(screen.getByRole('button', { name: 'Trial startup' })).toBeDisabled();
    expect(screen.getByText(/trial profile for v2.10.0 is still running/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Stop trial profile' }));

    await waitFor(() => expect(screen.getByText('Trial profile stopped.')).toBeInTheDocument());
    expect(conditionalStop).toHaveBeenCalledWith('managed-torch', 'generation-42');
    expect(unconditionalStop).not.toHaveBeenCalled();
  });

  it('distinguishes a passing core from an unavailable selected adapter and untested startup', async () => {
    getOptions.mockResolvedValue({ ...baseOptions, installed: [{ tag: 'v2.10.0', build: 'cpu', python: 'python3.12', adapter: 'flux2', qualification: 'unverified' }] });
    getProbe.mockResolvedValue({
      status: 'partial', core_status: 'passed', adapter_status: 'unavailable',
      recorded_at: '2026-09-23T12:00:00Z', stale: true, staleReasons: ['sidecar changed'],
      capabilities: {
        torch_import: { status: 'passed' },
        flux2_klein: { status: 'unavailable', scope: 'adapter imports', error: 'Pipeline symbol missing' },
        sidecar_startup: { status: 'not tested', scope: 'socket startup requires an explicit trial' },
      },
    });
    render(<TorchRuntimeProbePanel tag="v2.10.0" />);

    expect(await screen.findByText('Partial support; inspect the adapter checks below.', { exact: false })).toBeInTheDocument();
    expect(screen.getByText('Pipeline symbol missing')).toBeInTheDocument();
    expect(await screen.findByText('Installed: cpu · python3.12 · flux2 · unverified')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('Saved checks are stale. sidecar changed');
    expect(screen.getByText(/Socket startup was not tested during installation/)).toBeInTheDocument();
    expect(getProbe).toHaveBeenCalledWith('v2.10.0');
  });

  it('shows a saved startup failure distinctly from installed state', async () => {
    getOptions.mockResolvedValue(baseOptions);
    getProbe.mockResolvedValue({
      status: 'passed', core_status: 'passed', adapter_status: 'not selected',
      capabilities: { sidecar_startup: { status: 'failed', error: 'Socket bind failed' } },
    });
    render(<TorchRuntimeProbePanel tag="v2.9.1" />);
    expect(await screen.findByRole('alert')).toHaveTextContent('Startup failed: Socket bind failed');
  });
});
