/**
 * Python Sidecar Bridge
 *
 * Manages the Python backend process and provides RPC communication.
 * Handles process lifecycle, health checks, and automatic restarts.
 */

import { spawn, ChildProcess } from 'child_process';
import * as path from 'path';
import * as http from 'http';
import * as net from 'net';
import log from 'electron-log';

export interface PythonBridgeOptions {
  /** Path to the backend directory */
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
}

interface RPCResponse {
  result?: unknown;
  error?: string;
}

export class PythonBridge {
  private options: Required<PythonBridgeOptions>;
  private process: ChildProcess | null = null;
  private port: number = 0;
  private restartCount: number = 0;
  private isShuttingDown: boolean = false;
  private healthCheckInterval: NodeJS.Timeout | null = null;

  constructor(options: PythonBridgeOptions) {
    // Resolve venv path - defaults to ../venv relative to backendPath (project root/venv)
    const projectRoot = path.resolve(options.backendPath, '..');
    const defaultVenvPath = path.join(projectRoot, 'venv');
    const venvPath = options.venvPath || defaultVenvPath;

    // Use venv Python by default
    const defaultPythonPath = path.join(venvPath, 'bin', 'python');

    this.options = {
      pythonPath: defaultPythonPath,
      venvPath: venvPath,
      autoRestart: true,
      maxRestarts: 3,
      ...options,
    };
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
   * Start the Python sidecar process
   */
  async start(): Promise<void> {
    if (this.process) {
      log.warn('Python process already running');
      return;
    }

    // Find available port
    this.port = this.options.port || await this.findAvailablePort();
    log.info(`Starting Python bridge on port ${this.port}`);

    // Build command arguments
    const rpcServerPath = path.join(this.options.backendPath, 'rpc_server.py');
    const args = [
      rpcServerPath,
      '--port', String(this.port),
    ];

    if (this.options.debug) {
      args.push('--debug');
    }

    // Project root for PYTHONPATH
    const projectRoot = path.resolve(this.options.backendPath, '..');

    // Spawn Python process
    this.process = spawn(this.options.pythonPath, args, {
      cwd: this.options.backendPath,
      env: {
        ...process.env,
        PYTHONUNBUFFERED: '1',
        PYTHONPATH: projectRoot,
        // Ensure venv is recognized
        VIRTUAL_ENV: this.options.venvPath,
      },
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    // Handle stdout
    this.process.stdout?.on('data', (data: Buffer) => {
      const output = data.toString().trim();
      if (output) {
        log.info(`[Python] ${output}`);
      }
    });

    // Handle stderr
    this.process.stderr?.on('data', (data: Buffer) => {
      const output = data.toString().trim();
      if (output) {
        log.warn(`[Python] ${output}`);
      }
    });

    // Handle process exit
    this.process.on('exit', (code, signal) => {
      log.info(`Python process exited: code=${code}, signal=${signal}`);
      this.process = null;

      // Auto-restart if enabled and not shutting down
      if (
        !this.isShuttingDown &&
        this.options.autoRestart &&
        this.restartCount < this.options.maxRestarts
      ) {
        this.restartCount++;
        log.info(`Restarting Python process (attempt ${this.restartCount}/${this.options.maxRestarts})`);
        setTimeout(() => this.start(), 1000 * this.restartCount); // Exponential backoff
      }
    });

    // Handle process error
    this.process.on('error', (error) => {
      log.error('Python process error:', error);
    });

    // Wait for the server to be ready
    await this.waitForReady();

    // Start health check interval
    this.startHealthCheck();

    // Reset restart counter on successful start
    this.restartCount = 0;

    log.info('Python bridge started successfully');
  }

  /**
   * Wait for the RPC server to be ready
   */
  private async waitForReady(timeout: number = 30000): Promise<void> {
    const startTime = Date.now();

    while (Date.now() - startTime < timeout) {
      try {
        await this.healthCheck();
        return;
      } catch {
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
    }

    throw new Error('Python RPC server failed to start within timeout');
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
    this.healthCheckInterval = setInterval(async () => {
      if (this.isShuttingDown) return;

      const healthy = await this.healthCheck();
      if (!healthy && !this.isShuttingDown) {
        log.warn('Python health check failed');
      }
    }, 30000); // Check every 30 seconds
  }

  /**
   * Stop the Python sidecar process
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

    log.info('Stopping Python bridge...');

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
            log.warn('Force killing Python process');
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
    log.info('Python bridge stopped');
  }

  /**
   * Make an RPC call to the Python backend
   */
  async call(method: string, params: Record<string, unknown>): Promise<unknown> {
    if (!this.process) {
      throw new Error('Python bridge not running');
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
              reject(new Error(response.error));
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
