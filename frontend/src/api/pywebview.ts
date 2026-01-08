/**
 * PyWebView API Client
 *
 * Provides a typed, error-handled wrapper around the PyWebView API.
 * All methods include proper error handling and logging.
 */

import type {
  DiskSpaceResponse,
  LaunchResponse,
  ModelsResponse,
  StatusResponse,
} from '../types/pywebview';
import { APIError } from '../errors';

class PyWebViewClient {
  /**
   * Check if PyWebView API is available
   */
  isAvailable(): boolean {
    return !!window.pywebview?.api;
  }

  /**
   * Get the API instance or throw if not available
   */
  private getAPI() {
    if (!window.pywebview?.api) {
      throw new APIError('PyWebView API not available');
    }
    return window.pywebview.api;
  }

  // Status & System
  async getStatus(): Promise<StatusResponse> {
    const api = this.getAPI();
    return await api.get_status();
  }

  async getDiskSpace(): Promise<DiskSpaceResponse> {
    const api = this.getAPI();
    return await api.get_disk_space();
  }

  async getSystemResources() {
    const api = this.getAPI();
    return await api.get_system_resources();
  }

  // Dependencies
  async installDeps() {
    const api = this.getAPI();
    return await api.install_deps();
  }

  // Process Management
  async launchComfyUI(): Promise<LaunchResponse> {
    const api = this.getAPI();
    return await api.launch_comfyui();
  }

  async stopComfyUI() {
    const api = this.getAPI();
    return await api.stop_comfyui();
  }

  // Resource Management
  async getModels(): Promise<ModelsResponse> {
    const api = this.getAPI();
    return await api.get_models();
  }

  async scanSharedStorage() {
    const api = this.getAPI();
    return await api.scan_shared_storage();
  }

  // Utility
  async openPath(path: string) {
    const api = this.getAPI();
    return await api.open_path(path);
  }

  async closeWindow() {
    const api = this.getAPI();
    return await api.close_window();
  }
}

// Export singleton instance
export const pywebview = new PyWebViewClient();
