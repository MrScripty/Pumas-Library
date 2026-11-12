import { describe, expect, it, vi } from 'vitest';
import { Box } from 'lucide-react';
import type { AppConfig, SystemResources } from '../types/apps';
import type { StatusResponse } from '../types/api-system';
import {
  buildAppShellHeader,
  buildAppShellSidebar,
  buildManagedAppsState,
  getAppRunningState,
  getSelectedAppShellState,
} from './AppShellState';

const systemResources: SystemResources = {
  cpu: { usage: 10 },
  gpu: { usage: 20, memory: 1024, memory_total: 4096 },
  ram: { usage: 2048, total: 8192 },
  disk: { usage: 30, total: 1000, free: 700 },
};

const apps: AppConfig[] = [{
  id: 'ollama',
  name: 'ollama',
  displayName: 'Ollama',
  icon: Box,
  status: 'idle',
  iconState: 'offline',
  connectionUrl: 'http://localhost:11434',
}];

function createStatus(overrides: Partial<StatusResponse> = {}): StatusResponse {
  return {
    success: true,
    version: '1.0.0',
    message: 'Ready',
    ollama_running: false,
    torch_running: false,
    last_launch_error: null,
    last_launch_log: null,
    ...overrides,
  };
}

function lifecycleState() {
  return {
    isStarting: false,
    isStopping: false,
    launchError: null,
    installedVersions: [],
  };
}

describe('AppShellState', () => {
  it('projects only supported process flags into app state', () => {
    expect(getAppRunningState(createStatus({ ollama_running: true }))).toEqual({
      ollamaRunning: true,
      torchRunning: false,
    });
  });

  it('returns selected inference plugin metadata', () => {
    expect(getSelectedAppShellState(apps, 'ollama')).toEqual({
      appDisplayName: 'Ollama',
      connectionUrl: 'http://localhost:11434',
    });
  });

  it('builds managed state for supported inference plugins only', () => {
    const managed = buildManagedAppsState({
      running: getAppRunningState(createStatus({ ollama_running: true })),
      status: createStatus({ app_resources: { ollama: { ram_memory: 2048 } } }),
      systemResources,
      ollama: lifecycleState(),
      llamaCpp: { ...lifecycleState(), isRunning: false },
      onnxRuntime: { installedVersions: ['v1.24.2+cpu'] },
      torch: lifecycleState(),
    });

    expect(managed.ollama).toMatchObject({ isRunning: true, ramMemory: 2048 });
    expect(managed.onnxRuntime).toMatchObject({
      isRunning: false,
      installedVersions: ['v1.24.2+cpu'],
    });
    expect(Object.keys(managed).sort()).toEqual([
      'llamaCpp',
      'ollama',
      'onnxRuntime',
      'systemResources',
      'torch',
    ]);
  });

  it('builds a fixed inference sidebar contract', () => {
    const sidebar = buildAppShellSidebar({
      apps,
      selectedAppId: 'ollama',
      onLaunchApp: vi.fn(),
      onOpenLog: vi.fn(),
      onSelectApp: vi.fn(),
      onStopApp: vi.fn(),
    });

    expect(sidebar.apps).toBe(apps);
    expect(sidebar).not.toHaveProperty('onAddApp');
    expect(sidebar).not.toHaveProperty('onDeleteApp');
  });

  it('uses inference resource telemetry in the header', () => {
    const header = buildAppShellHeader({
      activeModelDownload: null,
      activeModelDownloadCount: 0,
      installationProgress: null,
      isCheckingLauncherUpdates: false,
      launcherLatestVersion: null,
      launcherUpdateAvailable: false,
      modelLibraryLoaded: true,
      networkAvailable: true,
      status: createStatus({ app_resources: { ollama: { ram_memory: 128 } } }),
      systemResources,
      onCheckLauncherUpdates: vi.fn().mockResolvedValue(undefined),
      onClose: vi.fn(),
      onDownloadLauncherUpdate: vi.fn().mockResolvedValue(undefined),
      onMinimize: vi.fn(),
    });

    expect(header.appResources).toEqual({ ram_memory: 128 });
  });
});
