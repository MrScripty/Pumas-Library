/**
 * Backend Sidecar Bridge
 *
 * Manages the backend process (Python or Rust) and provides RPC communication.
 * Handles process lifecycle, health checks, and automatic restarts.
 *
 * Backend Selection:
 * - Set PUMAS_RUST_BACKEND=1 to use the Rust backend
 * - Default is Python backend for backward compatibility
 */

import { spawn, ChildProcess } from 'child_process';
import * as path from 'path';
import * as fs from 'fs';
import * as http from 'http';
import * as net from 'net';
import log from 'electron-log';

/** Backend type selection */
export type BackendType = 'python' | 'rust';

export interface PythonBridgeOptions {
  /** Path to the backend directory (for Python) or project root */
  backendPath: string;
  /** Port for the RPC server (0 = auto-assign) */
  port: number;
  /** Enable debug mode */
  debug: boolean;
  /** Python executable path (defaults to venv python) */
  pythonPath?: string;
  /** Path to virtual environment (defaults to ../venv relative to backendPath) */
  venvPath?: string;
  /** Restart on crash */
  autoRestart?: boolean;
  /** Maximum restart attempts */
  maxRestarts?: number;
  /** Backend type to use (defaults to 'python', or 'rust' if PUMAS_RUST_BACKEND=1) */
  backendType?: BackendType;
  /** Path to Rust binary (defaults to rust/target/release/pumas-rpc) */
  rustBinaryPath?: string;
  /** Launcher root directory for Rust backend */
  launcherRoot?: string;
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

/**
 * Detect which backend type to use based on environment and availability
 */
function detectBackendType(projectRoot: string): BackendType {
  // Check environment variable first
  if (process.env.PUMAS_RUST_BACKEND === '1') {
    const rustBinary = path.join(projectRoot, 'rust', 'target', 'release', 'pumas-rpc');
    if (fs.existsSync(rustBinary)) {
      log.info('Using Rust backend (PUMAS_RUST_BACKEND=1)');
      return 'rust';
    } else {
      log.warn(`PUMAS_RUST_BACKEND=1 but Rust binary not found at ${rustBinary}, falling back to Python`);
      return 'python';
    }
  }
  return 'python';
}

export class PythonBridge {
  private options: Required<PythonBridgeOptions>;
  private process: ChildProcess | null = null;
  private port: number = 0;
  private restartCount: number = 0;
  private isShuttingDown: boolean = false;
  private healthCheckInterval: NodeJS.Timeout | null = null;
  private backendType: BackendType;

  constructor(options: PythonBridgeOptions) {
    // Resolve venv path - defaults to ../venv relative to backendPath (project root/venv)
    const projectRoot = path.resolve(options.backendPath, '..');
    const defaultVenvPath = path.join(projectRoot, 'venv');
    const venvPath = options.venvPath || defaultVenvPath;

    // Use venv Python by default
    const defaultPythonPath = path.join(venvPath, 'bin', 'python');

    // Detect backend type
    this.backendType = options.backendType || detectBackendType(projectRoot);

    // Default Rust binary path
    const defaultRustBinaryPath = path.join(projectRoot, 'rust', 'target', 'release', 'pumas-rpc');

    // Default launcher root (project root for development)
    const defaultLauncherRoot = projectRoot;

    this.options = {
      pythonPath: defaultPythonPath,
      venvPath: venvPath,
      autoRestart: true,
      maxRestarts: 3,
      backendType: this.backendType,
      rustBinaryPath: defaultRustBinaryPath,
      launcherRoot: defaultLauncherRoot,
      ...options,
    };

    log.info(`Backend bridge initialized with ${this.backendType} backend`);
  }

