/**
 * Model Import API
 *
 * Handles model import operations including batch import,
 * FTS5 search, and network status monitoring.
 */

import { APIError } from '../errors';
import type {
  FTSSearchResponse,
  ImportBatchResponse,
  ModelImportSpec,
  NetworkStatusResponse,
} from '../types/pywebview';

class ImportAPI {
  private getAPI() {
    if (!window.pywebview?.api) {
      throw new APIError('PyWebView API not available');
    }
    return window.pywebview.api;
  }

  /**
   * Search local model library using FTS5 full-text search.
   * Provides fast sub-20ms queries for large libraries.
   */
  async searchModelsFTS(
    query: string,
    limit = 100,
    offset = 0,
    modelType?: string | null,
    tags?: string[] | null
  ): Promise<FTSSearchResponse> {
    const api = this.getAPI();
    return await api.search_models_fts(query, limit, offset, modelType, tags);
  }

  /**
   * Import multiple models in a batch operation.
   * Supports drag-and-drop import of multiple files.
   */
  async importBatch(specs: ModelImportSpec[]): Promise<ImportBatchResponse> {
    const api = this.getAPI();
    return await api.import_batch(specs);
  }

  /**
   * Get network status including circuit breaker state.
   * Used to display offline indicators in the UI.
   */
  async getNetworkStatus(): Promise<NetworkStatusResponse> {
    const api = this.getAPI();
    return await api.get_network_status();
  }
}

export const importAPI = new ImportAPI();
