#!/usr/bin/env node
import { buildUsage } from './contract.mjs';
import { createLauncherContext } from './context.mjs';
import { LauncherError } from './errors.mjs';
import { logError } from './logger.mjs';
import { parseArgs } from './parse-args.mjs';
import { createPlatformService } from './platform-service.mjs';
import { executeAction } from './actions.mjs';

async function main(argv = process.argv.slice(2)) {
  const context = createLauncherContext();
  const platformService = createPlatformService();
  const runtime = { context, platformService };

  try {
    const parsedArgs = parseArgs(argv);
    return await executeAction(parsedArgs, runtime);
  } catch (error) {
    if (error instanceof LauncherError) {
      logError(`error: ${error.message}`);
      if (error.showUsage) {
        process.stdout.write(buildUsage(context.displayName));
      }
      return error.exitCode;
    }

    const unexpected = error instanceof Error ? error.message : String(error);
    logError(`error: ${unexpected}`);
    return 1;
  }
}

const exitCode = await main();
process.exit(exitCode);
