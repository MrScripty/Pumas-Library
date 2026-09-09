// Generated from pumas-rpc contract.rs; SHA256 389b8dc96af0d32e4794309c7afab451b8348c7e0ed062c6bf8cd0795f418a38. DO NOT EDIT.
import { validateAvailableVersionsOutcome, validateBackendStatusOutcome, validateCatalogSearchOutcome, validateConversionCancelledOutcome, validateConversionEnvironmentOutcome, validateConversionListOutcome, validateConversionProgressResponse, validateConversionSetupStartedOutcome, validateConversionSetupStatusOutcome, validateConversionStartedOutcome, validateDownloadIdParams, validateDownloadListOutcome, validateDownloadMutationOutcome, validateDownloadStartedOutcome, validateDownloadStatusOutcome, validateGetBackendSetupParams, validateGetHfDownloadDetailsParams, validateGithubCacheStatusOutcome, validateHfDownloadDetailsOutcome, validateInferenceSettingsOutcome, validateInstalledVersionsOutcome, validateLibraryModelMetadataOutcome, validateLinkHealthOutcome, validateModelIndexRefreshOutcome, validateModelsOutcome, validatePartialDownloadOutcome, validatePublicError, validateRecoverDownloadParams, validateSearchCatalogParams, validateStartBackendSetupParams, validateStartConversionSetupParams, validateSuccessOutcome, validateSupportedQuantTypesOutcome, validateUpdateInferenceSettingsOutcome, validateUpdateInferenceSettingsParams, validateUpdateModelNotesOutcome, validateUpdateModelNotesParams } from './desktop-contract.validators.js';
export type AvailableVersionsOutcome = (AvailableVersionsSuccess) | (AvailableVersionsRateLimited);
export type AvailableVersionsRateLimited = { "error": string; "rate_limited": true; "retry_after_secs": number | null; "success": false };
export type AvailableVersionsSuccess = { "success": true; "versions": ReadonlyArray<VersionReleaseInfo> };
export type BackendStatus = { "backend": (QuantBackend); "name": string; "ready": boolean };
export type BackendStatusOutcome = { "backends": ReadonlyArray<BackendStatus>; "success": true };
export type BundleComponentManifestEntry = { "class_name": string | null; "name": string; "relative_path": string; "source_library": string | null; "state": BundleComponentState };
export type BundleComponentState = "present" | "missing" | "unreadable" | "path_escape";
export type CatalogArtifactState = ({ "state": "complete" }) | ({ "downloadProgressFraction"?: number; "reasons": ReadonlyArray<CatalogPartialReason>; "recovery"?: CatalogRecoveryIdentity; "state": "partial" });
export type CatalogIntegrityState = ({ "state": "clean" }) | ({ "count": number; "otherModelIds": ReadonlyArray<string>; "state": "duplicate" });
export type CatalogModel = { "artifact": CatalogArtifactState; "dependencyCount": number; "displayDate"?: string; "displayName": string; "format"?: string; "id": string; "integrity": CatalogIntegrityState; "modelDir": string; "modelType": string; "quantization"?: string; "relatedAvailable": boolean; "sizeBytes"?: number };
export type CatalogPartialReason = "part_file_present" | "expected_files_missing";
export type CatalogRecoveryIdentity = { "recoveryToken": string; "repoId": string; "selectedArtifactFiles"?: ReadonlyArray<string>; "selectedArtifactId"?: string; "selectedArtifactQuant"?: string };
export type CatalogSearchOutcome = { "models": ReadonlyArray<CatalogModel>; "query": string; "query_time_ms": number; "success": true; "total_count": number };
export type ConversionCancelledOutcome = { "cancelled": boolean; "success": true };
export type ConversionDirection = ("gguf_to_safetensors") | ("safetensors_to_gguf") | ("safetensors_to_quantized_gguf") | ("gguf_to_quantized_gguf") | ("safetensors_to_nvfp4") | ("safetensors_to_sherry_qat");
export type ConversionEnvironmentOutcome = { "ready": boolean; "success": true };
export type ConversionListOutcome = { "conversions": ReadonlyArray<ConversionProgressOutcome>; "success": true };
export type ConversionProgressOutcome = { "bytesWritten": number | null; "conversionId": string; "currentTensor": string | null; "direction": ConversionDirection; "error": null | "The model conversion did not complete successfully."; "estimatedOutputSize": number | null; "outputModelId": string | null; "pipelineStep": number | null; "pipelineStepLabel": string | null; "pipelineStepsTotal": number | null; "progress": number | null; "sourceModelId": string; "status": ConversionStatus; "targetQuant": string | null; "tensorsCompleted": number | null; "tensorsTotal": number | null };
export type ConversionProgressResponse = { "progress": (ConversionProgressOutcome) | (null); "success": true };
export type ConversionSetupSnapshotOutcome = { "error": null | "Conversion environment setup did not complete successfully."; "operationId": string; "status": ConversionSetupStatus };
export type ConversionSetupStartedOutcome = { "setup": ConversionSetupSnapshotOutcome; "success": true };
export type ConversionSetupStatus = ("in_progress") | ("completed") | ("failed") | ("cancelled");
export type ConversionSetupStatusOutcome = { "setup": (ConversionSetupSnapshotOutcome) | (null); "success": true };
export type ConversionStartedOutcome = { "conversion_id": string; "success": true };
export type ConversionStatus = ("setting_up") | ("validating") | ("converting") | ("writing") | ("importing") | ("completed") | ("cancelled") | ("error") | ("building_toolchain") | ("generating_f16_gguf") | ("computing_imatrix") | ("quantizing") | ("calibrating") | ("training");
export type DesktopJsonValue = (null) | (boolean) | (string) | (number) | (ReadonlyArray<DesktopJsonValue>) | ({ readonly [key: string]: DesktopJsonValue });
export type DownloadIdParams = { "download_id": string };
export type DownloadListOutcome = { "downloads": ReadonlyArray<DownloadProgressOutcome>; "success": true };
export type DownloadMutationOutcome = { "error"?: string; "success": boolean };
export type DownloadOption = { "fileGroup"?: FileGroup; "quant": string; "sizeBytes": number | null };
export type DownloadProgressOutcome = { "downloadId": string; "downloadedBytes": number | null; "error": string | null; "etaSeconds": number | null; "libraryModelId": string | null; "modelName": string | null; "modelType": string | null; "nextRetryDelaySeconds": number | null; "progress": number | null; "repoId": string | null; "retryAttempt": number | null; "retryLimit": number | null; "retrying": boolean | null; "selectedArtifactId": string | null; "speed": number | null; "status": DownloadStatus; "totalBytes": number | null };
export type DownloadStartedFailure = { "error": string; "success": false };
export type DownloadStartedOutcome = (DownloadStartedSuccess) | (DownloadStartedFailure);
export type DownloadStartedSuccess = { "artifactId": string | null; "download_id": string; "selectedArtifactId": string | null; "success": true };
export type DownloadStatus = "queued" | "downloading" | "pausing" | "paused" | "cancelling" | "completed" | "cancelled" | "error";
export type DownloadStatusFoundOutcome = { "downloadId": string; "downloadedBytes": number | null; "error": string | null; "etaSeconds": number | null; "libraryModelId": string | null; "modelName": string | null; "modelType": string | null; "nextRetryDelaySeconds": number | null; "progress": number | null; "repoId": string | null; "retryAttempt": number | null; "retryLimit": number | null; "retrying": boolean | null; "selectedArtifactId": string | null; "speed": number | null; "status": DownloadStatus; "success": true; "totalBytes": number | null };
export type DownloadStatusMissingOutcome = { "error": string; "success": false };
export type DownloadStatusOutcome = (DownloadStatusFoundOutcome) | (DownloadStatusMissingOutcome);
export type EmbeddedMetadataResponse = { "file_type": string; "metadata": { readonly [key: string]: DesktopJsonValue } };
export type FileGroup = { "filenames": ReadonlyArray<string>; "label": string; "shardCount": number };
export type GetBackendSetupParams = { "backend": QuantBackend };
export type GetHfDownloadDetailsParams = ({ "quants"?: ReadonlyArray<string> | null; "repo_id": string }) | ({ "quants"?: ReadonlyArray<string> | null; "repoId": string });
export type GithubCacheStatusNoManager = { "has_cache": false; "is_fetching": false; "is_valid": false };
export type GithubCacheStatusOutcome = (GithubCacheStatusSnapshot) | (GithubCacheStatusNoManager);
export type GithubCacheStatusSnapshot = { "age_seconds": number | null; "has_cache": boolean; "is_fetching": boolean; "is_valid": boolean; "last_fetched": string | null; "releases_count": number | null };
export type HfDownloadDetails = { "downloadOptions": ReadonlyArray<DownloadOption>; "repoId": string; "totalSizeBytes": number | null };
export type HfDownloadDetailsFailure = { "error": string; "success": false };
export type HfDownloadDetailsOutcome = (HfDownloadDetailsSuccess) | (HfDownloadDetailsFailure);
export type HfDownloadDetailsSuccess = { "details": HfDownloadDetails; "success": true };
export type InferenceConstraintsInput = { "allowed_values"?: (null) | (ReadonlyArray<DesktopJsonValue>); "max"?: number | null; "min"?: number | null };
export type InferenceParamSchema = { "constraints": (ParamConstraints) | (null); "default": DesktopJsonValue; "description": string | null; "key": string; "label": string; "param_type": (ParamType) };
export type InferenceSettingInput = { "constraints"?: (InferenceConstraintsInput) | (null); "default": DesktopJsonValue; "description"?: string | null; "key": string; "label": string; "param_type": ParamType };
export type InferenceSettingsOutcome = { "inference_settings": ReadonlyArray<InferenceParamSchema>; "model_id": string; "success": true };
export type InstalledVersionsOutcome = { "success": true; "versions": ReadonlyArray<string> };
export type LibraryModelMetadataOutcome = { "component_manifest"?: ReadonlyArray<BundleComponentManifestEntry>; "effective_metadata"?: { readonly [key: string]: DesktopJsonValue }; "embedded_metadata"?: EmbeddedMetadataResponse; "model_id": string; "primary_file"?: string; "stored_metadata"?: { readonly [key: string]: DesktopJsonValue }; "success": true };
export type LinkHealthOutcome = (LinkHealthResponse);
export type LinkHealthResponse = { "broken_links": ReadonlyArray<string>; "error"?: null; "errors": ReadonlyArray<string>; "healthy_links": number; "orphaned_links": ReadonlyArray<string>; "status": "healthy" | "degraded"; "success": true; "total_links": number; "warnings": ReadonlyArray<string> };
export type ModelIndexRefreshOutcome = { "indexed_count": number; "success": true };
export type ModelsOutcome = { "models": { readonly [key: string]: CatalogModel }; "success": true };
export type ParamConstraints = { "allowed_values": (null) | (ReadonlyArray<DesktopJsonValue>); "max": number | null; "min": number | null };
export type ParamType = "Number" | "Integer" | "String" | "Boolean";
export type PartialDownloadActionName = "resume" | "recover" | "attach" | "none";
export type PartialDownloadOutcome = { "action": PartialDownloadActionName; "download_id": string | null; "error": string | null; "reason_code": (PartialDownloadReason) | (null); "status": (DownloadStatus) | (null); "success": boolean };
export type PartialDownloadReason = "hf_client_unavailable" | "download_root_busy" | "model_not_found" | "model_not_partial" | "recovery_unavailable" | "recovery_context_stale" | "resume_rejected" | "already_completed" | "already_cancelled" | "invalid_repo_id" | "repo_not_found" | "rate_limited" | "permission_denied" | "network_error" | "recover_failed";
export type PublicError = { "class": PublicErrorClass; "code": number; "message": string };
export type PublicErrorClass = "invalid_request" | "not_found" | "conflict" | "cancelled" | "unavailable" | "operation_failed" | "internal";
export type QuantBackend = ("python_conversion") | ("llama_cpp") | ("nvfp4") | ("sherry");
export type QuantOption = { "backend": (QuantBackend) | (null); "bitsPerWeight": number; "description": string; "imatrixRecommended": boolean; "name": string; "recommended": boolean };
export type RecoverDownloadParams = { "modelId": string; "recoveryToken": string };
export type SearchCatalogParams = { "limit"?: number | null; "offset"?: number | null; "query": string };
export type StartBackendSetupParams = { "backend": QuantBackend; "expected_previous_operation_id"?: string | null };
export type StartConversionSetupParams = { "expected_previous_operation_id"?: string | null };
export type SuccessOutcome = { "success": true };
export type SupportedQuantTypesOutcome = { "quant_types": ReadonlyArray<QuantOption>; "success": true };
export type UpdateInferenceSettingsOutcome = { "model_id": string; "success": true };
export type UpdateInferenceSettingsParams = ({ "model_id": string; "settings": ReadonlyArray<InferenceSettingInput> }) | ({ "inference_settings": ReadonlyArray<InferenceSettingInput>; "model_id": string }) | ({ "inferenceSettings": ReadonlyArray<InferenceSettingInput>; "model_id": string }) | ({ "modelId": string; "settings": ReadonlyArray<InferenceSettingInput> }) | ({ "inference_settings": ReadonlyArray<InferenceSettingInput>; "modelId": string }) | ({ "inferenceSettings": ReadonlyArray<InferenceSettingInput>; "modelId": string });
export type UpdateModelNotesFailure = { "error": string; "model_id": string; "success": false };
export type UpdateModelNotesOutcome = (UpdateModelNotesSuccess) | (UpdateModelNotesFailure);
export type UpdateModelNotesParams = ({ "model_id": string; "notes"?: string | null }) | ({ "model_id": string; "model_notes"?: string | null }) | ({ "modelId": string; "notes"?: string | null }) | ({ "modelId": string; "model_notes"?: string | null });
export type UpdateModelNotesSuccess = { "model_id": string; "notes"?: string; "success": true };
export type VersionReleaseAsset = { "downloadUrl": string; "name": string; "size": number };
export type VersionReleaseInfo = { "archiveSize": number | null; "assets": ReadonlyArray<VersionReleaseAsset>; "body": string | null; "dependenciesSize": number | null; "htmlUrl": string; "installing": boolean | null; "name": string; "prerelease": boolean; "publishedAt": string; "tagName": string; "totalSize": number | null };

