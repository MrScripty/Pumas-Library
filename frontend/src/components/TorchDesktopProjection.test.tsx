import { useState } from 'react';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ModelManagerProps } from './ModelManager';
import { TorchPanel } from './app-panels/TorchPanel';
import type { AppVersionState } from '../utils/appVersionState';
import { UNSUPPORTED_VERSION_STATE } from '../utils/appVersionState';

vi.mock('./ModelManager', () => ({ ModelManager: () => null }));
vi.mock('./app-panels/sections/RuntimeProfileSettingsSection', () => ({ RuntimeProfileSettingsSection: () => null }));

const installedTag = 'v2.9.1';
const releaseTag = 'v2.10.0';
const previewId = 'reviewed-v2.10.0-cpu-python3.12-flux2';
const release = {
  tagName: releaseTag,
  name: 'Torch 2.10.0',
  publishedAt: '2026-09-01T00:00:00Z',
  prerelease: false,
  totalSize: 1024,
};
const modelManagerProps = {
  libraryLoadStatus: 'ready',
  excludedModels: new Set(),
  modelGroups: [],
  onToggleLink: vi.fn(),
  onToggleStar: vi.fn(),
  selectedAppId: 'torch',
  starredModels: new Set(),
} as ModelManagerProps;

function Harness({ installVersion, switchVersion, refreshAll }: {
  installVersion: AppVersionState['installVersion'];
  switchVersion: AppVersionState['switchVersion'];
  refreshAll: AppVersionState['refreshAll'];
}) {
  const [installedVersions, setInstalledVersions] = useState([installedTag]);
  const [activeVersion, setActiveVersion] = useState<string | null>(installedTag);
  const [showVersionManager, setShowVersionManager] = useState(false);
  const versions: AppVersionState = {
    ...UNSUPPORTED_VERSION_STATE,
    appId: 'torch',
    isSupported: true,
    isLoading: false,
    installedVersions,
    activeVersion,
    availableVersions: [release],
    defaultVersion: null,
    refreshAll,
    installVersion: async (tag, retainedPreviewId) => {
      const installed = await installVersion(tag, retainedPreviewId);
      if (installed) setInstalledVersions((current) => [...current, tag]);
      return installed;
    },
    switchVersion: async (tag) => {
      const switched = await switchVersion(tag);
      if (switched) setActiveVersion(tag);
      return switched;
    },
  };
  return <TorchPanel
    appDisplayName="Torch"
    versions={versions}
    showVersionManager={showVersionManager}
    onShowVersionManager={setShowVersionManager}
    diskSpacePercent={0}
    modelManagerProps={modelManagerProps}
    isTorchRunning={false}
    modelGroups={[]}
  />;
}

afterEach(() => vi.unstubAllGlobals());

