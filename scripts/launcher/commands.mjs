import { spawn, spawnSync } from 'node:child_process';
import { LauncherError } from './errors.mjs';
import { EXIT_CODES } from './contract.mjs';

export function commandExists(command, args = ['--version']) {
  const result = spawnSync(command, args, { stdio: 'ignore' });
  return result.status === 0;
}

export async function runCommand(command, args, options = {}) {
  const env = { ...process.env, ...options.env };

  await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env,
      stdio: 'inherit',
      shell: false,
    });

    child.on('error', (error) => {
      reject(
        new LauncherError(
          `failed to run ${formatCommand(command, args)}: ${error.message}`,
          { exitCode: EXIT_CODES.OPERATION_FAILED }
        )
      );
    });

    child.on('close', (code, signal) => {
      if (signal) {
        reject(
          new LauncherError(
            `${formatCommand(command, args)} terminated by signal ${signal}`,
            { exitCode: EXIT_CODES.OPERATION_FAILED }
          )
        );
        return;
      }

      if (code !== 0) {
        reject(
          new LauncherError(
            `${formatCommand(command, args)} exited with code ${code}`,
            { exitCode: EXIT_CODES.OPERATION_FAILED }
          )
        );
        return;
      }

      resolve();
    });
  });
}

export async function runBoundedCommand(command, args, options = {}) {
  const env = { ...process.env, ...options.env };
  const minUptimeMs = options.minUptimeMs ?? 0;
  const maxUptimeMs = options.maxUptimeMs ?? 30_000;
  const detached = process.platform !== 'win32';

  await new Promise((resolve, reject) => {
    const startedAt = Date.now();
    let timedOut = false;

    const child = spawn(command, args, {
      cwd: options.cwd,
      env,
      detached,
      stdio: 'inherit',
      shell: false,
    });

    const killTimer = setTimeout(() => {
      timedOut = true;
      terminateBoundedChild(child, detached);
    }, maxUptimeMs);

    child.on('error', (error) => {
      clearTimeout(killTimer);
      reject(
        new LauncherError(
          `failed to run ${formatCommand(command, args)}: ${error.message}`,
          { exitCode: EXIT_CODES.OPERATION_FAILED }
        )
      );
    });

    child.on('close', (code, signal) => {
      clearTimeout(killTimer);
      const elapsedMs = Date.now() - startedAt;

      if (timedOut) {
        reject(
          new LauncherError(
            `${formatCommand(command, args)} exceeded smoke window (${maxUptimeMs}ms)`,
            { exitCode: EXIT_CODES.OPERATION_FAILED }
          )
        );
        return;
      }

      if (signal) {
        reject(
          new LauncherError(
            `${formatCommand(command, args)} terminated by signal ${signal}`,
            { exitCode: EXIT_CODES.OPERATION_FAILED }
          )
        );
        return;
      }

      if (code !== 0) {
        reject(
          new LauncherError(
            `${formatCommand(command, args)} exited with code ${code}`,
            { exitCode: EXIT_CODES.OPERATION_FAILED }
          )
        );
        return;
      }

      if (elapsedMs < minUptimeMs) {
        reject(
          new LauncherError(
            `${formatCommand(command, args)} exited before the minimum smoke window (${elapsedMs}ms < ${minUptimeMs}ms)`,
            { exitCode: EXIT_CODES.OPERATION_FAILED }
          )
        );
        return;
      }

      resolve();
    });
  });
}

function terminateBoundedChild(child, detached) {
  if (detached && child.pid) {
    try {
      process.kill(-child.pid, 'SIGTERM');
      return;
    } catch {
      child.kill('SIGTERM');
      return;
    }
  }

  child.kill('SIGTERM');
}

function formatCommand(command, args) {
  return [command, ...args].join(' ');
}
