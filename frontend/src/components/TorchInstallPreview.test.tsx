import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TorchInstallPreview } from './TorchInstallPreview';
import {
  decodeInstallationProgressOutcome,
  decodeTorchRuntimePreviewOutcome,
} from '@pumas/desktop-contract';
import type { TorchReleaseOptionsOutcome, TorchRuntimeOptions, TorchRuntimePreviewOutcome, TorchRuntimePreviewRequest, TorchVerifiedRuntime } from '../types/torch-install';

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
  modalities: ['text', 'image', 'audio'], extras: [],
  verified: [], adapterRequirements: [],
  installed: [],
};

const withNunchaku: TorchRuntimeOptions = {
  ...options, adapters: ['none', 'nunchaku'], extras: ['nunchaku'],
};

const nunchakuRequirement = { adapter: 'nunchaku', tag: 'v2.9.1', build: 'cu130', python: 'python3.12' };

const verifiedGeneral: TorchVerifiedRuntime = {
  tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapters: ['flux2'], extras: [],
  verifiedOn: '2026-09-23', scope: 'Imports passed.', sourceCommit: '5d373bf8',
  packages: [{ name: 'diffusers', version: '0.37.0' }],
};

const verifiedNunchaku: TorchVerifiedRuntime = {
  ...verifiedGeneral, adapters: ['nunchaku'], extras: ['nunchaku'],
};

function readySelection(request: TorchRuntimePreviewRequest): ReadySelection {
  const { adapter, extras, verified_versions: _pinned, ...selection } = request;
  return {
    status: 'ready',
    preview: {
      ...selection,
      adapter: adapter ?? (extras?.[0] ?? 'none'),
      extras: extras ?? [],
      previewId: 'selection-token', expiresInSeconds: 300,
      qualification: 'unverified', artifacts: [],
    },
  };
}

