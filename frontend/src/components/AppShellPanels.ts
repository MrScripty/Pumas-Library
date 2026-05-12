import type { ComponentProps } from 'react';
import type { AppPanelRenderer } from './app-panels/AppPanelRenderer';
import type { ModelManagerProps } from './ModelManager';
import type { ModelCategory } from '../types/apps';
import type { AppVersionState } from '../utils/appVersionState';

type AppShellPanels = ComponentProps<typeof AppPanelRenderer>;

interface BuildAppShellPanelsOptions {
  activeShortcutState?: { menu: boolean; desktop: boolean } | undefined;
  appDisplayName: string;
  appVersions: AppVersionState;
  comfyUIRunning: boolean;
  connectionUrl?: string | undefined;
  depsInstalled: boolean | null;
  diskSpacePercent: number;
  displayStatus: string;
  isCheckingDeps: boolean;
  isInstallingDeps: boolean;
  isOllamaRunning: boolean;
  isSetupComplete: boolean;
  isTorchRunning: boolean;
  modelGroups: ModelCategory[];
  modelManagerProps: ModelManagerProps;
  panelState: { showVersionManager: boolean };
  selectedAppId: string | null;
  onInstallDeps: () => void;
  onShowVersionManager: (show: boolean) => void;
}

export function buildAppShellPanels({
  activeShortcutState,
  appDisplayName,
  appVersions,
  comfyUIRunning,
  connectionUrl,
  depsInstalled,
  diskSpacePercent,
  displayStatus,
  isCheckingDeps,
  isInstallingDeps,
  isOllamaRunning,
  isSetupComplete,
  isTorchRunning,
  modelGroups,
  modelManagerProps,
  panelState,
  selectedAppId,
  onInstallDeps,
  onShowVersionManager,
}: BuildAppShellPanelsOptions): AppShellPanels {
  const sharedVersionProps = {
    appDisplayName,
    versions: appVersions,
    showVersionManager: panelState.showVersionManager,
    onShowVersionManager,
    activeShortcutState,
    diskSpacePercent,
  };

  return {
    selectedAppId,
    comfyUI: {
      ...sharedVersionProps,
      isCheckingDeps,
      depsInstalled,
      isInstallingDeps,
      comfyUIRunning,
      onInstallDeps,
      displayStatus,
      isSetupComplete,
    },
    ollama: {
      ...sharedVersionProps,
      connectionUrl,
      modelManagerProps,
      isOllamaRunning,
      modelGroups,
    },
    llamaCpp: {
      ...sharedVersionProps,
      connectionUrl,
      modelManagerProps,
    },
    onnxRuntime: {
      modelManagerProps,
    },
    torch: {
      ...sharedVersionProps,
      connectionUrl,
      modelManagerProps,
      isTorchRunning,
      modelGroups,
    },
    fallback: {
      appDisplayName,
      modelManagerProps,
    },
  };
}
