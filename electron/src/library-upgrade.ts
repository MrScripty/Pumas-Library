/** Explicit metadata-only upgrade of the main process's selected library. */
import { spawn, type ChildProcess } from 'child_process';
import type { LibraryUpgradeConfirmation, LibraryUpgradeResult, LibraryUpgradeReadyState } from './library-upgrade-contract';
export type { LibraryUpgradeConfirmation, LibraryUpgradeResult } from './library-upgrade-contract';
type ReadyState = LibraryUpgradeReadyState;
export interface LibraryUpgradeTarget {
  launcherRoot: string;
  rustBinaryPath: string;
}

export function validateLibraryUpgradeConfirmation(value: unknown): LibraryUpgradeConfirmation {
  if (!value || typeof value !== 'object' || Array.isArray(value) ||
      Object.keys(value).length !== 2 || !('oldWritersStopped' in value) ||
      value.oldWritersStopped !== true || !('noDowngradeAccepted' in value) ||
      value.noDowngradeAccepted !== true) {
    throw new Error('Library upgrade requires both confirmations.');
  }
  return { oldWritersStopped: true, noDowngradeAccepted: true };
}

export interface LibraryUpgradeAdapter {
  getTarget(): LibraryUpgradeTarget | null;
  upgrade(target: LibraryUpgradeTarget): Promise<void>;
  open(target: LibraryUpgradeTarget): Promise<ReadyState>;
  isClosing(): boolean;
  reportFailure(stage: 'upgrade' | 'open', error: unknown): void;
}

/** Own the entire mutation/open attempt, including application-close races. */
export class LibraryUpgradeCoordinator {
  private attempt: Promise<LibraryUpgradeResult> | null = null;
  private terminal: LibraryUpgradeResult | null = null;
  constructor(private readonly adapter: LibraryUpgradeAdapter) {}

  get active(): boolean { return this.attempt !== null; }

  run(confirmation: unknown): Promise<LibraryUpgradeResult> {
    validateLibraryUpgradeConfirmation(confirmation);
    if (this.adapter.isClosing()) return Promise.resolve({ status: 'unavailable' });
    if (this.attempt) return this.attempt;
    if (this.terminal) return Promise.resolve(this.terminal);
    const target = this.adapter.getTarget();
    if (!target) return Promise.resolve({ status: 'unavailable' });
    const owned = this.execute(target).then((result) => {
      // Never repeat a conversion after failed/uncertain publication or opening.
      this.terminal = result;
      return result;
    }).finally(() => {
      if (this.attempt === owned) this.attempt = null;
    });
    this.attempt = owned;
    return owned;
  }

  async settle(): Promise<void> { await this.attempt; }

  private reportFailure(stage: 'upgrade' | 'open', error: unknown): void {
    try { this.adapter.reportFailure(stage, error); }
    catch { /* Diagnostics must not turn uncertain publication into a retry. */ }
  }

  private async execute(target: LibraryUpgradeTarget): Promise<LibraryUpgradeResult> {
    try { await this.adapter.upgrade(target); }
    catch (error) {
      this.reportFailure('upgrade', error);
      return { status: 'failed', stage: 'upgrade' };
    }
    if (this.adapter.isClosing()) return { status: 'upgraded' };
    try { return { status: 'ready', state: await this.adapter.open(target) }; }
    catch (error) {
      this.reportFailure('open', error);
      return { status: 'failed', stage: 'open' };
    }
  }
}

const MAX_DIAGNOSTIC_CHARACTERS = 8192;
/** No shell, recursive backup, model enumeration, or server startup. */
export function runLibraryMetadataUpgrade(target: LibraryUpgradeTarget,
  spawnChild: typeof spawn = spawn): Promise<void> {
  return new Promise((resolve, reject) => {
    let child: ChildProcess;
    try {
      child = spawnChild(target.rustBinaryPath, [
        '--migrate-download-store-offline', '--launcher-root', target.launcherRoot,
        '--confirm-old-writers-stopped',
      ], { cwd: target.launcherRoot, stdio: ['ignore', 'pipe', 'pipe'] });
    } catch (error) {
      reject(error instanceof Error ? error : new Error('Metadata upgrade process could not start.', { cause: error }));
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
    // close drains diagnostics and acknowledges closure, including failed spawn.
    child.once('close', (code, signal) => {
      if (!failure && code === 0 && signal === null) resolve();
      else reject(new Error(`Metadata upgrade failed (code=${code}, signal=${signal}). ${diagnostic.trim()}`,
        { cause: failure }));
    });
  });
}
