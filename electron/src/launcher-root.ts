import * as fs from 'fs';
import * as path from 'path';

export interface LauncherRootResolutionOptions {
  appImagePath?: string;
  argv?: string[];
  devRoot: string;
  execPath: string;
  isPackaged: boolean;
  userDataPath: string;
}

export function resolveLauncherRoot(options: LauncherRootResolutionOptions): string {
  const override = resolveLauncherRootOverride(options.argv ?? process.argv, process.env);
  if (override) {
    return override;
  }

  const appImagePortableRoot = options.appImagePath
    ? path.join(path.dirname(options.appImagePath), 'pumas-data')
    : undefined;

  if (appImagePortableRoot && isExistingLauncherRoot(appImagePortableRoot)) {
    return appImagePortableRoot;
  }

  const candidateStarts = new Set<string>();
  if (options.appImagePath) {
    candidateStarts.add(path.dirname(options.appImagePath));
  }
  if (options.isPackaged) {
    candidateStarts.add(path.dirname(options.execPath));
  }

  for (const startDir of candidateStarts) {
    const existingRoot = findLauncherRootFrom(startDir);
    if (existingRoot) {
      return existingRoot;
    }
  }

  if (appImagePortableRoot) {
    return appImagePortableRoot;
  }

  if (options.isPackaged) {
    return options.userDataPath;
  }

  return options.devRoot;
}

function resolveLauncherRootOverride(argv: string[], env: NodeJS.ProcessEnv): string | null {
  const envOverride = env.PUMAS_LAUNCHER_ROOT?.trim();
  if (envOverride) {
    return path.resolve(envOverride);
  }

  for (let index = 0; index < argv.length; index += 1) {
    const current = argv[index];

    if (current === '--launcher-root') {
      const next = argv[index + 1];
      if (next && !next.startsWith('--')) {
        return path.resolve(next);
      }
      continue;
    }

    if (current.startsWith('--launcher-root=')) {
      const value = current.slice('--launcher-root='.length).trim();
      if (value) {
        return path.resolve(value);
      }
    }
  }

  return null;
}

function findLauncherRootFrom(startDir: string): string | null {
  let current = path.resolve(startDir);

  while (true) {
    if (isExistingLauncherRoot(current)) {
      return current;
    }

    const parent = path.dirname(current);
    if (parent === current) {
      return null;
    }
    current = parent;
  }
}

function isExistingLauncherRoot(candidate: string): boolean {
  return (
    fs.existsSync(path.join(candidate, 'shared-resources', 'models')) ||
    (fs.existsSync(path.join(candidate, 'launcher-data')) &&
      fs.existsSync(path.join(candidate, 'shared-resources')))
  );
}
