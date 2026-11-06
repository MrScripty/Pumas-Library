import type { OpenDialogOptions } from 'electron';
import {
  decodeS3TransferRetryParams,
  decodeS3DiscoveryParams,
  decodeS3AuthenticatedDiscoveryParams,
  decodeS3ImportParams,
  decodeS3AuthenticatedImportParams,
  decodeS3BundleImportParams,
  decodeS3AuthenticatedBundleImportParams,
  decodeS3ImportStatusParams,
  decodeS3ImportCancelParams,
  decodeDownloadIdParams,
  decodeGetBackendSetupParams,
  decodeCheckVersionDependenciesParams,
  decodeGetReleaseDependenciesParams,
  decodeInstallVersionDependenciesParams,
  decodeRuntimeLaunchParams,
  decodeSwitchVersionParams,
  decodeInstallVersionParams,
  decodeSetDefaultVersionParams,
  decodeStartBackendSetupParams,
  decodeSetHfTokenParams,
  decodeModelIdParams,
  decodeImportModelParams,
  decodeImportBatchParams,
  decodeGetTorchReleaseOptionsParams,
  decodePreviewTorchRuntimeParams,
  decodeGetTorchRuntimeProbeParams,
  decodeTrialTorchRuntimeParams,
  decodeStopRuntimeProfileGenerationParams,
  decodeFindTorchAlternativesParams,
  decodeGetHfDownloadDetailsParams,
  decodeSearchCatalogParams,
  decodeUpdateInferenceSettingsParams,
  decodeUpdateModelNotesParams,
  decodeStartConversionSetupParams,
  type DecodeOutcome,
} from './desktop-contract';
import {
  getRpcParamsValidationPolicy,
  getRpcRequestSchema,
  RPC_METHOD_REGISTRY,
  type RpcParamFieldType,
  type RpcMethodName,
  type RpcRequestSchema,
} from './rpc-method-registry';

export const ALLOWED_RPC_METHODS = RPC_METHOD_REGISTRY.methods;

export type ApiCallPayload = {
  method: RpcMethodName;
  params: Record<string, unknown>;
};

type DialogProperty = NonNullable<OpenDialogOptions['properties']>[number];

const ALLOWED_RPC_METHOD_SET = new Set<string>(ALLOWED_RPC_METHODS);

const ALLOWED_DIALOG_PROPERTIES = new Set<DialogProperty>([
  'openFile',
  'openDirectory',
  'multiSelections',
  'showHiddenFiles',
  'createDirectory',
  'promptToCreate',
  'noResolveAliases',
  'treatPackageAsDirectory',
  'dontAddToRecent',
]);

