import {
  decodeS3BundleImportObservation, type S3BundleImportObservation,
  decodeS3ImportOutcome, decodeS3ImportCancelOutcome,
  type S3ImportOutcome, type S3ImportCancelOutcome, type PublicError,
} from './generated/desktop-contract';

export const S3_RPC_FAILURE = 'S3 import RPC is unavailable. Observe the same operation without resubmitting.';
export const S3_RPC_RESPONSE_LIMIT = 64 * 1024;
export function isS3ImportMethod(method: string): boolean {
  return ['start_s3_model_import', 'get_s3_model_import', 'cancel_s3_model_import',
    'start_authenticated_s3_model_import', 'start_s3_model_bundle_import',
    'start_authenticated_s3_model_bundle_import', 'get_s3_model_bundle_import'].includes(method);
}
function safeError(error: PublicError): PublicError {
  // The local producer has static errors, but a malformed/reflected transport
  // response must not carry arbitrary message text into IPC or renderer logs.
  return { code: error.code, class: error.class, message: 'The S3 import request could not be completed.' };
}
function safeOutcome(value: S3ImportOutcome): S3ImportOutcome {
  if (value.status === 'rejected') return { ...value, error: safeError(value.error) };
  if (value.status === 'finished' && value.result.status === 'failed') {
    return { ...value, result: { ...value.result, error: safeError(value.result.error) } };
  }
  return value;
}
/** The privileged receiving boundary decodes before forwarding an IPC value. */
export function decodeS3ImportRpcResult(method: string, value: unknown): S3ImportOutcome | S3ImportCancelOutcome | S3BundleImportObservation {
  if (method === 'get_s3_model_bundle_import') {
    const decoded = decodeS3BundleImportObservation(value);
    if (decoded.status !== 'valid') throw new Error(S3_RPC_FAILURE);
    return { ...decoded.value, outcome: safeOutcome(decoded.value.outcome) };
  }
  if (method === 'cancel_s3_model_import') {
    const decoded = decodeS3ImportCancelOutcome(value);
    if (decoded.status !== 'valid') throw new Error(S3_RPC_FAILURE);
    return { accepted: decoded.value.accepted, outcome: safeOutcome(decoded.value.outcome) };
  }
  const decoded = decodeS3ImportOutcome(value);
  if (decoded.status !== 'valid') throw new Error(S3_RPC_FAILURE);
  return safeOutcome(decoded.value);
}
export async function receiveS3ImportRpc(method: string, invoke: () => Promise<unknown>): Promise<S3ImportOutcome | S3ImportCancelOutcome | S3BundleImportObservation> {
  try { return decodeS3ImportRpcResult(method, await invoke()); }
  catch { throw new Error(S3_RPC_FAILURE); }
}