describe('Torch desktop projection', () => {
  it('keeps installed versions visible through release discovery failure and runs the reviewed release through explicit selection and an owned startup trial', async () => {
    const installVersion = vi.fn().mockResolvedValue(true);
    const switchVersion = vi.fn().mockResolvedValue(true);
    const refreshAll = vi.fn().mockRejectedValue(new Error('release discovery unavailable'));
    const getOptions = vi.fn().mockResolvedValue({
      builds: ['cpu', 'cu130'],
      pythons: [{ id: 'python3.12', label: 'Python 3.12' }],
      adapters: ['none', 'flux2'],
      bundledPresetAvailable: true, defaultAdapter: 'flux2',
      preset: { tag: installedTag, build: 'cu130', python: 'python3.12', adapter: 'bundled' },
      installed: [
        { tag: installedTag, build: 'cu130', python: 'python3.12', adapter: 'bundled', qualification: 'qualified' },
        { tag: releaseTag, build: 'cpu', python: 'python3.12', adapter: 'flux2', qualification: 'unverified' },
      ],
    });
    const getReleaseOptions = vi.fn().mockResolvedValue({
      tag: releaseTag,
      status: 'matches',
      completeScan: true,
      checkedChannels: ['cpu', 'cu130'],
      combinations: [
        { build: 'cpu', python: 'python3.12', wheelUrl: 'https://download.pytorch.org/whl/cpu/torch.whl' },
        { build: 'cu130', python: 'python3.12', wheelUrl: 'https://download.pytorch.org/whl/cu130/torch.whl' },
      ],
      issues: [],
      detectedGpuVendors: [],
      driverStatus: { nvidia: 'not_present', amd: 'not_present' },
      recommended: { build: 'cpu', python: 'python3.12' },
      recommendationNote: null,
    });
    const preview = vi.fn().mockResolvedValue({
      status: 'resolved', preview: {
        previewId, expiresInSeconds: 300, tag: releaseTag, build: 'cpu', python: 'python3.12', adapter: 'flux2',
        qualification: 'unverified',
        artifacts: [{ name: 'torch', version: '2.10.0', url: 'https://download.pytorch.org/whl/cpu/torch.whl', sha256: 'abc123' }],
      },
    });
    const probe = vi.fn().mockResolvedValue({
      status: 'partial', core_status: 'passed', adapter_status: 'unavailable',
      capabilities: { sidecar_startup: { status: 'not tested' } },
    });
    const trial = vi.fn().mockResolvedValue({
      success: true, tag: releaseTag, profileId: 'managed-torch', startupStatus: 'passed',
      healthStatus: 'passed', protocol: 3, capabilities: ['torch_serve'],
      generation: 'owned-generation-42', startedByTrial: true, cleanup: 'not_needed',
    });
    const stopIfGeneration = vi.fn().mockResolvedValue({ success: true, stopped: true });
    const unconditionalStop = vi.fn();
    vi.stubGlobal('electronAPI', {
      get_torch_runtime_options: getOptions,
      get_torch_release_options: getReleaseOptions,
      preview_torch_runtime: preview,
      get_torch_runtime_probe: probe,
      get_runtime_profiles_snapshot: vi.fn().mockResolvedValue({ success: true, snapshot: { profiles: [
        { profile_id: 'managed-torch', provider: 'torch', provider_mode: 'torch_serve', management_mode: 'managed', enabled: true, name: 'Managed Torch' },
        { profile_id: 'external-torch', provider: 'torch', provider_mode: 'torch_serve', management_mode: 'external', enabled: true, name: 'External Torch' },
      ] } }),
      trial_torch_runtime: trial,
      stop_runtime_profile_if_generation: stopIfGeneration,
      stop_runtime_profile: unconditionalStop,
    });

    render(<Harness installVersion={installVersion} switchVersion={switchVersion} refreshAll={refreshAll} />);
    fireEvent.click(screen.getByTitle(/New version available:/));
    await waitFor(() => expect(refreshAll).toHaveBeenCalledWith(true));
    expect(await screen.findByRole('heading', { name: '2.9.1' })).toBeInTheDocument();
    expect(screen.getByText('Installed cu130 · python3.12 · bundled · qualified')).toBeInTheDocument();

    const releaseRow = screen.getByRole('heading', { name: '2.10.0' }).closest('div.w-full');
    if (!(releaseRow instanceof HTMLElement)) throw new TypeError('Expected release row');
    const installButton = within(releaseRow).getAllByRole('button')[0];
    if (!installButton) throw new TypeError('Expected install button');
    fireEvent.click(installButton);
    expect(installVersion).not.toHaveBeenCalled();
    expect(await screen.findByText('Pumas image dependencies are selected by Pumas for its FLUX.2 path; they are not part of upstream Torch.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Check selected combination' }));
    await waitFor(() => expect(screen.getByText('SHA-256: abc123')).toBeInTheDocument());
    expect(preview).toHaveBeenCalledWith({ tag: releaseTag, build: 'cpu', python: 'auto', adapter: 'flux2' });
    expect(screen.getByText('Selected Python: python3.12')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Install reviewed artifacts' }));
    await waitFor(() => expect(installVersion).toHaveBeenCalledWith(releaseTag, previewId));
    expect(switchVersion).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    fireEvent.click(screen.getByRole('button', { name: installedTag }));
    fireEvent.click(screen.getByRole('button', { name: `Switch to ${releaseTag}` }));
    await waitFor(() => expect(switchVersion).toHaveBeenCalledWith(releaseTag));
    await screen.findByRole('button', { name: releaseTag });
    expect(screen.queryByRole('region', { name: `Torch probe results for ${releaseTag}` })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Inspect active Torch runtime' }));
    expect(await screen.findByRole('region', { name: `Torch probe results for ${releaseTag}` })).toBeInTheDocument();
    expect(probe).toHaveBeenCalledWith(releaseTag);
    expect(await screen.findByText('Installed: cpu · python3.12 · flux2 · unverified')).toBeInTheDocument();

    expect(screen.getByRole('button', { name: 'Trial startup' })).toBeDisabled();
    expect(await screen.findByRole('option', { name: 'Managed Torch' })).toBeInTheDocument();
    expect(screen.queryByRole('option', { name: 'External Torch' })).not.toBeInTheDocument();
    fireEvent.change(screen.getByRole('combobox', { name: 'Managed Torch profile' }), { target: { value: 'managed-torch' } });
    fireEvent.click(screen.getByRole('button', { name: 'Trial startup' }));
    expect(await screen.findByText(/Startup trial passed/)).toBeInTheDocument();
    expect(trial).toHaveBeenCalledWith(releaseTag, 'managed-torch');
    fireEvent.click(screen.getByRole('button', { name: 'Stop trial profile' }));
    await waitFor(() => expect(screen.getByText('Trial profile stopped.')).toBeInTheDocument());
    expect(stopIfGeneration).toHaveBeenCalledWith('managed-torch', 'owned-generation-42');
    expect(unconditionalStop).not.toHaveBeenCalled();
  });
});