export type DecodeOutcome<T> = { readonly status: 'valid'; readonly value: T } | { readonly status: 'invalid' | 'unsupported' | 'unavailable'; readonly message: string };

class InvalidJsonRepresentation extends Error {}

function copyJson(value: unknown, ancestors: Set<object>, budget: { remaining: number }): unknown {
  if (--budget.remaining < 0) throw new InvalidJsonRepresentation('Oversized value');
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value !== 'object' || ancestors.has(value)) throw new InvalidJsonRepresentation('Invalid JSON value');
  ancestors.add(value);
  if (Object.getOwnPropertySymbols(value).length !== 0) throw new InvalidJsonRepresentation('Invalid symbol property');
  const descriptors = Object.getOwnPropertyDescriptors(value);
  const entries = Object.entries(descriptors).filter(([key]) => !(Array.isArray(value) && key === 'length'));
  if (Array.isArray(value) && (entries.length !== value.length || entries.some(([key]) => !/^(0|[1-9][0-9]*)$/.test(key)))) throw new InvalidJsonRepresentation('Invalid array properties');
  const result: Record<string, unknown> | unknown[] = Array.isArray(value) ? [] : Object.create(null) as Record<string, unknown>;
  for (const [key, descriptor] of entries) {
    if (!descriptor.enumerable || !('value' in descriptor)) throw new InvalidJsonRepresentation('Invalid property');
    Object.defineProperty(result, key, {value:copyJson(descriptor.value, ancestors, budget), enumerable:true, writable:false, configurable:false});
  }
  ancestors.delete(value);
  return Object.freeze(result);
}

