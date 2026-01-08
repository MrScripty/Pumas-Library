/**
 * Version Management API
 *
 * Handles all version-related API calls to PyWebView backend.
 */

import { APIError } from '../errors';

class VersionsAPI {
  private getAPI() {
    if (!window.pywebview?.api) {
      throw new APIError('PyWebView API not available');
    }
    return window.pywebview.api;
  }

  async getAvailableVersions(forceRefresh = false) {
    const api = this.getAPI();
    return await api.get_available_versions(forceRefresh);
  }

  async getInstalledVersions() {
    const api = this.getAPI();
    return await api.get_installed_versions();
  }

  async getActiveVersion() {
    const api = this.getAPI();
    return await api.get_active_version();
  }

  async installVersion(tag: string) {
    const api = this.getAPI();
    return await api.install_version(tag);
  }

  async removeVersion(tag: string) {
    const api = this.getAPI();
    return await api.remove_version(tag);
  }

  async switchVersion(tag: string) {
    const api = this.getAPI();
    return await api.switch_version(tag);
  }

  async getInstallationProgress() {
    const api = this.getAPI();
    return await api.get_installation_progress();
  }

  async cancelInstallation() {
    const api = this.getAPI();
    return await api.cancel_installation();
  }

  async validateInstallations() {
    const api = this.getAPI();
    return await api.validate_installations();
  }

  async getDefaultVersion() {
    const api = this.getAPI();
    return await api.get_default_version();
  }

  async setDefaultVersion(tag?: string | null) {
    const api = this.getAPI();
    return await api.set_default_version(tag);
  }

  async getVersionInfo(tag: string) {
    const api = this.getAPI();
    return await api.get_version_info(tag);
  }

  async launchVersion(tag: string, extraArgs?: string[]) {
    const api = this.getAPI();
    return await api.launch_version(tag, extraArgs);
  }

  async getCacheStatus() {
    const api = this.getAPI();
    return await api.get_github_cache_status();
  }
}

export const versionsAPI = new VersionsAPI();
