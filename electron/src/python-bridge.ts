/**
 * Backend Sidecar Bridge
 *
 * Manages the Rust backend process (pumas-rpc) and provides RPC communication.
 * Handles process lifecycle, health checks, and automatic restarts.
 */

import { spawn, ChildProcess } from 'child_process';
import * as fs from 'fs';
import * as http from 'http';
import * as net from 'net';
import log from 'electron-log';

type BridgeTimer = ReturnType<typeof setTimeout>;

export interface PythonBridgeTimerController {
  setTimeout(callback: () => void, delayMs: number): BridgeTimer;
  clearTimeout(timer: BridgeTimer): void;
}

const NODE_TIMER_CONTROLLER: PythonBridgeTimerController = {
  setTimeout: (callback, delayMs) => setTimeout(callback, delayMs),
  clearTimeout: (timer) => clearTimeout(timer),
};

export interface PythonBridgeOptions {
  /** Port for the RPC server (0 = auto-assign) */
  port: number;
  /** Enable debug mode */
  debug: boolean;
  /** Restart on crash */
  autoRestart?: boolean;
  /** Maximum restart attempts */
  maxRestarts?: number;
  /** Path to pumas-rpc binary */
  rustBinaryPath: string;
  /** Launcher root directory */
  launcherRoot: string;
  /** Startup cancellation, shutdown RPC and natural-exit grace, including HTTP/core drain (default: 30 seconds). */
  shutdownGraceMs?: number;
  /** Time to observe exit after forced termination (default: 5 seconds). */
  shutdownForceWaitMs?: number;
  /** Timer controller for lifecycle testing */
  timerController?: PythonBridgeTimerController;
}

interface RPCError {
  code: number;
  message: string;
  data?: unknown;
}

interface RPCResponse {
  result?: unknown;
  error?: RPCError | string;
}

export function rpcRequestTimeoutMs(method: string): number {
  // Managed Python may need a uv bootstrap, interpreter install, index scan,
  // and several wheel previews before the RPC returns any bytes. Allow these
  // operations 15 minutes while keeping a finite HTTP socket deadline.
  if (method === 'get_torch_runtime_options'
    || method === 'get_torch_release_options'
    || method === 'preview_torch_runtime'
    || method === 'find_torch_alternatives') return 900_000;
  if (method === 'trial_torch_runtime') return 90_000;
  return 60_000;
}

export type ModelLibraryUpdateListener = (payload: unknown) => void;
export type ModelDownloadUpdateListener = (payload: unknown) => void;
export type RuntimeProfileUpdateListener = (payload: unknown) => void;
export type ServingStatusUpdateListener = (payload: unknown) => void;
export type StatusTelemetryUpdateListener = (payload: unknown) => void;
type StreamErrorListener = (message: string) => void;

export interface ParsedSseChunk {
  buffer: string;
  payloads: unknown[];
}

function parseNamedSseChunk(
  previousBuffer: string,
  chunk: string,
  expectedEventName: string,
  warningLabel: string
): ParsedSseChunk {
  const combined = previousBuffer + chunk;
  const blocks = combined.split(/\r?\n\r?\n/);
  const buffer = blocks.pop() ?? '';
  const payloads: unknown[] = [];

  for (const block of blocks) {
    let eventName = 'message';
    const dataLines: string[] = [];

    for (const line of block.split(/\r?\n/)) {
      if (line.startsWith('event:')) {
        eventName = line.slice('event:'.length).trim();
      } else if (line.startsWith('data:')) {
        const rawData = line.slice('data:'.length);
        dataLines.push(rawData.startsWith(' ') ? rawData.slice(1) : rawData);
      }
    }

    if (eventName !== expectedEventName || dataLines.length === 0) {
      continue;
    }

    try {
      payloads.push(JSON.parse(dataLines.join('\n')));
    } catch (error) {
      log.warn(`Ignoring invalid ${warningLabel} SSE payload`, error);
    }
  }

  return { buffer, payloads };
}

export function parseModelLibraryUpdateSseChunk(
  previousBuffer: string,
  chunk: string
): ParsedSseChunk {
  return parseNamedSseChunk(previousBuffer, chunk, 'model-library-update', 'model-library');
}