function decode<T>(input: unknown, validate: (value: unknown) => boolean): DecodeOutcome<T> {
  try {
    const value = copyJson(input, new Set(), {remaining: 1_000_000});
    if (!validate(value)) return {status:'invalid', message:'Invalid desktop contract payload.'};
    // The generated complete validator establishes this representation.
    return {status:'valid', value:value as T};
  } catch {
    return {status:'invalid', message:'Invalid desktop contract payload.'};
  }
}
export function decodeAvailableVersionsOutcome(input: unknown): DecodeOutcome<AvailableVersionsOutcome> { return decode(input, validateAvailableVersionsOutcome); }
export function decodeBackendStatusOutcome(input: unknown): DecodeOutcome<BackendStatusOutcome> { return decode(input, validateBackendStatusOutcome); }
export function decodeCatalogSearchOutcome(input: unknown): DecodeOutcome<CatalogSearchOutcome> { return decode(input, validateCatalogSearchOutcome); }
export function decodeConversionCancelledOutcome(input: unknown): DecodeOutcome<ConversionCancelledOutcome> { return decode(input, validateConversionCancelledOutcome); }
export function decodeConversionEnvironmentOutcome(input: unknown): DecodeOutcome<ConversionEnvironmentOutcome> { return decode(input, validateConversionEnvironmentOutcome); }
export function decodeConversionListOutcome(input: unknown): DecodeOutcome<ConversionListOutcome> { return decode(input, validateConversionListOutcome); }
export function decodeConversionProgressResponse(input: unknown): DecodeOutcome<ConversionProgressResponse> { return decode(input, validateConversionProgressResponse); }
export function decodeConversionSetupStartedOutcome(input: unknown): DecodeOutcome<ConversionSetupStartedOutcome> { return decode(input, validateConversionSetupStartedOutcome); }
export function decodeConversionSetupStatusOutcome(input: unknown): DecodeOutcome<ConversionSetupStatusOutcome> { return decode(input, validateConversionSetupStatusOutcome); }
export function decodeConversionStartedOutcome(input: unknown): DecodeOutcome<ConversionStartedOutcome> { return decode(input, validateConversionStartedOutcome); }
export function decodeDownloadIdParams(input: unknown): DecodeOutcome<DownloadIdParams> { return decode(input, validateDownloadIdParams); }
export function decodeDownloadListOutcome(input: unknown): DecodeOutcome<DownloadListOutcome> { return decode(input, validateDownloadListOutcome); }
export function decodeDownloadMutationOutcome(input: unknown): DecodeOutcome<DownloadMutationOutcome> { return decode(input, validateDownloadMutationOutcome); }
export function decodeDownloadStartedOutcome(input: unknown): DecodeOutcome<DownloadStartedOutcome> { return decode(input, validateDownloadStartedOutcome); }
export function decodeDownloadStatusOutcome(input: unknown): DecodeOutcome<DownloadStatusOutcome> { return decode(input, validateDownloadStatusOutcome); }
export function decodeGetBackendSetupParams(input: unknown): DecodeOutcome<GetBackendSetupParams> { return decode(input, validateGetBackendSetupParams); }
export function decodeGetHfDownloadDetailsParams(input: unknown): DecodeOutcome<GetHfDownloadDetailsParams> { return decode(input, validateGetHfDownloadDetailsParams); }
export function decodeGithubCacheStatusOutcome(input: unknown): DecodeOutcome<GithubCacheStatusOutcome> { return decode(input, validateGithubCacheStatusOutcome); }
export function decodeHfDownloadDetailsOutcome(input: unknown): DecodeOutcome<HfDownloadDetailsOutcome> { return decode(input, validateHfDownloadDetailsOutcome); }
export function decodeInferenceSettingsOutcome(input: unknown): DecodeOutcome<InferenceSettingsOutcome> { return decode(input, validateInferenceSettingsOutcome); }
export function decodeInstalledVersionsOutcome(input: unknown): DecodeOutcome<InstalledVersionsOutcome> { return decode(input, validateInstalledVersionsOutcome); }
export function decodeLibraryModelMetadataOutcome(input: unknown): DecodeOutcome<LibraryModelMetadataOutcome> { return decode(input, validateLibraryModelMetadataOutcome); }
export function decodeLinkHealthOutcome(input: unknown): DecodeOutcome<LinkHealthOutcome> { return decode(input, validateLinkHealthOutcome); }
export function decodeModelIndexRefreshOutcome(input: unknown): DecodeOutcome<ModelIndexRefreshOutcome> { return decode(input, validateModelIndexRefreshOutcome); }
export function decodeModelsOutcome(input: unknown): DecodeOutcome<ModelsOutcome> { return decode(input, validateModelsOutcome); }
export function decodePartialDownloadOutcome(input: unknown): DecodeOutcome<PartialDownloadOutcome> { return decode(input, validatePartialDownloadOutcome); }
export function decodePublicError(input: unknown): DecodeOutcome<PublicError> { return decode(input, validatePublicError); }
export function decodeRecoverDownloadParams(input: unknown): DecodeOutcome<RecoverDownloadParams> { return decode(input, validateRecoverDownloadParams); }
export function decodeSearchCatalogParams(input: unknown): DecodeOutcome<SearchCatalogParams> { return decode(input, validateSearchCatalogParams); }
export function decodeStartBackendSetupParams(input: unknown): DecodeOutcome<StartBackendSetupParams> { return decode(input, validateStartBackendSetupParams); }
export function decodeStartConversionSetupParams(input: unknown): DecodeOutcome<StartConversionSetupParams> { return decode(input, validateStartConversionSetupParams); }
export function decodeSuccessOutcome(input: unknown): DecodeOutcome<SuccessOutcome> { return decode(input, validateSuccessOutcome); }
export function decodeSupportedQuantTypesOutcome(input: unknown): DecodeOutcome<SupportedQuantTypesOutcome> { return decode(input, validateSupportedQuantTypesOutcome); }
export function decodeUpdateInferenceSettingsOutcome(input: unknown): DecodeOutcome<UpdateInferenceSettingsOutcome> { return decode(input, validateUpdateInferenceSettingsOutcome); }
export function decodeUpdateInferenceSettingsParams(input: unknown): DecodeOutcome<UpdateInferenceSettingsParams> { return decode(input, validateUpdateInferenceSettingsParams); }
export function decodeUpdateModelNotesOutcome(input: unknown): DecodeOutcome<UpdateModelNotesOutcome> { return decode(input, validateUpdateModelNotesOutcome); }
export function decodeUpdateModelNotesParams(input: unknown): DecodeOutcome<UpdateModelNotesParams> { return decode(input, validateUpdateModelNotesParams); }
