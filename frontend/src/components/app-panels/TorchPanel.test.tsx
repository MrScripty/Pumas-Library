import { useState } from 'react';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ModelManagerProps } from '../ModelManager';
import type { VersionRelease } from '../../types/versions';
import type { AppVersionState } from '../../utils/appVersionState';
import { UNSUPPORTED_VERSION_STATE } from '../../utils/appVersionState';
import { TorchPanel } from './TorchPanel';

vi.mock('../ModelManager', () => ({
  ModelManager: () => <div>Model manager omitted from version flow</div>,
}));

vi.mock('./sections/RuntimeProfileSettingsSection', () => ({
  RuntimeProfileSettingsSection: () => null,
}));

const oldTag = 'torch-runtime-0.1.0';
const candidateTag = 'torch-runtime-0.2.0';

const releaseFixture: VersionRelease[] = [
  {
    tagName: candidateTag,
    name: 'Torch Runtime 0.2.0',
    publishedAt: '2026-04-12T00:00:00Z',
    prerelease: false,
    totalSize: 1024,
  },
  {
    tagName: oldTag,
    name: 'Torch Runtime 0.1.0',
    publishedAt: '2026-04-11T00:00:00Z',
    prerelease: false,
    totalSize: 512,
  },
];

const modelManagerProps: ModelManagerProps = {
  libraryLoadStatus: 'ready',
  excludedModels: new Set(),
  modelGroups: [],
  onToggleLink: vi.fn(),
  onToggleStar: vi.fn(),
  selectedAppId: 'torch',
  starredModels: new Set(),
};

interface TorchPanelHarnessActions {
  refreshAll: AppVersionState['refreshAll'];
  installVersion: AppVersionState['installVersion'];
  switchVersion: AppVersionState['switchVersion'];
  removeVersion: AppVersionState['removeVersion'];
  setDefaultVersion: AppVersionState['setDefaultVersion'];
}

function TorchPanelHarness({
  actions,
  isLoading = false,
  initialAvailableVersions = [],
}: {
  actions: TorchPanelHarnessActions;
  isLoading?: boolean;
  initialAvailableVersions?: VersionRelease[];
}) {
  const [installedVersions, setInstalledVersions] = useState([oldTag]);
  const [activeVersion, setActiveVersion] = useState<string | null>(oldTag);
  const [availableVersions, setAvailableVersions] = useState(initialAvailableVersions);
  const [showVersionManager, setShowVersionManager] = useState(false);

  const versions: AppVersionState = {
    ...UNSUPPORTED_VERSION_STATE,
    appId: 'torch',
    isSupported: true,
    isLoading,
    installedVersions,
    activeVersion,
    availableVersions,
    defaultVersion: null,
    refreshAll: async (forceRefresh) => {
      await actions.refreshAll(forceRefresh);
      setAvailableVersions(releaseFixture);
    },
    installVersion: async (tag, previewId) => {
      const installed = await actions.installVersion(tag, previewId);
      if (installed) {
        setInstalledVersions((current) => current.includes(tag) ? current : [...current, tag]);
      }
      return installed;
    },
    switchVersion: async (tag) => {
      const switched = await actions.switchVersion(tag);
      if (switched) setActiveVersion(tag);
      return switched;
    },
    removeVersion: async (tag) => {
      const removed = await actions.removeVersion(tag);
      if (removed) setInstalledVersions((current) => current.filter((version) => version !== tag));
      return removed;
    },
    setDefaultVersion: actions.setDefaultVersion,
  };

  return (
    <TorchPanel
      appDisplayName="Torch"
      versions={versions}
      showVersionManager={showVersionManager}
      onShowVersionManager={setShowVersionManager}
      diskSpacePercent={0}
      modelManagerProps={modelManagerProps}
      isTorchRunning={false}
      modelGroups={[]}
    />
  );
}

function getVersionRow(tag: string): HTMLElement {
  let node: HTMLElement | null = screen.getByRole('heading', { name: tag });
  while (node) {
    if (node.querySelectorAll('button').length >= 2) return node;
    node = node.parentElement;
  }
  throw new TypeError(`Could not find the version row for ${tag}`);
}

function expectActiveVersionNotDefault(tag: string) {
  const triggerGroup = screen.getByRole('button', { name: tag }).parentElement;
  if (!triggerGroup) throw new TypeError('Expected the version selector trigger group');
  expect(within(triggerGroup).getByTitle('Click to set as default')).toBeInTheDocument();
}

