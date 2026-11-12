import { useEffect, useState } from 'react';
import { DEFAULT_APPS } from '../config/apps';
import type { SystemResources, AppConfig, AppIconState } from '../types/apps';

/** Process lifecycle of an app the shell starts and stops itself. */
export interface ManagedAppProcessState {
  isRunning: boolean;
  isStarting: boolean;
  isStopping: boolean;
  launchError: string | null;
  ramMemory?: number | undefined;
  gpuMemory?: number | undefined;
}

export interface UseManagedAppsOptions {
  systemResources?: SystemResources | undefined;
  /** Installed versions of each version-managed app, keyed by app id. */
  installedVersionsByAppId: Readonly<Record<string, readonly string[]>>;
  /** Process lifecycle of each app with a shell-controlled process, keyed by app id. */
  processes: Readonly<Record<string, ManagedAppProcessState>>;
}

const IDLE_PROCESS: ManagedAppProcessState = {
  isRunning: false,
  isStarting: false,
  isStopping: false,
  launchError: null,
};

function calculateUsagePercent(used: number | undefined, total: number | undefined): number | undefined {
  if (!used || !total || total <= 0) {
    return undefined;
  }
  return Math.round((used / total) * 100);
}

function deriveIconState(
  { isRunning, isStarting, isStopping, launchError }: ManagedAppProcessState,
  installedVersions: readonly string[]
): AppIconState {
  if (isStopping) return 'stopping';
  if (isStarting) return 'starting';
  if (isRunning) return 'running';
  if (launchError) return 'error';
  if (installedVersions.length > 0) return 'offline';
  return 'uninstalled';
}

export function decorateManagedApps(
  apps: AppConfig[],
  { systemResources, installedVersionsByAppId, processes }: UseManagedAppsOptions
): AppConfig[] {
  return apps.map((app) => {
    const process = processes[app.id];
    const installedVersions = installedVersionsByAppId[app.id];
    if (!process && !installedVersions) {
      return app;
    }

    const iconState = deriveIconState(process ?? IDLE_PROCESS, installedVersions ?? []);
    if (!process) {
      // No shell-controlled process (ONNX Runtime runs per profile): the
      // state is whether a version is installed.
      return { ...app, iconState };
    }

    return {
      ...app,
      status: process.isRunning ? 'running' : 'idle',
      ramUsage: calculateUsagePercent(process.ramMemory, systemResources?.ram.total),
      gpuUsage: calculateUsagePercent(process.gpuMemory, systemResources?.gpu.memory_total),
      iconState,
    };
  });
}

export function useManagedApps(options: UseManagedAppsOptions) {
  const [apps, setApps] = useState<AppConfig[]>(DEFAULT_APPS);

  useEffect(() => {
    setApps((prevApps) => decorateManagedApps(prevApps, options));
  }, [options]);

  return {
    apps,
  };
}
