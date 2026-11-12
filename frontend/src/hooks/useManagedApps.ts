import { useEffect, useState } from 'react';
import { DEFAULT_APPS } from '../config/apps';
import type { SystemResources, AppConfig, AppIconState } from '../types/apps';

interface ManagedAppVisualState {
  isRunning: boolean;
  isStarting: boolean;
  isStopping: boolean;
  launchError: string | null;
  installedVersions: string[];
  ramMemory?: number;
  gpuMemory?: number;
}

interface UseManagedAppsOptions {
  systemResources?: SystemResources;
  llamaCpp: ManagedAppVisualState;
  ollama: ManagedAppVisualState;
  onnxRuntime: ManagedAppVisualState;
  torch: ManagedAppVisualState;
}

function calculateUsagePercent(used: number | undefined, total: number | undefined): number | undefined {
  if (!used || !total || total <= 0) {
    return undefined;
  }
  return Math.round((used / total) * 100);
}

function deriveIconState({
  isRunning,
  isStarting,
  isStopping,
  launchError,
  installedVersions,
}: ManagedAppVisualState): AppIconState {
  if (isStopping) return 'stopping';
  if (isStarting) return 'starting';
  if (isRunning) return 'running';
  if (launchError) return 'error';
  if (installedVersions.length > 0) return 'offline';
  return 'uninstalled';
}

export function decorateManagedApps(
  apps: AppConfig[],
  { systemResources, llamaCpp, ollama, onnxRuntime, torch }: UseManagedAppsOptions
): AppConfig[] {
  return apps.map((app) => {
    if (app.id === 'ollama') {
      return {
        ...app,
        status: ollama.isRunning ? 'running' : 'idle',
        ramUsage: calculateUsagePercent(ollama.ramMemory, systemResources?.ram.total),
        gpuUsage: calculateUsagePercent(ollama.gpuMemory, systemResources?.gpu.memory_total),
        iconState: deriveIconState(ollama),
      };
    }

    if (app.id === 'llama-cpp') {
      return {
        ...app,
        status: llamaCpp.isRunning ? 'running' : 'idle',
        ramUsage: calculateUsagePercent(llamaCpp.ramMemory, systemResources?.ram.total),
        gpuUsage: calculateUsagePercent(llamaCpp.gpuMemory, systemResources?.gpu.memory_total),
        iconState: deriveIconState(llamaCpp),
      };
    }

    if (app.id === 'onnx-runtime') {
      // ONNX Runtime executes in-process; its state is whether a downloaded
      // version is installed.
      return {
        ...app,
        iconState: deriveIconState(onnxRuntime),
      };
    }

    if (app.id === 'torch') {
      return {
        ...app,
        status: torch.isRunning ? 'running' : 'idle',
        iconState: deriveIconState(torch),
      };
    }

    return app;
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
