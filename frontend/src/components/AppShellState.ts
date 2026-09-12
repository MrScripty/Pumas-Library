import type { ComponentProps } from 'react';
import type { AppShell } from './AppShell';
import type { ModelManagerProps } from './ModelManager';
import type { useManagedApps } from '../hooks/useManagedApps';
import type { LauncherUpdateState } from '../hooks/useLauncherUpdates';
import type { AppConfig, ModelCategory, ModelInfo, SystemResources } from '../types/apps';
import type { RouterProfileSyncStatus, ServedModelStatus, ServingEndpointStatus } from '../types/api-serving';
import type { StatusResponse } from '../types/api-system';
import type { ServingControlObservation } from '../hooks/useServingStatus';

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
  ollamaRunning: boolean;
  torchRunning: boolean;
}

export interface SelectedAppShellState {
  appDisplayName: string;
  connectionUrl?: string | undefined;
}

interface BuildManagedAppsStateOptions {
  llamaCpp: AppProcessVisualState;
  ollama: AppProcessVisualState;
  running: AppRunningState;
  status: StatusResponse | null | undefined;
  systemResources?: SystemResources | undefined;
  torch: AppProcessVisualState;
}

interface BuildModelManagerPropsOptions {
  libraryLoadStatus: ModelManagerProps['libraryLoadStatus'];
  activeVersion: string | null;
  excludedModels: Set<string>;
  modelGroups: ModelCategory[];
  selectedAppId: string | null;
  servingEndpoint?: ServingEndpointStatus | null;
  servedModels?: ServedModelStatus[];
  routerProfiles?: RouterProfileSyncStatus[];
  servingControlObservation?: ServingControlObservation;
  starredModels: Set<string>;
  onAddModels: () => void;
  onChooseExistingLibrary: () => void;
  onModelsImported: () => void;
  onOpenModelsRoot: () => void;
  onServeModel: (model: ModelInfo) => void;
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
  onLaunchApp: (appId: string) => void;
  onOpenLog: (appId: string) => void;
  onSelectApp: (appId: string | null) => void;
  onStopApp: (appId: string) => void;
}

export function getAppRunningState(status: StatusResponse | null | undefined): AppRunningState {
  return {
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

export function getLauncherLatestVersion(
  launcherUpdateState: LauncherUpdateState | null
): string | null {
  return launcherUpdateState?.latestVersion ?? null;
}

export function buildManagedAppsState({
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
  libraryLoadStatus,
  activeVersion,
  excludedModels,
  modelGroups,
  selectedAppId,
  servingEndpoint = null,
  servedModels = [],
  routerProfiles = [],
  servingControlObservation,
  starredModels,
  onAddModels,
  onChooseExistingLibrary,
  onModelsImported,
  onOpenModelsRoot,
  onServeModel,
  onToggleLink,
  onToggleStar,
}: BuildModelManagerPropsOptions): ModelManagerProps {
  return {
    modelGroups,
    libraryLoadStatus,
    starredModels,
    excludedModels,
    onToggleStar,
    onToggleLink,
    selectedAppId,
    servingEndpoint,
    servedModels,
    routerProfiles,
    servingControlObservation,
    onAddModels,
    onOpenModelsRoot,
    onServeModel,
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
    appResources: status?.app_resources?.ollama,
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
  onLaunchApp,
  onOpenLog,
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
  };
}
