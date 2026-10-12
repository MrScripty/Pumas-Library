/** Path-free startup diagnostics from the owned RPC process, before readiness. */
const STARTUP_FAILURE_PREFIX = 'PUMAS_STARTUP_FAILURE=';
const MAX_STARTUP_LINE_LENGTH = 1024;

export type BackendStartupFailureReason = 'migration-required' | 'backend-unavailable';

export class BackendStartupError extends Error {
  constructor(readonly reason: BackendStartupFailureReason) {
    super('Library backend could not start');
    this.name = 'BackendStartupError';
  }
}

export class BackendStartupFailureReader {
  private line = '';
  private oversized = false;
  reason: BackendStartupFailureReason = 'backend-unavailable';

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
    try {
      const value: unknown = JSON.parse(line.slice(STARTUP_FAILURE_PREFIX.length));
      if (value && typeof value === 'object' && !Array.isArray(value) &&
          Object.keys(value).length === 2 && 'version' in value && value.version === 1 &&
          'reason' in value && value.reason === 'migration-required') {
        this.reason = value.reason;
      }
    } catch {
      // Logs and unsupported/malformed frames never authorize a migration action.
    }
  }
}
