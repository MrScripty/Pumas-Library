import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TorchInstallPreview } from './TorchInstallPreview';
import type {
  TorchAlternativesOutcome,
  TorchReleaseOptionsOutcome,
  TorchRuntimeOptions,
  TorchRuntimePreviewOutcome,
  TorchRuntimePreviewRequest,
} from '../types/torch-install';

const getReleaseOptions = vi.fn<(tag: string) => Promise<TorchReleaseOptionsOutcome>>();
const getPresetOptions = vi.fn<() => Promise<TorchRuntimeOptions>>();
const getPreview = vi.fn<(request: TorchRuntimePreviewRequest) => Promise<TorchRuntimePreviewOutcome>>();
const getAlternatives = vi.fn<(tag: string, build: string, python: string) => Promise<TorchAlternativesOutcome>>();

vi.mock('../api/adapter', () => ({
  api: {
    get_torch_release_options: (tag: string) => getReleaseOptions(tag),
    get_torch_runtime_options: () => getPresetOptions(),
    preview_torch_runtime: (request: TorchRuntimePreviewRequest) => getPreview(request),
    find_torch_alternatives: (tag: string, build: string, python: string) => getAlternatives(tag, build, python),
  },
}));

const release: TorchReleaseOptionsOutcome = {
  tag: 'v2.14.0', status: 'matches', completeScan: true,
  checkedChannels: ['cpu', 'cu130'],
  combinations: [
    { build: 'cpu', python: 'python3.11', wheelUrl: 'https://download.pytorch.org/whl/cpu/torch.whl' },
    { build: 'cu130', python: 'python3.12', wheelUrl: 'https://download.pytorch.org/whl/cu130/torch.whl' },
  ],
  issues: [], detectedGpuVendors: ['nvidia', 'intel'],
  driverStatus: { nvidia: 'available', amd: 'not_present' },
  recommended: { build: 'cu130', python: 'python3.12' }, recommendationNote: null,
};
const preset: TorchRuntimeOptions = {
  builds: ['cpu', 'cu130', 'cu134', 'rocm'],
  pythons: [{ id: 'python3.12', label: 'Python 3.12' }],
  adapters: ['none', 'flux2'], installed: [],
  bundledPresetAvailable: true, defaultAdapter: 'flux2',
  preset: { tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'bundled' },
};

beforeEach(() => {
  vi.clearAllMocks();
  getReleaseOptions.mockResolvedValue(release);
  getPresetOptions.mockResolvedValue(preset);
  getPreview.mockImplementation(async (request) => ({
    status: 'resolved',
    preview: {
      ...request, previewId: 'review-1', expiresInSeconds: 300,
      qualification: request.adapter === 'bundled' ? 'qualified' : 'unverified',
      artifacts: [{ name: 'torch', version: '2.14.0', url: 'https://download.pytorch.org/torch.whl', sha256: 'abc123' }],
    },
  }));
});

