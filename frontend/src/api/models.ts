/**
 * Model Management API
 *
 * Handles all model-related API calls to the backend.
 */

import { api, isAPIAvailable } from './adapter';
import { APIError } from '../errors';
import type {
  LibraryModelMetadataResponse,
  InferenceParamSchema,
  InferenceSettingsResponse,
  ModelExecutionDescriptor,
  ModelLibraryUpdateFeed,
  ModelPackageFactsSummaryResult,
  ModelPackageFactsSummarySnapshot,
  PumasModelRef,
  ResolvedModelPackageFacts,
  UpdateModelNotesResponse,
  UpdateInferenceSettingsResponse,
} from '../types/api';

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

  async searchHuggingFace(
    query: string,
    kind?: string | null,
    limit?: number,
    hydrateLimit?: number
  ) {
    const api = this.getAPI();
    return await api.search_hf_models(query, kind, limit, hydrateLimit);
  }

  async getHFDownloadDetails(repoId: string, quants?: string[] | null) {
    const api = this.getAPI();
    return await api.get_hf_download_details(repoId, quants);
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
    pipelineTag?: string | null,
    releaseDate?: string | null,
    downloadUrl?: string | null,
    quant?: string | null,
    filenames?: string[] | null
  ) {
    const api = this.getAPI();
    return await api.start_model_download_from_hf(
      repoId,
      family,
      officialName,
      modelType,
      pipelineTag,
      releaseDate,
      downloadUrl,
      quant,
      filenames
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

  async listInterruptedDownloads() {
    const api = this.getAPI();
    return await api.list_interrupted_downloads();
  }

  async recoverDownload(repoId: string, destDir: string) {
    const api = this.getAPI();
    return await api.recover_download(repoId, destDir);
  }

  async resumePartialDownload(repoId: string, destDir: string) {
    const api = this.getAPI();
    return await api.resume_partial_download(repoId, destDir);
  }

  async deleteModel(modelId: string) {
    const api = this.getAPI();
    return await api.delete_model_with_cascade(modelId);
  }

  /**
   * Get metadata for a library model (both stored and embedded).
   */
  async getLibraryModelMetadata(modelId: string): Promise<LibraryModelMetadataResponse> {
    const api = this.getAPI();
    return await api.get_library_model_metadata(modelId);
  }

  async resolveModelExecutionDescriptor(modelId: string): Promise<ModelExecutionDescriptor> {
    const api = this.getAPI();
    return await api.resolve_model_execution_descriptor(modelId);
  }

  async resolveModelPackageFacts(modelId: string): Promise<ResolvedModelPackageFacts> {
    const api = this.getAPI();
    return await api.resolve_model_package_facts(modelId);
  }

  async listModelLibraryUpdatesSince(
    cursor?: string | null,
    limit?: number
  ): Promise<ModelLibraryUpdateFeed> {
    const api = this.getAPI();
    return await api.list_model_library_updates_since(cursor, limit);
  }

  async resolveModelPackageFactsSummary(
    modelId: string
  ): Promise<ModelPackageFactsSummaryResult> {
    const api = this.getAPI();
    return await api.resolve_model_package_facts_summary(modelId);
  }

  async modelPackageFactsSummarySnapshot(
    limit?: number,
    offset?: number
  ): Promise<ModelPackageFactsSummarySnapshot> {
    const api = this.getAPI();
    return await api.model_package_facts_summary_snapshot(limit, offset);
  }

  async resolvePumasModelRef(input: string): Promise<PumasModelRef> {
    const api = this.getAPI();
    return await api.resolve_pumas_model_ref(input);
  }

  /**
   * Refetch model metadata from HuggingFace.
   *
   * Uses the stored repo_id if available, otherwise falls back to
   * filename-based lookup.
   */
  async refetchMetadataFromHF(modelId: string): Promise<{
    success: boolean;
    model_id: string;
    metadata: Record<string, unknown> | null;
    error?: string;
  }> {
    const api = this.getAPI();
    return await api.refetch_model_metadata_from_hf(modelId);
  }

  /**
   * Get inference settings schema for a model.
   */
  async getInferenceSettings(modelId: string): Promise<InferenceSettingsResponse> {
    const api = this.getAPI();
    return await api.get_inference_settings(modelId);
  }

  /**
   * Update (replace) inference settings schema for a model.
   */
  async updateInferenceSettings(
    modelId: string,
    inferenceSettings: InferenceParamSchema[]
  ): Promise<UpdateInferenceSettingsResponse> {
    const api = this.getAPI();
    return await api.update_inference_settings(modelId, inferenceSettings);
  }

  async updateModelNotes(
    modelId: string,
    notes?: string | null
  ): Promise<UpdateModelNotesResponse> {
    const api = this.getAPI();
    return await api.update_model_notes(modelId, notes);
  }
}

export const modelsAPI = new ModelsAPI();
