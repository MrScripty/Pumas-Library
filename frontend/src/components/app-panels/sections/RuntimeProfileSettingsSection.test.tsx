import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  RuntimeProfileConfig,
  RuntimeProfileStatus,
  RuntimeProfilesSnapshot,
} from '../../../types/api-runtime-profiles';
import { resetRuntimeProvidersForTests } from '../../../hooks/useRuntimeProviders';
import { TEST_RUNTIME_PROVIDERS, testRuntimeProvidersResponse } from '../../../test/runtimeProviders';
import { RuntimeProfileSettingsSection } from './RuntimeProfileSettingsSection';

const {
  getElectronAPIMock,
  getInstalledVersionsMock,
  getRuntimeProfilesSnapshotMock,
  getRuntimeProvidersMock,
  launchRuntimeProfileMock,
  stopRuntimeProfileMock,
  upsertRuntimeProfileMock,
} =
  vi.hoisted(() => ({
    getElectronAPIMock: vi.fn(),
    getRuntimeProvidersMock: vi.fn(),
    getInstalledVersionsMock:
      vi.fn<(appId?: string) => Promise<{ success: true; versions: string[] }>>(),
    getRuntimeProfilesSnapshotMock: vi.fn(),
    launchRuntimeProfileMock: vi.fn(),
    stopRuntimeProfileMock: vi.fn(),
    upsertRuntimeProfileMock:
      vi.fn<
        (profile: RuntimeProfileConfig) => Promise<{
          success: boolean;
          profile_id: string;
          snapshot_required: boolean;
        }>
      >(),
  }));

vi.mock('../../../api/adapter', () => ({
  getElectronAPI: getElectronAPIMock,
}));

