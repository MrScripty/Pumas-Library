import type { OpenDialogOptions } from 'electron';
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
  const params = normalizeApiParams(rawParams);
  validateApiParamsForMethod(method, params);

  return { method, params };
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
