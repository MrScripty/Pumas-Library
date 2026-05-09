import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { RuntimeProfilesSnapshot } from '../types/api-runtime-profiles';
import { ModelServeDialog } from './ModelServeDialog';

const { getElectronAPIMock, useRuntimeProfilesMock } = vi.hoisted(() => ({
  getElectronAPIMock: vi.fn(),
  useRuntimeProfilesMock: vi.fn(),
}));

vi.mock('../api/adapter', () => ({
  getElectronAPI: getElectronAPIMock,
}));

vi.mock('../hooks/useRuntimeProfiles', () => ({
  useRuntimeProfiles: useRuntimeProfilesMock,
}));

describe('ModelServeDialog', () => {
  const snapshot: RuntimeProfilesSnapshot = {
    schema_version: 1,
    cursor: 'runtime-profiles:1',
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
        profile_id: 'emily-llama',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_dedicated',
        management_mode: 'managed',
        name: 'Emily Llama',
        enabled: true,
        endpoint_url: null,
        port: null,
        device: { mode: 'gpu', gpu_layers: 32 },
        scheduler: { auto_load: true },
      },
      {
        profile_id: 'cpu-llama',
        provider: 'llama_cpp',
        provider_mode: 'llama_cpp_dedicated',
        management_mode: 'managed',
        name: 'CPU Llama',
        enabled: true,
        endpoint_url: null,
        port: null,
        device: { mode: 'cpu' },
        scheduler: { auto_load: true },
      },
    ],
    routes: [],
    statuses: [],
    default_profile_id: 'ollama-default',
  };

  beforeEach(() => {
    vi.clearAllMocks();
    useRuntimeProfilesMock.mockReturnValue({
      snapshot,
      profiles: snapshot.profiles,
      routes: snapshot.routes,
      statuses: snapshot.statuses,
      defaultProfileId: snapshot.default_profile_id,
      cursor: snapshot.cursor,
      isLoading: false,
      error: null,
      refreshRuntimeProfiles: vi.fn(),
    });
    getElectronAPIMock.mockReturnValue({
      get_serving_status: vi.fn().mockResolvedValue({
        success: true,
        snapshot: {
          cursor: 'serving:0',
          endpoint: { endpoint_mode: 'not_configured', model_count: 0 },
          served_models: [],
          recent_errors: [],
        },
      }),
    });
  });

  it('uses the route editor selected profile when opening the dialog', async () => {
    render(
      <ModelServeDialog
        model={{
          id: 'model-1',
          name: 'Model One',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    expect(screen.getByRole('combobox', { name: /runtime target/i })).toHaveValue('emily-llama');
    expect(screen.getByText('Ready to serve Model One with Emily Llama.')).toBeInTheDocument();
  });

  it('explains why the serve action is blocked', () => {
    render(
      <ModelServeDialog
        model={{
          id: 'model-2',
          name: 'Model Two',
          category: 'local',
          primaryFormat: 'safetensors',
        }}
        initialProfileId="emily-llama"
        onClose={vi.fn()}
      />
    );

    expect(
      screen.getByText('Cannot serve yet: Only GGUF models can be served locally in this flow.')
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Start serving' })).toBeDisabled();
  });

  it('hides GPU-only controls for CPU dedicated profiles and defaults context', () => {
    render(
      <ModelServeDialog
        model={{
          id: 'model-3',
          name: 'Model Three',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        displayMode="page"
        initialProfileId="cpu-llama"
        onBack={vi.fn()}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByRole('combobox', { name: /runtime target/i })).toHaveValue('cpu-llama');
    expect(screen.getByRole('combobox', { name: /model device/i })).toHaveValue('cpu');
    expect(screen.getByRole('spinbutton', { name: /context/i })).toHaveValue(4096);
    expect(screen.queryByRole('spinbutton', { name: /model gpu layers/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: /model tensor split/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: /device id/i })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Start serving' })).toBeEnabled();
  });

  it('prefers a running llama.cpp target for GGUF models when no route is selected', () => {
    useRuntimeProfilesMock.mockReturnValue({
      snapshot: {
        ...snapshot,
        statuses: [
          {
            profile_id: 'emily-llama',
            state: 'running',
            endpoint_url: 'http://127.0.0.1:18080',
            pid: 1234,
            log_path: null,
            last_error: null,
          },
        ],
      },
      profiles: snapshot.profiles,
      routes: snapshot.routes,
      statuses: [
        {
          profile_id: 'emily-llama',
          state: 'running',
          endpoint_url: 'http://127.0.0.1:18080',
          pid: 1234,
          log_path: null,
          last_error: null,
        },
      ],
      defaultProfileId: snapshot.default_profile_id,
      cursor: snapshot.cursor,
      isLoading: false,
      error: null,
      refreshRuntimeProfiles: vi.fn(),
    });

    render(
      <ModelServeDialog
        model={{
          id: 'model-4',
          name: 'Model Four',
          category: 'local',
          primaryFormat: 'gguf',
        }}
        onClose={vi.fn()}
      />
    );

    expect(screen.getByRole('combobox', { name: /runtime target/i })).toHaveValue('emily-llama');
    expect(screen.getByText('Ready to serve Model Four with Emily Llama.')).toBeInTheDocument();
  });
});
