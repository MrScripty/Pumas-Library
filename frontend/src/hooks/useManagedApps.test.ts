import { describe, expect, it } from 'vitest';
import { DEFAULT_APPS } from '../config/apps';
import { decorateManagedApps } from './useManagedApps';

function lifecycleState(overrides = {}) {
  return {
    isRunning: false,
    isStarting: false,
    isStopping: false,
    launchError: null,
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
      installedVersionsByAppId: {
        ollama: ['v1'],
        'llama-cpp': ['b9082'],
        'onnx-runtime': ['v1.24.2+cpu'],
        torch: [],
      },
      processes: {
        ollama: lifecycleState({ isRunning: true, ramMemory: 500, gpuMemory: 250 }),
        'llama-cpp': lifecycleState({ isStarting: true }),
        torch: lifecycleState(),
      },
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
    expect(decorated.find((app) => app.id === 'torch')).toMatchObject({
      status: 'idle',
      iconState: 'uninstalled',
    });
  });

  it('leaves an app untouched when it has neither a process nor version management', () => {
    const decorated = decorateManagedApps(DEFAULT_APPS, {
      installedVersionsByAppId: { ollama: ['v1'] },
      processes: {},
    });

    expect(decorated.find((app) => app.id === 'ollama')?.iconState).toBe('offline');
    expect(decorated.find((app) => app.id === 'onnx-runtime')).toBe(
      DEFAULT_APPS.find((app) => app.id === 'onnx-runtime')
    );
  });
});
