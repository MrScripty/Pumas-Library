import { ACTION_FLAGS, ACTION_NAMES, PASSTHROUGH_ACTIONS } from './contract.mjs';
import { LauncherError } from './errors.mjs';
import { EXIT_CODES } from './contract.mjs';

export function parseArgs(argv) {
  let action = null;
  let forwardedArgs = [];

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];

    if (arg === '--') {
      if (!PASSTHROUGH_ACTIONS.has(action)) {
        throw new LauncherError('-- is only valid with --run or --run-release', {
          exitCode: EXIT_CODES.USAGE_ERROR,
          showUsage: true,
        });
      }

      forwardedArgs = argv.slice(index + 1);
      break;
    }

    if (!arg.startsWith('--')) {
      throw new LauncherError(`unknown argument: ${arg}`, {
        exitCode: EXIT_CODES.USAGE_ERROR,
        showUsage: true,
      });
    }

    if (!ACTION_NAMES.has(arg)) {
      throw new LauncherError(`unknown argument: ${arg}`, {
        exitCode: EXIT_CODES.USAGE_ERROR,
        showUsage: true,
      });
    }

    if (action !== null) {
      throw new LauncherError('only one action flag is allowed', {
        exitCode: EXIT_CODES.USAGE_ERROR,
        showUsage: true,
      });
    }

    action = arg;
  }

  if (action === null) {
    throw new LauncherError('one action flag is required', {
      exitCode: EXIT_CODES.USAGE_ERROR,
      showUsage: true,
    });
  }

  if (
    forwardedArgs.length > 0 &&
    action !== ACTION_FLAGS.RUN &&
    action !== ACTION_FLAGS.RUN_RELEASE
  ) {
    throw new LauncherError(`${action} does not accept app args`, {
      exitCode: EXIT_CODES.USAGE_ERROR,
      showUsage: true,
    });
  }

  return { action, forwardedArgs };
}
