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
) as { id: string; installationType: string };

describe('DEFAULT_APPS', () => {
  it('registers ONNX Runtime as an in-process app without a connection URL', () => {
    const onnxRuntime = getAppById('onnx-runtime');

    expect(onnxRuntime).toBeDefined();
    expect(onnxRuntime?.displayName).toBe('ONNX Runtime');
    expect(onnxRuntime?.status).toBe('idle');
    expect(onnxRuntime?.iconState).toBe('offline');
    expect(onnxRuntime?.connectionUrl).toBeUndefined();
    expect(DEFAULT_APPS.some((app) => app.id === 'onnx-runtime')).toBe(true);
  });

  it('keeps hard-coded ONNX app surfaces aligned with plugin metadata', () => {
    expect(onnxPlugin).toMatchObject({
      id: 'onnx-runtime',
      installationType: 'in-process',
    });
    expect(getAppById(onnxPlugin.id)).toBeDefined();
    expect(isVersionSupportedAppId(onnxPlugin.id)).toBe(false);

    const decorated = decorateManagedApps(DEFAULT_APPS, {
      comfyui: lifecycleState(),
      llamaCpp: lifecycleState(),
      ollama: lifecycleState(),
      torch: lifecycleState(),
    });
    expect(decorated.find((app) => app.id === onnxPlugin.id)?.iconState).toBe('offline');

    const modelManagerProps: ModelManagerProps = {
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
      comfyUIRunning: false,
      depsInstalled: null,
      diskSpacePercent: 0,
      displayStatus: 'Idle',
      isCheckingDeps: false,
      isInstallingDeps: false,
      isOllamaRunning: false,
      isSetupComplete: true,
      isTorchRunning: false,
      modelGroups: [],
      modelManagerProps,
      panelState: { showVersionManager: false },
      selectedAppId: onnxPlugin.id,
      onInstallDeps: () => {},
      onShowVersionManager: () => {},
    });

    expect(panels.selectedAppId).toBe(onnxPlugin.id);
    expect(panels.onnxRuntime.modelManagerProps).toBe(modelManagerProps);
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
