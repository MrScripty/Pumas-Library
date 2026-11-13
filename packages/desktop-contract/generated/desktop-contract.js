// Generated from the pumas-rpc contract export and electron/contracts; SHA256 c8351051f726739de4a9cf5f5db576767efd05bc3e77ed55731bc92a5a4b3275. DO NOT EDIT.
import { validateAvailableVersionsOutcome, validateBackendStatusOutcome, validateCancelInstallationOutcome, validateCatalogSearchOutcome, validateCheckVersionDependenciesOutcome, validateCheckVersionDependenciesParams, validateConversionCancelledOutcome, validateConversionEnvironmentOutcome, validateConversionListOutcome, validateConversionProgressResponse, validateConversionSetupStartedOutcome, validateConversionSetupStatusOutcome, validateConversionStartedOutcome, validateDownloadIdParams, validateDownloadListOutcome, validateDownloadLocationReport, validateDownloadMutationOutcome, validateDownloadResolutionRequest, validateDownloadStartedOutcome, validateDownloadStatusOutcome, validateFindTorchAlternativesParams, validateFinishRetainedImportOutcome, validateGetBackendSetupParams, validateGetHfDownloadDetailsParams, validateGetReleaseDependenciesOutcome, validateGetReleaseDependenciesParams, validateGetTorchReleaseOptionsParams, validateGetTorchRuntimeProbeParams, validateGithubCacheStatusOutcome, validateHfDownloadDetailsOutcome, validateHfModelSearchOutcome, validateImportBatchParams, validateImportModelParams, validateInferenceSettingsOutcome, validateInstallVersionDependenciesOutcome, validateInstallVersionDependenciesParams, validateInstallVersionOutcome, validateInstallVersionParams, validateInstallationProgressOutcome, validateInstalledVersionsOutcome, validateLauncherRootSelectionResult, validateLauncherRootStartupState, validateLibraryModelMetadataOutcome, validateLibraryStatusResponse, validateLibraryUpgradeConfirmation, validateLibraryUpgradeResult, validateLinkHealthOutcome, validateModelCategoryOutcome, validateModelDownloadUpdateOutcome, validateModelIdParams, validateModelImportSelection, validateModelIndexRefreshOutcome, validateModelLibraryUpdateNotification, validateModelLookupOutcome, validateModelLookupParams, validateModelsOutcome, validateNetworkStatusResponse, validatePartialDownloadOutcome, validatePreviewTorchRuntimeParams, validatePublicError, validateRecoverDownloadParams, validateRemoveVersionOutcome, validateRouterProfileSyncStatus, validateRuntimeLaunchOutcome, validateRuntimeLaunchParams, validateRuntimeProfileUpdateFeed, validateRuntimeProfilesSnapshotResponse, validateRuntimeProvidersOutcome, validateRuntimeRunningOutcome, validateRuntimeStopOutcome, validateS3AuthenticatedBundleImportParams, validateS3AuthenticatedDiscoveryParams, validateS3AuthenticatedImportParams, validateS3BundleImportObservation, validateS3BundleImportParams, validateS3ConditionalFileParams, validateS3CredentialParams, validateS3DiscoveryOutcome, validateS3DiscoveryParams, validateS3ImportCancelOutcome, validateS3ImportCancelParams, validateS3ImportOutcome, validateS3ImportParams, validateS3ImportStatusParams, validateS3PersistedImportsWire, validateS3PinnedFileParams, validateS3ReadMode, validateS3SelectedFileParams, validateS3TransferRetryParams, validateS3TransferRetryState, validateSearchCatalogParams, validateSelectedVersionOutcome, validateServingStatusResponse, validateServingStatusUpdateFeed, validateSetDefaultVersionOutcome, validateSetDefaultVersionParams, validateSetHfTokenParams, validateStaleInstanceReleaseConfirmation, validateStaleInstanceReleaseResult, validateStartBackendSetupParams, validateStartConversionSetupParams, validateStartupFailureFrame, validateStatusTelemetrySnapshot, validateStatusTelemetryUpdateNotification, validateStopRuntimeProfileGenerationOutcome, validateStopRuntimeProfileGenerationParams, validateSuccessOutcome, validateSupportedQuantTypesOutcome, validateSwitchVersionOutcome, validateSwitchVersionParams, validateTorchAdapterRequirement, validateTorchAlternativeMatch, validateTorchAlternativesOutcome, validateTorchAlternativesStatus, validateTorchCapabilityProbe, validateTorchDriverAvailability, validateTorchGpuVendor, validateTorchInstalledRuntime, validateTorchProbeAdapterStatus, validateTorchProbeCoreStatus, validateTorchProbeStatus, validateTorchPythonChoice, validateTorchReleaseCombination, validateTorchReleaseDriverStatus, validateTorchReleaseOptionsOutcome, validateTorchReleaseOptionsStatus, validateTorchReleaseRecommendation, validateTorchRuntimeOptionsOutcome, validateTorchRuntimePreview, validateTorchRuntimePreviewArtifact, validateTorchRuntimePreviewOutcome, validateTorchRuntimePreviewQualification, validateTorchRuntimePreviewRejectionReason, validateTorchRuntimeProbeOutcome, validateTorchTrialCleanup, validateTorchTrialHealthStatus, validateTorchTrialStartupStatus, validateTorchVerifiedPackage, validateTorchVerifiedRuntime, validateTrialTorchRuntimeOutcome, validateTrialTorchRuntimeParams, validateUpdateInferenceSettingsOutcome, validateUpdateInferenceSettingsParams, validateUpdateModelNotesOutcome, validateUpdateModelNotesParams, validateUpdateStreamError, validateValidateInstallationsOutcome, validateVersionInfoOutcome, validateVersionStatusOutcome } from "./desktop-contract.validators.js";
class InvalidJsonRepresentation extends Error {
}
function copyJson(value, ancestors, budget) {
  if (--budget.remaining < 0) throw new InvalidJsonRepresentation("Oversized value");
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value !== "object" || ancestors.has(value)) throw new InvalidJsonRepresentation("Invalid JSON value");
  ancestors.add(value);
  if (Object.getOwnPropertySymbols(value).length !== 0) throw new InvalidJsonRepresentation("Invalid symbol property");
  const descriptors = Object.getOwnPropertyDescriptors(value);
  const entries = Object.entries(descriptors).filter(([key]) => !(Array.isArray(value) && key === "length"));
  if (Array.isArray(value) && (entries.length !== value.length || entries.some(([key]) => !/^(0|[1-9][0-9]*)$/.test(key)))) throw new InvalidJsonRepresentation("Invalid array properties");
  const result = Array.isArray(value) ? [] : /* @__PURE__ */ Object.create(null);
  for (const [key, descriptor] of entries) {
    if (!descriptor.enumerable || !("value" in descriptor)) throw new InvalidJsonRepresentation("Invalid property");
    Object.defineProperty(result, key, { value: copyJson(descriptor.value, ancestors, budget), enumerable: true, writable: false, configurable: false });
  }
  ancestors.delete(value);
  return Object.freeze(result);
}
function rejectionReason(validate) {
  const error = validate.errors?.[0];
  if (!error) return "Invalid desktop contract payload.";
  const field = error.params?.additionalProperty;
  const rule = error.keyword === "additionalProperties" && typeof field === "string" ? `unexpected field ${JSON.stringify(field.slice(0, 64))}` : error.message ?? error.keyword ?? "rejected";
  return `Invalid desktop contract payload at ${error.instancePath || "/"}: ${rule}.`.slice(0, 512);
}
function decode(input, validate) {
  try {
    const value = copyJson(input, /* @__PURE__ */ new Set(), { remaining: 1e6 });
    if (!validate(value)) return { status: "invalid", message: rejectionReason(validate) };
    return { status: "valid", value };
  } catch (error) {
    const reason = error instanceof InvalidJsonRepresentation ? `: ${error.message}` : "";
    return { status: "invalid", message: `Invalid desktop contract payload${reason}.` };
  }
}
function decodeAvailableVersionsOutcome(input) {
  return decode(input, validateAvailableVersionsOutcome);
}
function decodeBackendStatusOutcome(input) {
  return decode(input, validateBackendStatusOutcome);
}
function decodeCancelInstallationOutcome(input) {
  return decode(input, validateCancelInstallationOutcome);
}
function decodeCatalogSearchOutcome(input) {
  return decode(input, validateCatalogSearchOutcome);
}
function decodeCheckVersionDependenciesOutcome(input) {
  return decode(input, validateCheckVersionDependenciesOutcome);
}
function decodeCheckVersionDependenciesParams(input) {
  return decode(input, validateCheckVersionDependenciesParams);
}
function decodeConversionCancelledOutcome(input) {
  return decode(input, validateConversionCancelledOutcome);
}
function decodeConversionEnvironmentOutcome(input) {
  return decode(input, validateConversionEnvironmentOutcome);
}
function decodeConversionListOutcome(input) {
  return decode(input, validateConversionListOutcome);
}
function decodeConversionProgressResponse(input) {
  return decode(input, validateConversionProgressResponse);
}
function decodeConversionSetupStartedOutcome(input) {
  return decode(input, validateConversionSetupStartedOutcome);
}
function decodeConversionSetupStatusOutcome(input) {
  return decode(input, validateConversionSetupStatusOutcome);
}
function decodeConversionStartedOutcome(input) {
  return decode(input, validateConversionStartedOutcome);
}
function decodeDownloadIdParams(input) {
  return decode(input, validateDownloadIdParams);
}
function decodeDownloadListOutcome(input) {
  return decode(input, validateDownloadListOutcome);
}
function decodeDownloadLocationReport(input) {
  return decode(input, validateDownloadLocationReport);
}
function decodeDownloadMutationOutcome(input) {
  return decode(input, validateDownloadMutationOutcome);
}
function decodeDownloadResolutionRequest(input) {
  return decode(input, validateDownloadResolutionRequest);
}
function decodeDownloadStartedOutcome(input) {
  return decode(input, validateDownloadStartedOutcome);
}
function decodeDownloadStatusOutcome(input) {
  return decode(input, validateDownloadStatusOutcome);
}
function decodeFindTorchAlternativesParams(input) {
  return decode(input, validateFindTorchAlternativesParams);
}
function decodeFinishRetainedImportOutcome(input) {
  return decode(input, validateFinishRetainedImportOutcome);
}
function decodeGetBackendSetupParams(input) {
  return decode(input, validateGetBackendSetupParams);
}
function decodeGetHfDownloadDetailsParams(input) {
  return decode(input, validateGetHfDownloadDetailsParams);
}
function decodeGetReleaseDependenciesOutcome(input) {
  return decode(input, validateGetReleaseDependenciesOutcome);
}
function decodeGetReleaseDependenciesParams(input) {
  return decode(input, validateGetReleaseDependenciesParams);
}
function decodeGetTorchReleaseOptionsParams(input) {
  return decode(input, validateGetTorchReleaseOptionsParams);
}
function decodeGetTorchRuntimeProbeParams(input) {
  return decode(input, validateGetTorchRuntimeProbeParams);
}
function decodeGithubCacheStatusOutcome(input) {
  return decode(input, validateGithubCacheStatusOutcome);
}
function decodeHfDownloadDetailsOutcome(input) {
  return decode(input, validateHfDownloadDetailsOutcome);
}
function decodeHfModelSearchOutcome(input) {
  return decode(input, validateHfModelSearchOutcome);
}
function decodeImportBatchParams(input) {
  return decode(input, validateImportBatchParams);
}
function decodeImportModelParams(input) {
  return decode(input, validateImportModelParams);
}
function decodeInferenceSettingsOutcome(input) {
  return decode(input, validateInferenceSettingsOutcome);
}
function decodeInstallVersionDependenciesOutcome(input) {
  return decode(input, validateInstallVersionDependenciesOutcome);
}
function decodeInstallVersionDependenciesParams(input) {
  return decode(input, validateInstallVersionDependenciesParams);
}
function decodeInstallVersionOutcome(input) {
  return decode(input, validateInstallVersionOutcome);
}
function decodeInstallVersionParams(input) {
  return decode(input, validateInstallVersionParams);
}
function decodeInstallationProgressOutcome(input) {
  return decode(input, validateInstallationProgressOutcome);
}
function decodeInstalledVersionsOutcome(input) {
  return decode(input, validateInstalledVersionsOutcome);
}
function decodeLauncherRootSelectionResult(input) {
  return decode(input, validateLauncherRootSelectionResult);
}
function decodeLauncherRootStartupState(input) {
  return decode(input, validateLauncherRootStartupState);
}
function decodeLibraryModelMetadataOutcome(input) {
  return decode(input, validateLibraryModelMetadataOutcome);
}
function decodeLibraryStatusResponse(input) {
  return decode(input, validateLibraryStatusResponse);
}
function decodeLibraryUpgradeConfirmation(input) {
  return decode(input, validateLibraryUpgradeConfirmation);
}
function decodeLibraryUpgradeResult(input) {
  return decode(input, validateLibraryUpgradeResult);
}
function decodeLinkHealthOutcome(input) {
  return decode(input, validateLinkHealthOutcome);
}
function decodeModelCategoryOutcome(input) {
  return decode(input, validateModelCategoryOutcome);
}
function decodeModelDownloadUpdateOutcome(input) {
  return decode(input, validateModelDownloadUpdateOutcome);
}
function decodeModelIdParams(input) {
  return decode(input, validateModelIdParams);
}
function decodeModelImportSelection(input) {
  return decode(input, validateModelImportSelection);
}
function decodeModelIndexRefreshOutcome(input) {
  return decode(input, validateModelIndexRefreshOutcome);
}
function decodeModelLibraryUpdateNotification(input) {
  return decode(input, validateModelLibraryUpdateNotification);
}
function decodeModelLookupOutcome(input) {
  return decode(input, validateModelLookupOutcome);
}
function decodeModelLookupParams(input) {
  return decode(input, validateModelLookupParams);
}
function decodeModelsOutcome(input) {
  return decode(input, validateModelsOutcome);
}
function decodeNetworkStatusResponse(input) {
  return decode(input, validateNetworkStatusResponse);
}
function decodePartialDownloadOutcome(input) {
  return decode(input, validatePartialDownloadOutcome);
}
function decodePreviewTorchRuntimeParams(input) {
  return decode(input, validatePreviewTorchRuntimeParams);
}
function decodePublicError(input) {
  return decode(input, validatePublicError);
}
function decodeRecoverDownloadParams(input) {
  return decode(input, validateRecoverDownloadParams);
}
function decodeRemoveVersionOutcome(input) {
  return decode(input, validateRemoveVersionOutcome);
}
function decodeRouterProfileSyncStatus(input) {
  return decode(input, validateRouterProfileSyncStatus);
}
function decodeRuntimeLaunchOutcome(input) {
  return decode(input, validateRuntimeLaunchOutcome);
}
function decodeRuntimeLaunchParams(input) {
  return decode(input, validateRuntimeLaunchParams);
}
function decodeRuntimeProfileUpdateFeed(input) {
  return decode(input, validateRuntimeProfileUpdateFeed);
}
function decodeRuntimeProfilesSnapshotResponse(input) {
  return decode(input, validateRuntimeProfilesSnapshotResponse);
}
function decodeRuntimeProvidersOutcome(input) {
  return decode(input, validateRuntimeProvidersOutcome);
}
function decodeRuntimeRunningOutcome(input) {
  return decode(input, validateRuntimeRunningOutcome);
}
function decodeRuntimeStopOutcome(input) {
  return decode(input, validateRuntimeStopOutcome);
}
function decodeS3AuthenticatedBundleImportParams(input) {
  return decode(input, validateS3AuthenticatedBundleImportParams);
}
function decodeS3AuthenticatedDiscoveryParams(input) {
  return decode(input, validateS3AuthenticatedDiscoveryParams);
}
function decodeS3AuthenticatedImportParams(input) {
  return decode(input, validateS3AuthenticatedImportParams);
}
function decodeS3BundleImportObservation(input) {
  return decode(input, validateS3BundleImportObservation);
}
function decodeS3BundleImportParams(input) {
  return decode(input, validateS3BundleImportParams);
}
function decodeS3ConditionalFileParams(input) {
  return decode(input, validateS3ConditionalFileParams);
}
function decodeS3CredentialParams(input) {
  return decode(input, validateS3CredentialParams);
}
function decodeS3DiscoveryOutcome(input) {
  return decode(input, validateS3DiscoveryOutcome);
}
function decodeS3DiscoveryParams(input) {
  return decode(input, validateS3DiscoveryParams);
}
function decodeS3ImportCancelOutcome(input) {
  return decode(input, validateS3ImportCancelOutcome);
}
function decodeS3ImportCancelParams(input) {
  return decode(input, validateS3ImportCancelParams);
}
function decodeS3ImportOutcome(input) {
  return decode(input, validateS3ImportOutcome);
}
function decodeS3ImportParams(input) {
  return decode(input, validateS3ImportParams);
}
function decodeS3ImportStatusParams(input) {
  return decode(input, validateS3ImportStatusParams);
}
function decodeS3PersistedImportsWire(input) {
  return decode(input, validateS3PersistedImportsWire);
}
function decodeS3PinnedFileParams(input) {
  return decode(input, validateS3PinnedFileParams);
}
function decodeS3ReadMode(input) {
  return decode(input, validateS3ReadMode);
}
function decodeS3SelectedFileParams(input) {
  return decode(input, validateS3SelectedFileParams);
}
function decodeS3TransferRetryParams(input) {
  return decode(input, validateS3TransferRetryParams);
}
function decodeS3TransferRetryState(input) {
  return decode(input, validateS3TransferRetryState);
}
function decodeSearchCatalogParams(input) {
  return decode(input, validateSearchCatalogParams);
}
function decodeSelectedVersionOutcome(input) {
  return decode(input, validateSelectedVersionOutcome);
}
function decodeServingStatusResponse(input) {
  return decode(input, validateServingStatusResponse);
}
function decodeServingStatusUpdateFeed(input) {
  return decode(input, validateServingStatusUpdateFeed);
}
function decodeSetDefaultVersionOutcome(input) {
  return decode(input, validateSetDefaultVersionOutcome);
}
function decodeSetDefaultVersionParams(input) {
  return decode(input, validateSetDefaultVersionParams);
}
function decodeSetHfTokenParams(input) {
  return decode(input, validateSetHfTokenParams);
}
function decodeStaleInstanceReleaseConfirmation(input) {
  return decode(input, validateStaleInstanceReleaseConfirmation);
}
function decodeStaleInstanceReleaseResult(input) {
  return decode(input, validateStaleInstanceReleaseResult);
}
function decodeStartBackendSetupParams(input) {
  return decode(input, validateStartBackendSetupParams);
}
function decodeStartConversionSetupParams(input) {
  return decode(input, validateStartConversionSetupParams);
}
function decodeStartupFailureFrame(input) {
  return decode(input, validateStartupFailureFrame);
}
function decodeStatusTelemetrySnapshot(input) {
  return decode(input, validateStatusTelemetrySnapshot);
}
function decodeStatusTelemetryUpdateNotification(input) {
  return decode(input, validateStatusTelemetryUpdateNotification);
}
function decodeStopRuntimeProfileGenerationOutcome(input) {
  return decode(input, validateStopRuntimeProfileGenerationOutcome);
}
function decodeStopRuntimeProfileGenerationParams(input) {
  return decode(input, validateStopRuntimeProfileGenerationParams);
}
function decodeSuccessOutcome(input) {
  return decode(input, validateSuccessOutcome);
}
function decodeSupportedQuantTypesOutcome(input) {
  return decode(input, validateSupportedQuantTypesOutcome);
}
function decodeSwitchVersionOutcome(input) {
  return decode(input, validateSwitchVersionOutcome);
}
function decodeSwitchVersionParams(input) {
  return decode(input, validateSwitchVersionParams);
}
function decodeTorchAdapterRequirement(input) {
  return decode(input, validateTorchAdapterRequirement);
}
function decodeTorchAlternativeMatch(input) {
  return decode(input, validateTorchAlternativeMatch);
}
function decodeTorchAlternativesOutcome(input) {
  return decode(input, validateTorchAlternativesOutcome);
}
function decodeTorchAlternativesStatus(input) {
  return decode(input, validateTorchAlternativesStatus);
}
function decodeTorchCapabilityProbe(input) {
  return decode(input, validateTorchCapabilityProbe);
}
function decodeTorchDriverAvailability(input) {
  return decode(input, validateTorchDriverAvailability);
}
function decodeTorchGpuVendor(input) {
  return decode(input, validateTorchGpuVendor);
}
function decodeTorchInstalledRuntime(input) {
  return decode(input, validateTorchInstalledRuntime);
}
function decodeTorchProbeAdapterStatus(input) {
  return decode(input, validateTorchProbeAdapterStatus);
}
function decodeTorchProbeCoreStatus(input) {
  return decode(input, validateTorchProbeCoreStatus);
}
function decodeTorchProbeStatus(input) {
  return decode(input, validateTorchProbeStatus);
}
function decodeTorchPythonChoice(input) {
  return decode(input, validateTorchPythonChoice);
}
function decodeTorchReleaseCombination(input) {
  return decode(input, validateTorchReleaseCombination);
}
function decodeTorchReleaseDriverStatus(input) {
  return decode(input, validateTorchReleaseDriverStatus);
}
function decodeTorchReleaseOptionsOutcome(input) {
  return decode(input, validateTorchReleaseOptionsOutcome);
}
function decodeTorchReleaseOptionsStatus(input) {
  return decode(input, validateTorchReleaseOptionsStatus);
}
function decodeTorchReleaseRecommendation(input) {
  return decode(input, validateTorchReleaseRecommendation);
}
function decodeTorchRuntimeOptionsOutcome(input) {
  return decode(input, validateTorchRuntimeOptionsOutcome);
}
function decodeTorchRuntimePreview(input) {
  return decode(input, validateTorchRuntimePreview);
}
function decodeTorchRuntimePreviewArtifact(input) {
  return decode(input, validateTorchRuntimePreviewArtifact);
}
function decodeTorchRuntimePreviewOutcome(input) {
  return decode(input, validateTorchRuntimePreviewOutcome);
}
function decodeTorchRuntimePreviewQualification(input) {
  return decode(input, validateTorchRuntimePreviewQualification);
}
function decodeTorchRuntimePreviewRejectionReason(input) {
  return decode(input, validateTorchRuntimePreviewRejectionReason);
}
function decodeTorchRuntimeProbeOutcome(input) {
  return decode(input, validateTorchRuntimeProbeOutcome);
}
function decodeTorchTrialCleanup(input) {
  return decode(input, validateTorchTrialCleanup);
}
function decodeTorchTrialHealthStatus(input) {
  return decode(input, validateTorchTrialHealthStatus);
}
function decodeTorchTrialStartupStatus(input) {
  return decode(input, validateTorchTrialStartupStatus);
}
function decodeTorchVerifiedPackage(input) {
  return decode(input, validateTorchVerifiedPackage);
}
function decodeTorchVerifiedRuntime(input) {
  return decode(input, validateTorchVerifiedRuntime);
}
function decodeTrialTorchRuntimeOutcome(input) {
  return decode(input, validateTrialTorchRuntimeOutcome);
}
function decodeTrialTorchRuntimeParams(input) {
  return decode(input, validateTrialTorchRuntimeParams);
}
function decodeUpdateInferenceSettingsOutcome(input) {
  return decode(input, validateUpdateInferenceSettingsOutcome);
}
function decodeUpdateInferenceSettingsParams(input) {
  return decode(input, validateUpdateInferenceSettingsParams);
}
function decodeUpdateModelNotesOutcome(input) {
  return decode(input, validateUpdateModelNotesOutcome);
}
function decodeUpdateModelNotesParams(input) {
  return decode(input, validateUpdateModelNotesParams);
}
function decodeUpdateStreamError(input) {
  return decode(input, validateUpdateStreamError);
}
function decodeValidateInstallationsOutcome(input) {
  return decode(input, validateValidateInstallationsOutcome);
}
function decodeVersionInfoOutcome(input) {
  return decode(input, validateVersionInfoOutcome);
}
function decodeVersionStatusOutcome(input) {
  return decode(input, validateVersionStatusOutcome);
}
export {
  decodeAvailableVersionsOutcome,
  decodeBackendStatusOutcome,
  decodeCancelInstallationOutcome,
  decodeCatalogSearchOutcome,
  decodeCheckVersionDependenciesOutcome,
  decodeCheckVersionDependenciesParams,
  decodeConversionCancelledOutcome,
  decodeConversionEnvironmentOutcome,
  decodeConversionListOutcome,
  decodeConversionProgressResponse,
  decodeConversionSetupStartedOutcome,
  decodeConversionSetupStatusOutcome,
  decodeConversionStartedOutcome,
  decodeDownloadIdParams,
  decodeDownloadListOutcome,
  decodeDownloadLocationReport,
  decodeDownloadMutationOutcome,
  decodeDownloadResolutionRequest,
  decodeDownloadStartedOutcome,
  decodeDownloadStatusOutcome,
  decodeFindTorchAlternativesParams,
  decodeFinishRetainedImportOutcome,
  decodeGetBackendSetupParams,
  decodeGetHfDownloadDetailsParams,
  decodeGetReleaseDependenciesOutcome,
  decodeGetReleaseDependenciesParams,
  decodeGetTorchReleaseOptionsParams,
  decodeGetTorchRuntimeProbeParams,
  decodeGithubCacheStatusOutcome,
  decodeHfDownloadDetailsOutcome,
  decodeHfModelSearchOutcome,
  decodeImportBatchParams,
  decodeImportModelParams,
  decodeInferenceSettingsOutcome,
  decodeInstallVersionDependenciesOutcome,
  decodeInstallVersionDependenciesParams,
  decodeInstallVersionOutcome,
  decodeInstallVersionParams,
  decodeInstallationProgressOutcome,
  decodeInstalledVersionsOutcome,
  decodeLauncherRootSelectionResult,
  decodeLauncherRootStartupState,
  decodeLibraryModelMetadataOutcome,
  decodeLibraryStatusResponse,
  decodeLibraryUpgradeConfirmation,
  decodeLibraryUpgradeResult,
  decodeLinkHealthOutcome,
  decodeModelCategoryOutcome,
  decodeModelDownloadUpdateOutcome,
  decodeModelIdParams,
  decodeModelImportSelection,
  decodeModelIndexRefreshOutcome,
  decodeModelLibraryUpdateNotification,
  decodeModelLookupOutcome,
  decodeModelLookupParams,
  decodeModelsOutcome,
  decodeNetworkStatusResponse,
  decodePartialDownloadOutcome,
  decodePreviewTorchRuntimeParams,
  decodePublicError,
  decodeRecoverDownloadParams,
  decodeRemoveVersionOutcome,
  decodeRouterProfileSyncStatus,
  decodeRuntimeLaunchOutcome,
  decodeRuntimeLaunchParams,
  decodeRuntimeProfileUpdateFeed,
  decodeRuntimeProfilesSnapshotResponse,
  decodeRuntimeProvidersOutcome,
  decodeRuntimeRunningOutcome,
  decodeRuntimeStopOutcome,
  decodeS3AuthenticatedBundleImportParams,
  decodeS3AuthenticatedDiscoveryParams,
  decodeS3AuthenticatedImportParams,
  decodeS3BundleImportObservation,
  decodeS3BundleImportParams,
  decodeS3ConditionalFileParams,
  decodeS3CredentialParams,
  decodeS3DiscoveryOutcome,
  decodeS3DiscoveryParams,
  decodeS3ImportCancelOutcome,
  decodeS3ImportCancelParams,
  decodeS3ImportOutcome,
  decodeS3ImportParams,
  decodeS3ImportStatusParams,
  decodeS3PersistedImportsWire,
  decodeS3PinnedFileParams,
  decodeS3ReadMode,
  decodeS3SelectedFileParams,
  decodeS3TransferRetryParams,
  decodeS3TransferRetryState,
  decodeSearchCatalogParams,
  decodeSelectedVersionOutcome,
  decodeServingStatusResponse,
  decodeServingStatusUpdateFeed,
  decodeSetDefaultVersionOutcome,
  decodeSetDefaultVersionParams,
  decodeSetHfTokenParams,
  decodeStaleInstanceReleaseConfirmation,
  decodeStaleInstanceReleaseResult,
  decodeStartBackendSetupParams,
  decodeStartConversionSetupParams,
  decodeStartupFailureFrame,
  decodeStatusTelemetrySnapshot,
  decodeStatusTelemetryUpdateNotification,
  decodeStopRuntimeProfileGenerationOutcome,
  decodeStopRuntimeProfileGenerationParams,
  decodeSuccessOutcome,
  decodeSupportedQuantTypesOutcome,
  decodeSwitchVersionOutcome,
  decodeSwitchVersionParams,
  decodeTorchAdapterRequirement,
  decodeTorchAlternativeMatch,
  decodeTorchAlternativesOutcome,
  decodeTorchAlternativesStatus,
  decodeTorchCapabilityProbe,
  decodeTorchDriverAvailability,
  decodeTorchGpuVendor,
  decodeTorchInstalledRuntime,
  decodeTorchProbeAdapterStatus,
  decodeTorchProbeCoreStatus,
  decodeTorchProbeStatus,
  decodeTorchPythonChoice,
  decodeTorchReleaseCombination,
  decodeTorchReleaseDriverStatus,
  decodeTorchReleaseOptionsOutcome,
  decodeTorchReleaseOptionsStatus,
  decodeTorchReleaseRecommendation,
  decodeTorchRuntimeOptionsOutcome,
  decodeTorchRuntimePreview,
  decodeTorchRuntimePreviewArtifact,
  decodeTorchRuntimePreviewOutcome,
  decodeTorchRuntimePreviewQualification,
  decodeTorchRuntimePreviewRejectionReason,
  decodeTorchRuntimeProbeOutcome,
  decodeTorchTrialCleanup,
  decodeTorchTrialHealthStatus,
  decodeTorchTrialStartupStatus,
  decodeTorchVerifiedPackage,
  decodeTorchVerifiedRuntime,
  decodeTrialTorchRuntimeOutcome,
  decodeTrialTorchRuntimeParams,
  decodeUpdateInferenceSettingsOutcome,
  decodeUpdateInferenceSettingsParams,
  decodeUpdateModelNotesOutcome,
  decodeUpdateModelNotesParams,
  decodeUpdateStreamError,
  decodeValidateInstallationsOutcome,
  decodeVersionInfoOutcome,
  decodeVersionStatusOutcome
};
