import fs from 'node:fs';
import path from 'node:path';
import { EXIT_CODES } from './contract.mjs';
import { LauncherError } from './errors.mjs';
import { commandExists, runCommand } from './commands.mjs';
import { log } from './logger.mjs';
import { corepackPnpmArgs, installArgs } from './package-manager.mjs';

const DEPENDENCIES = [
  {
    name: 'cargo',
    check: ({ platformService }) =>
      commandExists(platformService.cargoCommand, ['--version']),
    install: async () => {
      log('[error] cargo missing; install Rust toolchain from https://rustup.rs');
      return false;
    },
  },
  {
    name: 'node',
    check: () => Boolean(process.execPath),
    install: async () => {
      log('[error] node missing; install Node.js from https://nodejs.org/');
      return false;
    },
  },
  {
    name: 'corepack',
    check: ({ platformService }) =>
      commandExists(platformService.corepackCommand, ['--version']),
    install: async () => {
      log('[error] corepack missing; install a Node.js release that includes Corepack');
      return false;
    },
  },
  {
    name: 'workspace_node_modules',
    check: ({ context }) =>
      fs.existsSync(path.join(context.repoRoot, 'node_modules')) &&
      fs.existsSync(path.join(context.frontendDir, 'node_modules')) &&
      fs.existsSync(path.join(context.electronDir, 'node_modules')),
    install: async ({ context, platformService }) => {
      await runCommand(platformService.corepackCommand, corepackPnpmArgs(installArgs()), {
        cwd: context.repoRoot,
      });
      return true;
    },
  },
];

export async function installDependencies(runtime) {
  for (const dependency of DEPENDENCIES) {
    if (dependency.check(runtime)) {
      log(`[ok] ${dependency.name} already satisfied`);
      continue;
    }

    log(`[install] ${dependency.name} missing; installing`);

    const installed = await dependency.install(runtime);
    if (!installed) {
      throw new LauncherError(`${dependency.name} install failed`, {
        exitCode: EXIT_CODES.OPERATION_FAILED,
      });
    }

    if (!dependency.check(runtime)) {
      throw new LauncherError(`${dependency.name} install failed verification`, {
        exitCode: EXIT_CODES.OPERATION_FAILED,
      });
    }

    log(`[done] ${dependency.name} installed`);
  }
}

export function ensureRuntimeDependencies(runtime) {
  for (const dependency of DEPENDENCIES) {
    if (dependency.check(runtime)) {
      continue;
    }

    log(`missing dependency: ${dependency.name}`);
    log(`run ${runtime.context.displayName} --install first`);
    throw new LauncherError(`missing dependency: ${dependency.name}`, {
      exitCode: EXIT_CODES.MISSING_DEPENDENCY,
    });
  }
}