// The button exists while choices are loading. Existence is not admission.
async function readyInstallButton(): Promise<HTMLElement> {
  await waitFor(() => {
    expect(screen.queryByText('Loading Torch choices…')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Install Torch' })).toBeEnabled();
  });
  return screen.getByRole('button', { name: 'Install Torch' });
}

beforeEach(() => {
  vi.resetAllMocks();
  getOptions.mockResolvedValue(options);
  getSelection.mockImplementation(async (request) => readySelection(request));
});

describe('TorchInstallPreview', () => {
  it('accepts the ready token through the generated desktop RPC decoder', () => {
    expect(decodeTorchRuntimePreviewOutcome(readySelection({
      tag: 'v2.14.0', build: 'auto', python: 'auto', extras: [],
    }))).toMatchObject({ status: 'valid', value: { status: 'ready' } });
  });

  it('rejects the removed resolved preview outcome at the generated decoder', () => {
    const ready = readySelection({ tag: 'v2.14.0', build: 'auto', python: 'auto', extras: [] });
    expect(decodeTorchRuntimePreviewOutcome({ status: 'resolved', preview: ready.preview })).toMatchObject({ status: 'invalid' });
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
    expect(await readyInstallButton()).toBeEnabled();
    expect(getReleaseOptions).not.toHaveBeenCalled();
    expect(getSelection).not.toHaveBeenCalled();
    expect(screen.queryByRole('button', { name: 'Check selected combination' })).not.toBeInTheDocument();
    expect(screen.getByText('Advanced setup').closest('details')).not.toHaveAttribute('open');
    // No adapter choice: every install carries the general modalities.
    expect(screen.queryByRole('radio')).not.toBeInTheDocument();
    expect(screen.queryByRole('group', { name: 'Optional extras' })).not.toBeInTheDocument();
    expect(screen.queryByText(/exact artifacts resolved/i)).not.toBeInTheDocument();
  });

  it('describes the general modalities the backend reports', async () => {
    getOptions.mockResolvedValue({ ...options, modalities: ['text', 'image', 'audio', 'video'] });
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    expect(await screen.findByText(/Every install includes text, image, audio, video support/)).toBeInTheDocument();
  });

  it.each(['inconclusive', 'none'] as const)('allows installation when release discovery would be %s', async (status) => {
    getReleaseOptions.mockResolvedValue({
      tag: 'v2.14.0', status, completeScan: false, checkedChannels: [], combinations: [],
      issues: [], detectedGpuVendors: [], driverStatus: { nvidia: 'not_present', amd: 'not_present' },
      recommended: null, recommendationNote: null,
    });
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    fireEvent.click(await readyInstallButton());
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getReleaseOptions).not.toHaveBeenCalled();
    expect(getSelection).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'auto', python: 'auto', extras: [] });
  });

  it('offers the Nunchaku extra only for the release its wheel was built for and applies its selection', async () => {
    getOptions.mockResolvedValue({ ...withNunchaku, adapterRequirements: [nunchakuRequirement] });
    const { unmount } = render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    await readyInstallButton();
    expect(screen.queryByRole('checkbox', { name: 'Nunchaku' })).not.toBeInTheDocument();
    unmount();
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={onInstall} />);
    const extra = await screen.findByRole('checkbox', { name: 'Nunchaku' });
    expect(extra).not.toBeChecked();
    fireEvent.click(extra);
    expect(screen.getByText(/installs only with the cu130 build and Python 3.12/)).toBeInTheDocument();
    fireEvent.click(await readyInstallButton());
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getSelection).toHaveBeenCalledWith({ tag: 'v2.9.1', build: 'cu130', python: 'python3.12', extras: ['nunchaku'] });
  });

  it('rejects a ready token for other extras', async () => {
    getOptions.mockResolvedValue(withNunchaku);
    const onInstall = vi.fn();
    getSelection.mockResolvedValueOnce(readySelection({ tag: 'v2.14.0', build: 'auto', python: 'auto', extras: [] }));
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    fireEvent.click(await screen.findByRole('checkbox', { name: 'Nunchaku' }));
    fireEvent.click(await readyInstallButton());
    expect(getSelection).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'auto', python: 'auto', extras: ['nunchaku'] });
    expect(await screen.findByRole('alert')).toHaveTextContent('Torch selection could not be confirmed');
    expect(onInstall).not.toHaveBeenCalled();

    fireEvent.click(await readyInstallButton());
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
  });

  it('resets extras for another tag and ignores its stale preview response', async () => {
    getOptions.mockResolvedValue(withNunchaku);
    let resolveSelection!: (value: TorchRuntimePreviewOutcome) => void;
    getSelection.mockImplementationOnce(() => new Promise((resolve) => { resolveSelection = resolve; }));
    const onInstall = vi.fn();
    const { rerender } = render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    fireEvent.click(await screen.findByRole('checkbox', { name: 'Nunchaku' }));
    fireEvent.click(await readyInstallButton());
    rerender(<TorchInstallPreview tag="v2.14.1" onBack={vi.fn()} onInstall={onInstall} />);
    expect(await screen.findByRole('checkbox', { name: 'Nunchaku' })).not.toBeChecked();
    await act(async () => { resolveSelection(readySelection({ tag: 'v2.14.0', build: 'auto', python: 'auto', extras: ['nunchaku'] })); });
    expect(onInstall).not.toHaveBeenCalled();
    fireEvent.click(await readyInstallButton());
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getSelection).toHaveBeenLastCalledWith({ tag: 'v2.14.1', build: 'auto', python: 'auto', extras: [] });
  });

  it('ignores options returned for a previous tag', async () => {
    let resolveOldOptions!: (value: TorchRuntimeOptions) => void;
    getOptions.mockImplementationOnce(() => new Promise((resolve) => { resolveOldOptions = resolve; }));
    const { rerender } = render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    rerender(<TorchInstallPreview tag="v2.14.1" onBack={vi.fn()} onInstall={vi.fn()} />);

    expect(await readyInstallButton()).toBeEnabled();
    await act(async () => { resolveOldOptions(withNunchaku); });
    expect(screen.queryByRole('checkbox', { name: 'Nunchaku' })).not.toBeInTheDocument();
  });

  it('waits for delayed choices before treating the existing button as installable', async () => {
    let resolveOptions!: (value: TorchRuntimeOptions) => void;
    getOptions.mockImplementationOnce(() => new Promise((resolve) => { resolveOptions = resolve; }));
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    const existingButton = await screen.findByRole('button', { name: 'Install Torch' });
    expect(existingButton).toBeDisabled();
    fireEvent.click(existingButton);
    expect(getSelection).not.toHaveBeenCalled();
    expect(onInstall).not.toHaveBeenCalled();
    await act(async () => { resolveOptions(options); });
    fireEvent.click(await readyInstallButton());
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
  });

  it('shows immediate pending status and starts the install with the selection token', async () => {
    let resolveSelection!: (value: TorchRuntimePreviewOutcome) => void;
    getSelection.mockImplementationOnce(() => new Promise((resolve) => { resolveSelection = resolve; }));
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    fireEvent.click(await readyInstallButton());
    expect(screen.getByRole('status')).toHaveTextContent('Starting installation…');
    expect(screen.getByRole('button', { name: 'Starting installation…' })).toBeDisabled();
    expect(onInstall).not.toHaveBeenCalled();
    await act(async () => { resolveSelection(readySelection({ tag: 'v2.14.0', build: 'auto', python: 'auto', extras: [] })); });
    expect(onInstall).toHaveBeenCalledOnce();
    expect(onInstall).toHaveBeenCalledWith('selection-token');
  });

  it('preserves an explicit CUDA build without pre-install discovery', async () => {
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);
    fireEvent.click(await screen.findByText('Advanced setup'));
    fireEvent.change(screen.getByRole('combobox', { name: 'Torch build' }), { target: { value: 'cu130' } });
    fireEvent.click(await readyInstallButton());
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getSelection).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'cu130', python: 'auto', extras: [] });
    expect(getReleaseOptions).not.toHaveBeenCalled();
  });

  it('offers a tested combination as a shortcut that still resolves current packages', async () => {
    getOptions.mockResolvedValue({ ...withNunchaku, verified: [verifiedGeneral, verifiedNunchaku] });
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={onInstall} />);
    const shortcut = await screen.findByRole('button', { name: 'cu130 · python3.12 · Nunchaku' });
    expect(screen.getByRole('button', { name: 'cu130 · python3.12 · general' })).toBeInTheDocument();
    expect(shortcut).toHaveAttribute('aria-pressed', 'false');
    fireEvent.click(shortcut);
    expect(shortcut).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByText(/Tested as working on 2026-09-23 with diffusers 0.37.0/)).toHaveTextContent('Installing resolves current package versions');
    expect(screen.getByRole('checkbox', { name: 'Nunchaku' })).toBeChecked();
    fireEvent.click(await readyInstallButton());
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getSelection).toHaveBeenCalledWith({ tag: 'v2.9.1', build: 'cu130', python: 'python3.12', extras: ['nunchaku'] });
  });

  it('pins the tested package versions only when asked and only for an exact tested selection', async () => {
    getOptions.mockResolvedValue({ ...options, verified: [verifiedGeneral] });
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={onInstall} />);
    fireEvent.click(await screen.findByRole('button', { name: 'cu130 · python3.12 · general' }));
    const pin = screen.getByRole('checkbox', { name: 'Install the verified package versions' });
    expect(pin).not.toBeChecked();
    fireEvent.click(pin);
    expect(screen.getByText(/Tested as working on 2026-09-23/)).toHaveTextContent('Installing uses exactly these package versions');
    fireEvent.click(await readyInstallButton());
    await waitFor(() => expect(onInstall).toHaveBeenCalledWith('selection-token'));
    expect(getSelection).toHaveBeenCalledWith({
      tag: 'v2.9.1', build: 'cu130', python: 'python3.12', extras: [], verified_versions: true,
    });
  });

  it('does not offer tested combinations for another release or an unavailable extra', async () => {
    getOptions.mockResolvedValue({ ...options, verified: [verifiedGeneral, verifiedNunchaku] });
    const { unmount } = render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    await screen.findByText('Advanced setup');
    expect(screen.queryByRole('group', { name: 'Verified combinations' })).not.toBeInTheDocument();
    unmount();
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={vi.fn()} />);
    expect(await screen.findByRole('button', { name: 'cu130 · python3.12 · general' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Nunchaku/ })).not.toBeInTheDocument();
  });

  it('rejects a ready token that changes the requested tested Python', async () => {
    getOptions.mockResolvedValue({ ...options, verified: [verifiedGeneral] });
    const onInstall = vi.fn();
    getSelection.mockImplementation(async (request) => readySelection({ ...request, python: 'auto' }));
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={onInstall} />);
    fireEvent.click(await screen.findByRole('button', { name: 'cu130 · python3.12 · general' }));
    fireEvent.click(await readyInstallButton());
    expect(await screen.findByRole('alert')).toHaveTextContent('Torch selection could not be confirmed');
    expect(onInstall).not.toHaveBeenCalled();
  });

  it('does not install a rejected or mismatched selection', async () => {
    const onInstall = vi.fn();
    getSelection.mockResolvedValueOnce({ status: 'rejected', reason: 'unsupported', message: 'Unavailable build' });
    const { rerender } = render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);
    fireEvent.click(await readyInstallButton());
    expect(await screen.findByRole('alert')).toHaveTextContent('Unsupported selection: Unavailable build');
    expect(onInstall).not.toHaveBeenCalled();

    getSelection.mockResolvedValueOnce(readySelection({ tag: 'wrong-tag', build: 'auto', python: 'auto', extras: [] }));
    fireEvent.click(await readyInstallButton());
    expect(await screen.findByRole('alert')).toHaveTextContent('Torch selection could not be confirmed');
    expect(onInstall).not.toHaveBeenCalled();
    rerender(<TorchInstallPreview tag="v2.14.1" onBack={vi.fn()} onInstall={onInstall} />);
    await waitFor(() => expect(getOptions).toHaveBeenCalledTimes(2));
  });
});