describe('TorchPanel shared version controls', () => {
  it('installs, activates, and removes versions through the shared manager flow', async () => {
    let finishRefresh!: () => void;
    const forcedRefresh = new Promise<void>((resolve) => {
      finishRefresh = resolve;
    });
    const actions: TorchPanelHarnessActions = {
      refreshAll: vi.fn(async (forceRefresh?: boolean) => {
        if (forceRefresh) await forcedRefresh;
      }),
      installVersion: vi.fn(async (_tag: string) => true),
      switchVersion: vi.fn(async (_tag: string) => true),
      removeVersion: vi.fn(async (_tag: string) => true),
      setDefaultVersion: vi.fn(async (_tag: string | null) => undefined),
    };

    const panel = render(
      <TorchPanelHarness
        actions={actions}
        initialAvailableVersions={releaseFixture}
        isLoading
      />
    );

    // Wait for startup loading, then refresh before cached choices become actionable.
    fireEvent.click(screen.getByTitle(/New version available:/));
    expect(screen.getByText('1 installed')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: candidateTag })).not.toBeInTheDocument();
    expect(actions.refreshAll).not.toHaveBeenCalled();

    panel.rerender(
      <TorchPanelHarness
        actions={actions}
        initialAvailableVersions={releaseFixture}
        isLoading={false}
      />
    );

    await waitFor(() => {
      expect(actions.refreshAll).toHaveBeenCalledWith(true);
    });
    expect(screen.queryByRole('heading', { name: candidateTag })).not.toBeInTheDocument();

    await act(async () => {
      finishRefresh();
      await forcedRefresh;
    });
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: candidateTag })).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    await waitFor(() => {
      expect(actions.refreshAll).toHaveBeenCalledTimes(2);
    });

    const candidateRow = getVersionRow(candidateTag);
    const [installButton] = within(candidateRow).getAllByRole('button');
    if (!installButton) throw new TypeError('Expected the candidate install button');
    vi.stubGlobal('electronAPI', {
      get_torch_release_options: vi.fn().mockResolvedValue({
        tag: candidateTag, status: 'matches', completeScan: true,
        checkedChannels: ['cpu'],
        combinations: [{ build: 'cpu', python: 'python3.12', wheelUrl: 'https://download.pytorch.org/whl/cpu/torch.whl' }],
        issues: [], detectedGpuVendors: [],
        driverStatus: { nvidia: 'not_present', amd: 'not_present' },
        recommended: { build: 'cpu', python: 'python3.12' }, recommendationNote: null,
      }),
      get_torch_runtime_options: vi.fn().mockResolvedValue({
        builds: ['cpu'], pythons: [{ id: 'python3.12', label: 'Python 3.12' }],
        adapters: ['none'], bundledPresetAvailable: false, defaultAdapter: 'none',
        preset: { tag: 'v2.9.1', build: 'cu130', python: 'python3.12', adapter: 'bundled' },
      }),
      preview_torch_runtime: vi.fn().mockResolvedValue({
        status: 'resolved', preview: {
          previewId: 'preview-1', expiresInSeconds: 300, tag: candidateTag, build: 'cpu', python: 'python3.12',
          adapter: 'none', qualification: 'unverified', artifacts: [],
        },
      }),
      get_torch_runtime_probe: vi.fn().mockResolvedValue({
        status: 'passed', core_status: 'passed', adapter_status: 'not selected', capabilities: {},
      }),
      get_runtime_profiles_snapshot: vi.fn().mockResolvedValue({ success: true, snapshot: { profiles: [] } }),
    });
    fireEvent.click(installButton);
    expect(actions.installVersion).not.toHaveBeenCalled();
    fireEvent.click(await screen.findByRole('button', { name: 'Check selected combination' }));
    const reviewedInstallButton = screen.getByRole('button', { name: 'Install reviewed artifacts' });
    await waitFor(() => expect(reviewedInstallButton).toBeEnabled());
    fireEvent.click(reviewedInstallButton);
    await waitFor(() => {
      expect(actions.installVersion).toHaveBeenCalledWith(candidateTag, 'preview-1');
      expect(within(getVersionRow(candidateTag)).getByRole('button', { name: 'Ready' }))
        .toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    fireEvent.click(screen.getByRole('button', { name: oldTag }));
    fireEvent.click(screen.getByRole('button', { name: `Switch to ${candidateTag}` }));

    await waitFor(() => {
      expect(actions.switchVersion).toHaveBeenCalledWith(candidateTag);
      expect(screen.getByRole('button', { name: candidateTag })).toBeInTheDocument();
    });
    expectActiveVersionNotDefault(candidateTag);
    expect(actions.setDefaultVersion).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTitle('Install new version'));
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: oldTag })).toBeInTheDocument();
    });
    const oldRow = getVersionRow(oldTag);
    fireEvent.pointerEnter(oldRow);
    const uninstallButton = await within(oldRow).findByRole('button', { name: 'Uninstall' });
    fireEvent.click(uninstallButton);

    await waitFor(() => {
      expect(actions.removeVersion).toHaveBeenCalledWith(oldTag);
      expect(within(getVersionRow(oldTag)).queryByRole('button', { name: 'Ready' }))
        .not.toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: 'Back' }));
    expect(screen.getByRole('button', { name: candidateTag })).toBeInTheDocument();
    expectActiveVersionNotDefault(candidateTag);
    expect(actions.setDefaultVersion).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });
});
