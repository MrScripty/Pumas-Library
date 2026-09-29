import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TorchInstallPreview } from './TorchInstallPreview';
import {
  decodeInstallationProgressOutcome,
  decodeTorchRuntimePreviewOutcome,
} from '../generated/desktop-contract';
import type { TorchReleaseOptionsOutcome, TorchRuntimeOptions, TorchRuntimePreviewOutcome, TorchRuntimePreviewRequest } from '../types/torch-install';

type ReadySelection = Extract<TorchRuntimePreviewOutcome, { status: 'ready' }>;

const getOptions = vi.fn<() => Promise<TorchRuntimeOptions>>();
const getReleaseOptions = vi.fn<(tag: string) => Promise<TorchReleaseOptionsOutcome>>();
const getSelection = vi.fn<(request: TorchRuntimePreviewRequest) => Promise<TorchRuntimePreviewOutcome>>();

vi.mock('../api/adapter', () => ({
  api: {
    get_torch_runtime_options: () => getOptions(),
    get_torch_release_options: (tag: string) => getReleaseOptions(tag),
    preview_torch_runtime: (request: TorchRuntimePreviewRequest) => getSelection(request),
  },
}));

const options: TorchRuntimeOptions = {
  builds: ['cpu', 'cu130'], defaultBuild: 'auto',
  pythons: [], adapters: ['none'], defaultAdapter: 'none',
  bundledPresetAvailable: false,
  preset: { tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'bundled' },
  installed: [],
};

function readySelection(request: TorchRuntimePreviewRequest): ReadySelection {
  return {
    status: 'ready',
    preview: {
      ...request, previewId: 'selection-token', expiresInSeconds: 300,
      qualification: 'unverified', artifacts: [],
    },
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  getOptions.mockResolvedValue(options);
  getSelection.mockImplementation(async (request) => readySelection(request));
});