export function parseModelDownloadUpdateSseChunk(
  previousBuffer: string,
  chunk: string
): ParsedSseChunk {
  return parseNamedSseChunk(previousBuffer, chunk, 'model-download-update', 'model download');
}

export function parseRuntimeProfileUpdateSseChunk(
  previousBuffer: string,
  chunk: string
): ParsedSseChunk {
  return parseNamedSseChunk(previousBuffer, chunk, 'runtime-profile-update', 'runtime-profile');
}

export function parseServingStatusUpdateSseChunk(
  previousBuffer: string,
  chunk: string
): ParsedSseChunk {
  return parseNamedSseChunk(previousBuffer, chunk, 'serving-status-update', 'serving-status');
}

export function parseStatusTelemetryUpdateSseChunk(
  previousBuffer: string,
  chunk: string
): ParsedSseChunk {
  return parseNamedSseChunk(previousBuffer, chunk, 'status-telemetry-update', 'status telemetry');
}

interface NamedSseStreamSpec {
  label: string;
  path: string;
  expectedEventName: string;
  warningLabel: string;
  supportsCursor: boolean;
}

interface NamedSseStreamRuntime {
  getPort(): number;
  isRunning(): boolean;
  isShuttingDown(): boolean;
}

class NamedSseStreamOwner {
  request: http.ClientRequest | null = null;
  buffer = '';
  cursor: string | null = null;
  listener: ((payload: unknown) => void) | null = null;
  errorListener: StreamErrorListener | null = null;
  reconnectTimer: BridgeTimer | null = null;

  constructor(
    private readonly spec: NamedSseStreamSpec,
    private readonly timerController: PythonBridgeTimerController,
    private readonly runtime: NamedSseStreamRuntime
  ) {}

  start(listener: (payload: unknown) => void, errorListener?: StreamErrorListener): void {
    this.listener = listener;
    this.errorListener = errorListener ?? null;
    this.cursor = null;
    if (!this.runtime.isRunning()) {
      throw new Error('Backend bridge not running');
    }
    this.open();
  }

  stop(): void {
    this.listener = null;
    this.errorListener = null;
    this.cursor = null;
    this.close();
    this.clearReconnectTimer();
  }

  resumeIfListening(): void {
    if (this.listener) {
      this.open();
    }
  }

  close(): void {
    if (this.request) {
      this.request.destroy();
      this.request = null;
    }
    this.buffer = '';
  }

