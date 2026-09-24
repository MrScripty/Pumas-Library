import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { TorchInstallPreview } from './TorchInstallPreview';
import type { TorchRuntimeOptions, TorchRuntimePreview, TorchRuntimePreviewRequest } from '../types/torch-install';

const getOptions = vi.fn<() => Promise<TorchRuntimeOptions>>();
const getPreview = vi.fn<(request: TorchRuntimePreviewRequest) => Promise<TorchRuntimePreview>>();

vi.mock('../api/adapter', () => ({
  api: {
    get_torch_runtime_options: () => getOptions(),
    preview_torch_runtime: (request: TorchRuntimePreviewRequest) => getPreview(request),
  },
}));

const options = {
  builds: ['cpu', 'cu130'],
  pythons: [{ id: 'python3.12', label: 'Python 3.12 at /usr/bin/python3.12' }],
  adapters: ['none', 'flux2'],
  installed: [],
  preset: { tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'bundled' },
};

describe('TorchInstallPreview', () => {
  it('shows exact artifacts and installs only the reviewed selection', async () => {
    getOptions.mockResolvedValue(options);
    getPreview.mockResolvedValue({
      previewId: 'review-1', tag: 'v2.10.0', build: 'cpu', python: 'python3.12',
      adapter: 'none', qualification: 'unverified',
      artifacts: [{ name: 'torch', version: '2.10.0', url: 'https://download.pytorch.org/whl/cpu/torch.whl', sha256: 'abc123' }],
    });
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.10.0" onBack={vi.fn()} onInstall={onInstall} />);

    const install = screen.getByRole('button', { name: 'Install reviewed artifacts' });
    expect(install).toBeDisabled();
    fireEvent.click(await screen.findByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(screen.getByText('SHA-256: abc123')).toBeInTheDocument());
    expect(getPreview).toHaveBeenCalledWith({ tag: 'v2.10.0', build: 'cpu', python: 'python3.12', adapter: 'none' });
    expect(screen.getByText('URL: https://download.pytorch.org/whl/cpu/torch.whl')).toBeInTheDocument();
    expect(install).toBeEnabled();

    fireEvent.change(screen.getByRole('combobox', { name: 'Torch build' }), { target: { value: 'cu130' } });
    expect(install).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(install).toBeDisabled());
    expect(onInstall).not.toHaveBeenCalled();
  });

  it('locks the legacy preset and keeps an inconclusive probe from installing', async () => {
    getOptions.mockResolvedValue(options);
    getPreview.mockRejectedValue(new Error('Network inconclusive: index unreachable'));
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={onInstall} />);

    expect(await screen.findByRole('combobox', { name: 'Torch build' })).toBeDisabled();
    expect(screen.getByRole('combobox', { name: 'Installed Python' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Network inconclusive');
    expect(screen.getByRole('button', { name: 'Install fixed preset' })).toBeDisabled();
    expect(onInstall).not.toHaveBeenCalled();
  });

  it('keeps a long artifact list and its install action inside the scrollable preview region', async () => {
    getOptions.mockResolvedValue(options);
    getPreview.mockResolvedValue({
      previewId: 'review-many', tag: 'v2.10.0', build: 'cpu', python: 'python3.12',
      adapter: 'none', qualification: 'unverified',
      artifacts: Array.from({ length: 30 }, (_, index) => ({
        name: `wheel-${index}`, version: '1.0',
        url: `https://download.pytorch.org/whl/cpu/wheel-${index}.whl`,
        sha256: 'a'.repeat(64),
      })),
    });
    render(<TorchInstallPreview tag="v2.10.0" onBack={vi.fn()} onInstall={vi.fn()} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Check selected combination' }));
    const previewRegion = screen.getByRole('region', { name: 'Torch installation preview for v2.10.0' });
    await waitFor(() => expect(screen.getByText('wheel-29 1.0')).toBeInTheDocument());
    expect(previewRegion).toHaveClass('flex-1', 'min-h-0', 'overflow-y-auto');
    expect(previewRegion).toContainElement(screen.getByRole('button', { name: 'Install reviewed artifacts' }));
  });
});
