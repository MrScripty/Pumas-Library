import { describe, expect, it } from 'vitest';
import type {
  ModelRuntimeRoute,
  RuntimeProfileConfig,
} from '../../../types/api-runtime-profiles';
import type { ModelCategory, ModelInfo } from '../../../types/apps';
import { buildOnnxRuntimeModelRows } from './onnxRuntimeLibraryViewModels';

function model(overrides: Partial<ModelInfo> = {}): ModelInfo {
  return {
    id: 'models/embedding.onnx',
    name: 'Embedding ONNX',
    category: 'embeddings',
    primaryFormat: 'onnx',
    ...overrides,
  };
}

function profile(overrides: Partial<RuntimeProfileConfig> = {}): RuntimeProfileConfig {
  return {
    profile_id: 'onnx-cpu',
    provider: 'onnx_runtime',
    provider_mode: 'onnx_serve',
    management_mode: 'managed',
    name: 'ONNX CPU',
    enabled: true,
    endpoint_url: 'http://127.0.0.1:18091/',
    port: 18091,
    device: { mode: 'cpu' },
    scheduler: { auto_load: true },
    ...overrides,
  };
}

function route(overrides: Partial<ModelRuntimeRoute> = {}): ModelRuntimeRoute {
  return {
    provider: 'onnx_runtime',
    model_id: 'models/embedding.onnx',
    profile_id: 'onnx-cpu',
    auto_load: true,
    ...overrides,
  };
}

describe('ONNX Runtime library view models', () => {
  it('keeps ONNX-compatible models and removes incompatible groups', () => {
    const groups: ModelCategory[] = [
      {
        category: 'embeddings',
        models: [
          model({ id: 'primary', primaryFormat: 'onnx' }),
          model({ id: 'format', format: 'ONNX' }),
          model({ id: 'artifact', selectedArtifactFiles: ['model.onnx'] }),
          model({ id: 'gguf', primaryFormat: 'gguf', path: '/models/model.gguf' }),
        ],
      },
      {
        category: 'chat',
        models: [model({ id: 'chat', primaryFormat: 'gguf' })],
      },
    ];

    const rows = buildOnnxRuntimeModelRows({ modelGroups: groups, profiles: [], routes: [] });

    expect(rows.map((row) => row.model.id)).toEqual(['primary', 'format', 'artifact']);
  });

  it('uses only ONNX Runtime profiles and routes for route state', () => {
    const rows = buildOnnxRuntimeModelRows({
      modelGroups: [
        {
          category: 'embeddings',
          models: [model()],
        },
      ],
      profiles: [
        profile(),
        profile({
          profile_id: 'llama-cpu',
          provider: 'llama_cpp',
          provider_mode: 'llama_cpp_dedicated',
        }),
      ],
      routes: [
        route(),
        route({
          provider: 'llama_cpp',
          profile_id: 'llama-cpu',
        }),
      ],
    });

    expect(rows).toHaveLength(1);
    expect(rows[0]?.routeState).toBe('routed');
    expect(rows[0]?.route?.provider).toBe('onnx_runtime');
    expect(rows[0]?.selectedProfile?.profile_id).toBe('onnx-cpu');
  });

  it('marks a saved ONNX route whose profile is missing', () => {
    const rows = buildOnnxRuntimeModelRows({
      modelGroups: [
        {
          category: 'embeddings',
          models: [model()],
        },
      ],
      profiles: [profile()],
      routes: [route({ profile_id: 'deleted-profile' })],
    });

    expect(rows).toHaveLength(1);
    expect(rows[0]?.routeState).toBe('missing_profile');
    expect(rows[0]?.selectedProfile).toBeNull();
  });
});
