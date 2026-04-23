import type { BaseResponse } from './api-common';
import type {
  AppStatusResponse,
  GetPluginResponse,
  GetPluginsResponse,
  PluginEndpointResponse,
  PluginHealthResponse,
} from './api-plugins';
import type {
  ApplyLauncherUpdateResponse,
  CheckLauncherUpdatesResponse,
  LaunchResponse,
  LauncherVersionResponse,
  RestartLauncherResponse,
} from './api-processes';
import type {
  CloseWindowResponse,
  OpenActiveInstallResponse,
  OpenPathResponse,
  OpenUrlResponse,
  SelectLauncherRootResponse,
} from './api-window';

export interface DesktopBridgeUtilityAPI {
  // ========================================
  // Custom Nodes
  // ========================================
  get_custom_nodes(versionTag: string): Promise<{
    success: boolean;
    nodes: string[];
    error?: string;
  }>;
  install_custom_node(
    gitUrl: string,
    versionTag: string,
    nodeName?: string
  ): Promise<BaseResponse>;
  update_custom_node(nodeName: string, versionTag: string): Promise<BaseResponse>;
  remove_custom_node(nodeName: string, versionTag: string): Promise<BaseResponse>;

  // ========================================
  // Size Calculation
  // ========================================
  calculate_release_size(
    tag: string,
    forceRefresh?: boolean,
    appId?: string
  ): Promise<{
    success: boolean;
    total_bytes?: number;
    error?: string;
  }>;
  calculate_all_release_sizes(): Promise<{
    success: boolean;
    sizes?: Record<string, number>;
    error?: string;
  }>;

  // ========================================
  // Launcher Updates
  // ========================================
  get_launcher_version(): Promise<LauncherVersionResponse>;
  check_launcher_updates(forceRefresh?: boolean): Promise<CheckLauncherUpdatesResponse>;
  apply_launcher_update(): Promise<ApplyLauncherUpdateResponse>;
  restart_launcher(): Promise<RestartLauncherResponse>;

  // ========================================
  // Utility
  // ========================================
  open_url(url: string): Promise<OpenUrlResponse>;
  open_path(path: string): Promise<OpenPathResponse>;
  select_launcher_root(): Promise<SelectLauncherRootResponse>;
  open_active_install(appId?: string): Promise<OpenActiveInstallResponse>;
  close_window(): Promise<CloseWindowResponse>;

  // Model Import
  // ========================================
  /** Open native file picker for model import */
  open_model_import_dialog(): Promise<{ success: boolean; paths: string[] }>;

  // ========================================
  // Plugin System
  // ========================================
  /** Get all registered plugins */
  get_plugins(): Promise<GetPluginsResponse>;

  /** Get a specific plugin by ID */
  get_plugin(appId: string): Promise<GetPluginResponse>;

  /** Call a plugin-defined API endpoint */
  call_plugin_endpoint(
    appId: string,
    endpointName: string,
    params: Record<string, string>
  ): Promise<PluginEndpointResponse>;

  /** Check if a plugin's API is healthy */
  check_plugin_health(appId: string): Promise<PluginHealthResponse>;

  /** Launch an app by its plugin ID */
  launch_app(appId: string, versionTag: string): Promise<LaunchResponse>;

  /** Stop a running app */
  stop_app(appId: string): Promise<BaseResponse>;

  /** Get the status of an app */
  get_app_status(appId: string): Promise<AppStatusResponse>;
}
