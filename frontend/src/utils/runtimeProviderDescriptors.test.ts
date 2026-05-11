import { describe, expect, it } from 'vitest';
import type { ModelCategory, ModelInfo } from '../types/apps';
import {
  deviceModeLabel,
  filterProviderCompatibleModelGroups,
  getRuntimeProviderDescriptor,
  isModelCompatibleWithProvider,
  modeLabel,
  providerLabel,
} from './runtimeProviderDescriptors';

function model(overrides: Partial<ModelInfo> = {}): ModelInfo {
  return {
    id: 'models/chat.gguf',
    name: 'Chat GGUF',
    category: 'chat',
    ...overrides,
  };
}

describe('runtime provider descriptors', () => {
  it('declares existing provider labels, modes, and capability flags', () => {
    expect(providerLabel('ollama')).toBe('Ollama');
    expect(providerLabel('llama_cpp')).toBe('llama.cpp');
    expect(modeLabel('llama_cpp_dedicated')).toBe('Dedicated');
    expect(deviceModeLabel('specific_device')).toBe('Specific device');

    expect(getRuntimeProviderDescriptor('ollama')).toMatchObject({
      profileModes: ['ollama_serve'],
      supportsGpuLayers: false,
      supportsContextSize: false,
    });
    expect(getRuntimeProviderDescriptor('llama_cpp')).toMatchObject({
      profileModes: ['llama_cpp_router', 'llama_cpp_dedicated'],
      dedicatedPlacementModes: ['llama_cpp_dedicated'],
      supportsGpuLayers: true,
      supportsContextSize: true,
    });
  });

  it('uses descriptor executable formats for model compatibility', () => {
    expect(
      isModelCompatibleWithProvider(
        model({
          id: 'artifact-case',
          path: '/models/artifact.Q4_K_M.GGUF',
        }),
        'llama_cpp'
      )
    ).toBe(true);
    expect(
      isModelCompatibleWithProvider(
        model({ id: 'artifact', path: '/models/artifact.Q4_K_M.gguf' }),
        'llama_cpp'
      )
    ).toBe(true);
    expect(
      isModelCompatibleWithProvider(
        model({ id: 'onnx', path: '/models/model.onnx' }),
        'llama_cpp'
      )
    ).toBe(false);
  });

  it('filters model groups through provider compatibility', () => {
    const groups: ModelCategory[] = [
      {
        category: 'language',
        models: [
          model({ id: 'chat', primaryFormat: 'gguf' }),
          model({ id: 'onnx', path: '/models/model.onnx' }),
        ],
      },
      {
        category: 'audio',
        models: [model({ id: 'audio', path: '/models/audio.onnx' })],
      },
    ];

    expect(filterProviderCompatibleModelGroups(groups, 'llama_cpp')).toEqual([
      {
        category: 'language',
        models: [expect.objectContaining({ id: 'chat' })],
      },
    ]);
  });
});
