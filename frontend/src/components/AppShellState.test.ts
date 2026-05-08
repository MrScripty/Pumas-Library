import { describe, expect, it, vi } from 'vitest';
import { Box } from 'lucide-react';
import type { AppConfig, ModelCategory, SystemResources } from '../types/apps';
import type { StatusResponse } from '../types/api-system';
import {
  buildAppShellHeader,
  buildAppShellSidebar,
  buildManagedAppsState,
  buildModelManagerProps,
  getAppRunningState,
  getLauncherLatestVersion,
  getSelectedAppShellState,
  getSetupDisplayState,
} from './AppShellState';

const systemResources: SystemResources = {
  cpu: { usage: 10 },
  gpu: { usage: 20, memory: 1024, memory_total: 4096 },
  ram: { usage: 2048, total: 8192 },
  disk: { usage: 30, total: 1000, free: 700 },
};

const modelGroups: ModelCategory[] = [
  {
    category: 'llm',
    models: [],
  },
];

const apps: AppConfig[] = [
  {
    id: 'comfyui',
    name: 'comfyui',
    displayName: 'ComfyUI',
    icon: Box,
    status: 'idle',
    iconState: 'offline',
    connectionUrl: 'http://localhost:8188',
  },
  {
    id: 'ollama',
    name: 'ollama',
    displayName: 'Ollama',
    icon: Box,
    status: 'idle',
    iconState: 'offline',
    connectionUrl: 'http://localhost:11434',
  },
];

function createStatus(overrides: Partial<StatusResponse> = {}): StatusResponse {
  return {
    success: true,
    version: '1.0.0',
    deps_ready: true,
    patched: true,
    menu_shortcut: true,
    desktop_shortcut: true,
    shortcut_version: null,
    message: 'Ready',
    comfyui_running: true,
    ollama_running: false,
    torch_running: false,
    last_launch_error: null,
    last_launch_log: null,
    ...overrides,
  };
}

describe('AppShellState', () => {
  it('projects backend process flags into app running state', () => {
    expect(getAppRunningState(createStatus({ ollama_running: true }))).toEqual({
      comfyUIRunning: true,
      ollamaRunning: true,
      torchRunning: false,
    });

    expect(getAppRunningState(null)).toEqual({
      comfyUIRunning: false,
      ollamaRunning: false,
      torchRunning: false,
    });
  });

  it('returns selected app display metadata with a fallback', () => {
    expect(getSelectedAppShellState(apps, 'ollama')).toEqual({
      appDisplayName: 'Ollama',
      connectionUrl: 'http://localhost:11434',
    });

    expect(getSelectedAppShellState(apps, null)).toEqual({
      appDisplayName: 'App',
      connectionUrl: undefined,
    });
  });

  it('derives setup display state and hides ready boilerplate', () => {
    const readyState = getSetupDisplayState(
      createStatus({ message: 'System ready. Configure options below' }),
      'comfyui'
    );

    expect(readyState).toEqual({
      activeShortcutState: { menu: true, desktop: true },
      depsInstalled: true,
      displayStatus: '',
      isSetupComplete: true,
    });

    expect(getSetupDisplayState(createStatus({ message: 'Installing' }), 'ollama')).toEqual({
      activeShortcutState: undefined,
      depsInstalled: true,
      displayStatus: 'Installing',
      isSetupComplete: true,
    });
  });

  it('builds managed app state from process and resource inputs', () => {
    const managedState = buildManagedAppsState({
      running: getAppRunningState(createStatus()),
      status: createStatus({
        app_resources: {
          comfyui: { ram_memory: 1024, gpu_memory: 512 },
          ollama: { ram_memory: 2048, gpu_memory: 1024 },
        },
      }),
      systemResources,
      comfyui: {
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: ['v1'],
      },
      ollama: {
        isStarting: true,
        isStopping: false,
        launchError: 'failed',
        installedVersions: [],
      },
      llamaCpp: {
        isStarting: false,
        isStopping: false,
        launchError: null,
        installedVersions: ['llama.cpp'],
      },
      torch: {
        isStarting: false,
        isStopping: true,
        launchError: null,
        installedVersions: ['torch'],
      },
    });

    expect(managedState.systemResources).toBe(systemResources);
    expect(managedState.comfyui).toMatchObject({
      isRunning: true,
      ramMemory: 1024,
      gpuMemory: 512,
      installedVersions: ['v1'],
    });
    expect(managedState.ollama).toMatchObject({
      isRunning: false,
      isStarting: true,
      launchError: 'failed',
      ramMemory: 2048,
      gpuMemory: 1024,
    });
    expect(managedState.torch).toMatchObject({
      isRunning: false,
      isStopping: true,
      installedVersions: ['torch'],
    });
    expect(managedState.llamaCpp).toMatchObject({
      isRunning: false,
      installedVersions: ['llama.cpp'],
    });
  });

  it('builds model manager and shell props without changing callbacks', () => {
    const onToggleStar = vi.fn();
    const onToggleLink = vi.fn();
    const onAddModels = vi.fn();
    const modelManagerProps = buildModelManagerProps({
      activeVersion: 'v1',
      excludedModels: new Set(['excluded']),
      modelGroups,
      selectedAppId: 'comfyui',
      starredModels: new Set(['starred']),
      onAddModels,
      onChooseExistingLibrary: vi.fn(),
      onModelsImported: vi.fn(),
      onOpenModelsRoot: vi.fn(),
      onToggleLink,
      onToggleStar,
    });

    const sidebar = buildAppShellSidebar({
      apps,
      selectedAppId: 'comfyui',
      onAddApp: vi.fn(),
      onDeleteApp: vi.fn(),
      onLaunchApp: vi.fn(),
      onOpenLog: vi.fn(),
      onReorderApps: vi.fn(),
      onSelectApp: vi.fn(),
      onStopApp: vi.fn(),
    });

    expect(modelManagerProps.activeVersion).toBe('v1');
    expect(modelManagerProps.modelGroups).toBe(modelGroups);
    expect(modelManagerProps.onToggleStar).toBe(onToggleStar);
    expect(modelManagerProps.onToggleLink).toBe(onToggleLink);
    expect(modelManagerProps.onAddModels).toBe(onAddModels);
    expect(sidebar.apps).toBe(apps);
    expect(sidebar.selectedAppId).toBe('comfyui');
  });

  it('builds header props and wraps async update actions', () => {
    const onCheckLauncherUpdates = vi.fn().mockResolvedValue(undefined);
    const onDownloadLauncherUpdate = vi.fn().mockResolvedValue(undefined);
    const header = buildAppShellHeader({
      activeModelDownload: null,
      activeModelDownloadCount: 2,
      installationProgress: null,
      isCheckingLauncherUpdates: true,
      launcherLatestVersion: getLauncherLatestVersion({ latestVersion: 'v2', releaseUrl: null, downloadUrl: null }),
      launcherUpdateAvailable: true,
      modelLibraryLoaded: true,
      networkAvailable: true,
      status: createStatus({ app_resources: { comfyui: { ram_memory: 128 } } }),
      systemResources,
      onCheckLauncherUpdates,
      onClose: vi.fn(),
      onDownloadLauncherUpdate,
      onMinimize: vi.fn(),
    });

    header.onCheckLauncherUpdates?.();
    header.onDownloadLauncherUpdate?.();

    expect(header.appResources).toEqual({ ram_memory: 128 });
    expect(header.launcherLatestVersion).toBe('v2');
    expect(onCheckLauncherUpdates).toHaveBeenCalledTimes(1);
    expect(onDownloadLauncherUpdate).toHaveBeenCalledTimes(1);
  });
});
