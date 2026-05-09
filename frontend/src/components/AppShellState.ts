import type { ComponentProps } from 'react';
import type { AppShell } from './AppShell';
import type { ModelManagerProps } from './ModelManager';
import type { useManagedApps } from '../hooks/useManagedApps';
import type { LauncherUpdateState } from '../hooks/useLauncherUpdates';
import type { AppConfig, ModelCategory, SystemResources } from '../types/apps';
import type { ServedModelStatus } from '../types/api-serving';
import type { StatusResponse } from '../types/api-system';

type AppShellProps = ComponentProps<typeof AppShell>;
type AppHeaderProps = AppShellProps['header'];
type AppSidebarProps = AppShellProps['sidebar'];
type ManagedAppsState = Parameters<typeof useManagedApps>[0];

interface AppProcessVisualState {
  isRunning?: boolean;
  installedVersions: string[];
  isStarting: boolean;
  isStopping: boolean;
  launchError: string | null;
}

export interface AppRunningState {
  comfyUIRunning: boolean;
  ollamaRunning: boolean;
  torchRunning: boolean;
}

export interface SelectedAppShellState {
  appDisplayName: string;
  connectionUrl?: string | undefined;
}

export interface SetupDisplayState {
  activeShortcutState?: { menu: boolean; desktop: boolean } | undefined;
  depsInstalled: boolean | null;
  displayStatus: string;
  isSetupComplete: boolean;
}

interface BuildManagedAppsStateOptions {
  comfyui: AppProcessVisualState;
  llamaCpp: AppProcessVisualState;
  ollama: AppProcessVisualState;
  running: AppRunningState;
  status: StatusResponse | null | undefined;
  systemResources?: SystemResources | undefined;
  torch: AppProcessVisualState;
}

interface BuildModelManagerPropsOptions {
  activeVersion: string | null;
  excludedModels: Set<string>;
  modelGroups: ModelCategory[];
  selectedAppId: string | null;
  servedModels?: ServedModelStatus[];
  starredModels: Set<string>;
  onAddModels: () => void;
  onChooseExistingLibrary: () => void;
  onModelsImported: () => void;
  onOpenModelsRoot: () => void;
  onToggleLink: (modelId: string) => void;
  onToggleStar: (modelId: string) => void;
}

interface BuildAppShellHeaderOptions {
  activeModelDownload: AppHeaderProps['activeModelDownload'];
  activeModelDownloadCount: number;
  installationProgress: AppHeaderProps['installationProgress'];
  isCheckingLauncherUpdates: boolean;
  launcherLatestVersion: string | null;
  launcherUpdateAvailable: boolean;
  modelLibraryLoaded: boolean | null;
  networkAvailable: boolean | null;
  status: StatusResponse | null | undefined;
  systemResources?: SystemResources | undefined;
  onCheckLauncherUpdates: () => Promise<void>;
  onClose: () => void;
  onDownloadLauncherUpdate: () => Promise<void>;
  onMinimize: () => void;
}

interface BuildAppShellSidebarOptions {
  apps: AppConfig[];
  selectedAppId: string | null;
  onAddApp: (insertAtIndex: number) => void;
  onDeleteApp: (appId: string) => void;
  onLaunchApp: (appId: string) => void;
  onOpenLog: (appId: string) => void;
  onReorderApps: (reorderedApps: AppConfig[]) => void;
  onSelectApp: (appId: string | null) => void;
  onStopApp: (appId: string) => void;
}

const COMPLETE_READY_MESSAGE = 'Setup complete \u2013 everything is ready';
const DEFAULT_READY_STATUS = 'system ready. configure options below';

export function getAppRunningState(status: StatusResponse | null | undefined): AppRunningState {
  return {
    comfyUIRunning: status?.comfyui_running ?? false,
    ollamaRunning: status?.ollama_running ?? false,
    torchRunning: status?.torch_running ?? false,
  };
}

export function getSelectedAppShellState(
  apps: AppConfig[],
  selectedAppId: string | null
): SelectedAppShellState {
  const selectedApp = apps.find((app) => app.id === selectedAppId);

  return {
    appDisplayName: selectedApp?.displayName ?? 'App',
    connectionUrl: selectedApp?.connectionUrl,
  };
}

