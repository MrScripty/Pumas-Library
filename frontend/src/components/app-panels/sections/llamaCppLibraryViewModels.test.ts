import { describe, expect, it } from 'vitest';
import type {
  ModelRuntimeRoute,
  RuntimeProfileConfig,
} from '../../../types/api-runtime-profiles';
import type { ServedModelStatus } from '../../../types/api-serving';
import type { ModelCategory, ModelInfo } from '../../../types/apps';
import {
  buildLlamaCppModelRows,
  buildServedInstanceKey,
  deriveLlamaCppServedState,
  filterLlamaCppCompatibleModelGroups,
  getLlamaCppPlacementLabel,
} from './llamaCppLibraryViewModels';

function model(overrides: Partial<ModelInfo> = {}): ModelInfo {
  return {
    id: 'models/chat.gguf',
    name: 'Chat GGUF',
    category: 'chat',
    ...overrides,
  };
}

function profile(overrides: Partial<RuntimeProfileConfig> = {}): RuntimeProfileConfig {
  return {
    profile_id: 'llama-cpu',
    provider: 'llama_cpp',
    provider_mode: 'llama_cpp_dedicated',
    management_mode: 'managed',
    name: 'llama.cpp CPU',
    enabled: true,
    endpoint_url: 'http://127.0.0.1:18080/',
    port: 18080,
    device: { mode: 'cpu' },
    scheduler: { auto_load: true },
    ...overrides,
  };
}

function route(overrides: Partial<ModelRuntimeRoute> = {}): ModelRuntimeRoute {
  return {
    model_id: 'models/chat.gguf',
    profile_id: 'llama-cpu',
    auto_load: false,
    ...overrides,
  };
}

function servedStatus(overrides: Partial<ServedModelStatus> = {}): ServedModelStatus {
  return {
    model_id: 'models/chat.gguf',
    model_alias: 'chat-cpu',
    provider: 'llama_cpp',
    profile_id: 'llama-cpu',
    load_state: 'loaded',
    device_mode: 'cpu',
    keep_loaded: true,
    ...overrides,
  };
}

