/**
 * Operator-confirmed release of the selected library's stale instance entry.
 *
 * A Pumas backend that was killed or crashed leaves an entry that refuses the
 * next start. Pumas never replaces it on its own (a free lock after process
 * loss does not prove no other writer survived). The backend reports it as a
 * `stale-instance` startup failure; the user confirms that exact entry; this
 * module runs the backend once with `--release-stale-instance`, which checks
 * everything again and removes only that entry, then the library opens.
 */
import { spawn, type ChildProcess } from 'child_process';
import {
  decodeStaleInstanceReleaseConfirmation,
  type LauncherRootReadyState,
  type LauncherRootStaleInstance,
  type StaleInstanceReleaseResult,
} from './desktop-contract';

export type { StaleInstanceReleaseConfirmation, StaleInstanceReleaseResult } from './desktop-contract';

export interface StaleInstanceReleaseTarget {
  launcherRoot: string;
  rustBinaryPath: string;
  /** The entry the backend reported, which the confirmation must name. */
  staleInstance: LauncherRootStaleInstance;
}

export interface StaleInstanceReleaseAdapter {
  getTarget(): StaleInstanceReleaseTarget | null;
  release(target: StaleInstanceReleaseTarget): Promise<void>;
  open(target: StaleInstanceReleaseTarget): Promise<LauncherRootReadyState>;
  isClosing(): boolean;
  reportFailure(stage: 'release' | 'open', error: unknown): void;
}

const MAX_MESSAGE_CHARACTERS = 1024;

function failureMessage(error: unknown): string | undefined {
  const text = error instanceof Error ? error.message.trim() : '';
  return text ? text.slice(0, MAX_MESSAGE_CHARACTERS) : undefined;
}

/** Own one release-then-open attempt at a time. */
export class StaleInstanceReleaseCoordinator {
  private attempt: Promise<StaleInstanceReleaseResult> | null = null;
  constructor(private readonly adapter: StaleInstanceReleaseAdapter) {}

  get active(): boolean { return this.attempt !== null; }

  run(confirmation: unknown): Promise<StaleInstanceReleaseResult> {
    const decoded = decodeStaleInstanceReleaseConfirmation(confirmation);
    if (decoded.status !== 'valid') return Promise.resolve({ status: 'unavailable' });
    if (this.adapter.isClosing()) return Promise.resolve({ status: 'unavailable' });
    if (this.attempt) return this.attempt;
    const target = this.adapter.getTarget();
    // The user confirmed one exact entry; anything else is not this release.
    if (!target || target.staleInstance.pid !== decoded.value.pid ||
        target.staleInstance.startedAt !== decoded.value.startedAt) {
      return Promise.resolve({ status: 'unavailable' });
    }
    const owned = this.execute(target).finally(() => {
      if (this.attempt === owned) this.attempt = null;
    });
    this.attempt = owned;
    return owned;
  }

  async settle(): Promise<void> { await this.attempt; }

  private reportFailure(stage: 'release' | 'open', error: unknown): void {
    try { this.adapter.reportFailure(stage, error); }
    catch { /* Diagnostics must not change the outcome. */ }
  }

  private async execute(target: StaleInstanceReleaseTarget): Promise<StaleInstanceReleaseResult> {
    try { await this.adapter.release(target); }
    catch (error) {
      this.reportFailure('release', error);
      const message = failureMessage(error);
      return { status: 'failed', stage: 'release', ...(message ? { message } : {}) };
    }
    if (this.adapter.isClosing()) return { status: 'unavailable' };
    try { return { status: 'ready', state: await this.adapter.open(target) }; }
    catch (error) {
      this.reportFailure('open', error);
      const message = failureMessage(error);
      return { status: 'failed', stage: 'open', ...(message ? { message } : {}) };
    }
  }
}

const MAX_DIAGNOSTIC_CHARACTERS = 8192;

/** The backend's own refusal line (`Error: …`), when it printed one. */
function refusalLine(diagnostic: string): string | undefined {
  const lines = diagnostic.split('\n').map((line) => line.trim()).filter(Boolean);
  const error = [...lines].reverse().find((line) => line.startsWith('Error: '));
  return error ? error.slice('Error: '.length) : undefined;
}

/** Run the backend once to release exactly the confirmed entry. No shell. */
export function runStaleInstanceRelease(target: StaleInstanceReleaseTarget,
  spawnChild: typeof spawn = spawn): Promise<void> {
  return new Promise((resolve, reject) => {
    let child: ChildProcess;
    try {
      child = spawnChild(target.rustBinaryPath, [
        '--release-stale-instance', '--launcher-root', target.launcherRoot,
        '--expected-pid', String(target.staleInstance.pid),
        '--expected-started-at', target.staleInstance.startedAt,
      ], { cwd: target.launcherRoot, stdio: ['ignore', 'pipe', 'pipe'] });
    } catch (error) {
      reject(error instanceof Error ? error : new Error('The release process could not start.', { cause: error }));
      return;
    }
    let failure: Error | undefined;
    let diagnostic = '';
    const capture = (chunk: Buffer): void => {
      diagnostic = (diagnostic + chunk.toString()).slice(-MAX_DIAGNOSTIC_CHARACTERS);
    };
    child.stdout?.on('data', capture);
    child.stderr?.on('data', capture);
    child.once('error', (error) => { failure = error; });
    child.once('close', (code, signal) => {
      if (!failure && code === 0 && signal === null) resolve();
      else reject(new Error(refusalLine(diagnostic)
        ?? `The release did not complete (code=${code}, signal=${signal}).`, { cause: failure }));
    });
  });
}
