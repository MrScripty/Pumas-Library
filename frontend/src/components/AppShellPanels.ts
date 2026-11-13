import type { ComponentProps } from 'react';
import type { AppPanelRenderer } from './app-panels/AppPanelRenderer';
import type { ModelManagerProps } from './ModelManager';
import type { ModelCategory } from '../types/apps';
import type { AppVersionState } from '../utils/appVersionState';

type AppShellPanels = ComponentProps<typeof AppPanelRenderer>;

interface BuildAppShellPanelsOptions {
  appDisplayName: string;
  appVersions: AppVersionState;
  connectionUrl?: string | undefined;
  diskSpacePercent: number | null;
  isOllamaRunning: boolean;
  isTorchRunning: boolean;
  modelGroups: ModelCategory[];
  modelManagerProps: ModelManagerProps;
  panelState: { showVersionManager: boolean };
  selectedAppId: string | null;
  onShowVersionManager: (show: boolean) => void;
}

export function buildAppShellPanels({
  appDisplayName,
  appVersions,
  connectionUrl,
  diskSpacePercent,
  isOllamaRunning,
  isTorchRunning,
  modelGroups,
  modelManagerProps,
  panelState,
  selectedAppId,
  onShowVersionManager,
}: BuildAppShellPanelsOptions): AppShellPanels {
  const sharedVersionProps = {
    appDisplayName,
    versions: appVersions,
    showVersionManager: panelState.showVersionManager,
    onShowVersionManager,
    diskSpacePercent,
  };

  return {
    selectedAppId,
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
      ...sharedVersionProps,
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
