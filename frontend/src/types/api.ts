/**
 * Common API Response Type Definitions
 *
 * Shared patterns and utility types for API interactions.
 */

// ============================================================================
// Base Response Types
// ============================================================================

/**
 * Base response interface for all API calls
 */
export interface BaseResponse {
  success: boolean;
  error?: string;
}

/**
 * Generic response with data payload
 */
export interface DataResponse<T> extends BaseResponse {
  data: T;
}

/**
 * Generic list response
 */
export interface ListResponse<T> extends BaseResponse {
  items: T[];
  total?: number;
}

// ============================================================================
// Progress & Status Types
// ============================================================================

/**
 * Generic progress status
 */
export interface ProgressStatus {
  status: 'idle' | 'in-progress' | 'complete' | 'error';
  progress: number; // 0-100
  message: string;
}

/**
 * Detailed progress with stages
 */
export interface StagedProgress {
  currentStage: string;
  totalStages: number;
  stageProgress: number; // 0-100
  overallProgress: number; // 0-100
  message: string;
}

// ============================================================================
// Network & Connectivity Types
// ============================================================================

/**
 * Network status
 */
export interface NetworkStatus {
  online: boolean;
  latency?: number;
  lastCheck?: string;
}

/**
 * Download progress
 */
export interface DownloadProgress {
  downloadedBytes: number;
  totalBytes: number | null;
  speed: number | null; // bytes per second
  etaSeconds: number | null;
  status: 'idle' | 'downloading' | 'paused' | 'complete' | 'error';
}

// ============================================================================
// Async Operation Status Types
// ============================================================================

/**
 * Type-safe async operation status using discriminated unions
 */
export type AsyncOperationStatus<TData = unknown, TError = Error> =
  | { state: 'idle' }
  | { state: 'loading' }
  | { state: 'success'; data: TData }
  | { state: 'error'; error: TError };

/**
 * Helper to create idle status
 */
export const createIdleStatus = (): AsyncOperationStatus => ({
  state: 'idle',
});

/**
 * Helper to create loading status
 */
export const createLoadingStatus = (): AsyncOperationStatus => ({
  state: 'loading',
});

/**
 * Helper to create success status
 */
export const createSuccessStatus = <T>(data: T): AsyncOperationStatus<T> => ({
  state: 'success',
  data,
});

/**
 * Helper to create error status
 */
export const createErrorStatus = <E = Error>(error: E): AsyncOperationStatus<unknown, E> => ({
  state: 'error',
  error,
});

// ============================================================================
// Pagination Types
// ============================================================================

/**
 * Pagination parameters
 */
export interface PaginationParams {
  page: number;
  pageSize: number;
  sortBy?: string;
  sortOrder?: 'asc' | 'desc';
}

/**
 * Paginated response
 */
export interface PaginatedResponse<T> extends BaseResponse {
  items: T[];
  pagination: {
    page: number;
    pageSize: number;
    total: number;
    totalPages: number;
    hasNext: boolean;
    hasPrevious: boolean;
  };
}

// ============================================================================
// Cache Types
// ============================================================================

/**
 * Cache metadata
 */
export interface CacheMetadata {
  hasCache: boolean;
  isValid: boolean;
  isFetching: boolean;
  ageSeconds?: number;
  lastFetched?: string;
  expiresAt?: string;
}

// ============================================================================
// Validation Types
// ============================================================================

/**
 * Validation error
 */
export interface ValidationError {
  field: string;
  message: string;
  code?: string;
}

/**
 * Validation result
 */
export interface ValidationResult extends BaseResponse {
  isValid: boolean;
  errors: ValidationError[];
}

// ============================================================================
// Type Guards
// ============================================================================

/**
 * Type guard to check if response is successful
 */
export function isSuccessResponse(response: BaseResponse): boolean {
  return response.success === true;
}

/**
 * Type guard to check if async operation is loading
 */
export function isLoading<T, E>(status: AsyncOperationStatus<T, E>): status is { state: 'loading' } {
  return status.state === 'loading';
}

/**
 * Type guard to check if async operation succeeded
 */
export function isSuccess<T, E>(
  status: AsyncOperationStatus<T, E>
): status is { state: 'success'; data: T } {
  return status.state === 'success';
}

/**
 * Type guard to check if async operation failed
 */
export function isError<T, E>(
  status: AsyncOperationStatus<T, E>
): status is { state: 'error'; error: E } {
  return status.state === 'error';
}

/**
 * Type guard to check if async operation is idle
 */
export function isIdle<T, E>(status: AsyncOperationStatus<T, E>): status is { state: 'idle' } {
  return status.state === 'idle';
}

// ============================================================================
// Result Type (Rust-inspired)
// ============================================================================

/**
 * Result type for operations that can succeed or fail
 */
export type Result<T, E = Error> = { ok: true; value: T } | { ok: false; error: E };

/**
 * Create a success result
 */
export function Ok<T>(value: T): Result<T, never> {
  return { ok: true, value };
}

/**
 * Create an error result
 */
export function Err<E>(error: E): Result<never, E> {
  return { ok: false, error };
}

/**
 * Type guard to check if result is ok
 */
export function isOk<T, E>(result: Result<T, E>): result is { ok: true; value: T } {
  return result.ok === true;
}

/**
 * Type guard to check if result is error
 */
export function isErr<T, E>(result: Result<T, E>): result is { ok: false; error: E } {
  return result.ok === false;
}
