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

function formatCommand(command, args) {
  return [command, ...args].join(' ');
}