describe('TorchInstallPreview', () => {
  it('uses the manager recommendation without requiring build, Python, or profile selectors', async () => {
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    expect(await screen.findByText(/Recommended setup: cu130 · python3.12 · Pumas image dependencies/)).toBeInTheDocument();
    expect(getReleaseOptions).toHaveBeenCalledWith('v2.14.0');
    expect(getPresetOptions).toHaveBeenCalledOnce();
    expect(screen.getByText('Advanced setup').closest('details')).not.toHaveAttribute('open');
    expect(screen.getByText(/selected by Pumas for its FLUX.2 path; they are not part of upstream Torch/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(getPreview).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'cu130', python: 'python3.12', adapter: 'flux2' }));
    expect(await screen.findByText('SHA-256: abc123')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Install reviewed artifacts' }));
    expect(onInstall).toHaveBeenCalledWith('review-1');
  });

  it('limits advanced build and Python overrides to this release’s returned combinations', async () => {
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    fireEvent.click(await screen.findByText('Advanced setup'));
    const build = screen.getByRole('combobox', { name: 'Torch build' });
    expect(within(build).getAllByRole('option').map((option) => option.textContent)).toEqual(['Choose a build', 'cpu', 'cu130']);
    expect(within(build).queryByRole('option', { name: /cu134|rocm/i })).not.toBeInTheDocument();
    const python = screen.getByRole('combobox', { name: 'Installed Python' });
    expect(within(python).getAllByRole('option').map((option) => option.textContent)).toEqual(['Choose installed Python', 'python3.12']);
    fireEvent.change(build, { target: { value: 'cpu' } });
    expect(python).toHaveValue('python3.11');
    expect(within(python).getAllByRole('option').map((option) => option.textContent)).toEqual(['Choose installed Python', 'python3.11']);
    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(getPreview).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'cpu', python: 'python3.11', adapter: 'flux2' }));
  });

  it('requires a deliberate advanced selection when no recommendation exists for a GPU-only release', async () => {
    getReleaseOptions.mockResolvedValue({
      ...release,
      checkedChannels: ['cu132'],
      combinations: [{ build: 'cu132', python: 'python3.12', wheelUrl: 'https://download.pytorch.org/whl/cu132/torch.whl' }],
      recommended: null,
      recommendationNote: 'No GPU driver could be verified.',
    });
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);

    expect(await screen.findByText(/No setup was recommended. Open Advanced setup/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Check selected combination' })).toBeDisabled();
    expect(getPreview).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText('Advanced setup'));
    const build = screen.getByRole('combobox', { name: 'Torch build' });
    expect(build).toHaveValue('');
    expect(screen.getByRole('combobox', { name: 'Installed Python' })).toBeDisabled();
    fireEvent.change(build, { target: { value: 'cu132' } });
    expect(screen.getByRole('combobox', { name: 'Installed Python' })).toHaveValue('python3.12');
    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(getPreview).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'cu132', python: 'python3.12', adapter: 'flux2' }));
  });

  it('shows the manager’s driver caveat alongside a CPU recommendation', async () => {
    getReleaseOptions.mockResolvedValue({
      ...release,
      driverStatus: { ...release.driverStatus, nvidia: 'unavailable' },
      recommended: { build: 'cpu', python: 'python3.11' },
      recommendationNote: 'NVIDIA hardware was detected, but its driver could not be verified. CPU is recommended.',
    });
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    expect(await screen.findByText(/Recommended setup: cpu · python3.11/)).toBeInTheDocument();
    expect(screen.getByText(/driver could not be verified. CPU is recommended/)).toBeInTheDocument();
  });

  it('makes Core runtime only a deliberate advanced choice', async () => {
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    fireEvent.click(await screen.findByText('Advanced setup'));
    fireEvent.change(screen.getByRole('combobox', { name: 'Dependency profile' }), { target: { value: 'none' } });
    expect(screen.getByText(/Recommended setup: cu130 · python3.12 · Core runtime only/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(getPreview).toHaveBeenCalledWith({ tag: 'v2.14.0', build: 'cu130', python: 'python3.12', adapter: 'none' }));
  });

  it('does not imply an incomplete discovery found no wheels', async () => {
    getReleaseOptions.mockResolvedValue({ ...release, status: 'inconclusive', completeScan: false, combinations: [], recommended: null, issues: ['index timed out'] });
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    expect(await screen.findByText(/Release discovery was inconclusive. Some official wheels may exist/)).toBeInTheDocument();
    expect(screen.getByText('index timed out')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Check selected combination' })).toBeDisabled();
    expect(screen.queryByText(/No official wheel matches were found/)).not.toBeInTheDocument();
  });

  it('requires an installed compatible Python before checking release wheels', async () => {
    getReleaseOptions.mockResolvedValue({
      ...release, status: 'inconclusive', completeScan: false,
      checkedChannels: [], combinations: [], recommended: null,
      issues: ['No installed host-matched CPython 3.10–3.13 interpreter could inspect official wheels'],
    });
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);

    expect(await screen.findByText(/No installed host-matched CPython 3.10–3.13 interpreter/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Check selected combination' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Install reviewed artifacts' })).toBeDisabled();
    expect(getPreview).not.toHaveBeenCalled();
  });

  it('keeps the qualified v2.9.1 bundled preset and an unverified upstream choice distinct', async () => {
    getReleaseOptions.mockResolvedValue({ ...release, tag: 'v2.9.1' });
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={onInstall} />);
    const qualified = await screen.findByRole('button', { name: 'Qualified fixed preset' });
    expect(qualified).toHaveAttribute('aria-pressed', 'true');
    expect(screen.queryByRole('combobox', { name: 'Torch build' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(getPreview).toHaveBeenCalledWith({ tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'bundled' }));
    fireEvent.click(screen.getByRole('button', { name: 'Install fixed preset' }));
    expect(onInstall).toHaveBeenCalledWith('review-1');

    fireEvent.click(screen.getByRole('button', { name: 'Other official wheels (unverified)' }));
    expect(screen.getByRole('button', { name: 'Install reviewed artifacts' })).toBeDisabled();
    expect(screen.getByText(/Pumas image dependencies are selected by Pumas/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(getPreview).toHaveBeenLastCalledWith({ tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'flux2' }));
    expect(await screen.findByText(/Artifact resolution is unverified/)).toBeInTheDocument();
  });

  it('skips the bundled v2.9.1 preset when the manager says it is unavailable', async () => {
    getPresetOptions.mockResolvedValue({ ...preset, bundledPresetAvailable: false, defaultAdapter: 'none', adapters: ['none'] });
    getReleaseOptions.mockResolvedValue({
      ...release, tag: 'v2.9.1', checkedChannels: ['cpu'],
      combinations: [{ build: 'cpu', python: 'python3.12', wheelUrl: 'https://download.pytorch.org/whl/cpu/torch.whl' }],
      recommended: { build: 'cpu', python: 'python3.12' },
    });
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={onInstall} />);

    expect(await screen.findByText(/Recommended setup: cpu · python3.12 · Core runtime only/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Qualified fixed preset' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Other official wheels (unverified)' })).not.toBeInTheDocument();
    expect(screen.queryByText(/bundled image dependencies \(fixed\)/)).not.toBeInTheDocument();
    expect(screen.getByText(/matching this host’s operating system and architecture/)).toBeInTheDocument();
    fireEvent.click(screen.getByText('Advanced setup'));
    const profile = screen.getByRole('combobox', { name: 'Dependency profile' });
    expect(within(profile).getAllByRole('option').map((option) => option.textContent)).toEqual(['Core runtime only']);
    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(getPreview).toHaveBeenCalledWith({ tag: 'v2.9.1', build: 'cpu', python: 'python3.12', adapter: 'none' }));
    fireEvent.click(screen.getByRole('button', { name: 'Install reviewed artifacts' }));
    expect(onInstall).toHaveBeenCalledWith('review-1');
  });

  it('leaves the upstream v2.9.1 choice unset when the manager has no recommendation', async () => {
    getReleaseOptions.mockResolvedValue({
      ...release, tag: 'v2.9.1', recommended: null,
      checkedChannels: ['cu132'],
      combinations: [{ build: 'cu132', python: 'python3.12', wheelUrl: 'https://download.pytorch.org/whl/cu132/torch.whl' }],
    });
    render(<TorchInstallPreview tag="v2.9.1" onBack={vi.fn()} onInstall={vi.fn()} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Other official wheels (unverified)' }));
    expect(await screen.findByText(/No setup was recommended. Open Advanced setup/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Check selected combination' })).toBeDisabled();
    expect(getPreview).not.toHaveBeenCalled();
  });

  it('keeps rejected previews from installing and classifies validation by enum', async () => {
    getPreview.mockResolvedValue({ status: 'rejected', reason: 'validation_failed', message: 'Unsupported combination: missing hash' });
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Check selected combination' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Resolved wheel report failed validation: Unsupported combination: missing hash');
    expect(screen.queryByRole('button', { name: 'Find compatible alternatives' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Install reviewed artifacts' })).toBeDisabled();
  });

  it('offers official wheel alternatives only after an unsupported preview and rechecks the chosen match', async () => {
    getPreview.mockResolvedValueOnce({ status: 'rejected', reason: 'unsupported', message: 'selected dependencies have no matching wheel' });
    getPreview.mockImplementationOnce(async (request) => ({
      status: 'resolved',
      preview: { ...request, previewId: 'review-cpu', expiresInSeconds: 300, qualification: 'unverified', artifacts: [] },
    }));
    getAlternatives.mockResolvedValue({
      selectedTag: 'v2.14.0', selectedBuild: 'cu130', selectedPython: 'python3.12',
      status: 'matches', incomplete: true, dependenciesNotChecked: true,
      checkedBuilds: ['cu130', 'cpu'], issues: [],
      matches: [{ tag: 'v2.14.0', build: 'cpu', python: 'python3.11', wheelUrl: 'https://download.pytorch.org/whl/cpu/torch.whl', sha256: 'a'.repeat(64) }],
    });
    const onInstall = vi.fn();
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={onInstall} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Check selected combination' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Unsupported combination');
    expect(screen.getByRole('button', { name: 'Install reviewed artifacts' })).toBeDisabled();
    fireEvent.click(screen.getByRole('button', { name: 'Find compatible alternatives' }));
    expect(await screen.findByText('Official Torch wheel matches only')).toBeInTheDocument();
    expect(screen.getByText(/Other dependencies have not been checked/)).toBeInTheDocument();
    expect(getAlternatives).toHaveBeenCalledWith('v2.14.0', 'cu130', 'python3.12');
    fireEvent.click(screen.getByRole('button', { name: 'Preview v2.14.0 · cpu · python3.11' }));
    await waitFor(() => expect(getPreview).toHaveBeenLastCalledWith({ tag: 'v2.14.0', build: 'cpu', python: 'python3.11', adapter: 'flux2' }));
    expect(await screen.findByText('Exact artifacts resolved')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Install reviewed artifacts' }));
    expect(onInstall).toHaveBeenCalledWith('review-cpu');
  });

  it('shows and previews only host-eligible alternatives from the release combinations', async () => {
    getReleaseOptions.mockResolvedValue({
      ...release,
      checkedChannels: ['cpu'],
      combinations: [{ build: 'cpu', python: 'python3.11', wheelUrl: 'https://download.pytorch.org/whl/cpu/torch.whl' }],
      recommended: { build: 'cpu', python: 'python3.11' },
    });
    getPreview.mockResolvedValueOnce({ status: 'rejected', reason: 'unsupported', message: 'dependency unavailable' });
    getPreview.mockImplementationOnce(async (request) => ({
      status: 'resolved',
      preview: { ...request, previewId: 'eligible-cpu', expiresInSeconds: 300, qualification: 'unverified', artifacts: [] },
    }));
    getAlternatives.mockResolvedValue({
      selectedTag: 'v2.14.0', selectedBuild: 'cpu', selectedPython: 'python3.11',
      status: 'matches', incomplete: true, dependenciesNotChecked: true,
      checkedBuilds: ['cpu', 'cu130', 'rocm6.4'], issues: [],
      matches: [
        { tag: 'v2.14.0', build: 'cpu', python: 'python3.11' },
        { tag: 'v2.14.0', build: 'cu130', python: 'python3.11' },
        { tag: 'v2.14.0', build: 'rocm6.4', python: 'python3.11' },
        { tag: 'v2.14.0', build: 'cpu', python: 'python3.12' },
        { tag: 'v2.13.0', build: 'cpu', python: 'python3.11' },
      ],
    });
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Check selected combination' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Find compatible alternatives' }));
    expect(await screen.findByText('Checked builds: cpu')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: /^Preview v/ })).toHaveLength(1);
    expect(screen.queryByRole('button', { name: /Preview .*cu130|Preview .*rocm6\.4|Preview v2\.13\.0/ })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Preview v2.14.0 · cpu · python3.11' }));
    await waitFor(() => expect(getPreview).toHaveBeenCalledTimes(2));
    expect(getPreview).toHaveBeenLastCalledWith({ tag: 'v2.14.0', build: 'cpu', python: 'python3.11', adapter: 'flux2' });
  });

  it('keeps a long artifact list and its install action inside the scrollable preview region', async () => {
    getPreview.mockImplementation(async (request) => ({
      status: 'resolved',
      preview: {
        ...request, previewId: 'review-many', expiresInSeconds: 300, qualification: 'unverified',
        artifacts: Array.from({ length: 30 }, (_, index) => ({
          name: `wheel-${index}`, version: '1.0',
          url: `https://download.pytorch.org/whl/cu130/wheel-${index}.whl`,
          sha256: 'a'.repeat(64),
        })),
      },
    }));
    render(<TorchInstallPreview tag="v2.14.0" onBack={vi.fn()} onInstall={vi.fn()} />);

    fireEvent.click(await screen.findByRole('button', { name: 'Check selected combination' }));
    const previewRegion = screen.getByRole('region', { name: 'Torch installation preview for v2.14.0' });
    await waitFor(() => expect(screen.getByText('wheel-29 1.0')).toBeInTheDocument());
    expect(previewRegion).toHaveClass('flex-1', 'min-h-0', 'overflow-y-auto');
    expect(previewRegion).toContainElement(screen.getByRole('button', { name: 'Install reviewed artifacts' }));
  });
});