export function getSetupDisplayState(
  status: StatusResponse | null | undefined,
  selectedAppId: string | null
): SetupDisplayState {
  const depsInstalled = status?.deps_ready ?? null;
  const isPatched = status?.patched ?? false;
  const menuShortcut = status?.menu_shortcut ?? false;
  const desktopShortcut = status?.desktop_shortcut ?? false;
  const isSetupComplete = depsInstalled === true && isPatched && menuShortcut && desktopShortcut;

  return {
    activeShortcutState: getActiveShortcutState(selectedAppId, menuShortcut, desktopShortcut),
    depsInstalled,
    displayStatus: getDisplayStatus(status?.message ?? ''),
    isSetupComplete,
  };
}

export function getLauncherLatestVersion(
  launcherUpdateState: LauncherUpdateState | null
): string | null {
  return launcherUpdateState?.latestVersion ?? null;
}

export function buildManagedAppsState({
  comfyui,
  llamaCpp,
  ollama,
  running,
  status,
  systemResources,
  torch,
}: BuildManagedAppsStateOptions): ManagedAppsState {
  const appResources = status?.app_resources;

  return {
    systemResources,
    comfyui: {
      ...comfyui,
      isRunning: running.comfyUIRunning,
      ramMemory: appResources?.comfyui?.ram_memory,
      gpuMemory: appResources?.comfyui?.gpu_memory,
    },
    ollama: {
      ...ollama,
      isRunning: running.ollamaRunning,
      ramMemory: appResources?.ollama?.ram_memory,
      gpuMemory: appResources?.ollama?.gpu_memory,
    },
    llamaCpp: {
      ...llamaCpp,
      isRunning: llamaCpp.isRunning ?? false,
    },
    torch: {
      ...torch,
      isRunning: running.torchRunning,
    },
  };
}

export function buildModelManagerProps({
  activeVersion,
  excludedModels,
  modelGroups,
  selectedAppId,
  servedModels = [],
  starredModels,
  onAddModels,
  onChooseExistingLibrary,
  onModelsImported,
  onOpenModelsRoot,
  onToggleLink,
  onToggleStar,
}: BuildModelManagerPropsOptions): ModelManagerProps {
  return {
    modelGroups,
    starredModels,
    excludedModels,
    onToggleStar,
    onToggleLink,
    selectedAppId,
    servedModels,
    onAddModels,
    onOpenModelsRoot,
    onModelsImported,
    activeVersion,
    onChooseExistingLibrary,
  };
}

export function buildAppShellHeader({
  activeModelDownload,
  activeModelDownloadCount,
  installationProgress,
  isCheckingLauncherUpdates,
  launcherLatestVersion,
  launcherUpdateAvailable,
  modelLibraryLoaded,
  networkAvailable,
  status,
  systemResources,
  onCheckLauncherUpdates,
  onClose,
  onDownloadLauncherUpdate,
  onMinimize,
}: BuildAppShellHeaderOptions): AppHeaderProps {
  return {
    systemResources,
    appResources: status?.app_resources?.comfyui,
    launcherUpdateAvailable,
    launcherLatestVersion,
    isCheckingLauncherUpdates,
    onCheckLauncherUpdates: () => {
      void onCheckLauncherUpdates();
    },
    onDownloadLauncherUpdate: () => {
      void onDownloadLauncherUpdate();
    },
    onMinimize,
    onClose,
    networkAvailable,
    modelLibraryLoaded,
    installationProgress,
    activeModelDownload,
    activeModelDownloadCount,
  };
}

export function buildAppShellSidebar({
  apps,
  selectedAppId,
  onAddApp,
  onDeleteApp,
  onLaunchApp,
  onOpenLog,
  onReorderApps,
  onSelectApp,
  onStopApp,
}: BuildAppShellSidebarOptions): AppSidebarProps {
  return {
    apps,
    selectedAppId,
    onSelectApp,
    onLaunchApp,
    onStopApp,
    onOpenLog,
    onDeleteApp,
    onReorderApps,
    onAddApp,
  };
}

function getActiveShortcutState(
  selectedAppId: string | null,
  menuShortcut: boolean,
  desktopShortcut: boolean
): SetupDisplayState['activeShortcutState'] {
  if (selectedAppId !== 'comfyui') {
    return undefined;
  }

  return {
    menu: menuShortcut,
    desktop: desktopShortcut,
  };
}

function getDisplayStatus(statusMessage: string): string {
  if (isReadyStatusMessage(statusMessage)) {
    return '';
  }

  return statusMessage;
}

function isReadyStatusMessage(statusMessage: string): boolean {
  const normalizedStatus = statusMessage.trim().toLowerCase();

  return (
    statusMessage === COMPLETE_READY_MESSAGE ||
    normalizedStatus === DEFAULT_READY_STATUS
  );
}
