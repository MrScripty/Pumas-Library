import { describe, expect, it, vi } from 'vitest';
import type { ModelCategory } from '../types/apps';
import { UNSUPPORTED_VERSION_STATE } from '../utils/appVersionState';
import type { ModelManagerProps } from './ModelManager';
import { buildAppShellPanels } from './AppShellPanels';

const modelGroups: ModelCategory[] = [
  {
    category: 'llm',
    models: [],
  },
];

const modelManagerProps: ModelManagerProps = {
  excludedModels: new Set(),
  modelGroups,
  onToggleLink: vi.fn(),
  onToggleStar: vi.fn(),
  selectedAppId: 'ollama',
  starredModels: new Set(),
};

describe('buildAppShellPanels', () => {
  it('builds shared version props and app-specific panel props', () => {
    const panels = buildAppShellPanels({
      activeShortcutState: { menu: true, desktop: false },
      appDisplayName: 'Ollama',
      appVersions: UNSUPPORTED_VERSION_STATE,
      comfyUIRunning: false,
      connectionUrl: 'http://localhost:11434',
      depsInstalled: true,
      diskSpacePercent: 42,
      displayStatus: 'Ready',
      isCheckingDeps: false,
      isInstallingDeps: false,
      isOllamaRunning: true,
      isSetupComplete: true,
      isTorchRunning: false,
      modelGroups,
      modelManagerProps,
      panelState: { showVersionManager: true },
      selectedAppId: 'ollama',
      onInstallDeps: vi.fn(),
      onShowVersionManager: vi.fn(),
    });

    expect(panels.selectedAppId).toBe('ollama');
    expect(panels.comfyUI.displayStatus).toBe('Ready');
    expect(panels.comfyUI.diskSpacePercent).toBe(42);
    expect(panels.ollama.connectionUrl).toBe('http://localhost:11434');
    expect(panels.ollama.isOllamaRunning).toBe(true);
    expect(panels.llamaCpp.connectionUrl).toBe('http://localhost:11434');
    expect(panels.onnxRuntime.modelManagerProps).toBe(modelManagerProps);
    expect(panels.torch.modelGroups).toBe(modelGroups);
    expect(panels.fallback.modelManagerProps).toBe(modelManagerProps);
  });
});
