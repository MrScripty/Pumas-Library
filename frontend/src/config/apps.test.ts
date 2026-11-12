import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { UNSUPPORTED_VERSION_STATE, isVersionSupportedAppId } from '../utils/appVersionState';
import { decorateManagedApps } from '../hooks/useManagedApps';
import { buildAppShellPanels } from '../components/AppShellPanels';
import type { ModelManagerProps } from '../components/ModelManager';
import { DEFAULT_APPS, getAppById } from './apps';

const onnxPlugin = JSON.parse(
  readFileSync(resolve(process.cwd(), '../launcher-data/plugins/onnx-runtime.json'), 'utf8')
) as {
  id: string;
  installationType: string;
  capabilities: { hasVersionManagement: boolean };
};

describe('DEFAULT_APPS', () => {
  it('registers ONNX Runtime as a downloaded in-process app without a connection URL', () => {
    const onnxRuntime = getAppById('onnx-runtime');

    expect(onnxRuntime).toBeDefined();
    expect(onnxRuntime?.displayName).toBe('ONNX Runtime');
    expect(onnxRuntime?.status).toBe('idle');
    expect(onnxRuntime?.iconState).toBe('uninstalled');
    expect(onnxRuntime?.connectionUrl).toBeUndefined();
    expect(DEFAULT_APPS.some((app) => app.id === 'onnx-runtime')).toBe(true);
  });

  it('does not expose a stale static connection URL for llama.cpp', () => {
    const llamaCpp = getAppById('llama-cpp');

    expect(llamaCpp).toBeDefined();
    expect(llamaCpp?.connectionUrl).toBeUndefined();
  });

  it('keeps hard-coded ONNX app surfaces aligned with plugin metadata', () => {
    expect(onnxPlugin).toMatchObject({
      id: 'onnx-runtime',
      installationType: 'in-process',
      capabilities: { hasVersionManagement: true },
    });
    expect(getAppById(onnxPlugin.id)).toBeDefined();
    expect(isVersionSupportedAppId(onnxPlugin.id)).toBe(
      onnxPlugin.capabilities.hasVersionManagement
    );

    const uninstalled = decorateManagedApps(DEFAULT_APPS, {
      llamaCpp: lifecycleState(),
      ollama: lifecycleState(),
      onnxRuntime: lifecycleState(),
      torch: lifecycleState(),
    });
    expect(uninstalled.find((app) => app.id === onnxPlugin.id)?.iconState).toBe('uninstalled');
    const installed = decorateManagedApps(DEFAULT_APPS, {
      llamaCpp: lifecycleState(),
      ollama: lifecycleState(),
      onnxRuntime: { ...lifecycleState(), installedVersions: ['v1.24.2+cpu'] },
      torch: lifecycleState(),
    });
    expect(installed.find((app) => app.id === onnxPlugin.id)?.iconState).toBe('offline');

    const modelManagerProps: ModelManagerProps = {
      libraryLoadStatus: 'ready',
      excludedModels: new Set(),
      modelGroups: [],
      onToggleLink: () => {},
      onToggleStar: () => {},
      selectedAppId: onnxPlugin.id,
      starredModels: new Set(),
    };
    const panels = buildAppShellPanels({
      appDisplayName: 'ONNX Runtime',
      appVersions: UNSUPPORTED_VERSION_STATE,
      diskSpacePercent: 0,
      isOllamaRunning: false,
      isTorchRunning: false,
      modelGroups: [],
      modelManagerProps,
      panelState: { showVersionManager: false },
      selectedAppId: onnxPlugin.id,
      onShowVersionManager: () => {},
    });

    expect(panels.selectedAppId).toBe(onnxPlugin.id);
    expect(panels.onnxRuntime.modelManagerProps).toBe(modelManagerProps);
    expect(panels.onnxRuntime.versions).toBe(UNSUPPORTED_VERSION_STATE);
    expect(panels.onnxRuntime.appDisplayName).toBe('ONNX Runtime');
  });

  it('does not register discontinued native applications', () => {
    expect(DEFAULT_APPS.map((app) => app.id)).toEqual([
      'ollama',
      'llama-cpp',
      'onnx-runtime',
      'torch',
    ]);
  });
});

function lifecycleState() {
  return {
    isRunning: false,
    isStarting: false,
    isStopping: false,
    launchError: null,
    installedVersions: [],
  };
}