  /**
   * Get the current backend type
   */
  getBackendType(): BackendType {
    return this.backendType;
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
   * Get the command and arguments for the selected backend
   */
  private getBackendCommand(): { cmd: string; args: string[]; cwd: string; env: NodeJS.ProcessEnv } {
    const projectRoot = path.resolve(this.options.backendPath, '..');

    if (this.backendType === 'rust') {
      return {
        cmd: this.options.rustBinaryPath,
        args: [
          '--port', String(this.port),
          '--launcher_root', this.options.launcherRoot,
          ...(this.options.debug ? ['--debug'] : []),
        ],
        cwd: projectRoot,
        env: {
          ...process.env,
          RUST_LOG: this.options.debug ? 'debug' : 'info',
        },
      };
    } else {
      // Python backend
      const rpcServerPath = path.join(this.options.backendPath, 'rpc_server.py');
      return {
        cmd: this.options.pythonPath,
        args: [
          rpcServerPath,
          '--port', String(this.port),
          ...(this.options.debug ? ['--debug'] : []),
        ],
        cwd: this.options.backendPath,
        env: {
          ...process.env,
          PYTHONUNBUFFERED: '1',
          PYTHONPATH: projectRoot,
          VIRTUAL_ENV: this.options.venvPath,
        },
      };
    }
  }

  /**
   * Start the backend sidecar process
   */
  async start(): Promise<void> {
    if (this.process) {
      log.warn(`${this.backendType} process already running`);
      return;
    }

    // Find available port
    this.port = this.options.port || await this.findAvailablePort();
    log.info(`Starting ${this.backendType} backend bridge on port ${this.port}`);

    // Get command configuration for selected backend
    const { cmd, args, cwd, env } = this.getBackendCommand();

    // Verify binary exists for Rust backend
    if (this.backendType === 'rust' && !fs.existsSync(cmd)) {
      throw new Error(`Rust backend binary not found at ${cmd}. Run 'cargo build --release' in the rust/ directory.`);
    }

    // Spawn process
    this.process = spawn(cmd, args, {
      cwd,
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    const backendLabel = this.backendType === 'rust' ? 'Rust' : 'Python';

    // Handle stdout
    this.process.stdout?.on('data', (data: Buffer) => {
      const output = data.toString().trim();
      if (output) {
        log.info(`[${backendLabel}] ${output}`);
      }
    });

    // Handle stderr
    this.process.stderr?.on('data', (data: Buffer) => {
      const output = data.toString().trim();
      if (output) {
        // Rust backend uses stderr for tracing logs, which is normal
        if (this.backendType === 'rust') {
          log.info(`[${backendLabel}] ${output}`);
        } else {
          log.warn(`[${backendLabel}] ${output}`);
        }
      }
    });

    // Handle process exit
    this.process.on('exit', (code, signal) => {
      log.info(`${backendLabel} process exited: code=${code}, signal=${signal}`);
      this.process = null;

      // Auto-restart if enabled and not shutting down
      if (
        !this.isShuttingDown &&
        this.options.autoRestart &&
        this.restartCount < this.options.maxRestarts
      ) {
        this.restartCount++;
        log.info(`Restarting ${backendLabel} process (attempt ${this.restartCount}/${this.options.maxRestarts})`);
        setTimeout(() => this.start(), 1000 * this.restartCount); // Exponential backoff
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
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
    }

    throw new Error(`${this.backendType} RPC server failed to start within timeout`);
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
    const backendLabel = this.backendType === 'rust' ? 'Rust' : 'Python';
    this.healthCheckInterval = setInterval(async () => {
      if (this.isShuttingDown) return;

      const healthy = await this.healthCheck();
      if (!healthy && !this.isShuttingDown) {
        log.warn(`${backendLabel} health check failed`);
      }
    }, 30000); // Check every 30 seconds
  }

  /**
   * Stop the backend sidecar process
   */
  async stop(): Promise<void> {
    this.isShuttingDown = true;

    // Stop health check interval
    if (this.healthCheckInterval) {
      clearInterval(this.healthCheckInterval);
      this.healthCheckInterval = null;
    }

    if (!this.process) {
      return;
    }

    const backendLabel = this.backendType === 'rust' ? 'Rust' : 'Python';
    log.info(`Stopping ${backendLabel} backend bridge...`);

    // Try graceful shutdown first
    try {
      await this.call('shutdown', {});
      await new Promise((resolve) => setTimeout(resolve, 1000));
    } catch {
      // Ignore errors during shutdown
    }

    // Force kill if still running
    if (this.process) {
      this.process.kill('SIGTERM');

      // Wait for process to exit
      await new Promise<void>((resolve) => {
        const timeout = setTimeout(() => {
          if (this.process) {
            log.warn(`Force killing ${backendLabel} process`);
            this.process.kill('SIGKILL');
          }
          resolve();
        }, 5000);

        if (this.process) {
          this.process.once('exit', () => {
            clearTimeout(timeout);
            resolve();
          });
        } else {
          clearTimeout(timeout);
          resolve();
        }
      });
    }

    this.process = null;
    log.info(`${backendLabel} backend bridge stopped`);
  }

  /**
   * Make an RPC call to the backend
   */
  async call(method: string, params: Record<string, unknown>): Promise<unknown> {
    if (!this.process) {
      throw new Error(`${this.backendType} backend bridge not running`);
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
          } catch (error) {
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
