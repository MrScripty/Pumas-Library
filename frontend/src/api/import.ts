/**
 * Model Import API
 *
 * Handles model import operations including batch import,
 * FTS5 search, network status monitoring, and HuggingFace metadata lookup.
 */

import { api, isAPIAvailable } from './adapter';
import { APIError } from '../errors';
import type {
  CheckFilesWritableResponse,
  DetectShardedSetsResponse,
  EmbeddedMetadataResponse,
  FileLinkCountResponse,
  FileTypeValidationResponse,
  FTSSearchResponse,
  GetLibraryStatusResponse,
  HFMetadataLookupResponse,
  ImportBatchResponse,
  ModelImportSpec,
  NetworkStatusResponse,
} from '../types/api';

class ImportAPI {
  private getAPI() {
    if (!isAPIAvailable()) {
      throw new APIError('API not available');
    }
    return api;
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

  /**
   * Look up HuggingFace metadata for a file using hybrid filename + hash matching.
   * Returns match confidence and requires_confirmation flag for fuzzy matches.
   */
  async lookupHFMetadata(
    filename: string,
    filePath?: string | null
  ): Promise<HFMetadataLookupResponse> {
    const api = this.getAPI();
    return await api.lookup_hf_metadata_for_file(filename, filePath);
  }

  /**
   * Detect and group sharded model files.
   * Identifies patterns like model-00001-of-00005.safetensors and validates completeness.
   */
  async detectShardedSets(filePaths: string[]): Promise<DetectShardedSetsResponse> {
    const api = this.getAPI();
    return await api.detect_sharded_sets(filePaths);
  }

  /**
   * Validate file type using magic bytes.
   * Prevents importing .txt/.html files masquerading as models.
   */
  async validateFileType(filePath: string): Promise<FileTypeValidationResponse> {
    const api = this.getAPI();
    return await api.validate_file_type(filePath);
  }

  /**
   * Get current library status including indexing state.
   */
  async getLibraryStatus(): Promise<GetLibraryStatusResponse> {
    const api = this.getAPI();
    return await api.get_library_status();
  }

  /**
   * Get number of hard links for a file.
   * Used to warn users about hard-linked files on NTFS.
   */
  async getFileLinkCount(filePath: string): Promise<FileLinkCountResponse> {
    const api = this.getAPI();
    return await api.get_file_link_count(filePath);
  }

  /**
   * Check if files can be safely deleted.
   * Returns writability status for each file.
   */
  async checkFilesWritable(filePaths: string[]): Promise<CheckFilesWritableResponse> {
    const api = this.getAPI();
    return await api.check_files_writable(filePaths);
  }

  /**
   * Get embedded metadata from a model file (GGUF or safetensors).
   * For GGUF files, extracts all metadata fields from the header.
   * For safetensors files, extracts the __metadata__ JSON header.
   */
  async getEmbeddedMetadata(filePath: string): Promise<EmbeddedMetadataResponse> {
    const api = this.getAPI();
    return await api.get_embedded_metadata(filePath);
  }
}

export const importAPI = new ImportAPI();
