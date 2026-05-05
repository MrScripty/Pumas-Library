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
export type RuntimeProfileUpdateListener = (payload: unknown) => void;

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

export function parseRuntimeProfileUpdateSseChunk(
  previousBuffer: string,
  chunk: string
): ParsedSseChunk {
  return parseNamedSseChunk(previousBuffer, chunk, 'runtime-profile-update', 'runtime-profile');
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
  private modelLibraryUpdateRequest: http.ClientRequest | null = null;
  private modelLibraryUpdateBuffer = '';
  private modelLibraryUpdateListener: ModelLibraryUpdateListener | null = null;
  private modelLibraryUpdateReconnectTimer: BridgeTimer | null = null;
  private runtimeProfileUpdateRequest: http.ClientRequest | null = null;
  private runtimeProfileUpdateBuffer = '';
  private runtimeProfileUpdateListener: RuntimeProfileUpdateListener | null = null;
  private runtimeProfileUpdateReconnectTimer: BridgeTimer | null = null;

  constructor(options: PythonBridgeOptions) {
    const { timerController, ...runtimeOptions } = options;
    this.options = {
      autoRestart: true,
      maxRestarts: 3,
      ...runtimeOptions,
    };
    this.timerController = timerController ?? NODE_TIMER_CONTROLLER;

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
      this.clearModelLibraryUpdateReconnectTimer();
      this.clearRuntimeProfileUpdateReconnectTimer();
      this.closeModelLibraryUpdateStream();
      this.closeRuntimeProfileUpdateStream();

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

    if (this.modelLibraryUpdateListener) {
      this.openModelLibraryUpdateStream();
    }
    if (this.runtimeProfileUpdateListener) {
      this.openRuntimeProfileUpdateStream();
    }

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
    this.stopRuntimeProfileUpdateStream();

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
    this.modelLibraryUpdateListener = listener;
    if (!this.process) {
      throw new Error('Backend bridge not running');
    }
    this.openModelLibraryUpdateStream();
  }

  stopModelLibraryUpdateStream(): void {
    this.modelLibraryUpdateListener = null;
    this.closeModelLibraryUpdateStream();
    this.clearModelLibraryUpdateReconnectTimer();
  }

  startRuntimeProfileUpdateStream(listener: RuntimeProfileUpdateListener): void {
    this.runtimeProfileUpdateListener = listener;
    if (!this.process) {
      throw new Error('Backend bridge not running');
    }
    this.openRuntimeProfileUpdateStream();
  }

  stopRuntimeProfileUpdateStream(): void {
    this.runtimeProfileUpdateListener = null;
    this.closeRuntimeProfileUpdateStream();
    this.clearRuntimeProfileUpdateReconnectTimer();
  }

  private openModelLibraryUpdateStream(): void {
    if (!this.process || !this.modelLibraryUpdateListener) {
      return;
    }

    this.closeModelLibraryUpdateStream();
    this.clearModelLibraryUpdateReconnectTimer();
    this.modelLibraryUpdateBuffer = '';

    const req = http.get({
      hostname: '127.0.0.1',
      port: this.port,
      path: '/events/model-library-updates',
      method: 'GET',
      headers: {
        Accept: 'text/event-stream',
      },
    }, (res) => {
      res.setEncoding('utf8');
      res.on('data', (chunk: string) => {
        const parsed = parseModelLibraryUpdateSseChunk(this.modelLibraryUpdateBuffer, chunk);
        this.modelLibraryUpdateBuffer = parsed.buffer;
        for (const payload of parsed.payloads) {
          this.modelLibraryUpdateListener?.(payload);
        }
      });
      res.on('end', () => {
        this.modelLibraryUpdateRequest = null;
        this.modelLibraryUpdateBuffer = '';
        if (!this.isShuttingDown) {
          log.warn('Model-library update stream ended');
          this.scheduleModelLibraryUpdateReconnect();
        }
      });
    });

    req.on('error', (error) => {
      this.modelLibraryUpdateRequest = null;
      this.modelLibraryUpdateBuffer = '';
      if (!this.isShuttingDown) {
        log.warn('Model-library update stream failed:', error);
        this.scheduleModelLibraryUpdateReconnect();
      }
    });

    this.modelLibraryUpdateRequest = req;
  }

  private closeModelLibraryUpdateStream(): void {
    if (this.modelLibraryUpdateRequest) {
      this.modelLibraryUpdateRequest.destroy();
      this.modelLibraryUpdateRequest = null;
    }
    this.modelLibraryUpdateBuffer = '';
  }

  private scheduleModelLibraryUpdateReconnect(): void {
    if (
      this.isShuttingDown ||
      !this.process ||
      !this.modelLibraryUpdateListener ||
      this.modelLibraryUpdateReconnectTimer
    ) {
      return;
    }

    this.modelLibraryUpdateReconnectTimer = this.timerController.setTimeout(() => {
      this.modelLibraryUpdateReconnectTimer = null;
      this.openModelLibraryUpdateStream();
    }, 1000);
  }

  private clearModelLibraryUpdateReconnectTimer(): void {
    if (this.modelLibraryUpdateReconnectTimer) {
      this.timerController.clearTimeout(this.modelLibraryUpdateReconnectTimer);
      this.modelLibraryUpdateReconnectTimer = null;
    }
  }

  private openRuntimeProfileUpdateStream(): void {
    if (!this.process || !this.runtimeProfileUpdateListener) {
      return;
    }

    this.closeRuntimeProfileUpdateStream();
    this.clearRuntimeProfileUpdateReconnectTimer();
    this.runtimeProfileUpdateBuffer = '';

    const req = http.get({
      hostname: '127.0.0.1',
      port: this.port,
      path: '/events/runtime-profile-updates',
      method: 'GET',
      headers: {
        Accept: 'text/event-stream',
      },
    }, (res) => {
      res.setEncoding('utf8');
      res.on('data', (chunk: string) => {
        const parsed = parseRuntimeProfileUpdateSseChunk(this.runtimeProfileUpdateBuffer, chunk);
        this.runtimeProfileUpdateBuffer = parsed.buffer;
        for (const payload of parsed.payloads) {
          this.runtimeProfileUpdateListener?.(payload);
        }
      });
      res.on('end', () => {
        this.runtimeProfileUpdateRequest = null;
        this.runtimeProfileUpdateBuffer = '';
        if (!this.isShuttingDown) {
          log.warn('Runtime-profile update stream ended');
          this.scheduleRuntimeProfileUpdateReconnect();
        }
      });
    });

    req.on('error', (error) => {
      this.runtimeProfileUpdateRequest = null;
      this.runtimeProfileUpdateBuffer = '';
      if (!this.isShuttingDown) {
        log.warn('Runtime-profile update stream failed:', error);
        this.scheduleRuntimeProfileUpdateReconnect();
      }
    });

    this.runtimeProfileUpdateRequest = req;
  }

  private closeRuntimeProfileUpdateStream(): void {
    if (this.runtimeProfileUpdateRequest) {
      this.runtimeProfileUpdateRequest.destroy();
      this.runtimeProfileUpdateRequest = null;
    }
    this.runtimeProfileUpdateBuffer = '';
  }

  private scheduleRuntimeProfileUpdateReconnect(): void {
    if (
      this.isShuttingDown ||
      !this.process ||
      !this.runtimeProfileUpdateListener ||
      this.runtimeProfileUpdateReconnectTimer
    ) {
      return;
    }

    this.runtimeProfileUpdateReconnectTimer = this.timerController.setTimeout(() => {
      this.runtimeProfileUpdateReconnectTimer = null;
      this.openRuntimeProfileUpdateStream();
    }, 1000);
  }

  private clearRuntimeProfileUpdateReconnectTimer(): void {
    if (this.runtimeProfileUpdateReconnectTimer) {
      this.timerController.clearTimeout(this.runtimeProfileUpdateReconnectTimer);
      this.runtimeProfileUpdateReconnectTimer = null;
    }
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