describe('llama.cpp library view models', () => {
  it('keeps GGUF-compatible models and removes incompatible or empty groups', () => {
    const groups: ModelCategory[] = [
      {
        category: 'language',
        models: [
          model({ id: 'primary', primaryFormat: 'gguf', path: '/models/primary.bin' }),
          model({ id: 'format', format: 'GGUF', path: '/models/format.bin' }),
          model({ id: 'artifact', path: '/models/artifact.Q4_K_M.gguf' }),
          model({ id: 'safe', primaryFormat: 'safetensors', path: '/models/safe.safetensors' }),
        ],
      },
      {
        category: 'diffusion',
        models: [
          model({
            id: 'image',
            category: 'image',
            format: 'safetensors',
            path: '/models/image.safetensors',
          }),
        ],
      },
      {
        category: 'audio',
        models: [
          model({
            id: 'audio',
            category: 'audio',
            format: 'onnx',
            path: '/models/audio.onnx',
          }),
        ],
      },
    ];

    expect(filterLlamaCppCompatibleModelGroups(groups)).toEqual([
      {
        category: 'language',
        models: [
          expect.objectContaining({ id: 'primary' }),
          expect.objectContaining({ id: 'format' }),
          expect.objectContaining({ id: 'artifact' }),
        ],
      },
    ]);
  });

  it('builds stable served-instance keys from model, profile, and alias', () => {
    expect(
      buildServedInstanceKey({
        modelId: 'models/chat.gguf',
        profileId: 'llama-gpu',
        modelAlias: 'chat/gpu',
      })
    ).toBe('["models/chat.gguf","llama-gpu","chat/gpu"]');

    expect(
      buildServedInstanceKey({
        modelId: 'models/chat.gguf',
        profileId: 'llama-gpu',
        modelAlias: null,
      })
    ).toBe('["models/chat.gguf","llama-gpu",""]');
  });

  it('groups multiple served statuses for the same model without collapsing instances', () => {
    const statuses = [
      servedStatus({ model_alias: 'chat-cpu', profile_id: 'llama-cpu', device_mode: 'cpu' }),
      servedStatus({ model_alias: 'chat-gpu', profile_id: 'llama-gpu', device_mode: 'gpu' }),
      servedStatus({
        model_alias: 'chat-ollama',
        provider: 'ollama',
        profile_id: 'ollama-default',
      }),
    ];

    const state = deriveLlamaCppServedState(statuses);

    expect(state.servedStatusesByModelId.get('models/chat.gguf')).toEqual(statuses.slice(0, 2));
    expect(state.servedStatusByInstanceKey.size).toBe(2);
    expect(
      state.servedStatusByInstanceKey.get(
        '["models/chat.gguf","llama-gpu","chat-gpu"]'
      )?.device_mode
    ).toBe('gpu');
  });

  it('derives selected-profile served status and backend-confirmed placement', () => {
    const rows = buildLlamaCppModelRows({
      modelGroups: [{ category: 'chat', models: [model({ primaryFormat: 'gguf' })] }],
      profiles: [
        profile({ profile_id: 'llama-cpu', device: { mode: 'cpu' } }),
        profile({ profile_id: 'llama-gpu', device: { mode: 'gpu' } }),
      ],
      routes: [route({ profile_id: 'llama-gpu' })],
      servedStatuses: [
        servedStatus({ profile_id: 'llama-cpu', model_alias: 'chat-cpu', device_mode: 'cpu' }),
        servedStatus({ profile_id: 'llama-gpu', model_alias: 'chat-gpu', device_mode: 'gpu' }),
      ],
    });

    expect(rows).toHaveLength(1);
    expect(rows[0]?.routeState).toBe('routed');
    expect(rows[0]?.selectedProfile?.profile_id).toBe('llama-gpu');
    expect(rows[0]?.selectedServedStatus?.model_alias).toBe('chat-gpu');
    expect(rows[0]?.servedStatuses).toHaveLength(2);
    expect(rows[0]?.servedInstanceKeys).toEqual([
      '["models/chat.gguf","llama-cpu","chat-cpu"]',
      '["models/chat.gguf","llama-gpu","chat-gpu"]',
    ]);
    expect(rows[0]?.servedPlacement).toEqual({
      label: 'GPU',
      source: 'served_status',
    });
  });

  it('marks a saved route whose profile is missing without choosing another profile', () => {
    const rows = buildLlamaCppModelRows({
      modelGroups: [{ category: 'chat', models: [model({ primaryFormat: 'gguf' })] }],
      profiles: [profile({ profile_id: 'llama-cpu' })],
      routes: [route({ profile_id: 'deleted-profile' })],
      servedStatuses: [],
    });

    expect(rows).toHaveLength(1);
    expect(rows[0]?.routeState).toBe('missing_profile');
    expect(rows[0]?.selectedProfile).toBeNull();
    expect(rows[0]?.selectedProfilePlacement).toBeNull();
    expect(rows[0]?.selectedServedStatus).toBeNull();
  });

  it('treats routes to non-llama profiles as missing on the llama.cpp page', () => {
    const rows = buildLlamaCppModelRows({
      modelGroups: [{ category: 'chat', models: [model({ primaryFormat: 'gguf' })] }],
      profiles: [
        profile({
          profile_id: 'ollama-default',
          provider: 'ollama',
          provider_mode: 'ollama_serve',
        }),
      ],
      routes: [route({ profile_id: 'ollama-default' })],
      servedStatuses: [
        servedStatus({
          provider: 'ollama',
          profile_id: 'ollama-default',
          model_alias: 'chat-ollama',
        }),
      ],
    });

    expect(rows).toHaveLength(1);
    expect(rows[0]?.routeState).toBe('missing_profile');
    expect(rows[0]?.selectedProfile).toBeNull();
    expect(rows[0]?.servedStatuses).toEqual([]);
  });

  it('uses profile placement when a selected profile has not loaded the model yet', () => {
    expect(
      getLlamaCppPlacementLabel({
        profile: profile({
          profile_id: 'llama-igpu',
          device: { mode: 'specific_device', device_id: 'integrated-gpu-0' },
        }),
      })
    ).toEqual({
      label: 'iGPU',
      source: 'profile',
    });
  });
});
