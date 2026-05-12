import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { ModelManagerProps } from '../ModelManager';
import { UNSUPPORTED_VERSION_STATE } from '../../utils/appVersionState';
import { AppPanelRenderer } from './AppPanelRenderer';

vi.mock('./ComfyUIPanel', () => ({
  ComfyUIPanel: () => <div>comfyui-panel</div>,
}));
vi.mock('./DefaultAppPanel', () => ({
  DefaultAppPanel: () => <div>default-panel</div>,
}));
vi.mock('./LlamaCppPanel', () => ({
  LlamaCppPanel: () => <div>llama-cpp-panel</div>,
}));
vi.mock('./OnnxRuntimePanel', () => ({
  OnnxRuntimePanel: () => <div>onnx-runtime-panel</div>,
}));
vi.mock('./OllamaPanel', () => ({
  OllamaPanel: () => <div>ollama-panel</div>,
}));
vi.mock('./TorchPanel', () => ({
  TorchPanel: () => <div>torch-panel</div>,
}));
vi.mock('../ModelManager', () => ({
  ModelManager: () => <div>model-manager</div>,
}));

const modelManagerProps: ModelManagerProps = {
  excludedModels: new Set(),
  modelGroups: [],
  onToggleLink: vi.fn(),
  onToggleStar: vi.fn(),
  selectedAppId: 'onnx-runtime',
  starredModels: new Set(),
};

const sharedVersionProps = {
  appDisplayName: 'Runtime',
  versions: UNSUPPORTED_VERSION_STATE,
  showVersionManager: false,
  onShowVersionManager: vi.fn(),
  diskSpacePercent: 0,
};

function renderPanel(selectedAppId: string | null) {
  render(
    <AppPanelRenderer
      selectedAppId={selectedAppId}
      comfyUI={{
        ...sharedVersionProps,
        comfyUIRunning: false,
        depsInstalled: null,
        displayStatus: 'Idle',
        isCheckingDeps: false,
        isInstallingDeps: false,
        isSetupComplete: true,
        onInstallDeps: vi.fn(),
      }}
      fallback={{
        appDisplayName: 'Fallback',
        modelManagerProps,
      }}
      llamaCpp={{
        ...sharedVersionProps,
        modelManagerProps,
      }}
      ollama={{
        ...sharedVersionProps,
        isOllamaRunning: false,
        modelGroups: [],
        modelManagerProps,
      }}
      onnxRuntime={{
        modelManagerProps,
      }}
      torch={{
        ...sharedVersionProps,
        isTorchRunning: false,
        modelGroups: [],
        modelManagerProps,
      }}
    />
  );
}

describe('AppPanelRenderer', () => {
  it('renders the first-class ONNX Runtime panel for the ONNX app id', () => {
    renderPanel('onnx-runtime');

    expect(screen.getByText('onnx-runtime-panel')).toBeInTheDocument();
    expect(screen.queryByText('default-panel')).not.toBeInTheDocument();
  });
});
