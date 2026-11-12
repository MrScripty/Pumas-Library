import { describe, expect, it } from 'vitest';
import { DEFAULT_APPS } from '../config/apps';
import { decorateManagedApps } from './useManagedApps';

function lifecycleState(overrides = {}) {
  return {
    isRunning: false,
    isStarting: false,
    isStopping: false,
    launchError: null,
    installedVersions: [],
    ...overrides,
  };
}

describe('decorateManagedApps', () => {
  it('decorates only the compiled inference plugins', () => {
    const decorated = decorateManagedApps(DEFAULT_APPS, {
      systemResources: {
        cpu: { usage: 0 },
        gpu: { usage: 0, memory: 0, memory_total: 1000 },
        ram: { usage: 0, total: 2000 },
        disk: { usage: 0, total: 1, free: 1 },
      },
      ollama: lifecycleState({
        isRunning: true,
        installedVersions: ['v1'],
        ramMemory: 500,
        gpuMemory: 250,
      }),
      llamaCpp: lifecycleState({ isStarting: true, installedVersions: ['b9082'] }),
      onnxRuntime: lifecycleState({ installedVersions: ['v1.24.2+cpu'] }),
      torch: lifecycleState(),
    });

    expect(decorated.map((app) => app.id)).toEqual([
      'ollama',
      'llama-cpp',
      'onnx-runtime',
      'torch',
    ]);
    expect(decorated.find((app) => app.id === 'ollama')).toMatchObject({
      status: 'running',
      iconState: 'running',
      ramUsage: 25,
      gpuUsage: 25,
    });
    expect(decorated.find((app) => app.id === 'llama-cpp')?.iconState).toBe('starting');
    expect(decorated.find((app) => app.id === 'onnx-runtime')?.iconState).toBe('offline');
  });
});