export function validateApiCallPayload(rawMethod: unknown, rawParams: unknown): ApiCallPayload {
  if (typeof rawMethod !== 'string' || rawMethod.length === 0) {
    throw new Error('Invalid API method payload');
  }

  if (!ALLOWED_RPC_METHOD_SET.has(rawMethod)) {
    throw new Error(`Unknown API method: ${rawMethod}`);
  }

  const method = rawMethod as RpcMethodName;
  if (method === 'inspect_persisted_s3_imports') {
    const decoded = decodeRuntimeLaunchParams(rawParams ?? {});
    if (decoded.status !== 'valid') throw new Error('Invalid S3 inspection parameters');
    return { method, params: decoded.value };
  }
  if (method === 'get_s3_transfer_retry' || method === 'retry_s3_model_transfer') {
    const decoded = method === 'get_s3_transfer_retry'
      ? decodeS3ImportStatusParams(rawParams) : decodeS3TransferRetryParams(rawParams);
    if (decoded.status !== 'valid') throw new Error('Invalid S3 retry parameters');
    return { method, params: decoded.value };
  }
  if (method.endsWith('_s3_prefix_discovery')) {
    const decoded = method === 'start_s3_prefix_discovery' ? decodeS3DiscoveryParams(rawParams)
      : method === 'start_authenticated_s3_prefix_discovery' ? decodeS3AuthenticatedDiscoveryParams(rawParams)
      : method === 'get_s3_prefix_discovery' ? decodeS3ImportStatusParams(rawParams) : decodeS3ImportCancelParams(rawParams);
    if (decoded.status !== 'valid') throw new Error('Invalid S3 discovery parameters');
    return {method, params: decoded.value};
  }
  if (method === 'start_s3_model_bundle_import' || method === 'start_authenticated_s3_model_bundle_import' || method === 'get_s3_model_bundle_import') {
    const decoded = method === 'start_s3_model_bundle_import' ? decodeS3BundleImportParams(rawParams)
      : method === 'start_authenticated_s3_model_bundle_import' ? decodeS3AuthenticatedBundleImportParams(rawParams)
      : decodeS3ImportStatusParams(rawParams);
    if (decoded.status !== 'valid') throw new Error('Invalid S3 bundle parameters');
    return { method, params: decoded.value };
  }
  if (method === 'start_authenticated_s3_model_import') {
    const decoded = decodeS3AuthenticatedImportParams(rawParams);
    if (decoded.status !== 'valid') throw new Error('Invalid authenticated S3 import parameters');
    return { method, params: decoded.value };
  }
  if (method === 'start_s3_model_import' || method === 'get_s3_model_import' || method === 'cancel_s3_model_import') {
    const decoded = method === 'start_s3_model_import'
      ? decodeS3ImportParams(rawParams)
      : method === 'get_s3_model_import'
        ? decodeS3ImportStatusParams(rawParams)
        : decodeS3ImportCancelParams(rawParams);
    if (decoded.status !== 'valid') throw new Error(`Invalid API params for method: ${method}`);
    return { method, params: decoded.value };
  }

  if (
    method === 'get_model_download_status'
    || method === 'pause_model_download'
    || method === 'resume_model_download'
    || method === 'cancel_model_download'
  ) {
    const decoded = decodeDownloadIdParams(rawParams);
    if (decoded.status !== 'valid' || decoded.value.download_id.trim().length === 0) {
      throw new Error(`Invalid API params for method: ${method}`);
    }
    return { method, params: decoded.value };
  }
  // These requests share Rust's complete generated contract.
  // Return its copied proof-bearing value, never the original renderer object.
  if (
    method === 'start_backend_setup'
    || method === 'get_backend_setup'
    || method === 'check_version_dependencies'
    || method === 'get_release_dependencies'
    || method === 'install_version_dependencies'
    || method === 'launch_ollama'
    || method === 'launch_torch'
    || method === 'stop_ollama'
    || method === 'stop_torch'
    || method === 'is_ollama_running'
    || method === 'is_torch_running'
    || method === 'switch_version'
    || method === 'install_version'
    || method === 'set_default_version'
  ) {
    const decoded = method === 'start_backend_setup'
      ? decodeStartBackendSetupParams(rawParams)
      : method === 'get_backend_setup'
        ? decodeGetBackendSetupParams(rawParams)
        : method === 'check_version_dependencies'
          ? decodeCheckVersionDependenciesParams(rawParams)
        : method === 'get_release_dependencies'
          ? decodeGetReleaseDependenciesParams(rawParams)
        : method === 'install_version_dependencies'
          ? decodeInstallVersionDependenciesParams(rawParams)
        : method === 'launch_ollama' || method === 'launch_torch'
          || method === 'stop_ollama' || method === 'stop_torch'
          || method === 'is_ollama_running' || method === 'is_torch_running'
          ? decodeRuntimeLaunchParams(rawParams === undefined ? {} : rawParams)
        : method === 'switch_version'
          ? decodeSwitchVersionParams(rawParams)
        : method === 'install_version'
          ? decodeInstallVersionParams(rawParams)
          : decodeSetDefaultVersionParams(rawParams);
    if (decoded.status !== 'valid') {
      throw new Error(`Invalid API params for method: ${method}`);
    }
    return { method, params: decoded.value };
  }
  const exportedDecoder = EXPORTED_REQUEST_DECODERS[method];
  if (exportedDecoder) {
    const decoded = exportedDecoder(withoutUndefinedFields(rawParams === undefined ? {} : rawParams));
    if (decoded.status !== 'valid') {
      throw new Error(`Invalid API params for method: ${method}: ${decoded.message}`);
    }
    return { method, params: decoded.value as Record<string, unknown> };
  }

  // Mutating requests decoded with Rust's exported request declarations.
  // Omitted optionals may arrive as undefined, which JSON (and the decoder)
  // cannot represent, so they are dropped first.
  if (
    method === 'set_hf_token'
    || method === 'delete_model_with_cascade'
    || method === 'get_links_for_model'
    || method === 'import_model'
    || method === 'import_batch'
  ) {
    const request = withoutUndefinedFields(rawParams);
    const decoded = method === 'set_hf_token'
      ? decodeSetHfTokenParams(request)
      : method === 'import_model'
        ? decodeImportModelParams(request)
        : method === 'import_batch'
          ? decodeImportBatchParams(request)
          : decodeModelIdParams(request);
    // The reason names the rule and location only; never a value (tokens).
    if (decoded.status !== 'valid') {
      throw new Error(`Invalid API params for method: ${method}: ${decoded.message}`);
    }
    if (method === 'set_hf_token' && (decoded.value as { token: string }).token.trim().length === 0) {
      throw new Error('Invalid API params for method: set_hf_token: token must not be empty');
    }
    if ((method === 'delete_model_with_cascade' || method === 'get_links_for_model')
      && (decoded.value as { model_id: string }).model_id.trim().length === 0) {
      throw new Error(`Invalid API params for method: ${method}: model_id must not be empty`);
    }
    return { method, params: decoded.value as Record<string, unknown> };
  }
  if (method === 'torch_configure') {
    return { method, params: validateTorchConfigureParams(rawParams) };
  }

  const params = normalizeApiParams(rawParams);
  validateApiParamsForMethod(method, params);

  return { method, params };
}