  clearReconnectTimer(): void {
    if (this.reconnectTimer) {
      this.timerController.clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }

  open(): void {
    if (!this.runtime.isRunning() || !this.listener) {
      return;
    }

    this.close();
    this.clearReconnectTimer();
    this.buffer = '';

    const cursorQuery = this.spec.supportsCursor && this.cursor
      ? `?cursor=${encodeURIComponent(this.cursor)}`
      : '';

    const req = http.get({
      hostname: '127.0.0.1',
      port: this.runtime.getPort(),
      path: `${this.spec.path}${cursorQuery}`,
      method: 'GET',
      headers: {
        Accept: 'text/event-stream',
      },
    }, (res) => {
      res.setEncoding('utf8');
      res.on('data', (chunk: string) => {
        const parsed = parseNamedSseChunk(
          this.buffer,
          chunk,
          this.spec.expectedEventName,
          this.spec.warningLabel
        );
        this.buffer = parsed.buffer;
        for (const payload of parsed.payloads) {
          if (
            this.spec.supportsCursor &&
            payload &&
            typeof payload === 'object' &&
            typeof (payload as { cursor?: unknown }).cursor === 'string'
          ) {
            this.cursor = (payload as { cursor: string }).cursor;
          }
          this.listener?.(payload);
        }
      });
      res.on('end', () => {
        this.request = null;
        this.buffer = '';
        if (!this.runtime.isShuttingDown()) {
          const message = `${this.spec.label} stream ended`;
          log.warn(message);
          this.errorListener?.(message);
          this.scheduleReconnect();
        }
      });
    });

    req.on('error', (error) => {
      this.request = null;
      this.buffer = '';
      if (!this.runtime.isShuttingDown()) {
        const message = `${this.spec.label} stream failed: ${error.message}`;
        log.warn(`${this.spec.label} stream failed:`, error);
        this.errorListener?.(message);
        this.scheduleReconnect();
      }
    });

    this.request = req;
  }

  scheduleReconnect(): void {
    if (
      this.runtime.isShuttingDown() ||
      !this.runtime.isRunning() ||
      !this.listener ||
      this.reconnectTimer
    ) {
      return;
    }

    this.reconnectTimer = this.timerController.setTimeout(() => {
      this.reconnectTimer = null;
      this.open();
    }, 1000);
  }
}

interface BackendExit {
  code: number | null;
  signal: NodeJS.Signals | null;
  startupError?: Error;
}

export class PythonBridge {
  private options: Required<Omit<PythonBridgeOptions, 'timerController'>>;
  private timerController: PythonBridgeTimerController;
  private process: ChildProcess | null = null;
  // Releasing the live slot does not discard the previous owner's cleanup receipt.
  private terminalExit: { child: ChildProcess; outcome: BackendExit } | null = null;
  private startPromise: Promise<void> | null = null;
  private cancelPortAllocation: (() => void) | null = null;
  private failedStarts = new WeakMap<ChildProcess, Error>();
  private stopOperation: {
    child: ChildProcess | null;
    startup: Promise<void> | null;
    allocation: (() => void) | null;
    promise: Promise<void>;
    settled: boolean;
  } | null = null;
  private serverReady = false;
  private port = 0;
  private restartCount = 0;
  private isShuttingDown = false;
  private healthCheckTimer: BridgeTimer | null = null;
  private restartTimer: BridgeTimer | null = null;
  private pendingRpcCalls = new Set<() => void>();
  private modelLibraryUpdateStream: NamedSseStreamOwner;
  private modelDownloadUpdateStream: NamedSseStreamOwner;
  private runtimeProfileUpdateStream: NamedSseStreamOwner;
  private servingStatusUpdateStream: NamedSseStreamOwner;
  private statusTelemetryUpdateStream: NamedSseStreamOwner;

  constructor(options: PythonBridgeOptions) {
    const { timerController, ...runtimeOptions } = options;
    this.options = {
      autoRestart: true,
      maxRestarts: 3,
      shutdownGraceMs: 30_000,
      shutdownForceWaitMs: 5_000,
      ...runtimeOptions,
    };
    for (const duration of [this.options.shutdownGraceMs, this.options.shutdownForceWaitMs]) {
      if (!Number.isSafeInteger(duration) || duration <= 0 || duration > 2_147_483_647) {
        throw new Error('Shutdown deadlines must be positive integer milliseconds within the timer range');
      }
    }
    this.timerController = timerController ?? NODE_TIMER_CONTROLLER;

    const streamRuntime: NamedSseStreamRuntime = {
      getPort: () => this.port,
      isRunning: () => this.process !== null,
      isShuttingDown: () => this.isShuttingDown,
    };
    this.modelLibraryUpdateStream = new NamedSseStreamOwner({
      label: 'Model-library update',
      path: '/events/model-library-updates',
      expectedEventName: 'model-library-update',
      warningLabel: 'model-library',
      supportsCursor: true,
    }, this.timerController, streamRuntime);
    this.modelDownloadUpdateStream = new NamedSseStreamOwner({
      label: 'Model download update',
      path: '/events/model-download-updates',
      expectedEventName: 'model-download-update',
      warningLabel: 'model download',
      supportsCursor: true,
    }, this.timerController, streamRuntime);
    this.runtimeProfileUpdateStream = new NamedSseStreamOwner({
      label: 'Runtime-profile update',
      path: '/events/runtime-profile-updates',
      expectedEventName: 'runtime-profile-update',
      warningLabel: 'runtime-profile',
      supportsCursor: false,
    }, this.timerController, streamRuntime);
    this.servingStatusUpdateStream = new NamedSseStreamOwner({
      label: 'Serving-status update',
      path: '/events/serving-status-updates',
      expectedEventName: 'serving-status-update',
      warningLabel: 'serving-status',
      supportsCursor: false,
    }, this.timerController, streamRuntime);
    this.statusTelemetryUpdateStream = new NamedSseStreamOwner({
      label: 'Status telemetry update',
      path: '/events/status-telemetry-updates',
      expectedEventName: 'status-telemetry-update',
      warningLabel: 'status telemetry',
      supportsCursor: true,
    }, this.timerController, streamRuntime);

    log.info(`Backend bridge initialized: ${this.options.rustBinaryPath}`);
  }