describe('RuntimeProfileSettingsSection', () => {
  const snapshot: RuntimeProfilesSnapshot = {
    schema_version: 1,
    cursor: 'runtime-profiles:0',
    profiles: [
      {
        profile_id: 'ollama-default',
        provider: 'ollama',
        provider_mode: 'ollama_serve',
        management_mode: 'managed',
        name: 'Ollama Default',
        enabled: true,
        endpoint_url: 'http://127.0.0.1:11434/',
        port: 11434,
        device: { mode: 'auto' },
        scheduler: { auto_load: true },
      },
      {
        profile_id: 'llama-router',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_router',
        management_mode: 'managed',
        name: 'llama.cpp Router',
        enabled: true,
        endpoint_url: 'http://127.0.0.1:18080/',
        port: 18080,
        device: { mode: 'cpu' },
        scheduler: { auto_load: true },
      },
      {
        profile_id: 'onnx-runtime-default',
        provider: 'onnx_runtime',
        provider_mode: 'onnx_serve',
        management_mode: 'managed',
        name: 'ONNX Runtime Profile',
        enabled: true,
        endpoint_url: null,
        port: null,
        device: { mode: 'cpu' },
        scheduler: { auto_load: true },
      },
    ],
    routes: [],
    statuses: [
      { profile_id: 'ollama-default', state: 'stopped' },
      { profile_id: 'llama-router', state: 'stopped' },
      { profile_id: 'onnx-runtime-default', state: 'stopped' },
    ],
    default_profile_id: 'ollama-default',
  };

  beforeEach(() => {
    vi.clearAllMocks();
    resetRuntimeProvidersForTests();
    getRuntimeProvidersMock.mockResolvedValue(testRuntimeProvidersResponse());
    getRuntimeProfilesSnapshotMock.mockResolvedValue({
      success: true,
      snapshot,
    });
    upsertRuntimeProfileMock.mockResolvedValue({
      success: true,
      profile_id: 'runtime-new',
      snapshot_required: false,
    });
    launchRuntimeProfileMock.mockResolvedValue({
      success: true,
      ready: true,
      log_path: '/tmp/runtime.log',
    });
    stopRuntimeProfileMock.mockResolvedValue({
      success: true,
    });
    getInstalledVersionsMock.mockImplementation((appId?: string) =>
      Promise.resolve({
        success: true,
        versions: appId === 'llama-cpp' ? ['b9082+cpu', 'b9100+cuda-12'] : [],
      })
    );
    getElectronAPIMock.mockReturnValue({
      get_installed_versions: getInstalledVersionsMock,
      get_runtime_profiles_snapshot: getRuntimeProfilesSnapshotMock,
      get_runtime_providers: getRuntimeProvidersMock,
      launch_runtime_profile: launchRuntimeProfileMock,
      stop_runtime_profile: stopRuntimeProfileMock,
      upsert_runtime_profile: upsertRuntimeProfileMock,
      onRuntimeProfileUpdate: vi.fn(() => vi.fn()),
    });
  });

  it('keeps a new managed profile draft from inheriting the default Ollama port', async () => {
    const user = userEvent.setup();

    render(<RuntimeProfileSettingsSection provider="ollama" />);

    expect(await screen.findByDisplayValue('Ollama Default')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'New runtime profile' }));

    expect(screen.getByDisplayValue('New Ollama Profile')).toBeInTheDocument();
    expect(screen.getByRole('spinbutton', { name: /managed process port/i })).toHaveValue(null);

    await user.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(upsertRuntimeProfileMock).toHaveBeenCalledTimes(1);
    });
    const savedProfile = upsertRuntimeProfileMock.mock.calls[0]?.[0];
    expect(savedProfile).toBeDefined();
    expect(savedProfile?.profile_id).not.toBe('ollama-default');
    expect(savedProfile).toMatchObject({
      management_mode: 'managed',
      endpoint_url: null,
      port: null,
    });
  });

  it('does not allow saved profile ids to be edited into duplicate profiles', async () => {
    const user = userEvent.setup();

    render(<RuntimeProfileSettingsSection provider="ollama" />);

    expect(await screen.findByDisplayValue('Ollama Default')).toBeInTheDocument();

    const profileIdInput = screen.getByLabelText(/profile id/i);
    expect(profileIdInput).toBeDisabled();
    expect(profileIdInput).toHaveValue('ollama-default');

    await user.clear(screen.getByLabelText('Name'));
    await user.type(screen.getByLabelText('Name'), 'Emily Llama');
    await user.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(upsertRuntimeProfileMock).toHaveBeenCalledTimes(1);
    });
    expect(upsertRuntimeProfileMock).toHaveBeenCalledWith(
      expect.objectContaining({
        profile_id: 'ollama-default',
        name: 'Emily Llama',
        port: 11434,
      })
    );
  });

  it('starts a selected managed runtime profile from settings', async () => {
    const user = userEvent.setup();

    render(<RuntimeProfileSettingsSection provider="ollama" />);

    expect(await screen.findByDisplayValue('Ollama Default')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Start runtime' }));

    await waitFor(() => {
      expect(launchRuntimeProfileMock).toHaveBeenCalledTimes(1);
    });
    expect(launchRuntimeProfileMock).toHaveBeenCalledWith('ollama-default');
  });

  it('scopes profiles and new drafts to the selected runtime provider', async () => {
    const user = userEvent.setup();

    render(<RuntimeProfileSettingsSection provider="llama_cpp" />);

    expect(await screen.findByDisplayValue('llama.cpp Router')).toBeInTheDocument();
    expect(screen.queryByText('Ollama Default')).not.toBeInTheDocument();
    expect(screen.getByText('Runtime')).toBeInTheDocument();
    expect(screen.getAllByText('llama.cpp').length).toBeGreaterThan(0);
    expect(screen.queryByLabelText(/device id/i)).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/gpu layers/i)).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'New runtime profile' }));
    expect(screen.getByDisplayValue('New llama.cpp Profile')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(upsertRuntimeProfileMock).toHaveBeenCalledTimes(1);
    });
    expect(upsertRuntimeProfileMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_router',
      })
    );
  });

  it('scopes ONNX Runtime settings to managed onnx_serve profiles', async () => {
    const user = userEvent.setup();

    render(<RuntimeProfileSettingsSection provider="onnx_runtime" />);

    expect(await screen.findByDisplayValue('ONNX Runtime Profile')).toBeInTheDocument();
    expect(screen.queryByText('Ollama Default')).not.toBeInTheDocument();
    expect(screen.getAllByText('ONNX Runtime').length).toBeGreaterThan(0);
    expect(screen.getByLabelText('Mode')).toHaveValue('onnx_serve');
    expect(screen.getByLabelText('Management')).toHaveValue('managed');
    expect(screen.queryByRole('option', { name: 'External' })).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/gpu layers/i)).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'New runtime profile' }));
    expect(screen.getByDisplayValue('New ONNX Runtime Profile')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(upsertRuntimeProfileMock).toHaveBeenCalledTimes(1);
    });
    expect(upsertRuntimeProfileMock).toHaveBeenCalledWith(
      expect.objectContaining({
        provider: 'onnx_runtime',
        provider_mode: 'onnx_serve',
        management_mode: 'managed',
        endpoint_url: null,
        port: null,
      })
    );
  });

  it('offers the device modes the backend registers for the provider', async () => {
    const user = userEvent.setup();

    render(<RuntimeProfileSettingsSection provider="onnx_runtime" />);

    expect(await screen.findByDisplayValue('ONNX Runtime Profile')).toBeInTheDocument();
    const deviceOptions = () =>
      Array.from(screen.getByLabelText<HTMLSelectElement>('Device').options).map(
        (option) => option.value
      );
    expect(deviceOptions()).toEqual(['auto', 'cpu', 'gpu']);
    expect(getInstalledVersionsMock).toHaveBeenCalledWith('onnx-runtime');

    await user.selectOptions(screen.getByLabelText('Device'), 'gpu');
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => {
      expect(upsertRuntimeProfileMock).toHaveBeenCalledTimes(1);
    });
    expect(upsertRuntimeProfileMock).toHaveBeenLastCalledWith(
      expect.objectContaining({
        provider: 'onnx_runtime',
        device: expect.objectContaining({ mode: 'gpu' }) as RuntimeProfileConfig['device'],
      })
    );
  });

  it('follows the runtime app and modes the backend reports rather than a local copy', async () => {
    getRuntimeProvidersMock.mockResolvedValue(
      testRuntimeProvidersResponse(
        TEST_RUNTIME_PROVIDERS.map((info) =>
          info.provider === 'llama_cpp'
            ? { ...info, app_id: 'llama-cpp-next', device_modes: ['cpu'] }
            : info
        )
      )
    );

    render(<RuntimeProfileSettingsSection provider="llama_cpp" />);

    expect(await screen.findByDisplayValue('llama.cpp Router')).toBeInTheDocument();
    await waitFor(() => {
      expect(getInstalledVersionsMock).toHaveBeenCalledWith('llama-cpp-next');
    });
    expect(getInstalledVersionsMock).not.toHaveBeenCalledWith('llama-cpp');
    expect(
      Array.from(screen.getByLabelText<HTMLSelectElement>('Device').options).map(
        (option) => option.value
      )
    ).toEqual(['cpu']);
  });

  it('does not list installed versions for a runtime the backend does not version-manage', async () => {
    getRuntimeProvidersMock.mockResolvedValue(
      testRuntimeProvidersResponse(
        TEST_RUNTIME_PROVIDERS.map((info) =>
          info.provider === 'llama_cpp' ? { ...info, version_managed: false } : info
        )
      )
    );

    render(<RuntimeProfileSettingsSection provider="llama_cpp" />);

    expect(await screen.findByDisplayValue('llama.cpp Router')).toBeInTheDocument();
    expect(getInstalledVersionsMock).not.toHaveBeenCalled();
  });

  it('waits for the provider registry and offers a retry when it is unavailable', async () => {
    const user = userEvent.setup();
    getRuntimeProvidersMock.mockRejectedValueOnce(new Error('backend restarting'));

    render(<RuntimeProfileSettingsSection provider="ollama" />);

    expect(screen.getByText('Loading runtime provider details...')).toBeInTheDocument();
    expect(
      await screen.findByText('Runtime provider details unavailable: backend restarting')
    ).toBeInTheDocument();
    expect(screen.queryByLabelText('Device')).not.toBeInTheDocument();

    await user.click(
      screen.getByRole('button', { name: 'Retry loading runtime provider details' })
    );
    expect(await screen.findByDisplayValue('Ollama Default')).toBeInTheDocument();
    expect(getRuntimeProvidersMock).toHaveBeenCalledTimes(2);
  });

  it('defaults llama.cpp GPU profiles to full layer offload', async () => {
    const user = userEvent.setup();

    render(<RuntimeProfileSettingsSection provider="llama_cpp" />);

    expect(await screen.findByDisplayValue('llama.cpp Router')).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText('Device'), 'gpu');

    expect(screen.getByRole('spinbutton', { name: /gpu layers/i })).toHaveValue(-1);

    await user.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(upsertRuntimeProfileMock).toHaveBeenCalledTimes(1);
    });
    expect(upsertRuntimeProfileMock).toHaveBeenCalledWith(
      expect.objectContaining({
        device: expect.objectContaining({
          mode: 'gpu',
          gpu_layers: -1,
        }) as RuntimeProfileConfig['device'],
      })
    );
  });

  describe('runtime version', () => {
    const withLlamaRouter = (
      profile: Partial<RuntimeProfileConfig>,
      status: Partial<RuntimeProfileStatus> = {}
    ): RuntimeProfilesSnapshot => ({
      ...snapshot,
      profiles: snapshot.profiles.map((item) =>
        item.profile_id === 'llama-router' ? { ...item, ...profile } : item
      ),
      statuses: snapshot.statuses.map((item) =>
        item.profile_id === 'llama-router' ? { ...item, ...status } : item
      ),
    });

    it('offers the installed versions of the profile runtime from the backend', async () => {
      render(<RuntimeProfileSettingsSection provider="llama_cpp" />);

      expect(await screen.findByDisplayValue('llama.cpp Router')).toBeInTheDocument();
      const select = screen.getByLabelText(/runtime version/i);
      await waitFor(() => {
        expect(screen.getByRole('option', { name: 'b9100+cuda-12' })).toBeInTheDocument();
      });
      expect(getInstalledVersionsMock).toHaveBeenCalledWith('llama-cpp');
      expect(select).toHaveValue('');
      expect(
        Array.from((select as HTMLSelectElement).options).map((option) => option.textContent)
      ).toEqual(['Follow active version', 'b9082+cpu', 'b9100+cuda-12']);
    });

    it('saves a selected pin and clears it back to following the active version', async () => {
      const user = userEvent.setup();

      render(<RuntimeProfileSettingsSection provider="llama_cpp" />);

      expect(await screen.findByDisplayValue('llama.cpp Router')).toBeInTheDocument();
      await screen.findByRole('option', { name: 'b9082+cpu' });

      await user.selectOptions(screen.getByLabelText(/runtime version/i), 'b9082+cpu');
      await user.click(screen.getByRole('button', { name: 'Save' }));
      await waitFor(() => {
        expect(upsertRuntimeProfileMock).toHaveBeenCalledTimes(1);
      });
      expect(upsertRuntimeProfileMock).toHaveBeenLastCalledWith(
        expect.objectContaining({ profile_id: 'llama-router', runtime_version: 'b9082+cpu' })
      );

      await user.selectOptions(screen.getByLabelText(/runtime version/i), '');
      await user.click(screen.getByRole('button', { name: 'Save' }));
      await waitFor(() => {
        expect(upsertRuntimeProfileMock).toHaveBeenCalledTimes(2);
      });
      expect(upsertRuntimeProfileMock).toHaveBeenLastCalledWith(
        expect.objectContaining({ profile_id: 'llama-router', runtime_version: null })
      );
    });

    it('keeps a saved pin that is no longer installed visible', async () => {
      getRuntimeProfilesSnapshotMock.mockResolvedValue({
        success: true,
        snapshot: withLlamaRouter({ runtime_version: 'b8000+cpu' }),
      });

      render(<RuntimeProfileSettingsSection provider="llama_cpp" />);

      expect(await screen.findByDisplayValue('llama.cpp Router')).toBeInTheDocument();
      expect(
        await screen.findByRole('option', { name: 'b8000+cpu (not installed)' })
      ).toBeInTheDocument();
      expect(screen.getByLabelText(/runtime version/i)).toHaveValue('b8000+cpu');
    });

    it('hides the field for external profiles and drops the pin', async () => {
      const user = userEvent.setup();
      getRuntimeProfilesSnapshotMock.mockResolvedValue({
        success: true,
        snapshot: withLlamaRouter({ runtime_version: 'b9082+cpu' }),
      });

      render(<RuntimeProfileSettingsSection provider="llama_cpp" />);

      expect(await screen.findByDisplayValue('llama.cpp Router')).toBeInTheDocument();
      expect(screen.getByLabelText(/runtime version/i)).toBeInTheDocument();

      await user.selectOptions(screen.getByLabelText('Management'), 'external');
      expect(screen.queryByLabelText(/runtime version/i)).not.toBeInTheDocument();

      await user.click(screen.getByRole('button', { name: 'Save' }));
      await waitFor(() => {
        expect(upsertRuntimeProfileMock).toHaveBeenCalledTimes(1);
      });
      expect(upsertRuntimeProfileMock).toHaveBeenLastCalledWith(
        expect.objectContaining({ management_mode: 'external', runtime_version: null })
      );
    });

    it('shows the backend refusal when a pin cannot be saved', async () => {
      const user = userEvent.setup();
      upsertRuntimeProfileMock.mockResolvedValue({
        success: false,
        error: 'runtime version b9082+cpu is not installed for llama.cpp; installed: b9100+cuda-12',
        profile_id: 'llama-router',
        snapshot_required: false,
      } as Awaited<ReturnType<typeof upsertRuntimeProfileMock>>);

      render(<RuntimeProfileSettingsSection provider="llama_cpp" />);

      expect(await screen.findByDisplayValue('llama.cpp Router')).toBeInTheDocument();
      await screen.findByRole('option', { name: 'b9082+cpu' });
      await user.selectOptions(screen.getByLabelText(/runtime version/i), 'b9082+cpu');
      await user.click(screen.getByRole('button', { name: 'Save' }));

      expect(
        await screen.findByText(
          'runtime version b9082+cpu is not installed for llama.cpp; installed: b9100+cuda-12'
        )
      ).toBeInTheDocument();
    });

    it('shows the version a running instance launched from', async () => {
      getRuntimeProfilesSnapshotMock.mockResolvedValue({
        success: true,
        snapshot: withLlamaRouter({}, { state: 'running', runtime_version: 'b9082+cpu' }),
      });

      render(<RuntimeProfileSettingsSection provider="llama_cpp" />);

      expect(await screen.findByDisplayValue('llama.cpp Router')).toBeInTheDocument();
      expect(screen.getByText('running b9082+cpu')).toBeInTheDocument();
      expect(screen.getByText('State: running b9082+cpu')).toBeInTheDocument();
    });
  });
});
