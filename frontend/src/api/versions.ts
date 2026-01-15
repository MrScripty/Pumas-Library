/**
 * Version Management API
 *
 * Handles all version-related API calls to the backend.
 */

import { api, isAPIAvailable } from './adapter';
import { APIError } from '../errors';

class VersionsAPI {
  private getAPI() {
    if (!isAPIAvailable()) {
      throw new APIError('API not available');
    }
    return api;
  }

  async getAvailableVersions(forceRefresh = false, appId?: string) {
    const api = this.getAPI();
    return await api.get_available_versions(forceRefresh, appId);
  }

  async getInstalledVersions(appId?: string) {
    const api = this.getAPI();
    return await api.get_installed_versions(appId);
  }

  async getActiveVersion(appId?: string) {
    const api = this.getAPI();
    return await api.get_active_version(appId);
  }

  async installVersion(tag: string, appId?: string) {
    const api = this.getAPI();
    return await api.install_version(tag, appId);
  }

  async removeVersion(tag: string, appId?: string) {
    const api = this.getAPI();
    return await api.remove_version(tag, appId);
  }

  async switchVersion(tag: string, appId?: string) {
    const api = this.getAPI();
    return await api.switch_version(tag, appId);
  }

  async getInstallationProgress(appId?: string) {
    const api = this.getAPI();
    return await api.get_installation_progress(appId);
  }

  async cancelInstallation(appId?: string) {
    const api = this.getAPI();
    return await api.cancel_installation(appId);
  }

  async validateInstallations(appId?: string) {
    const api = this.getAPI();
    return await api.validate_installations(appId);
  }

  async getDefaultVersion(appId?: string) {
    const api = this.getAPI();
    return await api.get_default_version(appId);
  }

  async setDefaultVersion(tag?: string | null, appId?: string) {
    const api = this.getAPI();
    return await api.set_default_version(tag, appId);
  }

  async getVersionInfo(tag: string, appId?: string) {
    const api = this.getAPI();
    return await api.get_version_info(tag, appId);
  }

  async launchVersion(tag: string, extraArgs?: string[], appId?: string) {
    const api = this.getAPI();
    return await api.launch_version(tag, extraArgs, appId);
  }

  async getCacheStatus(appId?: string) {
    const api = this.getAPI();
    return await api.get_github_cache_status(appId);
  }
}

export const versionsAPI = new VersionsAPI();
