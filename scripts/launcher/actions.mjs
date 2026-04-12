import fs from 'node:fs';
import { ACTION_FLAGS, EXIT_CODES, buildUsage } from './contract.mjs';
import { LauncherError } from './errors.mjs';
import { installDependencies, ensureRuntimeDependencies } from './dependencies.mjs';
import { log } from './logger.mjs';
import { runCommand } from './commands.mjs';

export async function executeAction(parsedArgs, runtime) {
  const { action, forwardedArgs } = parsedArgs;

  switch (action) {
    case ACTION_FLAGS.HELP:
      process.stdout.write(buildUsage(runtime.context.displayName));
      return EXIT_CODES.SUCCESS;
    case ACTION_FLAGS.INSTALL:
      await installDependencies(runtime);
      return EXIT_CODES.SUCCESS;
    case ACTION_FLAGS.BUILD:
      await buildApp('dev', runtime);
      return EXIT_CODES.SUCCESS;
    case ACTION_FLAGS.BUILD_RELEASE:
      await buildApp('release', runtime);
      return EXIT_CODES.SUCCESS;
    case ACTION_FLAGS.RUN:
      await runDevApp(forwardedArgs, runtime);
      return EXIT_CODES.SUCCESS;
    case ACTION_FLAGS.RUN_RELEASE:
      await runReleaseApp(forwardedArgs, runtime);
      return EXIT_CODES.SUCCESS;
    case ACTION_FLAGS.TEST:
      await runTestSuite(runtime);
      return EXIT_CODES.SUCCESS;
    default:
      throw new LauncherError(`invalid action: ${action}`, {
        exitCode: EXIT_CODES.USAGE_ERROR,
        showUsage: true,
      });
  }
}

async function buildApp(mode, runtime) {
  const { context, platformService } = runtime;

  ensureRuntimeDependencies(runtime);

  switch (mode) {
    case 'dev':
      log(`[build] compiling debug backend binary: ${context.appBin}`);
      await runCommand(
        platformService.cargoCommand,
        ['build', '--manifest-path', context.rustManifestPath, '-p', 'pumas-rpc', '--bin', context.appBin],
        { cwd: context.repoRoot }
      );
      break;
    case 'release':
      log(`[build] compiling release backend binary: ${context.appBin}`);
      await runCommand(
        platformService.cargoCommand,
        [
          'build',
          '--manifest-path',
          context.rustManifestPath,
          '-p',
          'pumas-rpc',
          '--release',
          '--bin',
          context.appBin,
        ],
        { cwd: context.repoRoot }
      );
      break;
    default:
      throw new LauncherError(`invalid build mode: ${mode}`, {
        exitCode: EXIT_CODES.USAGE_ERROR,
        showUsage: true,
      });
  }

  log('[build] compiling frontend assets');
  await runCommand(platformService.npmCommand, ['--workspace', 'frontend', 'run', 'build'], {
    cwd: context.repoRoot,
  });

  log('[build] compiling electron main process');
  await runCommand(platformService.npmCommand, ['--workspace', 'electron', 'run', 'build'], {
    cwd: context.repoRoot,
  });

  log(`[done] build completed (${mode})`);
}

async function runDevApp(runArgs, runtime) {
  const { context, platformService } = runtime;

  ensureRuntimeDependencies(runtime);
  ensureDevRuntimeArtifacts(runtime);

  log('[run] launching development runtime');
  await runCommand(
    platformService.npmCommand,
    ['--workspace', 'electron', 'run', 'dev', '--', ...runArgs],
    {
      cwd: context.repoRoot,
      env: { PUMAS_RUST_BACKEND: '1' },
    }
  );
}

async function runReleaseApp(runArgs, runtime) {
  const { context, platformService } = runtime;

  ensureRuntimeDependencies(runtime);
  ensureReleaseArtifacts(runtime);

  log('[run] launching release runtime');
  await runCommand(
    platformService.npmCommand,
    ['--workspace', 'electron', 'run', 'run:launcher-release', '--', ...runArgs],
    {
      cwd: context.repoRoot,
      env: { PUMAS_RUST_BACKEND: '1' },
    }
  );
}

async function runTestSuite(runtime) {
  const { context, platformService } = runtime;

  ensureRuntimeDependencies(runtime);

  log('[test] running Rust workspace tests');
  await runCommand(
    platformService.cargoCommand,
    ['test', '--workspace', '--exclude', 'pumas_rustler', '--manifest-path', context.rustManifestPath],
    { cwd: context.repoRoot }
  );

  log('[test] running launcher tests');
  await runCommand(platformService.npmCommand, ['run', 'test:launcher'], {
    cwd: context.repoRoot,
  });

  log('[test] running frontend tests');
  await runCommand(platformService.npmCommand, ['run', '-w', 'frontend', 'test:run'], {
    cwd: context.repoRoot,
  });

  log('[test] running frontend type checks');
  await runCommand(platformService.npmCommand, ['run', '-w', 'frontend', 'check:types'], {
    cwd: context.repoRoot,
  });

  log('[test] validating electron shell');
  await runCommand(platformService.npmCommand, ['run', '-w', 'electron', 'validate'], {
    cwd: context.repoRoot,
  });
}

function ensureDevRuntimeArtifacts(runtime) {
  const { context, platformService } = runtime;
  const releaseBackendBinary = platformService.releaseBackendBinary(context);

  if (!fs.existsSync(releaseBackendBinary)) {
    throw new LauncherError(
      `missing runtime backend binary: ${releaseBackendBinary} (run ${context.displayName} --build-release first)`,
      { exitCode: EXIT_CODES.OPERATION_FAILED }
    );
  }
}

function ensureReleaseArtifacts(runtime) {
  const { context, platformService } = runtime;
  const releaseBackendBinary = platformService.releaseBackendBinary(context);

  if (!fs.existsSync(releaseBackendBinary)) {
    throw new LauncherError(
      `missing release binary: ${releaseBackendBinary} (run ${context.displayName} --build-release first)`,
      { exitCode: EXIT_CODES.MISSING_RELEASE_ARTIFACT }
    );
  }

  if (!fs.existsSync(context.frontendDistIndex)) {
    throw new LauncherError(
      `missing release frontend artifact: ${context.frontendDistIndex} (run ${context.displayName} --build-release first)`,
      { exitCode: EXIT_CODES.MISSING_RELEASE_ARTIFACT }
    );
  }

  if (!fs.existsSync(context.electronDistMain)) {
    throw new LauncherError(
      `missing release electron artifact: ${context.electronDistMain} (run ${context.displayName} --build-release first)`,
      { exitCode: EXIT_CODES.MISSING_RELEASE_ARTIFACT }
    );
  }
}
