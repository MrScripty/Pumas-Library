export function corepackPnpmArgs(args = []) {
  return ['pnpm', ...args];
}

export function rootScriptArgs(script, forwardedArgs = []) {
  return scriptArgs([], script, forwardedArgs);
}

export function workspaceScriptArgs(workspacePath, script, forwardedArgs = []) {
  return scriptArgs(['--filter', workspacePath], script, forwardedArgs);
}

export function installArgs() {
  return ['install', '--frozen-lockfile'];
}

function scriptArgs(prefixArgs, script, forwardedArgs) {
  const args = [...prefixArgs, 'run', script];

  if (forwardedArgs.length > 0) {
    args.push('--', ...forwardedArgs);
  }

  return args;
}
