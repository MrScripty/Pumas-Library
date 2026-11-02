/**
 * Startup diagnostics from the owned RPC process, before readiness.
 *
 * The backend writes one `PUMAS_STARTUP_FAILURE=` line holding a
 * `StartupFailureFrame` (schema in the generated desktop contract). The backend
 * may come from another build, so unknown fields are ignored and an unknown
 * reason is treated as `backend-unavailable`.
 */
import { decodeStartupFailureFrame } from './generated/desktop-contract';

const STARTUP_FAILURE_PREFIX = 'PUMAS_STARTUP_FAILURE=';
const MAX_STARTUP_LINE_LENGTH = 1024;
const SUPPORTED_FRAME_VERSION = 1;

export type BackendStartupFailureReason = 'migration-required' | 'backend-unavailable';

export class BackendStartupError extends Error {
  constructor(
    readonly reason: BackendStartupFailureReason,
    /** The backend's explanation, when it gave one. */
    readonly detail?: string
  ) {
    super(detail ? `Library backend could not start: ${detail}` : 'Library backend could not start');
    this.name = 'BackendStartupError';
  }
}

export class BackendStartupFailureReader {
  private line = '';
  private oversized = false;
  reason: BackendStartupFailureReason = 'backend-unavailable';
  detail: string | undefined;

  push(chunk: string): void {
    for (const character of chunk) {
      if (character === '\n') {
        if (!this.oversized) this.readLine(this.line);
        this.line = '';
        this.oversized = false;
      } else if (!this.oversized) {
        if (this.line.length >= MAX_STARTUP_LINE_LENGTH) {
          this.line = '';
          this.oversized = true;
        } else {
          this.line += character;
        }
      }
    }
  }

  private readLine(line: string): void {
    if (!line.startsWith(STARTUP_FAILURE_PREFIX)) return;
    let value: unknown;
    try {
      value = JSON.parse(line.slice(STARTUP_FAILURE_PREFIX.length));
    } catch {
      return; // A malformed line is a log line, not a diagnostic.
    }
    const decoded = decodeStartupFailureFrame(value);
    if (decoded.status !== 'valid' || decoded.value.version !== SUPPORTED_FRAME_VERSION) return;
    const frame = decoded.value;
    this.reason = frame.reason === 'migration-required' ? 'migration-required' : 'backend-unavailable';
    this.detail = typeof frame.detail === 'string' ? frame.detail : undefined;
  }
}
