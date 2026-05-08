import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { RuntimeProfilesSnapshot } from '../../../types/api-runtime-profiles';
import { RuntimeProfileSettingsSection } from './RuntimeProfileSettingsSection';

const { getElectronAPIMock, getRuntimeProfilesSnapshotMock, upsertRuntimeProfileMock } =
  vi.hoisted(() => ({
    getElectronAPIMock: vi.fn(),
    getRuntimeProfilesSnapshotMock: vi.fn(),
    upsertRuntimeProfileMock: vi.fn(),
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
    ],
    routes: [],
    statuses: [{ profile_id: 'ollama-default', state: 'stopped' }],
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
    getElectronAPIMock.mockReturnValue({
      get_runtime_profiles_snapshot: getRuntimeProfilesSnapshotMock,
      upsert_runtime_profile: upsertRuntimeProfileMock,
      onRuntimeProfileUpdate: vi.fn(() => vi.fn()),
    });
  });

  it('keeps a new managed profile draft from inheriting the default Ollama port', async () => {
    const user = userEvent.setup();

    render(<RuntimeProfileSettingsSection />);

    expect(await screen.findByDisplayValue('Ollama Default')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'New runtime profile' }));

    expect(screen.getByDisplayValue('New Runtime Profile')).toBeInTheDocument();
    expect(screen.getByRole('spinbutton', { name: /managed process port/i })).toHaveValue(null);

    await user.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(upsertRuntimeProfileMock).toHaveBeenCalledTimes(1);
    });
    expect(upsertRuntimeProfileMock).toHaveBeenCalledWith(
      expect.objectContaining({
        profile_id: expect.not.stringMatching(/^ollama-default$/),
        management_mode: 'managed',
        endpoint_url: null,
        port: null,
      })
    );
  });
});