/**
 * Requests whose Rust declaration is exported and which preload already
 * decodes; main decodes them again because a renderer can bypass preload.
 */
const EXPORTED_REQUEST_DECODERS: Partial<Record<RpcMethodName, (value: unknown) => DecodeOutcome<unknown>>> = {
  get_torch_release_options: decodeGetTorchReleaseOptionsParams,
  preview_torch_runtime: decodePreviewTorchRuntimeParams,
  get_torch_runtime_probe: decodeGetTorchRuntimeProbeParams,
  trial_torch_runtime: decodeTrialTorchRuntimeParams,
  stop_runtime_profile_if_generation: decodeStopRuntimeProfileGenerationParams,
  find_torch_alternatives: decodeFindTorchAlternativesParams,
  get_hf_download_details: decodeGetHfDownloadDetailsParams,
  search_models_fts: decodeSearchCatalogParams,
  update_inference_settings: decodeUpdateInferenceSettingsParams,
  update_model_notes: decodeUpdateModelNotesParams,
  start_conversion_setup: decodeStartConversionSetupParams,
};

/** Drop undefined-valued fields from plain objects, recursively (JSON omits them). */
function withoutUndefinedFields(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(withoutUndefinedFields);
  if (!isPlainRecord(value)) return value;
  const result: Record<string, unknown> = {};
  for (const [key, field] of Object.entries(value)) {
    if (field !== undefined) result[key] = withoutUndefinedFields(field);
  }
  return result;
}

const TORCH_CONFIG_FIELDS = ['api_port', 'host', 'max_loaded_models', 'lan_access'] as const;