describe('TorchInstallPreview', () => {
  it('accepts the ready token through the generated desktop RPC decoder', () => {
    expect(decodeTorchRuntimePreviewOutcome(readySelection({
      tag: 'v2.14.0', build: 'auto', python: 'auto', adapter: 'none',
    }))).toMatchObject({ status: 'valid', value: { status: 'ready' } });
  });

  it('accepts the resolving progress stage through the generated desktop RPC decoder', () => {
    expect(decodeInstallationProgressOutcome({
      tag: 'v2.14.0', startedAt: '2026-09-26T00:00:00Z', stage: 'resolving',
      stageProgress: 0, overallProgress: 0,
      currentItem: 'Preparing managed Python and resolving Torch packages',
      downloadSourceUrl: null, downloadActive: false, downloadMeasurementAvailable: null,
      downloadSpeed: null, etaSeconds: null, totalSize: null, downloadedBytes: 0,
      dependencyCount: null, completedDependencies: 0, completedItems: [],
      error: null, completedAt: null, success: null, logPath: null,
    })).toMatchObject({ status: 'valid', value: { stage: 'resolving' } });
  });

  it('loads local choices without release discovery or a separate package check', async () => {
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);

    expect(screen.getByRole('status')).toHaveTextContent('Loading Torch choices…');
    expect(screen.getByRole('button', { name: 'Install Torch' })).toBeDisabled();
    expect(screen.getByText(/Package and Python resolution, wheel downloads, and installation happen within the install task/)).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: 'Install Torch' })).toBeEnabled();
    expect(getReleaseOptions).not.toHaveBeenCalled();
    expect(getSelection).not.toHaveBeenCalled();
    expect(screen.queryByRole('button', { name: 'Check selected combination' })).not.toBeInTheDocument();
    expect(screen.getByText('Advanced setup').closest('details')).not.toHaveAttribute('open');
    expect(screen.queryByRole('radio', { name: 'FLUX.2 image generation' })).not.toBeInTheDocument();
    expect(screen.queryByText(/exact artifacts resolved/i)).not.toBeInTheDocument();
  });

  it.each(['inconclusive', 'none'] as const)('allows installation when release discovery would be %s', async (status) => {
    getReleaseOptions.mockResolvedValue({
      tag: 'v2.14.0', status, completeScan: false, checkedChannels: [], combinations: [],
      issues: [], detectedGpuVendors: [], driverStatus: { nvidia: 'not_present', amd: 'not_present' },
      recommended: null, recommendationNote: null,
    });
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Install Torch' }));
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getReleaseOptions).not.toHaveBeenCalled();
    expect(getSelection).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'auto', python: 'auto', adapter: 'none' });
  });

  it('offers FLUX.2 only when supported and keeps the current manager Core default', async () => {
    getOptions.mockResolvedValue({ ...options, adapters: ['none', 'flux2'] });
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    expect(await screen.findByRole('radio', { name: 'Core Torch' })).toBeChecked();
    expect(screen.getByRole('radio', { name: 'FLUX.2 image generation' })).not.toBeChecked();
    fireEvent.click(screen.getByRole('button', { name: 'Install Torch' }));
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getSelection).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'auto', python: 'auto', adapter: 'none' });
  });

  it('honors a supported manager default adapter and falls back to Core if it is unsupported', async () => {
    getOptions.mockResolvedValueOnce({ ...options, adapters: ['none', 'flux2'], defaultAdapter: 'flux2' });
    const onInstall = vi.fn();
    const { rerender } = render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    expect(await screen.findByRole('radio', { name: 'FLUX.2 image generation' })).toBeChecked();
    fireEvent.click(screen.getByRole('button', { name: 'Install Torch' }));
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getSelection).toHaveBeenLastCalledWith({ tag: 'v2.14.0', build: 'auto', python: 'auto', adapter: 'flux2' });

    getOptions.mockResolvedValueOnce({ ...options, defaultAdapter: 'flux2' });
    rerender(<TorchInstallPreview tag="v2.14.1" onBack={vi.fn()} onInstall={onInstall} />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Install Torch' })).toBeEnabled());
    expect(screen.queryByRole('radio', { name: 'FLUX.2 image generation' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Install Torch' }));
    await waitFor(() => expect(onInstall).toHaveBeenCalledTimes(2));
    expect(getSelection).toHaveBeenLastCalledWith({ tag: 'v2.14.1', build: 'auto', python: 'auto', adapter: 'none' });
  });

  it('previews the selected FLUX.2 adapter and rejects a token for another adapter', async () => {
    getOptions.mockResolvedValue({ ...options, adapters: ['none', 'flux2'] });
    const onInstall = vi.fn();
    getSelection.mockResolvedValueOnce(readySelection({ tag: 'v2.14.0', build: 'auto', python: 'auto', adapter: 'none' }));
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    fireEvent.click(await screen.findByRole('radio', { name: 'FLUX.2 image generation' }));
    fireEvent.click(screen.getByRole('button', { name: 'Install Torch' }));
    expect(getSelection).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'auto', python: 'auto', adapter: 'flux2' });
    expect(await screen.findByRole('alert')).toHaveTextContent('Torch selection could not be confirmed');
    expect(onInstall).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Install Torch' }));
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
  });

  it('resets the adapter for another tag and ignores its stale preview response', async () => {
    getOptions.mockResolvedValue({ ...options, adapters: ['none', 'flux2'] });
    let resolveSelection!: (value: TorchRuntimePreviewOutcome) => void;
    getSelection.mockImplementationOnce(() => new Promise((resolve) => { resolveSelection = resolve; }));
    const onInstall = vi.fn();
    const { rerender } = render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    fireEvent.click(await screen.findByRole('radio', { name: 'FLUX.2 image generation' }));
    fireEvent.click(screen.getByRole('button', { name: 'Install Torch' }));
    rerender(<TorchInstallPreview tag="v2.14.1" onBack={vi.fn()} onInstall={onInstall} />);
    expect(await screen.findByRole('radio', { name: 'Core Torch' })).toBeChecked();
    await act(async () => { resolveSelection(readySelection({ tag: 'v2.14.0', build: 'auto', python: 'auto', adapter: 'flux2' })); });
    expect(onInstall).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Install Torch' }));
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getSelection).toHaveBeenLastCalledWith({ tag: 'v2.14.1', build: 'auto', python: 'auto', adapter: 'none' });
  });

  it('ignores options returned for a previous tag', async () => {
    let resolveOldOptions!: (value: TorchRuntimeOptions) => void;
    getOptions.mockImplementationOnce(() => new Promise((resolve) => { resolveOldOptions = resolve; }));
    const { rerender } = render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    rerender(<TorchInstallPreview tag="v2.14.1" onBack={vi.fn()} onInstall={vi.fn()} />);

    expect(await screen.findByRole('button', { name: 'Install Torch' })).toBeEnabled();
    await act(async () => { resolveOldOptions({ ...options, adapters: ['none', 'flux2'] }); });
    expect(screen.queryByRole('radio', { name: 'FLUX.2 image generation' })).not.toBeInTheDocument();
  });

  it('shows immediate pending status and starts the install with the selection token', async () => {
    let resolveSelection!: (value: TorchRuntimePreviewOutcome) => void;
    getSelection.mockImplementationOnce(() => new Promise((resolve) => { resolveSelection = resolve; }));
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Install Torch' }));
    expect(screen.getByRole('status')).toHaveTextContent('Starting installation…');
    expect(screen.getByRole('button', { name: 'Starting installation…' })).toBeDisabled();
    expect(onInstall).not.toHaveBeenCalled();
    await act(async () => { resolveSelection(readySelection({ tag: 'v2.14.0', build: 'auto', python: 'auto', adapter: 'none' })); });
    expect(onInstall).toHaveBeenCalledOnce();
    expect(onInstall).toHaveBeenCalledWith('selection-token');
  });

  it('falls back to automatic build when the quick options omit defaultBuild', async () => {
    let resolveOptions!: (value: TorchRuntimeOptions) => void;
    getOptions.mockImplementation(() => new Promise((resolve) => {
      resolveOptions = resolve;
    }));
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);
    expect(screen.getByRole('button', { name: 'Install Torch' })).toBeDisabled();
    await act(async () => { resolveOptions({ ...options, defaultBuild: undefined }); });
    const installButton = screen.getByRole('button', { name: 'Install Torch' });
    expect(installButton).toBeEnabled();
    fireEvent.click(installButton);
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getSelection).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'auto', python: 'auto', adapter: 'none' });
  });

  it('preserves an explicit CUDA build and Linux image dependencies without pre-install discovery', async () => {
    getOptions.mockResolvedValue({ ...options, adapters: ['none', 'flux2'] });
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);
    fireEvent.click(await screen.findByText('Advanced setup'));
    fireEvent.change(screen.getByRole('combobox', { name: 'Torch build' }), { target: { value: 'cu130' } });
    fireEvent.click(screen.getByRole('radio', { name: 'FLUX.2 image generation' }));
    fireEvent.click(screen.getByRole('button', { name: 'Install Torch' }));
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getSelection).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'cu130', python: 'auto', adapter: 'flux2' });
    expect(getReleaseOptions).not.toHaveBeenCalled();
  });

  it('keeps the historical fixed preset distinct and submits its exact interpreter tuple', async () => {
    getOptions.mockResolvedValue({ ...options, bundledPresetAvailable: true });
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={onInstall} />);
    const presetButton = await screen.findByRole('button', { name: 'Fixed v2.9.1 preset' });
    expect(presetButton).toHaveAttribute('aria-pressed', 'false');
    fireEvent.click(presetButton);
    expect(screen.getByText(/This tuple has historical qualification/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Install Torch' }));
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getSelection).toHaveBeenCalledWith({ tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'bundled' });
    expect(presetButton).toBeDisabled();
  });

  it('does not expose platform-unavailable adapters or a preset for another release', async () => {
    getOptions.mockResolvedValue({ ...options, bundledPresetAvailable: true });
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    await screen.findByText('Advanced setup');
    expect(screen.queryByRole('option', { name: 'Pumas FLUX.2 image dependencies' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Fixed v2.9.1 preset' })).not.toBeInTheDocument();
  });

  it('rejects a ready token that changes the requested preset Python', async () => {
    getOptions.mockResolvedValue({ ...options, bundledPresetAvailable: true });
    const onInstall = vi.fn();
    getSelection.mockImplementation(async (request) => readySelection({ ...request, python: 'auto' }));
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={onInstall} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Fixed v2.9.1 preset' }));
    fireEvent.click(screen.getByRole('button', { name: 'Install Torch' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Torch selection could not be confirmed');
    expect(onInstall).not.toHaveBeenCalled();
  });

  it('does not install a rejected or mismatched selection', async () => {
    const onInstall = vi.fn();
    getSelection.mockResolvedValueOnce({ status: 'rejected', reason: 'unsupported', message: 'Unavailable build' });
    const { rerender } = render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Install Torch' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Unsupported selection: Unavailable build');
    expect(onInstall).not.toHaveBeenCalled();

    getSelection.mockResolvedValueOnce(readySelection({ tag: 'wrong-tag', build: 'auto', python: 'auto', adapter: 'none' }));
    fireEvent.click(screen.getByRole('button', { name: 'Install Torch' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Torch selection could not be confirmed');
    expect(onInstall).not.toHaveBeenCalled();
    rerender(<TorchInstallPreview tag="v2.14.1" onBack={vi.fn()} onInstall={onInstall} />);
    await waitFor(() => expect(getOptions).toHaveBeenCalledTimes(2));
  });

  it('does not treat a legacy resolved artifact preview as a quick selection token', async () => {
    const onInstall = vi.fn();
    const ready = readySelection({ tag: 'v2.14.0', build: 'auto', python: 'auto', adapter: 'none' });
    getSelection.mockResolvedValueOnce({ status: 'resolved', preview: ready.preview });
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Install Torch' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Torch selection could not be confirmed');
    expect(onInstall).not.toHaveBeenCalled();
  });
});
