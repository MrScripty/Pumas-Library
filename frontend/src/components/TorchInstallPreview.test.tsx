import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { TorchInstallPreview } from './TorchInstallPreview';
import type { TorchAlternativesOutcome, TorchRuntimeOptions, TorchRuntimePreview, TorchRuntimePreviewRequest } from '../types/torch-install';

const getOptions = vi.fn<() => Promise<TorchRuntimeOptions>>();
const getPreview = vi.fn<(request: TorchRuntimePreviewRequest) => Promise<TorchRuntimePreview>>();
const getAlternatives = vi.fn<(tag: string, build: string, python: string) => Promise<TorchAlternativesOutcome>>();

vi.mock('../api/adapter', () => ({
  api: {
    get_torch_runtime_options: () => getOptions(),
    preview_torch_runtime: (request: TorchRuntimePreviewRequest) => getPreview(request),
    find_torch_alternatives: (tag: string, build: string, python: string) => getAlternatives(tag, build, python),
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
    expect(screen.getByText(/does not install Python/)).toBeInTheDocument();
    expect(screen.getByText(/Source compilation is a separate unsupported path/)).toBeInTheDocument();
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
    expect(screen.queryByRole('button', { name: 'Find compatible alternatives' })).not.toBeInTheDocument();
    expect(onInstall).not.toHaveBeenCalled();
  });

  it('requires an already installed compatible Python before checking wheels', async () => {
    getOptions.mockResolvedValue({ ...options, pythons: [] });
    render(<TorchInstallPreview tag="v2.10.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    expect(await screen.findByText(/Install CPython 3.10–3.13 for Linux x86_64/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Check selected combination' })).toBeDisabled();
  });

  it('offers official wheel matches only after unsupported preview and rechecks a chosen candidate', async () => {
    getOptions.mockResolvedValue(options);
    getPreview.mockRejectedValueOnce(new Error('Unsupported combination: no matching wheel'));
    getPreview.mockResolvedValueOnce({
      previewId: 'review-cu130', tag: 'v2.10.0', build: 'cu130', python: 'python3.12',
      adapter: 'none', qualification: 'unverified', artifacts: [],
    });
    getAlternatives.mockResolvedValue({
      selectedTag: 'v2.10.0', selectedBuild: 'cpu', selectedPython: 'python3.12',
      status: 'matches', incomplete: true, dependenciesNotChecked: true,
      checkedBuilds: ['cpu', 'cu130'], issues: [],
      matches: [{ tag: 'v2.10.0', build: 'cu130', python: 'python3.12', wheelUrl: 'https://download.pytorch.org/torch.whl', sha256: 'a'.repeat(64) }],
    });
    render(<TorchInstallPreview tag="v2.10.0" onBack={vi.fn()} onInstall={vi.fn()} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Check selected combination' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Find compatible alternatives' }));
    expect(await screen.findByText('Official Torch wheel matches only')).toBeInTheDocument();
    expect(screen.getByText(/Other dependencies have not been checked/)).toBeInTheDocument();
    expect(getAlternatives).toHaveBeenCalledWith('v2.10.0', 'cpu', 'python3.12');
    fireEvent.click(screen.getByRole('button', { name: 'Preview v2.10.0 · cu130 · python3.12' }));
    await waitFor(() => expect(getPreview).toHaveBeenLastCalledWith({ tag: 'v2.10.0', build: 'cu130', python: 'python3.12', adapter: 'none' }));
    expect(await screen.findByText('Exact artifacts resolved')).toBeInTheDocument();
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