/**
 * `torch_configure` reaches `pumas_app_manager::TorchServerConfig`, which has
 * no exported schema yet. Mirror its complete shape here: `{ connection_url?,
 * config: { api_port, host, max_loaded_models, lan_access } }`. Host and LAN
 * policy stay with `TorchServerConfig::validate` in Rust.
 */
function validateTorchConfigureParams(rawParams: unknown): Record<string, unknown> {
  const params = withoutUndefinedFields(rawParams);
  const fail = (reason: string): never => {
    throw new Error(`Invalid API params for method: torch_configure: ${reason}`);
  };
  if (!isPlainRecord(params)) return fail('params must be an object');
  for (const key of Object.keys(params)) {
    if (key !== 'config' && key !== 'connection_url') fail(`unexpected field ${JSON.stringify(key.slice(0, 64))}`);
  }
  if (params.connection_url !== undefined && params.connection_url !== null
    && (typeof params.connection_url !== 'string' || params.connection_url.length === 0)) {
    fail('connection_url must be a non-empty string');
  }
  const config = params.config;
  if (!isPlainRecord(config)) return fail('config must be an object');
  for (const key of Object.keys(config)) {
    if (!(TORCH_CONFIG_FIELDS as readonly string[]).includes(key)) fail(`unexpected config field ${JSON.stringify(key.slice(0, 64))}`);
  }
  if (!Number.isInteger(config.api_port) || (config.api_port as number) < 1 || (config.api_port as number) > 65_535) {
    fail('config.api_port must be an integer port (1-65535)');
  }
  if (typeof config.host !== 'string' || config.host.length === 0) fail('config.host must be a non-empty string');
  if (!Number.isSafeInteger(config.max_loaded_models) || (config.max_loaded_models as number) < 0) {
    fail('config.max_loaded_models must be a non-negative integer');
  }
  if (typeof config.lan_access !== 'boolean') fail('config.lan_access must be a boolean');
  return params;
}

function normalizeApiParams(rawParams: unknown): Record<string, unknown> {
  if (rawParams === undefined || rawParams === null) {
    return {};
  }

  if (!isPlainRecord(rawParams)) {
    throw new Error('Invalid API params payload');
  }

  return rawParams;
}

function validateApiParamsForMethod(method: RpcMethodName, params: Record<string, unknown>) {
  const paramsValidation = getRpcParamsValidationPolicy(method);
  if (paramsValidation === 'empty-record' && Object.keys(params).length > 0) {
    throw new Error(`Unexpected API params for method: ${method}`);
  }

  const requestSchema = getRpcRequestSchema(method);
  if (requestSchema) {
    validateRequestSchema(method, params, requestSchema);
  }
}

function validateRequestSchema(
  method: RpcMethodName,
  params: Record<string, unknown>,
  schema: RpcRequestSchema
) {
  const required = schema.required ?? {};
  const optional = schema.optional ?? {};
  const nullable = schema.nullable ?? {};
  const knownFields = new Set([
    ...Object.keys(required),
    ...Object.keys(optional),
    ...Object.keys(nullable),
  ]);

  for (const [field, fieldType] of Object.entries(required)) {
    if (!Object.hasOwn(params, field)) {
      throw new Error(`Missing required API param for method ${method}: ${field}`);
    }
    validateParamField(method, field, params[field], fieldType, {
      allowNull: false,
      allowUndefined: false,
    });
  }

  for (const [field, value] of Object.entries(params)) {
    const requiredType = required[field];
    const optionalType = optional[field];
    const nullableType = nullable[field];

    if (requiredType) {
      continue;
    }
    if (optionalType) {
      validateParamField(method, field, value, optionalType, {
        allowNull: false,
        allowUndefined: true,
      });
      continue;
    }
    if (nullableType) {
      validateParamField(method, field, value, nullableType, {
        allowNull: true,
        allowUndefined: true,
      });
      continue;
    }
    if (!schema.allowUnknown && !knownFields.has(field)) {
      throw new Error(`Unexpected API param for method ${method}: ${field}`);
    }
  }
}

