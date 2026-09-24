import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { TorchInstallPreview } from './TorchInstallPreview';
import type { TorchAlternativesOutcome, TorchRuntimeOptions, TorchRuntimePreview, TorchRuntimePreviewOutcome, TorchRuntimePreviewRequest } from '../types/torch-install';

const getOptions = vi.fn<() => Promise<TorchRuntimeOptions>>();
const getPreview = vi.fn<(request: TorchRuntimePreviewRequest) => Promise<TorchRuntimePreviewOutcome>>();
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

const resolved = (preview: TorchRuntimePreview): TorchRuntimePreviewOutcome => ({ status: 'resolved', preview });

describe('TorchInstallPreview', () => {
  it('shows exact artifacts and installs only the reviewed selection', async () => {
    getOptions.mockResolvedValue(options);
    getPreview.mockResolvedValue(resolved({
      previewId: 'review-1', expiresInSeconds: 300, tag: 'v2.10.0', build: 'cpu', python: 'python3.12',
      adapter: 'none', qualification: 'unverified',
      artifacts: [{ name: 'torch', version: '2.10.0', url: 'https://download.pytorch.org/whl/cpu/torch.whl', sha256: 'abc123' }],
    }));
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
    getPreview.mockResolvedValue({ status: 'rejected', reason: 'network_inconclusive', message: 'index unreachable' });
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

  it('offers the qualified preset and an unverified exact-wheel choice for the same release', async () => {
    getOptions.mockResolvedValue(options);
    getPreview.mockImplementation(async (request) => resolved({
      previewId: request.adapter === 'bundled' ? 'qualified-preset' : 'upstream-cpu',
      expiresInSeconds: 300,
      ...request,
      qualification: request.adapter === 'bundled' ? 'qualified' : 'unverified',
      artifacts: [],
    }));
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={onInstall} />);

    const presetChoice = await screen.findByRole('button', { name: 'Qualified fixed preset' });
    expect(presetChoice).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('combobox', { name: 'Torch build' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(getPreview).toHaveBeenCalledWith({
      tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'bundled',
    }));
    fireEvent.click(screen.getByRole('button', { name: 'Install fixed preset' }));
    expect(onInstall).toHaveBeenCalledWith('qualified-preset');

    fireEvent.click(screen.getByRole('button', { name: 'Other official wheels (unverified)' }));
    expect(screen.getByRole('button', { name: 'Install reviewed artifacts' })).toBeDisabled();
    expect(screen.getByRole('combobox', { name: 'Torch build' })).toBeEnabled();
    expect(screen.getByRole('combobox', { name: 'Image adapter' })).toBeEnabled();
    expect(screen.getByRole('combobox', { name: 'Image adapter' })).toHaveValue('none');
    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(getPreview).toHaveBeenLastCalledWith({
      tag: 'v2.9.1', build: 'cpu', python: 'python3.12', adapter: 'none',
    }));
    expect(screen.getByText(/Artifact resolution is unverified/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Install reviewed artifacts' }));
    expect(onInstall).toHaveBeenLastCalledWith('upstream-cpu');
  });

  it('requires an already installed compatible Python before checking wheels', async () => {
    getOptions.mockResolvedValue({ ...options, pythons: [] });
    render(<TorchInstallPreview tag="v2.10.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    expect(await screen.findByText(/Install CPython 3.10–3.13 for Linux x86_64/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Check selected combination' })).toBeDisabled();
  });

  it('reports a failed wheel report validation distinctly without offering alternatives', async () => {
    getOptions.mockResolvedValue(options);
    getPreview.mockResolvedValue({ status: 'rejected', reason: 'validation_failed', message: 'Unsupported combination: hash missing for torch' });
    render(<TorchInstallPreview tag="v2.10.0" onBack={vi.fn()} onInstall={vi.fn()} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Check selected combination' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Resolved wheel report failed validation: Unsupported combination: hash missing for torch');
    expect(screen.getByText(/cpu · python3.12 · none: failed validation/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Install reviewed artifacts' })).toBeDisabled();
    expect(screen.queryByRole('button', { name: 'Find compatible alternatives' })).not.toBeInTheDocument();
    expect(screen.queryByText(/Probe inconclusive/)).not.toBeInTheDocument();
  });

  it('treats transport exceptions as inconclusive even when their text resembles an unsupported outcome', async () => {
    getOptions.mockResolvedValue(options);
    getPreview.mockRejectedValue(new Error('Unsupported combination: transport failed'));
    render(<TorchInstallPreview tag="v2.10.0" onBack={vi.fn()} onInstall={vi.fn()} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Check selected combination' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Probe inconclusive: Unsupported combination: transport failed');
    expect(screen.queryByRole('button', { name: 'Find compatible alternatives' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Install reviewed artifacts' })).toBeDisabled();
  });

  it('offers official wheel matches only after unsupported preview and rechecks a chosen candidate', async () => {
    getOptions.mockResolvedValue(options);
    getPreview.mockResolvedValueOnce({ status: 'rejected', reason: 'unsupported', message: 'no matching wheel' });
    getPreview.mockResolvedValueOnce(resolved({
      previewId: 'review-cu130', expiresInSeconds: 300, tag: 'v2.10.0', build: 'cu130', python: 'python3.12',
      adapter: 'none', qualification: 'unverified', artifacts: [],
    }));
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
    getPreview.mockResolvedValue(resolved({
      previewId: 'review-many', expiresInSeconds: 300, tag: 'v2.10.0', build: 'cpu', python: 'python3.12',
      adapter: 'none', qualification: 'unverified',
      artifacts: Array.from({ length: 30 }, (_, index) => ({
        name: `wheel-${index}`, version: '1.0',
        url: `https://download.pytorch.org/whl/cpu/wheel-${index}.whl`,
        sha256: 'a'.repeat(64),
      })),
    }));
    render(<TorchInstallPreview tag="v2.10.0" onBack={vi.fn()} onInstall={vi.fn()} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Check selected combination' }));
    const previewRegion = screen.getByRole('region', { name: 'Torch installation preview for v2.10.0' });
    await waitFor(() => expect(screen.getByText('wheel-29 1.0')).toBeInTheDocument());
    expect(previewRegion).toHaveClass('flex-1', 'min-h-0', 'overflow-y-auto');
    expect(previewRegion).toContainElement(screen.getByRole('button', { name: 'Install reviewed artifacts' }));
  });
});
