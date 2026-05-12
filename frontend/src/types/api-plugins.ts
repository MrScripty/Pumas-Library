import type { BaseResponse } from './api-common';

// ============================================================================
// Plugin Types
// ============================================================================

/**
 * Plugin configuration from backend
 */
export interface PluginConfigResponse {
  id: string;
  displayName: string;
  description: string;
  icon?: string;
  githubRepo?: string;
  installationType: 'binary' | 'in-process' | 'python-venv' | 'docker';
  capabilities: {
    hasVersionManagement: boolean;
    supportsShortcuts: boolean;
    hasDependencies: boolean;
    hasConnectionUrl: boolean;
    hasModelLibrary: boolean;
    hasStats: boolean;
  };
  connection?: {
    defaultPort: number;
    protocol: string;
    healthEndpoint?: string;
  };
  modelCompatibility?: {
    supportedFormats: string[];
    importCommand?: string;
  };
  panelLayout: Array<{
    type: string;
    config?: Record<string, unknown>;
  }>;
  sidebarPriority: number;
  enabledByDefault: boolean;
}

export interface GetPluginsResponse extends BaseResponse {
  plugins: PluginConfigResponse[];
}

export interface GetPluginResponse extends BaseResponse {
  plugin?: PluginConfigResponse;
}

export interface PluginEndpointResponse extends BaseResponse {
  data?: unknown;
}

export interface PluginHealthResponse extends BaseResponse {
  healthy: boolean;
}

export interface AppStatusResponse extends BaseResponse {
  running: boolean;
  pid?: number;
  port?: number;
  uptime_secs?: number;
}
