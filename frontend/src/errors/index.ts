/**
 * Frontend Error Hierarchy
 *
 * Custom error types for the ComfyUI Launcher frontend.
 * Mirrors the backend exception hierarchy from backend/exceptions.py
 */

/**
 * Base error class for all ComfyUI Launcher frontend errors
 */
export class ComfyUILauncherError extends Error {
  constructor(message: string, public override cause?: Error) {
    super(message);
    this.name = this.constructor.name;
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, this.constructor);
    }
  }
}

/**
 * Network-related errors (HTTP, WebSocket, etc.)
 */
export class NetworkError extends ComfyUILauncherError {
  constructor(
    message: string,
    public url?: string,
    public status?: number,
    cause?: Error
  ) {
    super(message, cause);
  }
}

/**
 * PyWebView API call failures
 */
export class APIError extends ComfyUILauncherError {
  constructor(
    message: string,
    public endpoint?: string,
    cause?: Error
  ) {
    super(message, cause);
  }
}

/**
 * Input validation failures
 */
export class ValidationError extends ComfyUILauncherError {
  constructor(
    message: string,
    public field?: string,
    cause?: Error
  ) {
    super(message, cause);
  }
}

/**
 * Metadata corruption or parsing errors
 */
export class MetadataError extends ComfyUILauncherError {
  constructor(
    message: string,
    public filePath?: string,
    cause?: Error
  ) {
    super(message, cause);
  }
}

/**
 * Process management errors (launch, stop, etc.)
 */
export class ProcessError extends ComfyUILauncherError {
  constructor(
    message: string,
    public exitCode?: number,
    cause?: Error
  ) {
    super(message, cause);
  }
}

/**
 * Resource management errors (disk space, memory, etc.)
 */
export class ResourceError extends ComfyUILauncherError {
  constructor(
    message: string,
    public resourceType?: string,
    cause?: Error
  ) {
    super(message, cause);
  }
}

/**
 * Type guard helper to check if an error is a known ComfyUI error
 */
export function isKnownError(error: unknown): error is ComfyUILauncherError {
  return error instanceof ComfyUILauncherError;
}
