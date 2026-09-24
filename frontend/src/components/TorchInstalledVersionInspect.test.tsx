import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { InstallDialog } from './InstallDialog';

afterEach(() => vi.unstubAllGlobals());

describe('installed Torch version review', () => {
  it('shows each saved build and inspects a nonactive version without selecting it', async () => {
    const probe = vi.fn().mockResolvedValue({
      status: 'partial', core_status: 'passed', adapter_status: 'unavailable',
      capabilities: { sidecar_startup: { status: 'not tested' } },
    });
    const switchVersion = vi.fn();
    vi.stubGlobal('electronAPI', {
      get_torch_runtime_options: vi.fn().mockResolvedValue({
        builds: ['cpu', 'cu130'], pythons: [], adapters: ['none', 'flux2'],
        preset: { tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'bundled' },
        installed: [
          { tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'bundled', qualification: 'qualified' },
          { tag: 'v2.10.0', build: 'cpu', python: 'python3.12', adapter: 'flux2', qualification: 'unverified' },
        ],
      }),
      get_torch_runtime_probe: probe,
      switch_version: switchVersion,
    });

    render(<InstallDialog
      isOpen={true} onClose={vi.fn()} appId="torch" availableVersions={[]}
      installedVersions={['v2.9.1', 'v2.10.0']} isLoading={false}
      onInstallVersion={vi.fn().mockResolvedValue(true)}
      onCancelInstallation={vi.fn().mockResolvedValue(true)}
      onRefreshAll={vi.fn().mockResolvedValue(undefined)}
      onRemoveVersion={vi.fn().mockResolvedValue(true)}
    />);

    await waitFor(() => expect(screen.getByText('Installed cpu · python3.12 · flux2 · unverified')).toBeInTheDocument());
    expect(screen.getByText('Installed cu130 · python3.12 · bundled · qualified')).toBeInTheDocument();
    const row = screen.getByRole('heading', { name: '2.10.0' }).closest('div.w-full');
    if (!(row instanceof HTMLElement)) throw new TypeError('Expected installed version row');
    fireEvent.click(within(row).getByRole('button', { name: 'Inspect saved checks' }));

    expect(await screen.findByRole('region', { name: 'Torch probe results for v2.10.0' })).toBeInTheDocument();
    expect(probe).toHaveBeenCalledWith('v2.10.0');
    expect(switchVersion).not.toHaveBeenCalled();
  });
});
