/**
 * Plugin configuration types matching the backend plugin schema.
 *
 * These types allow the frontend to work with plugin configurations
 * loaded from JSON files via the backend API.
 */

/** How the app is installed and managed */
export type InstallationType = 'binary' | 'python-venv' | 'docker';

/** App capabilities that affect available features */
export interface AppCapabilities {
  hasVersionManagement: boolean;
  supportsShortcuts: boolean;
  hasDependencies: boolean;
  hasConnectionUrl: boolean;
  hasModelLibrary: boolean;
  hasStats: boolean;
}

/** Connection configuration for the app */
export interface ConnectionConfig {
  defaultPort: number;
  protocol: string;
  healthEndpoint?: string;
}

/** Version filtering rules for GitHub releases */
export interface VersionFilter {
  includePrereleases: boolean;
  excludePatterns: string[];
  platformAssets: Record<string, string>;
}

/** Model format compatibility */
export interface ModelCompatibility {
  supportedFormats: string[];
  importCommand?: string;
}

/** An API endpoint definition */
export interface ApiEndpoint {
  method: string;
  endpoint: string;
  bodyTemplate?: Record<string, unknown>;
  responseMapping: Record<string, string>;
  pollingIntervalMs?: number;
}

/** Python-specific configuration */
export interface PythonConfig {
  requirementsFile: string;
  entryPoint: string;
  pythonVersion?: string;
}

/** A panel section type for the UI */
export interface PanelSection {
  type: string;
  config?: Record<string, unknown>;
}

/** Complete plugin configuration */
export interface PluginConfig {
  id: string;
  displayName: string;
  description: string;
  icon?: string;
  githubRepo?: string;
  installationType: InstallationType;
  capabilities: AppCapabilities;
  connection?: ConnectionConfig;
  versionFilter?: VersionFilter;
  modelCompatibility?: ModelCompatibility;
  pythonConfig?: PythonConfig;
  api?: Record<string, ApiEndpoint>;
  panelLayout: PanelSection[];
  sidebarPriority: number;
  enabledByDefault: boolean;
}

/** Process status from the backend */
export interface ProcessStatus {
  pid?: number;
  port?: number;
  ramBytes?: number;
  gpuBytes?: number;
  uptimeSecs?: number;
  healthy: boolean;
}

/** Result of launching a process */
export interface ProcessHandle {
  success: boolean;
  logFile?: string;
  error?: string;
  ready: boolean;
}

/** Get the connection URL for a plugin */
export function getPluginConnectionUrl(plugin: PluginConfig): string | undefined {
  if (!plugin.connection) return undefined;
  return `${plugin.connection.protocol}://localhost:${plugin.connection.defaultPort}`;
}

/** Check if a plugin supports a model format */
export function pluginSupportsFormat(plugin: PluginConfig, format: string): boolean {
  if (!plugin.modelCompatibility) return false;
  return plugin.modelCompatibility.supportedFormats.some(
    (f) => f.toLowerCase() === format.toLowerCase()
  );
}
