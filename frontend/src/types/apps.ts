import type { LucideIcon } from 'lucide-react';

export type AppStatus = 'idle' | 'running' | 'installing' | 'error';
export type AppIconState = 'running' | 'offline' | 'uninstalled' | 'error';

export interface AppConfig {
  id: string;
  name: string;
  displayName: string;
  icon: LucideIcon;
  status: AppStatus;
  iconState: AppIconState;
  installPath?: string;
  version?: string;
  description?: string;
  starred?: boolean;
  linked?: boolean;
  ramUsage?: number;      // RAM usage percentage (0-100)
  gpuUsage?: number;      // GPU usage percentage (0-100), derived from GPU memory
}

export interface ModelInfo {
  id: string;
  name: string;
  category: string;
  path?: string;
  size?: number;
  date?: string;
  starred?: boolean;
  linkedApps?: string[]; // App IDs this model is linked to
}

export interface ModelCategory {
  category: string;
  models: ModelInfo[];
}

export interface RemoteModelInfo {
  repoId: string;
  name: string;
  developer: string;
  kind: string;
  formats: string[];
  quants: string[];
  url: string;
}

export interface SystemResources {
  cpu: {
    usage: number;
    temp?: number;
  };
  gpu: {
    usage: number;
    memory: number;
    memory_total: number;
    temp?: number;
  };
  ram: {
    usage: number;
    total: number;
  };
  disk: {
    usage: number;
    total: number;
    free: number;
  };
}
