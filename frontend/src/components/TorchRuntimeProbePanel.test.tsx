import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { TorchRuntimeProbePanel } from './TorchRuntimeProbePanel';
import type { TorchRuntimeOptions, TorchRuntimeProbeReport } from '../types/torch-install';

const getProbe = vi.fn<(tag: string) => Promise<TorchRuntimeProbeReport>>();
const getOptions = vi.fn<() => Promise<TorchRuntimeOptions>>();
const baseOptions: TorchRuntimeOptions = {
  builds: ['cpu'], pythons: [], adapters: ['none'],
  preset: { tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'bundled' },
  installed: [],
};
vi.mock('../api/adapter', () => ({
  api: {
    get_torch_runtime_probe: (tag: string) => getProbe(tag),
    get_torch_runtime_options: () => getOptions(),
    get_runtime_profiles_snapshot: () => Promise.resolve({ success: true, snapshot: { profiles: [] } }),
  },
}));

describe('TorchRuntimeProbePanel', () => {
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
