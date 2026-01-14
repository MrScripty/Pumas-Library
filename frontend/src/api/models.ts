/**
 * Model Management API
 *
 * Handles all model-related API calls to the backend.
 */

import { api, isAPIAvailable } from './adapter';
import { APIError } from '../errors';

class ModelsAPI {
  private getAPI() {
    if (!isAPIAvailable()) {
      throw new APIError('API not available');
    }
    return api;
  }

  async getModels() {
    const api = this.getAPI();
    return await api.get_models();
  }

  async scanSharedStorage() {
    const api = this.getAPI();
    return await api.scan_shared_storage();
  }

  async searchHuggingFace(query: string, kind?: string | null, limit?: number) {
    const api = this.getAPI();
    return await api.search_hf_models(query, kind, limit);
  }

  async getRelatedModels(modelId: string, limit?: number) {
    const api = this.getAPI();
    return await api.get_related_models(modelId, limit);
  }

  async startModelDownload(
    repoId: string,
    family: string,
    officialName: string,
    modelType?: string | null,
    subtype?: string | null,
    quant?: string | null
  ) {
    const api = this.getAPI();
    return await api.start_model_download_from_hf(
      repoId,
      family,
      officialName,
      modelType,
      subtype,
      quant
    );
  }

  async getDownloadStatus(downloadId: string) {
    const api = this.getAPI();
    return await api.get_model_download_status(downloadId);
  }

  async cancelDownload(downloadId: string) {
    const api = this.getAPI();
    return await api.cancel_model_download(downloadId);
  }
}

export const modelsAPI = new ModelsAPI();