function validateParamField(
  method: RpcMethodName,
  field: string,
  value: unknown,
  fieldType: RpcParamFieldType,
  options: {
    allowNull: boolean;
    allowUndefined: boolean;
  }
) {
  if (options.allowUndefined && value === undefined) {
    return;
  }

  if (value === null) {
    if (options.allowNull) {
      return;
    }
    throw new Error(`Invalid API param for method ${method}: ${field}`);
  }

  if (!isValidParamFieldType(value, fieldType)) {
    throw new Error(`Invalid API param for method ${method}: ${field}`);
  }
}

function isValidParamFieldType(value: unknown, fieldType: RpcParamFieldType): boolean {
  switch (fieldType) {
    case 'boolean':
      return typeof value === 'boolean';
    case 'number':
      return typeof value === 'number' && Number.isFinite(value);
    case 'string':
      return typeof value === 'string' && value.length > 0;
    case 'string-array':
      return Array.isArray(value) && value.every((item) => typeof item === 'string');
    case 'string-record':
      return isPlainRecord(value) && Object.values(value).every((item) => typeof item === 'string');
    case 'unknown-record':
      return isPlainRecord(value);
    case 'unknown-array':
      return Array.isArray(value);
  }
}

export function sanitizeOpenDialogOptions(rawOptions: unknown): OpenDialogOptions {
  if (!isPlainRecord(rawOptions)) {
    throw new Error('Invalid dialog options payload');
  }

  const options: OpenDialogOptions = {};

  if (typeof rawOptions.title === 'string') {
    options.title = rawOptions.title;
  }

  if (typeof rawOptions.defaultPath === 'string') {
    options.defaultPath = rawOptions.defaultPath;
  }

  if (typeof rawOptions.buttonLabel === 'string') {
    options.buttonLabel = rawOptions.buttonLabel;
  }

  if (typeof rawOptions.message === 'string') {
    options.message = rawOptions.message;
  }

  if (Array.isArray(rawOptions.properties)) {
    const properties = rawOptions.properties.filter(isDialogProperty);
    if (properties.length > 0) {
      options.properties = properties;
    }
  }

  if (Array.isArray(rawOptions.filters)) {
    const filters = rawOptions.filters
      .map((filter) => sanitizeDialogFilter(filter))
      .filter((filter): filter is NonNullable<OpenDialogOptions['filters']>[number] =>
        filter !== null
      );
    if (filters.length > 0) {
      options.filters = filters;
    }
  }

  return options;
}

export function validateExternalUrl(rawUrl: unknown): string {
  if (typeof rawUrl !== 'string') {
    throw new Error('Invalid URL payload');
  }

  let parsedUrl: URL;
  try {
    parsedUrl = new URL(rawUrl);
  } catch {
    throw new Error('Invalid URL');
  }

  if (parsedUrl.protocol !== 'http:' && parsedUrl.protocol !== 'https:') {
    throw new Error('Only http/https URLs are allowed');
  }

  return parsedUrl.toString();
}

function sanitizeDialogFilter(
  rawFilter: unknown
): NonNullable<OpenDialogOptions['filters']>[number] | null {
  if (!isPlainRecord(rawFilter)) {
    return null;
  }

  if (typeof rawFilter.name !== 'string' || !Array.isArray(rawFilter.extensions)) {
    return null;
  }

  const extensions = rawFilter.extensions.filter(
    (extension): extension is string => typeof extension === 'string' && extension.length > 0
  );

  if (extensions.length === 0) {
    return null;
  }

  return {
    name: rawFilter.name,
    extensions,
  };
}

function isDialogProperty(value: unknown): value is DialogProperty {
  return typeof value === 'string' && ALLOWED_DIALOG_PROPERTIES.has(value as DialogProperty);
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
