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

export type ModelLibraryUpdateListener = (payload: unknown) => void;
export type ModelDownloadUpdateListener = (payload: unknown) => void;
export type RuntimeProfileUpdateListener = (payload: unknown) => void;
export type ServingStatusUpdateListener = (payload: unknown) => void;
export type StatusTelemetryUpdateListener = (payload: unknown) => void;

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
  reconnectTimer: BridgeTimer | null = null;

  constructor(
    private readonly spec: NamedSseStreamSpec,
    private readonly timerController: PythonBridgeTimerController,
    private readonly runtime: NamedSseStreamRuntime
  ) {}

  start(listener: (payload: unknown) => void): void {
    this.listener = listener;
    this.cursor = null;
    if (!this.runtime.isRunning()) {
      throw new Error('Backend bridge not running');
    }
    this.open();
  }

  stop(): void {
    this.listener = null;
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
          log.warn(`${this.spec.label} stream ended`);
          this.scheduleReconnect();
        }
      });
    });

    req.on('error', (error) => {
      this.request = null;
      this.buffer = '';
      if (!this.runtime.isShuttingDown()) {
        log.warn(`${this.spec.label} stream failed:`, error);
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

export class PythonBridge {
  private options: Required<Omit<PythonBridgeOptions, 'timerController'>>;
  private timerController: PythonBridgeTimerController;
  private process: ChildProcess | null = null;
  private port = 0;
  private restartCount = 0;
  private isShuttingDown = false;
  private healthCheckTimer: BridgeTimer | null = null;
  private restartTimer: BridgeTimer | null = null;
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
      ...runtimeOptions,
    };
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
      server.listen(0, '127.0.0.1', () => {
        const address = server.address();
        if (address && typeof address === 'object') {
          const port = address.port;
          server.close(() => resolve(port));
        } else {
          reject(new Error('Failed to get server address'));
        }
      });
      server.on('error', reject);
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
  async start(): Promise<void> {
    if (this.process) {
      log.warn('Backend process already running');
      return;
    }

    this.isShuttingDown = false;
    this.clearRestartTimer();
    this.clearHealthCheckTimer();

    // Find available port
    this.port = this.options.port || await this.findAvailablePort();
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
    this.process = spawn(cmd, args, {
      cwd,
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    const backendLabel = 'Rust';

    // Handle stdout
    this.process.stdout?.on('data', (data: Buffer) => {
      const output = data.toString().trim();
      if (output) {
        log.info(`[${backendLabel}] ${output}`);
      }
    });

    // Handle stderr (Rust uses stderr for tracing logs, which is normal)
    this.process.stderr?.on('data', (data: Buffer) => {
      const output = data.toString().trim();
      if (output) {
        log.info(`[${backendLabel}] ${output}`);
      }
    });

    // Handle process exit
    this.process.on('exit', (code, signal) => {
      log.info(`${backendLabel} process exited: code=${code}, signal=${signal}`);
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
    });

    // Handle process error
    this.process.on('error', (error) => {
      log.error(`${backendLabel} process error:`, error);
    });

    // Wait for the server to be ready
    await this.waitForReady();

    // Start health check interval
    this.startHealthCheck();

    // Reset restart counter on successful start
    this.restartCount = 0;

    this.resumeListeningUpdateStreams();

    log.info(`${backendLabel} backend bridge started successfully`);
  }

  /**
   * Wait for the RPC server to be ready
   */
  private async waitForReady(timeout: number = 30000): Promise<void> {
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
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
   * Stop the backend sidecar process
   */
  async stop(): Promise<void> {
    this.isShuttingDown = true;

    // Stop health check interval
    this.clearHealthCheckTimer();
    this.clearRestartTimer();
    this.stopModelLibraryUpdateStream();
    this.stopModelDownloadUpdateStream();
    this.stopRuntimeProfileUpdateStream();
    this.stopServingStatusUpdateStream();
    this.stopStatusTelemetryUpdateStream();

    if (!this.process) {
      return;
    }

    const backendLabel = 'Rust';
    log.info(`Stopping ${backendLabel} backend bridge...`);

    // Try graceful shutdown first
    try {
      await this.call('shutdown', {});
      await this.delay(1000);
    } catch {
      // Ignore errors during shutdown
    }

    // Force kill if still running
    if (this.process) {
      this.process.kill('SIGTERM');

      // Wait for process to exit
      await new Promise<void>((resolve) => {
        const timeout = this.timerController.setTimeout(() => {
          if (this.process) {
            log.warn(`Force killing ${backendLabel} process`);
            this.process.kill('SIGKILL');
          }
          resolve();
        }, 5000);

        if (this.process) {
          this.process.once('exit', () => {
            this.timerController.clearTimeout(timeout);
            resolve();
          });
        } else {
          this.timerController.clearTimeout(timeout);
          resolve();
        }
      });
    }

    this.process = null;
    this.restartCount = 0;
    log.info(`${backendLabel} backend bridge stopped`);
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

  startServingStatusUpdateStream(listener: ServingStatusUpdateListener): void {
    this.servingStatusUpdateStream.start(listener);
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
      void this.start().catch((error: unknown) => {
        log.error(`Failed to restart ${backendLabel} process:`, error);
      });
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

      const options: http.RequestOptions = {
        hostname: '127.0.0.1',
        port: this.port,
        path: '/rpc',
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Content-Length': Buffer.byteLength(requestBody),
        },
        timeout: 60000, // 60 second timeout
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
              reject(new Error(errorMessage));
            } else {
              resolve(response.result);
            }
          } catch {
            reject(new Error(`Invalid JSON response: ${data}`));
          }
        });
      });

      req.on('error', (error) => {
        reject(new Error(`RPC request failed: ${error.message}`));
      });

      req.on('timeout', () => {
        req.destroy();
        reject(new Error('RPC request timeout'));
      });

      req.write(requestBody);
      req.end();
    });
  }

  /**
   * Check if the bridge is running
   */
  isRunning(): boolean {
    return this.process !== null;
  }

  /**
   * Get the RPC server port
   */
  getPort(): number {
    return this.port;
  }
}
