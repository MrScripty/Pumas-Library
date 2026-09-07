// Generated from pumas-rpc contract.rs; SHA256 c77f776a40f57d6ea7101504f4aa7e6c8ca8058153aa5db5c2862b1f0f16538c. DO NOT EDIT.
import { validateCatalogSearchOutcome, validateConversionListOutcome, validateConversionProgressResponse, validateDownloadIdParams, validateDownloadListOutcome, validateDownloadMutationOutcome, validateDownloadStartedOutcome, validateDownloadStatusOutcome, validateLinkHealthOutcome, validateModelIndexRefreshOutcome, validateModelsOutcome, validatePartialDownloadOutcome, validatePublicError, validateRecoverDownloadParams, validateSearchCatalogParams } from './desktop-contract.validators.js';
export type CatalogArtifactState = ({ "state": "complete" }) | ({ "downloadProgressFraction"?: number; "reasons": ReadonlyArray<CatalogPartialReason>; "recovery"?: CatalogRecoveryIdentity; "state": "partial" });
export type CatalogIntegrityState = ({ "state": "clean" }) | ({ "count": number; "otherModelIds": ReadonlyArray<string>; "state": "duplicate" });
export type CatalogModel = { "artifact": CatalogArtifactState; "dependencyCount": number; "displayDate"?: string; "displayName": string; "format"?: string; "id": string; "integrity": CatalogIntegrityState; "modelDir": string; "modelType": string; "quantization"?: string; "relatedAvailable": boolean; "sizeBytes"?: number };
export type CatalogPartialReason = "part_file_present" | "expected_files_missing";
export type CatalogRecoveryIdentity = { "recoveryToken": string; "repoId": string; "selectedArtifactFiles"?: ReadonlyArray<string>; "selectedArtifactId"?: string; "selectedArtifactQuant"?: string };
export type CatalogSearchOutcome = { "models": ReadonlyArray<CatalogModel>; "query": string; "query_time_ms": number; "success": true; "total_count": number };
export type ConversionDirection = ("gguf_to_safetensors") | ("safetensors_to_gguf") | ("safetensors_to_quantized_gguf") | ("gguf_to_quantized_gguf") | ("safetensors_to_nvfp4") | ("safetensors_to_sherry_qat");
export type ConversionListOutcome = { "conversions": ReadonlyArray<ConversionProgressOutcome>; "success": true };
export type ConversionProgressOutcome = { "bytesWritten": number | null; "conversionId": string; "currentTensor": string | null; "direction": ConversionDirection; "error": null | "The model conversion did not complete successfully."; "estimatedOutputSize": number | null; "outputModelId": string | null; "pipelineStep": number | null; "pipelineStepLabel": string | null; "pipelineStepsTotal": number | null; "progress": number | null; "sourceModelId": string; "status": ConversionStatus; "targetQuant": string | null; "tensorsCompleted": number | null; "tensorsTotal": number | null };
export type ConversionProgressResponse = { "progress": (ConversionProgressOutcome) | (null); "success": true };
export type ConversionStatus = ("setting_up") | ("validating") | ("converting") | ("writing") | ("importing") | ("completed") | ("cancelled") | ("error") | ("building_toolchain") | ("generating_f16_gguf") | ("computing_imatrix") | ("quantizing") | ("calibrating") | ("training");
export type DownloadIdParams = { "download_id": string };
export type DownloadListOutcome = { "downloads": ReadonlyArray<DownloadProgressOutcome>; "success": true };
export type DownloadMutationOutcome = { "error"?: string; "success": boolean };
export type DownloadProgressOutcome = { "downloadId": string; "downloadedBytes": number | null; "error": string | null; "etaSeconds": number | null; "libraryModelId": string | null; "modelName": string | null; "modelType": string | null; "nextRetryDelaySeconds": number | null; "progress": number | null; "repoId": string | null; "retryAttempt": number | null; "retryLimit": number | null; "retrying": boolean | null; "selectedArtifactId": string | null; "speed": number | null; "status": DownloadStatus; "totalBytes": number | null };
export type DownloadStartedFailure = { "error": string; "success": false };
export type DownloadStartedOutcome = (DownloadStartedSuccess) | (DownloadStartedFailure);
export type DownloadStartedSuccess = { "artifactId": string | null; "download_id": string; "selectedArtifactId": string | null; "success": true };
export type DownloadStatus = "queued" | "downloading" | "pausing" | "paused" | "cancelling" | "completed" | "cancelled" | "error";
export type DownloadStatusFoundOutcome = { "downloadId": string; "downloadedBytes": number | null; "error": string | null; "etaSeconds": number | null; "libraryModelId": string | null; "modelName": string | null; "modelType": string | null; "nextRetryDelaySeconds": number | null; "progress": number | null; "repoId": string | null; "retryAttempt": number | null; "retryLimit": number | null; "retrying": boolean | null; "selectedArtifactId": string | null; "speed": number | null; "status": DownloadStatus; "success": true; "totalBytes": number | null };
export type DownloadStatusMissingOutcome = { "error": string; "success": false };
export type DownloadStatusOutcome = (DownloadStatusFoundOutcome) | (DownloadStatusMissingOutcome);
export type LinkHealthOutcome = (LinkHealthResponse);
export type LinkHealthResponse = { "broken_links": ReadonlyArray<string>; "error"?: null; "errors": ReadonlyArray<string>; "healthy_links": number; "orphaned_links": ReadonlyArray<string>; "status": "healthy" | "degraded"; "success": true; "total_links": number; "warnings": ReadonlyArray<string> };
export type ModelIndexRefreshOutcome = { "indexed_count": number; "success": true };
export type ModelsOutcome = { "models": Readonly<Record<string, CatalogModel>>; "success": true };
export type PartialDownloadActionName = "resume" | "recover" | "attach" | "none";
export type PartialDownloadOutcome = { "action": PartialDownloadActionName; "download_id": string | null; "error": string | null; "reason_code": (PartialDownloadReason) | (null); "status": (DownloadStatus) | (null); "success": boolean };
export type PartialDownloadReason = "hf_client_unavailable" | "download_root_busy" | "model_not_found" | "model_not_partial" | "recovery_unavailable" | "recovery_context_stale" | "resume_rejected" | "already_completed" | "already_cancelled" | "invalid_repo_id" | "repo_not_found" | "rate_limited" | "permission_denied" | "network_error" | "recover_failed";
export type PublicError = { "class": PublicErrorClass; "code": number; "message": string };
export type PublicErrorClass = "invalid_request" | "not_found" | "conflict" | "cancelled" | "unavailable" | "operation_failed" | "internal";
export type RecoverDownloadParams = { "modelId": string; "recoveryToken": string };
export type SearchCatalogParams = { "limit"?: number | null; "offset"?: number | null; "query": string };

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
export function decodeCatalogSearchOutcome(input: unknown): DecodeOutcome<CatalogSearchOutcome> { return decode(input, validateCatalogSearchOutcome); }
export function decodeConversionListOutcome(input: unknown): DecodeOutcome<ConversionListOutcome> { return decode(input, validateConversionListOutcome); }
export function decodeConversionProgressResponse(input: unknown): DecodeOutcome<ConversionProgressResponse> { return decode(input, validateConversionProgressResponse); }
export function decodeDownloadIdParams(input: unknown): DecodeOutcome<DownloadIdParams> { return decode(input, validateDownloadIdParams); }
export function decodeDownloadListOutcome(input: unknown): DecodeOutcome<DownloadListOutcome> { return decode(input, validateDownloadListOutcome); }
export function decodeDownloadMutationOutcome(input: unknown): DecodeOutcome<DownloadMutationOutcome> { return decode(input, validateDownloadMutationOutcome); }
export function decodeDownloadStartedOutcome(input: unknown): DecodeOutcome<DownloadStartedOutcome> { return decode(input, validateDownloadStartedOutcome); }
export function decodeDownloadStatusOutcome(input: unknown): DecodeOutcome<DownloadStatusOutcome> { return decode(input, validateDownloadStatusOutcome); }
export function decodeLinkHealthOutcome(input: unknown): DecodeOutcome<LinkHealthOutcome> { return decode(input, validateLinkHealthOutcome); }
export function decodeModelIndexRefreshOutcome(input: unknown): DecodeOutcome<ModelIndexRefreshOutcome> { return decode(input, validateModelIndexRefreshOutcome); }
export function decodeModelsOutcome(input: unknown): DecodeOutcome<ModelsOutcome> { return decode(input, validateModelsOutcome); }
export function decodePartialDownloadOutcome(input: unknown): DecodeOutcome<PartialDownloadOutcome> { return decode(input, validatePartialDownloadOutcome); }
export function decodePublicError(input: unknown): DecodeOutcome<PublicError> { return decode(input, validatePublicError); }
export function decodeRecoverDownloadParams(input: unknown): DecodeOutcome<RecoverDownloadParams> { return decode(input, validateRecoverDownloadParams); }
export function decodeSearchCatalogParams(input: unknown): DecodeOutcome<SearchCatalogParams> { return decode(input, validateSearchCatalogParams); }
