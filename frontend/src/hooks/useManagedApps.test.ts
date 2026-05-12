import { describe, expect, it } from 'vitest';
import { DEFAULT_APPS } from '../config/apps';
import { decorateManagedApps } from './useManagedApps';

describe('decorateManagedApps', () => {
  it('prioritizes transition states over offline and error states', () => {
    const decorated = decorateManagedApps(DEFAULT_APPS, {
      comfyui: {
        isRunning: false,
        isStarting: true,
        isStopping: false,
        launchError: 'failed previously',
        installedVersions: ['v1'],
      },
      ollama: {
        isRunning: false,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: [],
      },
      llamaCpp: {
        isRunning: false,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: [],
      },
      torch: {
        isRunning: false,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: [],
      },
    });

    expect(decorated.find((app) => app.id === 'comfyui')?.iconState).toBe('starting');
  });

  it('derives resource percentages for managed apps with memory data', () => {
    const decorated = decorateManagedApps(DEFAULT_APPS, {
      systemResources: {
        cpu: { usage: 0 },
        gpu: { usage: 0, memory: 0, memory_total: 1000 },
        ram: { usage: 0, total: 2000 },
        disk: { usage: 0, total: 1, free: 1 },
      },
      comfyui: {
        isRunning: true,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: ['v1'],
        ramMemory: 500,
        gpuMemory: 250,
      },
      ollama: {
        isRunning: false,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: [],
      },
      llamaCpp: {
        isRunning: false,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: ['b1'],
      },
      torch: {
        isRunning: false,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: [],
      },
    });

    const comfyui = decorated.find((app) => app.id === 'comfyui');
    expect(comfyui?.ramUsage).toBe(25);
    expect(comfyui?.gpuUsage).toBe(25);
    expect(comfyui?.status).toBe('running');
    expect(decorated.find((app) => app.id === 'llama-cpp')?.iconState).toBe('offline');
  });

  it('marks llama.cpp as running when runtime state reports it running', () => {
    const decorated = decorateManagedApps(DEFAULT_APPS, {
      comfyui: {
        isRunning: false,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: [],
      },
      ollama: {
        isRunning: false,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: [],
      },
      llamaCpp: {
        isRunning: true,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: ['b9082'],
      },
      torch: {
        isRunning: false,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: [],
      },
    });

    const llamaCpp = decorated.find((app) => app.id === 'llama-cpp');
    expect(llamaCpp?.status).toBe('running');
    expect(llamaCpp?.iconState).toBe('running');
  });

  it('keeps in-process ONNX Runtime out of version-managed decoration', () => {
    const decorated = decorateManagedApps(DEFAULT_APPS, {
      comfyui: {
        isRunning: true,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: ['v1'],
      },
      ollama: {
        isRunning: true,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: ['v1'],
      },
      llamaCpp: {
        isRunning: true,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: ['v1'],
      },
      torch: {
        isRunning: true,
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: ['v1'],
      },
    });

    const onnxRuntime = decorated.find((app) => app.id === 'onnx-runtime');
    expect(onnxRuntime?.status).toBe('idle');
    expect(onnxRuntime?.iconState).toBe('offline');
  });
});
