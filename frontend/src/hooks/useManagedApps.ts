import { useCallback, useEffect, useState } from 'react';
import { Box } from 'lucide-react';
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
  comfyui: ManagedAppVisualState;
  ollama: ManagedAppVisualState;
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
  { systemResources, comfyui, ollama, torch }: UseManagedAppsOptions
): AppConfig[] {
  return apps.map((app) => {
    if (app.id === 'comfyui') {
      return {
        ...app,
        status: comfyui.isRunning ? 'running' : 'idle',
        ramUsage: calculateUsagePercent(comfyui.ramMemory, systemResources?.ram.total),
        gpuUsage: calculateUsagePercent(comfyui.gpuMemory, systemResources?.gpu.memory_total),
        iconState: deriveIconState(comfyui),
      };
    }

    if (app.id === 'ollama') {
      return {
        ...app,
        status: ollama.isRunning ? 'running' : 'idle',
        ramUsage: calculateUsagePercent(ollama.ramMemory, systemResources?.ram.total),
        gpuUsage: calculateUsagePercent(ollama.gpuMemory, systemResources?.gpu.memory_total),
        iconState: deriveIconState(ollama),
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

  const deleteApp = useCallback((appId: string) => {
    setApps((prevApps) => prevApps.filter((app) => app.id !== appId));
  }, []);

  const reorderApps = useCallback((reorderedApps: AppConfig[]) => {
    setApps(reorderedApps);
  }, []);

  const addApp = useCallback((insertAtIndex: number) => {
    setApps((prevApps) => {
      const newAppNumber = prevApps.length + 1;
      const newApp: AppConfig = {
        id: `app-${Date.now()}`,
        name: `new-app-${newAppNumber}`,
        displayName: `New App ${newAppNumber}`,
        icon: Box,
        status: 'idle',
        iconState: 'uninstalled',
        ramUsage: 0,
        gpuUsage: 0,
      };

      const nextApps = [...prevApps];
      nextApps.splice(insertAtIndex, 0, newApp);
      return nextApps;
    });
  }, []);

  return {
    apps,
    deleteApp,
    reorderApps,
    addApp,
  };
}