  /**
   * Find an available port
   */
  private async findAvailablePort(): Promise<number> {
    return new Promise((resolve, reject) => {
      const server = net.createServer();
      let closing = false;
      let settled = false;
      let cancelled = false;
      let port: number | null = null;
      let failure: Error | null = null;
      const finish = (error?: Error): void => {
        if (settled) return;
        settled = true;
        if (error && (!('code' in error) || error.code !== 'ERR_SERVER_NOT_RUNNING')) {
          // Retain the cancellation owner when listener closure is unconfirmed.
          reject(error);
          return;
        }
        server.removeListener('error', onError);
        if (this.cancelPortAllocation === cancel) this.cancelPortAllocation = null;
        if (cancelled) reject(new Error('Backend bridge stopped during startup'));
        else if (failure) reject(failure);
        else if (port === null) reject(new Error('Failed to get server address'));
        else resolve(port);
      };
      const close = (): void => {
        if (closing) return;
        closing = true;
        server.close(finish);
      };
      const onError = (error: Error): void => {
        failure = error;
        close();
      };
      const cancel = (): void => {
        cancelled = true;
        close();
      };
      this.cancelPortAllocation = cancel;
      server.on('error', onError);
      try {
        server.listen(0, '127.0.0.1', () => {
          if (settled || cancelled) return;
          const address = server.address();
          if (address && typeof address === 'object') port = address.port;
          close();
        });
      } catch (error) {
        onError(error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  /**
   * Get the command and arguments for the backend
   */
  private getBackendCommand(): { cmd: string; args: string[]; cwd: string; env: NodeJS.ProcessEnv } {
    return {
      cmd: this.options.rustBinaryPath,
      args: [
        '--port', String(this.port),
        '--launcher-root', this.options.launcherRoot,
        ...(this.options.debug ? ['--debug'] : []),
      ],
      cwd: this.options.launcherRoot,
      env: {
        ...process.env,
        RUST_LOG: this.options.debug ? 'debug' : 'info',
      },
    };
  }

  /**
   * Start the backend sidecar process
   */
  start(): Promise<void> {
    if ((this.stopOperation && !this.stopOperation.settled)
      || (this.isShuttingDown && (this.process || this.startPromise))) {
      return Promise.reject(new Error('Backend bridge stopping; previous cleanup may be unconfirmed'));
    }
    if (this.startPromise) return this.startPromise;
    if (this.cancelPortAllocation) {
      return Promise.reject(new Error('Backend startup allocation closure is unconfirmed'));
    }
    if (this.process) {
      log.warn('Backend process already running');
      return Promise.resolve();
    }
    const starting = this.startProcess().finally(() => {
      if (this.startPromise === starting) this.startPromise = null;
    });
    this.startPromise = starting;
    return starting;
  }

  private async startProcess(): Promise<void> {
    this.isShuttingDown = false;
    this.serverReady = false;
    this.clearRestartTimer();
    this.clearHealthCheckTimer();

    // Find available port
    this.port = this.options.port || await this.findAvailablePort();
    if (this.isShuttingDown) {
      throw new Error('Backend bridge stopped during startup');
    }
    log.info(`Starting backend bridge on port ${this.port}`);

    // Get command configuration
    const { cmd, args, cwd, env } = this.getBackendCommand();

    // Verify binary exists
    if (!fs.existsSync(cmd)) {
      throw new Error(`Backend binary not found at ${cmd}. Run 'cargo build --release' in the rust/ directory.`);
    }

    // Ensure working directory exists
    if (!fs.existsSync(cwd)) {
      fs.mkdirSync(cwd, { recursive: true });
    }

    // Spawn process
    const child = spawn(cmd, args, {
      cwd,
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    this.process = child;
    this.stopOperation = null;

    const backendLabel = 'Rust';

    // Handle stdout
    child.stdout?.on('data', (data: Buffer) => {
      const output = data.toString().trim();
      if (output) {
        log.info(`[${backendLabel}] ${output}`);
      }
    });

    // Handle stderr (Rust uses stderr for tracing logs, which is normal)
    child.stderr?.on('data', (data: Buffer) => {
      const output = data.toString().trim();
      if (output) {
        log.info(`[${backendLabel}] ${output}`);
      }
    });

    // Exit and failed-spawn close share one terminal receipt owner.
    const completeExit = (code: number | null, signal: NodeJS.Signals | null, startupError?: Error): void => {
      if (this.process !== child) return;
      log.info(`${backendLabel} process exited: code=${code}, signal=${signal}`);
      this.serverReady = false;
      this.terminalExit = { child, outcome: { code, signal, startupError } };
      this.process = null;
      this.clearHealthCheckTimer();
      this.closeAllUpdateStreams();
      this.clearAllUpdateStreamReconnectTimers();

      // Auto-restart if enabled and not shutting down
      if (
        !this.isShuttingDown &&
        this.options.autoRestart &&
        this.restartCount < this.options.maxRestarts
      ) {
        this.scheduleRestart(backendLabel);
      }
    };
    child.on('exit', (code, signal) => completeExit(code, signal));
    child.on('close', (code, signal) => {
      const failure = this.failedStarts.get(child);
      if (failure) completeExit(code, signal, failure);
    });

    // Handle process error
    child.on('error', (error) => {
      if (this.process !== child) return;
      if (child.pid === undefined) this.failedStarts.set(child, error);
      this.serverReady = false;
      log.error(`${backendLabel} process error:`, error);
    });

    // Wait for the server to be ready
    await this.waitForReady();
    if (this.isShuttingDown || this.process !== child) {
      throw new Error('Backend bridge stopped during startup');
    }

    // Start health check interval
    this.startHealthCheck();

    // Reset restart counter on successful start
    this.restartCount = 0;
    this.serverReady = true;
    this.terminalExit = null;

    this.resumeListeningUpdateStreams();

    log.info(`${backendLabel} backend bridge started successfully`);
  }

  /**
   * Wait for the RPC server to be ready
   */
  private async waitForReady(timeout: number = 30000): Promise<void> {
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      if (this.isShuttingDown || !this.process) {
        throw new Error('Backend bridge stopped during startup');
      }
      try {
        const healthy = await this.healthCheck();
        if (healthy) {
          return;
        }
      } catch {
        await this.delay(100);
      }
    }

    throw new Error('RPC server failed to start within timeout');
  }

  /**
   * Perform a health check
   */
  private async healthCheck(): Promise<boolean> {
    try {
      const response = await this.call('health_check', {}) as { status?: string } | null;
      return response !== null && response?.status === 'ok';
    } catch {
      return false;
    }
  }

  /**
   * Start periodic health checks
   */
  private startHealthCheck(): void {
    const backendLabel = 'Rust';
    this.clearHealthCheckTimer();
    this.healthCheckTimer = this.timerController.setTimeout(async () => {
      this.healthCheckTimer = null;
      if (this.isShuttingDown) {
        return;
      }

      const healthy = await this.healthCheck();
      if (!healthy && !this.isShuttingDown) {
        log.warn(`${backendLabel} health check failed`);
      }

      if (!this.isShuttingDown && this.process) {
        this.startHealthCheck();
      }
    }, 30000);
  }

  /**
   * Stop one captured child. The shutdown RPC only acknowledges admission;
   * natural exit code 0 is the backend's receipt for completed owned cleanup.
   * Replay the same outcome for captured child, startup and allocation owners, including failure.
   */
  stop(): Promise<void> {
    const child = this.process ?? this.terminalExit?.child ?? null;
    const startup = this.startPromise;
    const allocation = this.cancelPortAllocation;
    if (this.stopOperation && (!this.stopOperation.settled
      || ((this.stopOperation.child === child || !child)
        && (!startup || this.stopOperation.startup === startup)
        && (!allocation || this.stopOperation.allocation === allocation)))) {
      return this.stopOperation.promise;
    }
    let resolve!: () => void;
    let reject!: (error: unknown) => void;
    const promise = new Promise<void>((res, rej) => { resolve = res; reject = rej; });
    const operation = { child, startup, allocation, promise, settled: false };
    this.stopOperation = operation;
    this.isShuttingDown = true;
    this.serverReady = false;
    void this.stopChild(operation.child, operation.startup).then(() => {
      operation.settled = true;
      resolve();
    }, (error: unknown) => {
      operation.settled = true;
      reject(error);
    });
    return promise;
  }

  private async stopChild(child: ChildProcess | null, starting: Promise<void> | null): Promise<void> {
    let exit: BackendExit | null = null;
    let recordExit!: (code: number | null, signal: NodeJS.Signals | null, startupError?: Error) => void;
    const exited = new Promise<'exited'>((resolve) => {
      recordExit = (code, signal, startupError) => {
        exit = { code, signal, startupError };
        resolve('exited');
      };
    });
    // Observe before cancelling requests or sending RPC/signals: each can race exit.
    const recordFailedStart = (code: number | null, signal: NodeJS.Signals | null): void => {
      const failure = child ? this.failedStarts.get(child) : undefined;
      if (failure) recordExit(code, signal, failure);
    };
    child?.once('exit', recordExit);
    child?.once('close', recordFailedStart);
    const terminal = this.terminalExit;
    if (child && terminal?.child === child) {
      recordExit(terminal.outcome.code, terminal.outcome.signal, terminal.outcome.startupError);
    } else if (child && !this.failedStarts.has(child)
      && (child.exitCode !== null || child.signalCode !== null)) {
      recordExit(child.exitCode, child.signalCode);
    }
    let graceTimer: BridgeTimer | null = null;
    let forceTimer: BridgeTimer | null = null;
    let forced = false;
    let signalFailure: unknown;
    try {
      this.clearHealthCheckTimer();
      this.clearRestartTimer();
      this.stopModelLibraryUpdateStream();
      this.stopModelDownloadUpdateStream();
      this.stopRuntimeProfileUpdateStream();
      this.stopServingStatusUpdateStream();
      this.stopStatusTelemetryUpdateStream();
      for (const cancel of [...this.pendingRpcCalls]) cancel();
      this.cancelPortAllocation?.();
      if (!child && !starting && !this.cancelPortAllocation) return;

      log.info('Stopping Rust backend bridge...');
      const graceExpired = new Promise<'expired'>((resolve) => {
        graceTimer = this.timerController.setTimeout(() => resolve('expired'), this.options.shutdownGraceMs);
      });
      const settleStartup = async (): Promise<void> => {
        if (starting) {
          const settled = await Promise.race([
            starting.then(() => true, () => true),
            graceExpired.then(() => false),
          ]);
          if (!settled) throw new Error('Backend startup settlement unconfirmed; cleanup incomplete');
        }
        if (this.cancelPortAllocation) {
          throw new Error('Backend startup allocation closure unconfirmed; cleanup incomplete');
        }
      };
      if (!child) {
        await settleStartup();
        return;
      }
      if (!exit) {
        const acknowledgement = this.call('shutdown', {}).then((result) => {
          // Coordinated internal RPC contract: no managed-process completion counts.
          if (!result || typeof result !== 'object' || Array.isArray(result)
            || !('status' in result) || result.status !== 'shutting_down'
            || Object.keys(result).length !== 1) {
            throw new Error('Invalid backend shutdown acknowledgement');
          }
          return 'acknowledged' as const;
        }).catch((error: unknown) => {
          log.warn('Backend shutdown RPC unavailable:', error);
          return 'rpc-failed' as const;
        });
        const first = await Promise.race([exited, graceExpired, acknowledgement]);
        // Unix SIGTERM is a second graceful request. On Windows kill() terminates
        // the process, so leave the remaining grace for an already accepted RPC.
        if (first === 'rpc-failed' && !exit && !this.failedStarts.has(child)
          && process.platform !== 'win32') {
          try {
            if (!child.kill('SIGTERM')) signalFailure = new Error('SIGTERM was not delivered');
          } catch (error) {
            signalFailure = error;
          }
        }
        if (!exit && first !== 'expired') await Promise.race([exited, graceExpired]);
      }
      // Cancel a hanging shutdown transport even when exit wins the RPC race.
      for (const cancel of [...this.pendingRpcCalls]) cancel();
      if (!exit) {
        if (this.failedStarts.has(child)) {
          throw new Error('Backend failed-start closure unconfirmed; cleanup incomplete', { cause: this.failedStarts.get(child) });
        }
        forced = true;
        const forceExpired = new Promise<'expired'>((resolve) => {
          forceTimer = this.timerController.setTimeout(() => resolve('expired'), this.options.shutdownForceWaitMs);
        });
        try {
          if (!child.kill('SIGKILL')) signalFailure = new Error('SIGKILL was not delivered');
        } catch (error) {
          signalFailure = error;
        }
        await Promise.race([exited, forceExpired]);
      }
      await settleStartup();
      // The callback owns mutation; read its observed terminal receipt here.
      const observed = exit as BackendExit | null;
      if (!observed) {
        throw new Error('Backend exit unconfirmed after forced termination; cleanup incomplete', { cause: signalFailure });
      }
      if (this.process === child) {
        this.terminalExit = { child, outcome: observed };
        this.process = null;
      }
      this.restartCount = 0;
      if (observed.startupError) {
        throw new Error('Backend failed to start; cleanup observed after process closure', { cause: observed.startupError });
      }
      if (forced) {
        throw new Error(`Backend exit observed after forced termination attempt (code=${observed.code}, signal=${observed.signal}); cleanup unconfirmed`, { cause: signalFailure });
      }
      if (observed.code !== 0 || observed.signal !== null) {
        throw new Error(`Backend exited with code=${observed.code}, signal=${observed.signal}; cleanup failed`);
      }
      log.info('Rust backend bridge stopped');
    } finally {
      child?.removeListener('exit', recordExit);
      child?.removeListener('close', recordFailedStart);
      if (graceTimer !== null) this.timerController.clearTimeout(graceTimer);
      if (forceTimer !== null) this.timerController.clearTimeout(forceTimer);
      for (const cancel of [...this.pendingRpcCalls]) cancel();
    }
  }

  startModelLibraryUpdateStream(listener: ModelLibraryUpdateListener): void {
    this.modelLibraryUpdateStream.start(listener);
  }

  stopModelLibraryUpdateStream(): void {
    this.modelLibraryUpdateStream.stop();
  }

  startModelDownloadUpdateStream(listener: ModelDownloadUpdateListener): void {
    this.modelDownloadUpdateStream.start(listener);
  }

  stopModelDownloadUpdateStream(): void {
    this.modelDownloadUpdateStream.stop();
  }

  startRuntimeProfileUpdateStream(listener: RuntimeProfileUpdateListener): void {
    this.runtimeProfileUpdateStream.start(listener);
  }

  stopRuntimeProfileUpdateStream(): void {
    this.runtimeProfileUpdateStream.stop();
  }

  startServingStatusUpdateStream(
    listener: ServingStatusUpdateListener,
    errorListener?: StreamErrorListener
  ): void {
    this.servingStatusUpdateStream.start(listener, errorListener);
  }

  stopServingStatusUpdateStream(): void {
    this.servingStatusUpdateStream.stop();
  }

  startStatusTelemetryUpdateStream(listener: StatusTelemetryUpdateListener): void {
    this.statusTelemetryUpdateStream.start(listener);
  }

  stopStatusTelemetryUpdateStream(): void {
    this.statusTelemetryUpdateStream.stop();
  }

  private resumeListeningUpdateStreams(): void {
    this.modelLibraryUpdateStream.resumeIfListening();
    this.modelDownloadUpdateStream.resumeIfListening();
    this.runtimeProfileUpdateStream.resumeIfListening();
    this.servingStatusUpdateStream.resumeIfListening();
    this.statusTelemetryUpdateStream.resumeIfListening();
  }

  private closeAllUpdateStreams(): void {
    this.modelLibraryUpdateStream.close();
    this.modelDownloadUpdateStream.close();
    this.runtimeProfileUpdateStream.close();
    this.servingStatusUpdateStream.close();
    this.statusTelemetryUpdateStream.close();
  }

  private clearAllUpdateStreamReconnectTimers(): void {
    this.modelLibraryUpdateStream.clearReconnectTimer();
    this.modelDownloadUpdateStream.clearReconnectTimer();
    this.runtimeProfileUpdateStream.clearReconnectTimer();
    this.servingStatusUpdateStream.clearReconnectTimer();
    this.statusTelemetryUpdateStream.clearReconnectTimer();
  }

  private clearHealthCheckTimer(): void {
    if (this.healthCheckTimer) {
      this.timerController.clearTimeout(this.healthCheckTimer);
      this.healthCheckTimer = null;
    }
  }

  private clearRestartTimer(): void {
    if (this.restartTimer) {
      this.timerController.clearTimeout(this.restartTimer);
      this.restartTimer = null;
    }
  }

  private scheduleRestart(backendLabel: string): void {
    this.clearRestartTimer();
    this.restartCount++;
    log.info(`Restarting ${backendLabel} process (attempt ${this.restartCount}/${this.options.maxRestarts})`);
    this.restartTimer = this.timerController.setTimeout(() => {
      this.restartTimer = null;
      if (this.isShuttingDown) return;
      const restart = (): void => {
        if (this.isShuttingDown || this.process) return;
        void this.start().catch((error: unknown) => {
          log.error(`Failed to restart ${backendLabel} process:`, error);
        });
      };
      if (this.startPromise) void this.startPromise.then(restart, restart);
      else restart();
    }, 1000 * this.restartCount);
  }

  private async delay(delayMs: number): Promise<void> {
    await new Promise<void>((resolve) => {
      this.timerController.setTimeout(resolve, delayMs);
    });
  }

  /**
   * Make an RPC call to the backend
   */
  async call(method: string, params: Record<string, unknown>): Promise<unknown> {
    if (this.isShuttingDown && method !== 'shutdown') {
      throw new Error('Backend bridge stopping');
    }
    if (!this.process) {
      throw new Error('Backend bridge not running');
    }

    return new Promise((resolve, reject) => {
      const requestBody = JSON.stringify({
        jsonrpc: '2.0',
        method,
        params,
        id: Date.now(),
      });
      const requestTimeoutMs = rpcRequestTimeoutMs(method);
      let deadline: BridgeTimer | null = null;
      let settled = false;
      let cancelPending: (() => void) | null = null;
      const finish = (error: Error | null, result?: unknown): void => {
        if (settled) return;
        settled = true;
        if (cancelPending) this.pendingRpcCalls.delete(cancelPending);
        if (deadline !== null) {
          this.timerController.clearTimeout(deadline);
          deadline = null;
        }
        if (error) reject(error);
        else resolve(result);
      };

      const options: http.RequestOptions = {
        hostname: '127.0.0.1',
        port: this.port,
        path: '/rpc',
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Content-Length': Buffer.byteLength(requestBody),
        },
        timeout: requestTimeoutMs,
      };

      const req = http.request(options, (res) => {
        let data = '';

        res.on('data', (chunk) => {
          data += chunk;
        });

        res.on('end', () => {
          try {
            const response: RPCResponse = JSON.parse(data);
            if (response.error) {
              // Handle both string errors and JSON-RPC error objects
              const errorMessage = typeof response.error === 'string'
                ? response.error
                : response.error.message || JSON.stringify(response.error);
              finish(new Error(errorMessage));
            } else {
              finish(null, response.result);
            }
          } catch {
            finish(new Error(`Invalid JSON response: ${data}`));
          }
        });
        res.on('error', (error) => {
          finish(new Error(`RPC response failed: ${error.message}`));
        });
        res.on('aborted', () => {
          finish(new Error('RPC response aborted'));
        });
      });

      req.on('error', (error) => {
        finish(new Error(`RPC request failed: ${error.message}`));
      });

      req.on('timeout', () => {
        if (settled) return;
        finish(new Error('RPC request timeout'));
        req.destroy();
      });

      cancelPending = () => {
        if (settled) return;
        finish(new Error('Backend bridge stopped'));
        req.destroy();
      };
      this.pendingRpcCalls.add(cancelPending);
      deadline = this.timerController.setTimeout(() => {
        if (settled) return;
        finish(new Error('RPC request timeout'));
        req.destroy();
      }, requestTimeoutMs);

      try {
        req.write(requestBody);
        req.end();
      } catch (error) {
        finish(error instanceof Error ? error : new Error(String(error)));
        req.destroy();
      }
    });
  }

  /**
   * Check if the bridge is running
   */
  isRunning(): boolean {
    return this.process !== null && this.serverReady;
  }

  /**
   * Get the RPC server port
   */
  getPort(): number {
    return this.port;
  }
}
