import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TorchStartupTrial } from './TorchStartupTrial';

const snapshot = vi.fn<() => Promise<unknown>>();
const trial = vi.fn<(tag: string, profileId: string) => Promise<unknown>>();
const conditionalStop = vi.fn<(profileId: string, generation: string) => Promise<unknown>>();
const unconditionalStop = vi.fn<(profileId: string) => Promise<unknown>>();
vi.mock('../api/adapter', () => ({
  api: {
    get_runtime_profiles_snapshot: () => snapshot(),
    trial_torch_runtime: (tag: string, profileId: string) => trial(tag, profileId),
    stop_runtime_profile_if_generation: (profileId: string, generation: string) => conditionalStop(profileId, generation),
    stop_runtime_profile: (profileId: string) => unconditionalStop(profileId),
  },
}));

const managedProfile = {
  profile_id: 'torch-managed', provider: 'torch', provider_mode: 'torch_serve', management_mode: 'managed', enabled: true, name: 'Local Torch',
};

beforeEach(() => {
  vi.clearAllMocks();
  snapshot.mockResolvedValue({
    success: true, snapshot: { profiles: [
      managedProfile,
      { ...managedProfile, profile_id: 'external', management_mode: 'external', name: 'External Torch' },
      { ...managedProfile, profile_id: 'disabled', enabled: false, name: 'Disabled Torch' },
    ] },
  });
});

describe('TorchStartupTrial', () => {
  it('requires managed profile selection and conditionally stops only the launched generation', async () => {
    trial.mockResolvedValue({
      success: true, tag: 'v2.10.0', profileId: 'torch-managed', startupStatus: 'passed',
      healthStatus: 'passed', protocol: 3, capabilities: ['torch_serve'], generation: '42',
      startedByTrial: true, cleanup: 'not_needed',
    });
    conditionalStop.mockResolvedValue({ success: true, stopped: true });
    render(<TorchStartupTrial tag="v2.10.0" />);

    const start = screen.getByRole('button', { name: 'Trial startup' });
    expect(start).toBeDisabled();
    const select = await screen.findByRole('combobox', { name: 'Managed Torch profile' });
    await waitFor(() => expect(screen.getByRole('option', { name: 'Local Torch' })).toBeInTheDocument());
    expect(screen.queryByRole('option', { name: 'External Torch' })).not.toBeInTheDocument();
    expect(screen.queryByRole('option', { name: 'Disabled Torch' })).not.toBeInTheDocument();
    fireEvent.change(select, { target: { value: 'torch-managed' } });
    fireEvent.click(start);

    expect(await screen.findByText(/Startup trial passed · Health: passed · Protocol: 3/)).toBeInTheDocument();
    expect(trial).toHaveBeenCalledWith('v2.10.0', 'torch-managed');
    expect(screen.getByText(/Image generation and GPU model inference are not tested/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Stop trial profile' }));
    await waitFor(() => expect(screen.getByText('Trial profile stopped.')).toBeInTheDocument());
    expect(conditionalStop).toHaveBeenCalledWith('torch-managed', '42');
    expect(unconditionalStop).not.toHaveBeenCalled();
  });

  it('does not show Stop when the backend already stopped the owned generation', async () => {
    trial.mockResolvedValue({
      success: false, tag: 'v2.10.0', profileId: 'torch-managed', startupStatus: 'failed',
      healthStatus: 'failed', protocol: null, capabilities: [], generation: '43',
      startedByTrial: true, cleanup: 'stopped_owned_generation', error: 'Health check failed',
    });
    render(<TorchStartupTrial tag="v2.10.0" />);
    fireEvent.change(await screen.findByRole('combobox', { name: 'Managed Torch profile' }), { target: { value: 'torch-managed' } });
    fireEvent.click(screen.getByRole('button', { name: 'Trial startup' }));
    expect(await screen.findByText(/Startup trial failed/)).toBeInTheDocument();
    expect(screen.getByText('Health check failed')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Stop trial profile' })).not.toBeInTheDocument();
  });
});
