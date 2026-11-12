import type { ComponentProps } from 'react';
import type { AppShell } from './AppShell';
import type { ModelManagerProps } from './ModelManager';
import type { UseManagedAppsOptions } from '../hooks/useManagedApps';
import type { LauncherUpdateState } from '../hooks/useLauncherUpdates';
import type { AppConfig, ModelCategory, ModelInfo, SystemResources } from '../types/apps';
import type { RouterProfileSyncStatus, ServedModelStatus, ServingEndpointStatus } from '../types/api-serving';
import type { StatusResponse } from '../types/api-system';
import type { ServingControlObservation } from '../hooks/useServingStatus';

type AppShellProps = ComponentProps<typeof AppShell>;
type AppHeaderProps = AppShellProps['header'];
type AppSidebarProps = AppShellProps['sidebar'];
interface AppProcessVisualState {
  isRunning?: boolean;
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
  installedVersionsByAppId: UseManagedAppsOptions['installedVersionsByAppId'];
  llamaCpp: AppProcessVisualState;
  ollama: AppProcessVisualState;
  running: AppRunningState;
  status: StatusResponse | null | undefined;
  systemResources?: SystemResources | undefined;
  torch: AppProcessVisualState;
}

interface BuildModelManagerPropsOptions {
  libraryLoadStatus: ModelManagerProps['libraryLoadStatus'];
  libraryDiagnostics?: ModelManagerProps['libraryDiagnostics'];
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
  appId?: AppHeaderProps['appId'];
  activeModelDownload: AppHeaderProps['activeModelDownload'];
  activeModelDownloadCount: number;
  activeModelDownloadError?: string | null;
  installationProgress: AppHeaderProps['installationProgress'];
  installingTag?: AppHeaderProps['installingTag'];
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

/**
 * Sidebar inputs: installed versions come keyed by app id from the backend's
 * version-managed apps; process lifecycle exists only for apps the shell
 * starts itself.
 */
export function buildManagedAppsState({
  installedVersionsByAppId,
  llamaCpp,
  ollama,
  running,
  status,
  systemResources,
  torch,
}: BuildManagedAppsStateOptions): UseManagedAppsOptions {
  const appResources = status?.app_resources;

  return {
    systemResources,
    installedVersionsByAppId,
    processes: {
      ollama: {
        ...ollama,
        isRunning: running.ollamaRunning,
        ramMemory: appResources?.ollama?.ram_memory ?? undefined,
        gpuMemory: appResources?.ollama?.gpu_memory ?? undefined,
      },
      'llama-cpp': {
        ...llamaCpp,
        isRunning: llamaCpp.isRunning ?? false,
      },
      torch: {
        ...torch,
        isRunning: running.torchRunning,
      },
    },
  };
}

export function buildModelManagerProps({
  libraryLoadStatus,
  libraryDiagnostics,
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
    libraryDiagnostics,
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
  appId,
  activeModelDownload,
  activeModelDownloadCount,
  activeModelDownloadError = null,
  installationProgress,
  installingTag,
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
    appId,
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
    installingTag,
    activeModelDownload,
    activeModelDownloadCount,
    activeModelDownloadError,
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
