import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type {
  RuntimeProfileConfig,
  RuntimeProfilesSnapshot,
} from '../../../types/api-runtime-profiles';
import { RuntimeProfileSettingsSection } from './RuntimeProfileSettingsSection';

const {
  getElectronAPIMock,
  getRuntimeProfilesSnapshotMock,
  launchRuntimeProfileMock,
  stopRuntimeProfileMock,
  upsertRuntimeProfileMock,
} =
  vi.hoisted(() => ({
    getElectronAPIMock: vi.fn(),
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
    ],
    routes: [],
    statuses: [
      { profile_id: 'ollama-default', state: 'stopped' },
      { profile_id: 'llama-router', state: 'stopped' },
    ],
    default_profile_id: 'ollama-default',
  };

  beforeEach(() => {
    vi.clearAllMocks();
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
    getElectronAPIMock.mockReturnValue({
      get_runtime_profiles_snapshot: getRuntimeProfilesSnapshotMock,
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
});
